use super::{SwarmMember, SwarmTaskProgress, VersionedPlan};
use crate::protocol::ServerEvent;
use crate::storage;
use jcode_swarm_core::control_log::{SwarmControlEvent, read_from as read_control_log_from};
use jcode_swarm_core::{
    MemberLifecycleEvent, MemberLifecycleState, SwarmLifecycleStatus, SwarmMemberRecord, SwarmRole,
};
use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex as StdMutex, Weak};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Directory name under the durable state dir (`~/.jcode/state`).
const SWARM_STATE_DIR: &str = "swarm";
/// Pre-0.36 location under the runtime dir (tmpfs on Linux, wiped on reboot).
const LEGACY_SWARM_STATE_DIR: &str = "jcode-swarm-state";
const QUARANTINE_DIR: &str = "quarantine";

/// Serialize each swarm's complete snapshot/read/write operation. Callers must
/// acquire this before reading the independently locked in-memory maps so an
/// older snapshot cannot finish after a newer one.
static SWARM_OPERATION_LOCKS: LazyLock<StdMutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

/// Protect primary/backup comparisons and filesystem updates, including tests
/// and recovery paths that invoke the synchronous persistence helpers directly.
static SWARM_FILE_LOCKS: LazyLock<StdMutex<HashMap<String, Weak<StdMutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SwarmStateFileVersion(Option<Vec<u8>>);

pub(super) fn swarm_operation_lock(swarm_id: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut locks = SWARM_OPERATION_LOCKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(swarm_id).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    locks.insert(swarm_id.to_string(), Arc::downgrade(&lock));
    lock
}

fn swarm_file_lock(swarm_id: &str) -> Arc<StdMutex<()>> {
    let mut locks = SWARM_FILE_LOCKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(swarm_id).and_then(Weak::upgrade) {
        return lock;
    }
    let lock = Arc::new(StdMutex::new(()));
    locks.insert(swarm_id.to_string(), Arc::downgrade(&lock));
    lock
}

