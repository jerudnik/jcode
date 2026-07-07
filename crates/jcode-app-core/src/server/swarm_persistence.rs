use super::{SwarmMember, SwarmTaskProgress, VersionedPlan};
use crate::protocol::ServerEvent;
use crate::storage;
use jcode_swarm_core::control_log::{SwarmControlEvent, read_from as read_control_log_from};
use jcode_swarm_core::{SwarmLifecycleStatus, SwarmMemberRecord, SwarmRole};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Directory name under the durable state dir (`~/.jcode/state`).
const SWARM_STATE_DIR: &str = "swarm";
/// Pre-0.36 location under the runtime dir (tmpfs on Linux, wiped on reboot).
const LEGACY_SWARM_STATE_DIR: &str = "jcode-swarm-state";

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

fn from_persisted_plan(mut plan: PersistedVersionedPlan, updated_at_unix_ms: u64) -> VersionedPlan {
    let mut plan = VersionedPlan {
        items: std::mem::take(&mut plan.items),
        version: plan.version,
        participants: std::mem::take(&mut plan.participants).into_iter().collect(),
        task_progress: std::mem::take(&mut plan.task_progress),
        mode: std::mem::take(&mut plan.mode),
        node_meta: std::mem::take(&mut plan.node_meta),
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
    }
}

fn to_persisted_member(member: &SwarmMember) -> PersistedSwarmMember {
    PersistedSwarmMember {
        record: member.durable_record(),
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
) -> (SwarmLifecycleStatus, Option<String>) {
    if status == SwarmLifecycleStatus::Running {
        return (
            SwarmLifecycleStatus::Crashed,
            append_recovery_detail(detail, "recovered after reload while running"),
        );
    }

    // Ready/Done headless members finished their work before the reload:
    // nothing in-flight was lost, their completion report is preserved, and
    // startup recovery re-registers the agent, so the reload is invisible to
    // them. Marking them crashed here is wrong and races ahead of recovery,
    // making cleanly-finished workers report as "(crashed)" to await_members
    // watchers that resume before recovery rewrites the status (#swarm).
    if is_headless
        && !matches!(
            status,
            SwarmLifecycleStatus::Ready
                | SwarmLifecycleStatus::Completed
                | SwarmLifecycleStatus::Done
                | SwarmLifecycleStatus::Failed
                | SwarmLifecycleStatus::Stopped
        )
    {
        return (
            SwarmLifecycleStatus::Crashed,
            append_recovery_detail(detail, "headless session did not survive reload"),
        );
    }

    (status, detail)
}

fn recovered_member_event_tx() -> mpsc::UnboundedSender<ServerEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    drop(rx);
    tx
}

fn from_persisted_member(member: PersistedSwarmMember) -> SwarmMember {
    let record = member.record;
    let (status, detail) = recover_member_status(record.status, record.detail, record.is_headless);
    SwarmMember::from_record(
        SwarmMemberRecord {
            status,
            detail,
            ..record
        },
        recovered_member_event_tx(),
    )
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
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        // `.bak` files are corruption-recovery fallbacks, not co-equal
        // snapshots. When the primary `.json` still exists, reading the
        // `.bak` alongside it can resurrect state the primary deliberately
        // dropped (e.g. a cleared plan: the rotate-on-write keeps the old
        // plan-bearing snapshot as `.bak`, and a union-load would re-insert
        // that plan forever). `read_json` already falls back to the `.bak`
        // internally when the primary is corrupt, so skipping it here loses
        // nothing.
        if path.extension().and_then(|ext| ext.to_str()) == Some("bak")
            && path.with_extension("json").is_file()
        {
            continue;
        }
        let Ok(mut state) = storage::read_json::<PersistedSwarmState>(&path) else {
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
            swarms_by_id
                .entry(member_swarm_id.clone())
                .or_insert_with(HashSet::new)
                .insert(member.record.session_id.clone());
            members.insert(
                member.record.session_id.clone(),
                from_persisted_member(member),
            );
        }
    }
    LoadedSwarmRuntimeState {
        plans,
        coordinators,
        members,
        swarms_by_id,
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
                        latest_completion_report: None,
                        role: SwarmRole::from(role),
                        is_headless: true,
                    },
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
    if swarm_plan.is_none() && coordinator_session_id.is_none() && swarm_members.is_empty() {
        let _ = std::fs::remove_file(state_path(swarm_id));
        return;
    }

    let mut members = swarm_members
        .iter()
        .map(to_persisted_member)
        .collect::<Vec<_>>();
    members.sort_by(|left, right| left.record.session_id.cmp(&right.record.session_id));

    let state = PersistedSwarmState {
        swarm_id: swarm_id.to_string(),
        plan: swarm_plan.map(to_persisted_plan),
        coordinator_session_id: coordinator_session_id.map(str::to_string),
        members,
        updated_at_unix_ms: now_unix_ms(),
        control_log_covered_offset,
    };

    if let Err(err) = storage::write_json_fast(&state_path(swarm_id), &state) {
        crate::logging::warn(&format!(
            "Failed to persist swarm state {}: {}",
            swarm_id, err
        ));
    }
}

pub(super) fn remove_swarm_state(swarm_id: &str) {
    let _ = std::fs::remove_file(state_path(swarm_id));
}

#[cfg(test)]
#[path = "swarm_persistence_tests.rs"]
mod swarm_persistence_tests;