pub(super) struct LoadedSwarmRuntimeState {
    pub plans: HashMap<String, VersionedPlan>,
    pub coordinators: HashMap<String, String>,
    pub members: HashMap<String, SwarmMember>,
    pub swarms_by_id: HashMap<String, HashSet<String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedSwarmState {
    swarm_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plan: Option<PersistedVersionedPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    coordinator_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    members: Vec<PersistedSwarmMember>,
    updated_at_unix_ms: u64,
    /// W1 step 4: byte offset into the per-swarm control log covered by this
    /// snapshot. The snapshot is the compaction checkpoint: recovery replays
    /// log events past this offset over the snapshot, so control-plane
    /// changes that never reached a snapshot write (member status/role flips
    /// via broadcast_swarm_status) survive a restart. 0 (the serde default
    /// for pre-W1 snapshots) replays the whole log, which is safe because
    /// replay is idempotent over the snapshot state.
    #[serde(default)]
    control_log_covered_offset: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedVersionedPlan {
    items: Vec<crate::plan::PlanItem>,
    version: u64,
    participants: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    task_progress: HashMap<String, SwarmTaskProgress>,
    #[serde(default = "default_plan_mode", skip_serializing_if = "is_light_mode")]
    mode: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    node_meta: HashMap<String, crate::plan::NodeMeta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_nodes: Option<usize>,
    #[serde(default)]
    frozen: bool,
    /// Plan-level budget accounting. Persisted so a restart cannot hand a plan a
    /// fresh allowance: dropping this on reload would reset every budget clock
    /// and turn a restart into an unlimited-spend loophole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    safety_ledger: Option<jcode_plan::dag::PlanSafetyLedger>,
}

fn default_plan_mode() -> String {
    "light".to_string()
}

fn is_light_mode(mode: &str) -> bool {
    mode == "light"
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedSwarmMember {
    #[serde(flatten)]
    record: SwarmMemberRecord,
    /// Wall-clock time when the member entered its current terminal status.
    /// Legacy snapshots omit this; their snapshot timestamp becomes the
    /// conservative migration fallback so reports are not discarded eagerly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terminal_since_unix_ms: Option<u64>,
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn state_dir() -> PathBuf {
    storage::durable_state_dir().join(SWARM_STATE_DIR)
}

fn legacy_state_dir() -> PathBuf {
    storage::runtime_dir().join(LEGACY_SWARM_STATE_DIR)
}

/// One-time migration from the legacy runtime-dir location (tmpfs, wiped on
/// reboot) to the durable state dir. Copies legacy snapshots only when the
/// new dir has none, so an already-migrated dir is never clobbered.
fn migrate_legacy_state() {
    let new_dir = state_dir();
    let has_new_state = std::fs::read_dir(&new_dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        })
        .unwrap_or(false);
    if has_new_state {
        return;
    }

    let legacy_dir = legacy_state_dir();
    let Ok(entries) = std::fs::read_dir(&legacy_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Some(file_name) = path.file_name() else {
            continue;
        };
        if let Err(err) = storage::ensure_dir(&new_dir) {
            crate::logging::warn(&format!(
                "Failed to create swarm state dir {}: {}",
                new_dir.display(),
                err
            ));
            return;
        }
        if let Err(err) = std::fs::copy(&path, new_dir.join(file_name)) {
            crate::logging::warn(&format!(
                "Failed to migrate legacy swarm state {}: {}",
                path.display(),
                err
            ));
        }
    }
}

fn sanitize_swarm_id(swarm_id: &str) -> String {
    swarm_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn state_path(swarm_id: &str) -> PathBuf {
    state_dir().join(format!("{}.json", sanitize_swarm_id(swarm_id)))
}

/// Path of the per-swarm control-plane event log (W1). Lives next to the
/// snapshot so archive/GC of a swarm is one directory glob.
pub(super) fn control_log_path(swarm_id: &str) -> PathBuf {
    state_dir().join(format!("{}.control.jsonl", sanitize_swarm_id(swarm_id)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SwarmStateArtifactKind {
    Snapshot { swarm_id: String },
    Backup { swarm_id: String },
    ControlLog { swarm_id: String },
    Temp,
    Quarantine,
    Other,
}

fn classify_state_artifact(path: &Path) -> SwarmStateArtifactKind {
    if path
        .components()
        .any(|component| component.as_os_str() == QUARANTINE_DIR)
    {
        return SwarmStateArtifactKind::Quarantine;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return SwarmStateArtifactKind::Other;
    };
    if name.ends_with(".tmp") || name.starts_with('.') {
        return SwarmStateArtifactKind::Temp;
    }
    if let Some(id) = name.strip_suffix(".control.jsonl") {
        return SwarmStateArtifactKind::ControlLog {
            swarm_id: id.to_string(),
        };
    }
    if let Some(id) = name.strip_suffix(".json") {
        return SwarmStateArtifactKind::Snapshot {
            swarm_id: id.to_string(),
        };
    }
    if let Some(id) = name.strip_suffix(".bak") {
        return SwarmStateArtifactKind::Backup {
            swarm_id: id.to_string(),
        };
    }
    SwarmStateArtifactKind::Other
}

fn quarantine_path(original: &Path, label: &str, stamp: u128, counter: u32) -> PathBuf {
    let name = original
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("swarm-state");
    state_dir()
        .join(QUARANTINE_DIR)
        .join(format!("{name}.{label}.{stamp}.{counter}.corrupt"))
}

fn quarantine_bytes_at_stamp(
    original: &Path,
    label: &str,
    bytes: &[u8],
    stamp: u128,
) -> Option<PathBuf> {
    let dir = state_dir().join(QUARANTINE_DIR);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        crate::logging::warn(&format!(
            "swarm_state_quarantine_failed path={} quarantine_dir={} error={}",
            original.display(),
            dir.display(),
            error
        ));
        return None;
    }

    for counter in 0..1000u32 {
        let path = quarantine_path(original, label, stamp, counter);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => match file.write_all(bytes) {
                Ok(()) => {
                    crate::logging::warn(&format!(
                        "swarm_state_corrupt_quarantined path={} quarantine={} bytes={}",
                        original.display(),
                        path.display(),
                        bytes.len()
                    ));
                    return Some(path);
                }
                Err(error) => {
                    if let Err(remove_error) = std::fs::remove_file(&path)
                        && remove_error.kind() != std::io::ErrorKind::NotFound
                    {
                        crate::logging::warn(&format!(
                            "swarm_state_quarantine_cleanup_failed quarantine={} error={}",
                            path.display(),
                            remove_error
                        ));
                    }
                    crate::logging::warn(&format!(
                        "swarm_state_quarantine_failed path={} quarantine={} error={}",
                        original.display(),
                        path.display(),
                        error
                    ));
                    return None;
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                match std::fs::read(&path) {
                    Ok(existing) if existing == bytes => {
                        crate::logging::warn(&format!(
                            "swarm_state_corrupt_already_quarantined path={} quarantine={} bytes={}",
                            original.display(),
                            path.display(),
                            bytes.len()
                        ));
                        return Some(path);
                    }
                    Ok(_) => continue,
                    Err(read_error) => {
                        crate::logging::warn(&format!(
                            "swarm_state_quarantine_failed path={} quarantine={} error={}",
                            original.display(),
                            path.display(),
                            read_error
                        ));
                        return None;
                    }
                }
            }
            Err(error) => {
                crate::logging::warn(&format!(
                    "swarm_state_quarantine_failed path={} quarantine={} error={}",
                    original.display(),
                    path.display(),
                    error
                ));
                return None;
            }
        }
    }

    crate::logging::warn(&format!(
        "swarm_state_quarantine_failed path={} quarantine_dir={} error=collision_limit_exhausted",
        original.display(),
        dir.display()
    ));
    None
}

fn quarantine_bytes(original: &Path, label: &str, bytes: &[u8]) {
    let stamp = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    drop(quarantine_bytes_at_stamp(
        original,
        label,
        bytes,
        u128::from(stamp),
    ));
}

/// Current byte length of the per-swarm control log (0 when absent). Used as
/// the covered offset for snapshots whose in-memory state already reflects
/// the full log, so tail replay is a no-op.
fn current_control_log_len(swarm_id: &str) -> u64 {
    std::fs::metadata(control_log_path(swarm_id))
        .map(|meta| meta.len())
        .unwrap_or(0)
}

fn prune_terminal_control_logs(
    dir: &Path,
    retention: Duration,
    retained_control_logs: &HashSet<PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let SwarmStateArtifactKind::ControlLog { swarm_id } = classify_state_artifact(&path) else {
            continue;
        };
        // A control log is the replay tail beyond its snapshot checkpoint, so it
        // must be preserved whenever a loadable snapshot still exists. Both the
        // primary `.json` and an orphan `.bak` count: startup recovery loads the
        // backup when the primary is missing (see the `[bak, json]` candidate
        // order in read_swarm_snapshot_with_quarantine), so pruning the log on
        // the strength of a missing `.json` alone would silently drop events
        // past the backup's covered offset (F27 gap F25-1).
        let state_json = state_path(&swarm_id);
        if state_json.exists() || state_json.with_extension("bak").exists() {
            continue;
        }
        if retained_control_logs.contains(&path) {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|meta| meta.modified()) else {
            continue;
        };
        let Ok(age) = now.duration_since(modified) else {
            continue;
        };
        if age < retention {
            continue;
        }
        super::control_log_sync::reset_cached_control_log(&swarm_id);
        if let Err(error) = std::fs::remove_file(&path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            crate::logging::warn(&format!(
                "Failed to prune terminal swarm control log {}: {}",
                path.display(),
                error
            ));
        }
    }
}

fn read_primary_version(swarm_id: &str) -> SwarmStateFileVersion {
    SwarmStateFileVersion(std::fs::read(state_path(swarm_id)).ok())
}

pub(super) fn capture_swarm_state_version(swarm_id: &str) -> SwarmStateFileVersion {
    let file_lock = swarm_file_lock(swarm_id);
    let _guard = file_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    read_primary_version(swarm_id)
}

fn remove_snapshot_files(swarm_id: &str) -> bool {
    super::control_log_sync::reset_cached_control_log(swarm_id);
    let path = state_path(swarm_id);
    // First atomically replace the primary with an empty tombstone. The write
    // may rotate the old primary to `.bak`, but load_runtime_state ignores that
    // backup while the tombstone exists. Thus every crash point is safe: before
    // rename the deletion did not happen, and after rename the old state is
    // already logically invalid even if physical cleanup is interrupted.
    let tombstone = PersistedSwarmState {
        swarm_id: swarm_id.to_string(),
        plan: None,
        coordinator_session_id: None,
        members: Vec::new(),
        updated_at_unix_ms: now_unix_ms(),
        // Cover the whole current log so the tombstone is never resurrected
        // by control-log tail replay.
        control_log_covered_offset: current_control_log_len(swarm_id),
    };
    if let Err(err) = storage::write_json_fast(&path, &tombstone) {
        crate::logging::warn(&format!(
            "Failed to tombstone swarm state {}: {}",
            path.display(),
            err
        ));
        return false;
    }

    let mut removed = true;
    for candidate in [path.with_extension("bak"), path] {
        if let Err(err) = std::fs::remove_file(&candidate)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            removed = false;
            crate::logging::warn(&format!(
                "Failed to remove swarm state {}: {}",
                candidate.display(),
                err
            ));
        }
    }
    removed
}

fn from_persisted_plan(mut plan: PersistedVersionedPlan, updated_at_unix_ms: u64) -> VersionedPlan {
    let mut plan = VersionedPlan {
        items: std::mem::take(&mut plan.items),
        version: plan.version,
        participants: std::mem::take(&mut plan.participants).into_iter().collect(),
        task_progress: std::mem::take(&mut plan.task_progress),
        mode: std::mem::take(&mut plan.mode),
        node_meta: std::mem::take(&mut plan.node_meta),
        max_nodes: plan.max_nodes,
        frozen: plan.frozen,
        safety_ledger: plan.safety_ledger.take(),
    };
    mark_running_items_stale(&mut plan, updated_at_unix_ms);
    plan
}

/// Post-restart staleness pass: anything "running" cannot actually be running
/// (the worker did not survive the reload), so mark it stale for the reaper.
fn mark_running_items_stale(plan: &mut VersionedPlan, stale_since_unix_ms: u64) {
    for item in &mut plan.items {
        if item.status == "running" {
            item.status = "running_stale".to_string();
            plan.task_progress
                .entry(item.id.clone())
                .or_default()
                .stale_since_unix_ms
                .get_or_insert(stale_since_unix_ms);
        }
    }
}

fn to_persisted_plan(plan: &VersionedPlan) -> PersistedVersionedPlan {
    let mut participants: Vec<String> = plan.participants.iter().cloned().collect();
    participants.sort();
    PersistedVersionedPlan {
        items: plan.items.clone(),
        version: plan.version,
        participants,
        task_progress: plan.task_progress.clone(),
        mode: plan.mode.clone(),
        node_meta: plan.node_meta.clone(),
        max_nodes: plan.max_nodes,
        frozen: plan.frozen,
        safety_ledger: plan.safety_ledger.clone(),
    }
}

fn to_persisted_member(member: &SwarmMember, snapshot_unix_ms: u64) -> PersistedSwarmMember {
    let terminal_since_unix_ms =
        super::swarm::member_status_is_terminal(&member.status).then(|| {
            snapshot_unix_ms.saturating_sub(member.last_status_change.elapsed().as_millis() as u64)
        });
    PersistedSwarmMember {
        record: member.durable_record(),
        terminal_since_unix_ms,
    }
}

fn append_recovery_detail(detail: Option<String>, note: &str) -> Option<String> {
    match detail {
        Some(existing) if !existing.trim().is_empty() => Some(format!("{} ({})", existing, note)),
        _ => Some(note.to_string()),
    }
}

fn recover_member_status(
    status: SwarmLifecycleStatus,
    detail: Option<String>,
    is_headless: bool,
    recovered_at_unix_ms: u64,
) -> (SwarmLifecycleStatus, Option<String>) {
    // Compare the lifecycle state, not the whole status. A status read back
    // from disk carries the epoch, revision and timestamp it was written with,
    // so it never equals one of the zero-valued named constants; comparing
    // whole values here silently matched nothing and left crashed members
    // recorded as still running. Transitions go through `reduce` for the same
    // reason: it keeps the assignment epoch a coordinator needs to tell a
    // stale report from a live one.
    let mut status = status;
    if status.state == MemberLifecycleState::Running {
        let detail = append_recovery_detail(detail, "recovered after reload while running");
        status.reduce(
            MemberLifecycleEvent::ProcessLost {
                reason: detail.clone(),
            },
            recovered_at_unix_ms,
        );
        return (status, detail);
    }

    // An idle headless worker has no process to drive it after a server restart.
    // Keep its completion report, but mark it stopped instead of eagerly loading
    // its full session history and tool registry forever. Coordinators can spawn
    // a fresh worker when more work arrives.
    if is_headless && status.state == MemberLifecycleState::Ready {
        let detail = append_recovery_detail(detail, "idle worker not restored after server restart");
        status.reduce(
            MemberLifecycleEvent::StopConfirmed {
                epoch: status.assignment_epoch,
                reason: detail.clone(),
            },
            recovered_at_unix_ms,
        );
        return (status, detail);
    }

    // Done headless members finished their work before the reload. Nothing
    // in-flight was lost and their completion report remains available.
    // Ask the state machine rather than listing status constants: several old
    // names now share one state (`Completed` and `Done` are both `Succeeded`),
    // so an alias list reads as if it covers more than it does and leaves arms
    // the compiler can prove dead. `is_terminal` also covers `Lost`, which was
    // missing here and is already the state this branch would assign.
    if is_headless && !status.is_terminal() {
        let detail = append_recovery_detail(detail, "headless session did not survive reload");
        status.reduce(
            MemberLifecycleEvent::ProcessLost {
                reason: detail.clone(),
            },
            recovered_at_unix_ms,
        );
        return (status, detail);
    }

    (status, detail)
}

fn recovered_member_event_tx() -> mpsc::UnboundedSender<ServerEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    drop(rx);
    tx
}

fn from_persisted_member(
    member: PersistedSwarmMember,
    snapshot_updated_at_unix_ms: u64,
    loaded_at_unix_ms: u64,
    terminal_retention: Duration,
) -> Option<SwarmMember> {
    let record = member.record;
    let original_status = record.status.as_str();
    let was_terminal_before_recovery =
        super::swarm::member_status_is_terminal(original_status.as_ref());
    let (status, detail) = recover_member_status(
        record.status,
        record.detail,
        record.is_headless,
        loaded_at_unix_ms,
    );
    let status_text = status.as_str();
    let terminal_since_unix_ms = super::swarm::member_status_is_terminal(status_text.as_ref())
        .then(|| {
            member
                .terminal_since_unix_ms
                .unwrap_or(if was_terminal_before_recovery {
                    snapshot_updated_at_unix_ms
                } else {
                    loaded_at_unix_ms
                })
        });
    if terminal_since_unix_ms.is_some_and(|terminal_since| {
        loaded_at_unix_ms.saturating_sub(terminal_since) >= terminal_retention.as_millis() as u64
    }) {
        return None;
    }

    let mut recovered = SwarmMember::from_record(
        SwarmMemberRecord {
            status,
            detail,
            ..record
        },
        recovered_member_event_tx(),
    );
    if let Some(terminal_since) = terminal_since_unix_ms {
        let terminal_age = Duration::from_millis(loaded_at_unix_ms.saturating_sub(terminal_since));
        recovered.last_status_change = Instant::now()
            .checked_sub(terminal_age)
            .unwrap_or_else(Instant::now);
    }
    Some(recovered)
}

pub(super) fn load_runtime_state() -> LoadedSwarmRuntimeState {
    migrate_legacy_state();
    let dir = state_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return LoadedSwarmRuntimeState {
            plans: HashMap::new(),
            coordinators: HashMap::new(),
            members: HashMap::new(),
            swarms_by_id: HashMap::new(),
        };
    };

    let mut plans = HashMap::new();
    let mut coordinators = HashMap::new();
    let mut members = HashMap::new();
    let mut swarms_by_id = HashMap::new();
    let loaded_at_unix_ms = now_unix_ms();
    let terminal_retention = super::swarm::swarm_terminal_member_retention();
    // Pending awaits persist absolute byte cursors into their swarm's control
    // log. Keep those logs until the await is finalized or its durable state
    // becomes stale, including awaits whose deadline elapsed while offline.
    let retained_control_logs =
        super::await_members_state::all_pending_await_members_including_expired()
            .into_iter()
            .map(|state| control_log_path(&state.swarm_id))
            .collect();
    prune_terminal_control_logs(&dir, terminal_retention, &retained_control_logs);
    let mut pruned_terminal_members = 0usize;
    let mut pruned_members_by_swarm: HashMap<String, HashSet<String>> = HashMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match classify_state_artifact(&path) {
            SwarmStateArtifactKind::Snapshot { .. } => {}
            SwarmStateArtifactKind::Backup { .. } if !path.with_extension("json").is_file() => {}
            SwarmStateArtifactKind::Backup { .. }
            | SwarmStateArtifactKind::ControlLog { .. }
            | SwarmStateArtifactKind::Temp
            | SwarmStateArtifactKind::Quarantine
            | SwarmStateArtifactKind::Other => continue,
        }
        let Ok(mut state) = read_swarm_snapshot_with_quarantine(&path) else {
            continue;
        };
        // W1 step 4: the snapshot is the compaction checkpoint, the log is
        // the source of truth. Replay control events past the snapshot's
        // covered offset over the persisted records BEFORE the recovery
        // transforms run, so control-plane changes that never reached a
        // snapshot write (status/role flips via broadcast_swarm_status)
        // survive the restart and still get the same crash-recovery pass.
        apply_control_log_tail(&mut state);
        let swarm_id = state.swarm_id.clone();
        if let Some(plan) = state.plan {
            plans.insert(
                swarm_id.clone(),
                from_persisted_plan(plan, state.updated_at_unix_ms),
            );
        }
        if let Some(coordinator_session_id) = state.coordinator_session_id {
            coordinators.insert(swarm_id, coordinator_session_id);
        }
        for member in state.members {
            let Some(member_swarm_id) = member.record.swarm_id.clone() else {
                continue;
            };
            let member_session_id = member.record.session_id.clone();
            let Some(member) = from_persisted_member(
                member,
                state.updated_at_unix_ms,
                loaded_at_unix_ms,
                terminal_retention,
            ) else {
                pruned_terminal_members += 1;
                pruned_members_by_swarm
                    .entry(member_swarm_id)
                    .or_default()
                    .insert(member_session_id);
                continue;
            };
            swarms_by_id
                .entry(member_swarm_id.clone())
                .or_insert_with(HashSet::new)
                .insert(member_session_id.clone());
            members.insert(member_session_id, member);
        }
    }
    coordinators.retain(|swarm_id, session_id| {
        !pruned_members_by_swarm
            .get(swarm_id)
            .is_some_and(|pruned| pruned.contains(session_id))
    });
    for (swarm_id, pruned_session_ids) in &pruned_members_by_swarm {
        if let Some(plan) = plans.get_mut(swarm_id) {
            plan.participants
                .retain(|session_id| !pruned_session_ids.contains(session_id));
        }
    }
    // Rewrite every affected snapshot once so startup collection shrinks the
    // durable state too. Without this, the same expired records would be parsed
    // and discarded on every restart forever.
    for swarm_id in pruned_members_by_swarm.keys() {
        let retained_members = swarms_by_id
            .get(swarm_id)
            .into_iter()
            .flat_map(|session_ids| session_ids.iter())
            .filter_map(|session_id| members.get(session_id).cloned())
            .collect::<Vec<_>>();
        persist_swarm_state(
            swarm_id,
            plans.get(swarm_id),
            coordinators.get(swarm_id).map(String::as_str),
            &retained_members,
            // The in-memory state was already replayed past the log tail
            // above, so the rewritten snapshot covers the whole current log.
            current_control_log_len(swarm_id),
        );
    }
    if pruned_terminal_members > 0 {
        crate::logging::info(&format!(
            "Pruned {pruned_terminal_members} expired terminal swarm member(s) while loading durable state"
        ));
    }
    LoadedSwarmRuntimeState {
        plans,
        coordinators,
        members,
        swarms_by_id,
    }
}

fn read_swarm_snapshot_with_quarantine(path: &Path) -> anyhow::Result<PersistedSwarmState> {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<PersistedSwarmState>(&bytes) {
            Ok(state) => Ok(state),
            Err(primary_error) => {
                quarantine_bytes(path, "snapshot", &bytes);
                let bak_path = path.with_extension("bak");
                // When the caller hands us a `.bak` directly (orphaned backup
                // whose primary `.json` is gone), `with_extension` yields the
                // SAME path: "recovering" would re-read the corrupt bytes,
                // quarantine the same file twice, and copy it onto itself.
                if bak_path == path {
                    return Err(anyhow::anyhow!(
                        "corrupt orphaned swarm backup {} ({primary_error})",
                        path.display(),
                    ));
                }
                let bak_bytes = std::fs::read(&bak_path)?;
                match serde_json::from_slice::<PersistedSwarmState>(&bak_bytes) {
                    Ok(state) => {
                        crate::logging::warn(&format!(
                            "swarm_snapshot_recovered_from_backup primary={} backup={} error={}",
                            path.display(),
                            bak_path.display(),
                            primary_error
                        ));
                        if let Err(error) = std::fs::copy(&bak_path, path) {
                            crate::logging::warn(&format!(
                                "swarm_snapshot_backup_restore_failed primary={} backup={} error={}",
                                path.display(),
                                bak_path.display(),
                                error
                            ));
                        }
                        Ok(state)
                    }
                    Err(backup_error) => {
                        quarantine_bytes(&bak_path, "snapshot-backup", &bak_bytes);
                        Err(anyhow::anyhow!(
                            "corrupt swarm snapshot {} ({primary_error}) and backup {} ({backup_error})",
                            path.display(),
                            bak_path.display()
                        ))
                    }
                }
            }
        },
        Err(error) => Err(error.into()),
    }
}

/// Replay the per-swarm control log past the snapshot's covered offset,
/// mutating the persisted records in place. Only swarms with a snapshot are
/// replayed (a missing snapshot means the swarm was retired; its log is kept
/// as an observation dataset, not as live state).
fn apply_control_log_tail(state: &mut PersistedSwarmState) {
    let path = control_log_path(&state.swarm_id);
    let Ok(read) = read_control_log_from(&path, state.control_log_covered_offset) else {
        return;
    };
    for corrupt in &read.corrupt_lines {
        let label = format!(
            "control-log-{}-{}",
            corrupt.start_offset, corrupt.end_offset
        );
        quarantine_bytes(&path, &label, &corrupt.bytes);
    }
    if read.envelopes.is_empty() {
        return;
    }
    crate::logging::info(&format!(
        "swarm {}: replaying {} control event(s) past snapshot offset {}",
        state.swarm_id,
        read.envelopes.len(),
        state.control_log_covered_offset
    ));
    for (_offset, envelope) in read.envelopes {
        if envelope.swarm_id != state.swarm_id {
            continue;
        }
        apply_control_event_to_snapshot(state, envelope.event);
    }
}

fn find_member_mut<'a>(
    state: &'a mut PersistedSwarmState,
    session_id: &str,
) -> Option<&'a mut SwarmMemberRecord> {
    state
        .members
        .iter_mut()
        .map(|member| &mut member.record)
        .find(|record| record.session_id == session_id)
}

fn apply_control_event_to_snapshot(state: &mut PersistedSwarmState, event: SwarmControlEvent) {
    match event {
        SwarmControlEvent::MemberJoined {
            session_id,
            friendly_name,
            role,
        } => {
            if let Some(record) = find_member_mut(state, &session_id) {
                record.role = SwarmRole::from(role);
                record.friendly_name = friendly_name;
                record.status = SwarmLifecycleStatus::Ready;
            } else {
                // A join the snapshot never saw. Restore it headless: the
                // session has no live client after a restart, so the
                // recovery pass will mark it crashed unless terminal -
                // truthful, and visible to salvage/reap flows instead of
                // silently vanishing.
                state.members.push(PersistedSwarmMember {
                    record: SwarmMemberRecord {
                        session_id,
                        working_dir: None,
                        swarm_id: Some(state.swarm_id.clone()),
                        swarm_enabled: true,
                        status: SwarmLifecycleStatus::Ready,
                        detail: None,
                        task_label: None,
                        subagent_type: None,
                        friendly_name,
                        report_back_to_session_id: None,
                        initial_prompt_delivered: None,
                        latest_completion_report: None,
                        role: SwarmRole::from(role),
                        is_headless: true,
                    },
                    terminal_since_unix_ms: None,
                });
            }
        }
        SwarmControlEvent::MemberLeft { session_id } => {
            state
                .members
                .retain(|member| member.record.session_id != session_id);
        }
        SwarmControlEvent::RoleChanged { session_id, role } => {
            if let Some(record) = find_member_mut(state, &session_id) {
                record.role = SwarmRole::from(role);
            }
        }
        SwarmControlEvent::MemberStatusChanged { session_id, status } => {
            if let Some(record) = find_member_mut(state, &session_id) {
                record.status = SwarmLifecycleStatus::from(status);
            }
        }
        SwarmControlEvent::MemberRenamed {
            session_id,
            friendly_name,
        } => {
            if let Some(record) = find_member_mut(state, &session_id) {
                record.friendly_name = friendly_name;
            }
        }
        SwarmControlEvent::TaskAssigned {
            task_id,
            assigned_to,
        } => {
            if let Some(plan) = state.plan.as_mut()
                && let Some(item) = plan.items.iter_mut().find(|item| item.id == task_id)
            {
                item.assigned_to = assigned_to.clone();
                plan.task_progress
                    .entry(task_id)
                    .or_default()
                    .assigned_session_id = assigned_to;
            }
        }
        SwarmControlEvent::TaskStatusChanged { task_id, status } => {
            if let Some(plan) = state.plan.as_mut()
                && let Some(item) = plan.items.iter_mut().find(|item| item.id == task_id)
            {
                item.status = status;
            }
        }
        SwarmControlEvent::TaskHeartbeat { task_id, wall_ms } => {
            if let Some(plan) = state.plan.as_mut() {
                plan.task_progress
                    .entry(task_id)
                    .or_default()
                    .last_heartbeat_unix_ms = Some(wall_ms);
            }
        }
        SwarmControlEvent::TaskRemoved { task_id } => {
            if let Some(plan) = state.plan.as_mut() {
                plan.items.retain(|item| item.id != task_id);
                plan.task_progress.remove(&task_id);
            }
        }
        SwarmControlEvent::ArtifactFiled { .. } => {
            // Evidence marker (W2). The snapshot's plan carries the full
            // artifact in node metadata already; nothing to reapply here.
        }
    }
}

pub(super) fn persist_swarm_state(
    swarm_id: &str,
    swarm_plan: Option<&VersionedPlan>,
    coordinator_session_id: Option<&str>,
    swarm_members: &[SwarmMember],
    control_log_covered_offset: u64,
) {
    let file_lock = swarm_file_lock(swarm_id);
    let _guard = file_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    if swarm_plan.is_none() && coordinator_session_id.is_none() && swarm_members.is_empty() {
        let _ = remove_snapshot_files(swarm_id);
        return;
    }

    // A snapshot can be captured before another task advances the plan and
    // reach disk afterwards. Never let that stale completion regress the
    // durable plan. Full member/coordinator ordering is provided by the
    // per-swarm operation lock around load_runtime + this write.
    if let Some(candidate_plan) = swarm_plan
        && let Ok(current) = storage::read_json::<PersistedSwarmState>(&state_path(swarm_id))
        && current
            .plan
            .as_ref()
            .is_some_and(|plan| plan.version > candidate_plan.version)
    {
        return;
    }

    let snapshot_unix_ms = now_unix_ms();
    let mut members = swarm_members
        .iter()
        .map(|member| to_persisted_member(member, snapshot_unix_ms))
        .collect::<Vec<_>>();
    members.sort_by(|left, right| left.record.session_id.cmp(&right.record.session_id));

    let state = PersistedSwarmState {
        swarm_id: swarm_id.to_string(),
        plan: swarm_plan.map(to_persisted_plan),
        coordinator_session_id: coordinator_session_id.map(str::to_string),
        members,
        updated_at_unix_ms: snapshot_unix_ms,
        control_log_covered_offset,
    };

    if let Err(err) = storage::write_json_fast(&state_path(swarm_id), &state) {
        crate::logging::warn(&format!(
            "Failed to persist swarm state {}: {}",
            swarm_id, err
        ));
    }
}

#[cfg(test)]
pub(super) fn remove_swarm_state(swarm_id: &str) {
    let file_lock = swarm_file_lock(swarm_id);
    let _guard = file_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = remove_snapshot_files(swarm_id);
}

pub(super) fn remove_swarm_state_if_version(
    swarm_id: &str,
    expected: &SwarmStateFileVersion,
) -> bool {
    let file_lock = swarm_file_lock(swarm_id);
    let _guard = file_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if &read_primary_version(swarm_id) != expected {
        return false;
    }
    remove_snapshot_files(swarm_id)
}

#[cfg(test)]
#[path = "swarm_persistence_tests.rs"]
mod swarm_persistence_tests;

#[cfg(test)]
#[path = "swarm_persistence_hygiene_tests.rs"]
mod swarm_persistence_hygiene_tests;
