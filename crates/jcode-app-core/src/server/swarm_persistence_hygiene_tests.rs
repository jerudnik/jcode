use super::swarm_persistence_tests::test_env;
use super::*;
use jcode_swarm_core::control_log::{ControlLogWriter, LOCAL_ORIGIN, SwarmControlEvent, replay};
use std::time::{Duration, Instant};

struct RetentionEnvGuard(Option<std::ffi::OsString>);

impl RetentionEnvGuard {
    fn one_second() -> Self {
        let previous = std::env::var_os("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS");
        crate::env::set_var("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS", "1");
        Self(previous)
    }
}

impl Drop for RetentionEnvGuard {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            crate::env::set_var("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS", value);
        } else {
            crate::env::remove_var("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS");
        }
    }
}

fn member_for(swarm_id: &str, session_id: &str, role: &str, status: &str) -> SwarmMember {
    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    SwarmMember {
        session_id: session_id.to_string(),
        event_tx,
        event_txs: HashMap::new(),
        working_dir: None,
        swarm_id: Some(swarm_id.to_string()),
        swarm_enabled: true,
        status: status.to_string(),
        detail: None,
        task_label: None,
        subagent_type: None,
        friendly_name: Some(session_id.to_string()),
        report_back_to_session_id: None,
        initial_prompt_delivered: None,
        latest_completion_report: None,
        role: role.to_string(),
        joined_at: Instant::now(),
        last_status_change: Instant::now(),
        is_headless: true,
        output_tail: None,
        todo_progress: None,
        todo_items: Vec::new(),
        runtime: crate::protocol::SwarmMemberRuntime::default(),
    }
}

#[cfg(unix)]
fn set_mtime_secs_ago(path: &std::path::Path, secs_ago: i64) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let seconds = now - secs_ago;
    let c_path = CString::new(path.as_os_str().as_bytes()).expect("path cstring");
    let times = [
        libc::timespec {
            tv_sec: seconds,
            tv_nsec: 0,
        },
        libc::timespec {
            tv_sec: seconds,
            tv_nsec: 0,
        },
    ];
    let rc = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
    assert_eq!(rc, 0, "utimensat failed for {}", path.display());
}

fn quarantine_files(dir: &std::path::Path) -> Vec<Vec<u8>> {
    let _ = dir;
    let qdir = state_dir().join("quarantine");
    let Ok(entries) = std::fs::read_dir(qdir) else {
        return Vec::new();
    };
    let mut bytes = entries
        .flatten()
        .filter_map(|entry| std::fs::read(entry.path()).ok())
        .collect::<Vec<_>>();
    bytes.sort();
    bytes
}

#[test]
fn quarantine_collision_never_overwrites_existing_evidence() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let original = state_path("swarm-collision");
    let stamp = 42;
    let occupied = quarantine_path(&original, "snapshot", stamp, 0);
    std::fs::create_dir_all(occupied.parent().expect("quarantine parent")).expect("quarantine dir");
    std::fs::write(&occupied, b"existing quarantine evidence").expect("occupied quarantine");

    let created = quarantine_bytes_at_stamp(&original, "snapshot", b"new corrupt bytes", stamp)
        .expect("collision should advance to a new path");

    assert_ne!(created, occupied, "a collision must choose another path");
    assert_eq!(
        std::fs::read(&occupied).expect("existing evidence"),
        b"existing quarantine evidence",
        "quarantine creation must never truncate prior evidence"
    );
    assert_eq!(
        std::fs::read(&created).expect("new evidence"),
        b"new corrupt bytes"
    );
    assert_eq!(
        quarantine_bytes_at_stamp(&original, "snapshot", b"new corrupt bytes", stamp),
        Some(created),
        "repeated quarantine must reuse the byte-identical collision slot"
    );
}

#[test]
fn malformed_snapshot_matrix_quarantines_exact_bytes_and_recovers_when_possible() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    std::fs::create_dir_all(state_dir()).expect("state dir");

    let primary_bad = b"{not primary json".to_vec();
    let backup_bad = b"{not backup json".to_vec();
    let valid_backup = serde_json::json!({
        "swarm_id": "swarm-corrupt-primary",
        "coordinator_session_id": "coord-from-backup",
        "updated_at_unix_ms": 1u64
    });
    std::fs::write(state_path("swarm-corrupt-primary"), &primary_bad).expect("bad primary");
    std::fs::write(
        state_path("swarm-corrupt-primary").with_extension("bak"),
        serde_json::to_vec(&valid_backup).unwrap(),
    )
    .expect("valid backup");

    std::fs::write(state_path("swarm-both-corrupt"), b"{bad p").expect("bad primary");
    std::fs::write(
        state_path("swarm-both-corrupt").with_extension("bak"),
        &backup_bad,
    )
    .expect("bad backup");

    let valid_primary = serde_json::json!({
        "swarm_id": "swarm-valid-primary",
        "coordinator_session_id": "coord-primary",
        "updated_at_unix_ms": 2u64
    });
    std::fs::write(
        state_path("swarm-valid-primary"),
        serde_json::to_vec(&valid_primary).unwrap(),
    )
    .expect("valid primary");
    std::fs::write(
        state_path("swarm-valid-primary").with_extension("bak"),
        b"{stale corrupt backup",
    )
    .expect("stale backup");
    let mut adjacent_log = ControlLogWriter::open(
        &control_log_path("swarm-valid-primary"),
        "swarm-valid-primary",
        LOCAL_ORIGIN,
    )
    .expect("adjacent control log");
    adjacent_log
        .append(SwarmControlEvent::MemberLeft {
            session_id: "gone".into(),
        })
        .expect("valid adjacent control log line");
    let corrupt_log_line = b"{malformed control log envelope}\n";
    let log_path = control_log_path("swarm-valid-primary");
    let mut log_bytes = std::fs::read(&log_path).expect("valid adjacent log bytes");
    log_bytes.extend_from_slice(corrupt_log_line);
    std::fs::write(&log_path, log_bytes).expect("complete corrupt control log line");

    let loaded = load_runtime_state();
    assert_eq!(
        loaded.coordinators.get("swarm-corrupt-primary"),
        Some(&"coord-from-backup".to_string()),
        "malformed primary should recover from valid backup"
    );
    assert_eq!(
        loaded.coordinators.get("swarm-valid-primary"),
        Some(&"coord-primary".to_string()),
        "valid primary must win without parsing stale backup"
    );
    assert!(
        !loaded.coordinators.contains_key("swarm-both-corrupt"),
        "malformed primary + malformed backup must not synthesize state"
    );

    let quarantined = quarantine_files(dir.path());
    assert!(
        quarantined.contains(&primary_bad),
        "primary corrupt bytes preserved exactly"
    );
    assert!(
        quarantined.contains(&backup_bad),
        "backup corrupt bytes preserved exactly"
    );
    assert!(
        !quarantined.iter().any(|bytes| bytes
            .windows(b"member_left".len())
            .any(|w| w == b"member_left")),
        "adjacent control log must not be treated as a malformed snapshot"
    );
    assert!(
        quarantined
            .iter()
            .any(|bytes| bytes.as_slice() == corrupt_log_line),
        "complete corrupt control-log bytes must be quarantined exactly"
    );
    let _ = load_runtime_state();
    assert_eq!(
        quarantine_files(dir.path()),
        quarantined,
        "repeated startup must reuse byte-identical quarantine evidence"
    );
}

#[test]
fn terminal_control_log_retention_preserves_old_log_with_orphan_backup() {
    // F27 gap F25-1: a crash shape with the primary `.json` gone but a valid
    // `.bak` snapshot present must NOT lose its control-log tail. Startup
    // recovery loads the backup when the primary is missing, so the log holds
    // the events past the backup's covered offset. Pruning it on the strength
    // of a missing `.json` alone would silently drop replayable state.
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let _retention = RetentionEnvGuard::one_second();
    std::fs::create_dir_all(state_dir()).expect("state dir");

    let swarm_id = "orphan-bak-with-tail";
    // Valid orphan backup: no primary `.json`, only the `.bak` snapshot.
    let backup = serde_json::json!({
        "swarm_id": swarm_id,
        "coordinator_session_id": "coord-from-orphan-bak",
        "updated_at_unix_ms": 1u64
    });
    std::fs::write(
        state_path(swarm_id).with_extension("bak"),
        serde_json::to_vec(&backup).unwrap(),
    )
    .expect("orphan backup");

    // An old control log carrying a replayable event past the snapshot.
    let mut writer = ControlLogWriter::open(&control_log_path(swarm_id), swarm_id, LOCAL_ORIGIN)
        .expect("open control log");
    writer
        .append(SwarmControlEvent::MemberLeft {
            session_id: "tail-event".into(),
        })
        .expect("append tail event");
    drop(writer);
    // Age the log past the retention window so a count-only guard would prune it.
    std::thread::sleep(Duration::from_millis(1200));

    let loaded = load_runtime_state();
    // The orphan backup must still load as coordinator state.
    assert_eq!(
        loaded.coordinators.get(swarm_id),
        Some(&"coord-from-orphan-bak".to_string()),
        "valid orphan backup should load"
    );
    // The control-log tail must survive pruning and remain replayable.
    assert!(
        control_log_path(swarm_id).exists(),
        "control-log tail must be preserved when a valid orphan backup exists"
    );
    let (folded, _) = replay(&control_log_path(swarm_id)).expect("tail still replayable");
    assert_eq!(
        folded.events_applied, 1,
        "the tail event past the backup offset must survive"
    );
}

#[test]
fn terminal_control_log_retention_removes_only_old_orphan_logs() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let _retention = RetentionEnvGuard::one_second();
    std::fs::create_dir_all(state_dir()).expect("state dir");

    std::fs::write(control_log_path("old-orphan"), b"old").expect("old orphan log");
    std::thread::sleep(Duration::from_millis(1200));
    std::fs::write(control_log_path("young-orphan"), b"young").expect("young orphan log");
    let active_snapshot = serde_json::json!({
        "swarm_id": "active-old",
        "coordinator_session_id": "coord-active",
        "updated_at_unix_ms": 1u64
    });
    std::fs::write(
        state_path("active-old"),
        serde_json::to_vec(&active_snapshot).unwrap(),
    )
    .expect("active snapshot");
    std::fs::write(control_log_path("active-old"), b"active old").expect("active log");
    std::fs::write(state_dir().join("unrelated.jsonl"), b"keep").expect("unrelated jsonl");

    let loaded = load_runtime_state();
    assert_eq!(
        loaded.coordinators.get("active-old"),
        Some(&"coord-active".to_string())
    );
    assert!(
        !control_log_path("old-orphan").exists(),
        "old terminal/orphan log should be removed"
    );
    assert!(
        control_log_path("young-orphan").exists(),
        "young terminal/orphan log should be preserved"
    );
    assert!(
        control_log_path("active-old").exists(),
        "active log with snapshot should be preserved regardless of age"
    );
    assert!(
        state_dir().join("unrelated.jsonl").exists(),
        "unrelated JSONL must be preserved"
    );
}

#[test]
fn malformed_snapshot_quarantines_exact_bytes_and_recovers_from_valid_backup() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let swarm_id = "swarm-corrupt-primary";
    let member = member_for(swarm_id, "coord", "coordinator", "ready");

    persist_swarm_state(
        swarm_id,
        None,
        Some("coord"),
        std::slice::from_ref(&member),
        0,
    );
    let primary = state_path(swarm_id);
    let backup = primary.with_extension("bak");
    std::fs::copy(&primary, &backup).expect("seed backup");
    let corrupt = b"{not valid snapshot json";
    std::fs::write(&primary, corrupt).expect("corrupt primary");

    let loaded = load_runtime_state();
    assert_eq!(
        loaded.coordinators.get(swarm_id),
        Some(&"coord".to_string())
    );
    let quarantine_dir = state_dir().join("quarantine");
    let quarantined = std::fs::read_dir(&quarantine_dir)
        .expect("quarantine dir")
        .flatten()
        .map(|entry| std::fs::read(entry.path()).expect("quarantined bytes"))
        .collect::<Vec<_>>();
    assert!(
        quarantined.iter().any(|bytes| bytes == corrupt),
        "exact corrupt primary bytes should survive quarantine"
    );
}

#[test]
fn corrupt_orphaned_backup_is_quarantined_once_without_self_recovery() {
    // An orphaned `.bak` (its primary `.json` is gone) is read directly by
    // load_runtime_state. with_extension("bak") on a `.bak` path yields the
    // SAME path, so the recovery branch would re-read the corrupt bytes,
    // quarantine the same file twice, and copy it onto itself. It must fail
    // cleanly with exactly one quarantine entry instead.
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let swarm_id = "swarm-orphan-bak";
    let primary = state_path(swarm_id);
    let backup = primary.with_extension("bak");
    std::fs::create_dir_all(state_dir()).expect("state dir");
    std::fs::write(&backup, b"orphan bad").expect("bad orphan backup");

    let loaded = load_runtime_state();
    assert!(!loaded.coordinators.contains_key(swarm_id));
    let quarantined = quarantine_files(&state_dir().join("quarantine"));
    assert_eq!(
        quarantined
            .iter()
            .filter(|bytes| bytes.as_slice() == b"orphan bad")
            .count(),
        1,
        "orphaned backup must be quarantined exactly once"
    );
    // The corrupt orphan must not have been "restored" onto itself as a
    // fresh primary.
    assert!(!primary.exists(), "no primary must be fabricated");
}

#[test]
fn malformed_snapshot_and_backup_quarantine_both_without_losing_adjacent_control_log() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let swarm_id = "swarm-corrupt-both";
    let primary = state_path(swarm_id);
    let backup = primary.with_extension("bak");
    std::fs::create_dir_all(state_dir()).expect("state dir");
    std::fs::write(&primary, b"primary bad").expect("bad primary");
    std::fs::write(&backup, b"backup bad").expect("bad backup");
    let mut writer = ControlLogWriter::open(&control_log_path(swarm_id), swarm_id, LOCAL_ORIGIN)
        .expect("open adjacent log");
    writer
        .append(SwarmControlEvent::MemberLeft {
            session_id: "x".into(),
        })
        .expect("append adjacent log");

    let loaded = load_runtime_state();
    assert!(!loaded.coordinators.contains_key(swarm_id));
    let quarantined = std::fs::read_dir(state_dir().join("quarantine"))
        .expect("quarantine dir")
        .flatten()
        .map(|entry| std::fs::read(entry.path()).expect("quarantined bytes"))
        .collect::<Vec<_>>();
    assert!(quarantined.iter().any(|bytes| bytes == b"primary bad"));
    assert!(quarantined.iter().any(|bytes| bytes == b"backup bad"));
    let (folded, _) = replay(&control_log_path(swarm_id)).expect("adjacent log still readable");
    assert_eq!(folded.events_applied, 1);
}

#[cfg(unix)]
#[test]
fn terminal_control_log_retention_preserves_active_and_young_logs() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let _retention = RetentionEnvGuard::one_second();

    let active = "active-old";
    persist_swarm_state(
        active,
        None,
        Some("coord"),
        &[member_for(active, "coord", "coordinator", "ready")],
        0,
    );
    std::fs::write(control_log_path(active), b"active old log").expect("active log");
    set_mtime_secs_ago(&control_log_path(active), 5);

    let old_terminal = control_log_path("old-terminal");
    std::fs::write(&old_terminal, b"old terminal log").expect("old terminal");
    set_mtime_secs_ago(&old_terminal, 5);

    let young_terminal = control_log_path("young-terminal");
    std::fs::write(&young_terminal, b"young terminal log").expect("young terminal");
    set_mtime_secs_ago(&young_terminal, 0);

    let unrelated = state_dir().join("unrelated.jsonl");
    std::fs::write(&unrelated, b"unrelated").expect("unrelated");
    set_mtime_secs_ago(&unrelated, 5);

    let _ = load_runtime_state();
    assert!(
        control_log_path(active).exists(),
        "active old log must be preserved"
    );
    assert!(!old_terminal.exists(), "old terminal log must be pruned");
    assert!(
        young_terminal.exists(),
        "young terminal log must be preserved"
    );
    assert!(unrelated.exists(), "unrelated JSONL must be preserved");
}

#[cfg(unix)]
#[test]
fn terminal_control_log_retention_preserves_pending_await_cursor() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let _retention = RetentionEnvGuard::one_second();

    let swarm_id = "awaiting-old";
    let log = control_log_path(swarm_id);
    std::fs::create_dir_all(state_dir()).expect("state dir");
    std::fs::write(&log, b"old log covered by a pending await cursor").expect("old log");
    set_mtime_secs_ago(&log, 5);

    let pending = crate::server::await_members_state::PersistedAwaitMembersState {
        key: "awaiting-old-key".to_string(),
        session_id: "requester".to_string(),
        swarm_id: swarm_id.to_string(),
        target_status: vec!["completed".to_string()],
        requested_ids: vec!["worker".to_string()],
        mode: Some("all".to_string()),
        created_at_unix_ms: now_unix_ms(),
        deadline_unix_ms: now_unix_ms() + 60_000,
        background: true,
        notify: true,
        wake: true,
        scan_offset: 8,
        final_response: None,
    };
    crate::server::await_members_state::save_state(&pending);

    remove_swarm_state(swarm_id);
    assert_eq!(
        std::fs::read(&log).expect("pending await log must survive dissolution"),
        b"old log covered by a pending await cursor",
        "dissolution must not invalidate a persisted absolute-byte await cursor"
    );
    let _ = load_runtime_state();
    assert_eq!(
        std::fs::read(&log).expect("pending await log must survive retention"),
        b"old log covered by a pending await cursor",
        "retention must not invalidate a persisted absolute-byte await cursor"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn pruning_control_log_resets_cached_writer_without_closing_live_notifier() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let _retention = RetentionEnvGuard::one_second();
    let swarm_id = "swarm-cache-reset";
    let first = member_for(swarm_id, "first", "agent", "ready");
    crate::server::control_log_sync::sync_swarm_control_log_members(swarm_id, &[first]);
    let log = control_log_path(swarm_id);
    assert!(log.exists(), "initial log exists");
    set_mtime_secs_ago(&log, 5);
    let mut receiver = crate::server::control_log_sync::subscribe_control_log(swarm_id);
    let _current_offset = *receiver.borrow_and_update();

    let _ = load_runtime_state();
    assert!(!log.exists(), "old orphan log must be pruned");
    assert!(
        tokio::time::timeout(Duration::from_millis(10), receiver.changed())
            .await
            .is_err(),
        "a live idle append notifier must remain pending, not close and hot-loop its await watcher"
    );
    crate::server::control_log_sync::sync_swarm_control_log_members(swarm_id, &[]);
    assert!(
        !log.exists(),
        "empty replacement sync must not recreate the pruned log with a cached ghost removal"
    );
}

/// Delete-vs-write interleaving between `remove_persisted_swarm_state_for`
/// and a concurrent persist (wiring-audit.bak-resurrection, part b).
///
/// `remove_persisted_swarm_state_for` (server.rs:120) is `load_runtime()
/// .await` followed by an unserialized `remove_swarm_state`. Like the
/// persist inversion race above, `load_runtime` observes the four state
/// maps across multiple await points, so a remover that saw an all-empty
/// (dissolved) runtime can park, lose the race to a swarm re-creation plus
/// persist, then resume and delete the FRESH snapshot the re-creation just
/// wrote. Two failures compound:
///   1. Orphaned live swarm: the recreated swarm (coordinator registered
///      in memory) has no primary snapshot, so a clean restart loses it.
///   2. Zombie resurrection: the persist that the remover clobbered
///      hard-linked the PRE-dissolution snapshot to `.bak`, and
///      `load_runtime_state` reads `.bak` files, so restart restores the
///      stale pre-dissolution state instead.
///
/// Same gate technique as
/// `stale_persist_cannot_regress_newer_plan_version`:
/// park A inside `load_runtime` at the contended `members.read()`, run
/// mutator B's re-creation and persist while A is parked, release A.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn stale_remove_cannot_delete_fresh_snapshot_or_restore_backup() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);

    // The previous incarnation's snapshot is on disk; the swarm has since
    // been dissolved, so the in-memory runtime is empty.
    persist_swarm_state("swarm-del-race", None, Some("coord-stale"), &[], 0);
    let swarm_state = crate::server::SwarmState::new(
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );

    // Gate: hold members.write() so remover A parks inside load_runtime at
    // the final members.read(), AFTER it has already observed the
    // dissolved (all-empty) plans/coordinators/swarms_by_id state.
    let gate = swarm_state.members.write().await;

    let a = tokio::spawn({
        let swarm_state = swarm_state.clone();
        async move {
            crate::server::remove_persisted_swarm_state_for("swarm-del-race", &swarm_state).await;
        }
    });
    // Current-thread test runtime: yielding runs A until it parks on the
    // contended members.read().await.
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }

    // Mutator B: the swarm is recreated while A is parked. B registers a
    // new coordinator in memory ...
    {
        let mut coordinators = swarm_state.coordinators.write().await;
        coordinators.insert("swarm-del-race".to_string(), "coord-new".to_string());
    }
    // ... and B's persist half runs to completion (in production this is
    // B's own persist_swarm_state_for on another worker thread, whose
    // uncontended lock reads resolve without suspending). This overwrite
    // also hard-links the stale pre-dissolution snapshot to `.bak`.
    persist_swarm_state("swarm-del-race", None, Some("coord-new"), &[], 0);
    let on_disk = storage::read_json::<PersistedSwarmState>(&state_path("swarm-del-race"))
        .expect("fresh snapshot");
    assert_eq!(
        on_disk.coordinator_session_id.as_deref(),
        Some("coord-new"),
        "fresh snapshot must be durably on disk before A resumes"
    );

    // Release A: its stale all-empty runtime passes has_any_state(), but the
    // compare-and-delete guard must notice that the durable snapshot changed.
    drop(gate);
    a.await.expect("remove task");

    assert!(
        state_path("swarm-del-race").exists(),
        "a stale remove must not delete a freshly persisted snapshot"
    );
    let loaded = load_runtime_state();
    assert_eq!(
        loaded.coordinators.get("swarm-del-race"),
        Some(&"coord-new".to_string()),
        "restart must restore the fresh incarnation, not its stale backup"
    );
}

/// W1 step 4: restart recovery replays control-log events past the
/// snapshot's covered offset. A status flip, a role handoff, and a task
/// status change that happened AFTER the last snapshot write (e.g. via
/// broadcast_swarm_status, which never persists) must survive the restart.
#[test]
fn recovery_replays_control_log_tail_past_snapshot_offset() {
    use jcode_swarm_core::control_log::{ControlLogWriter, LOCAL_ORIGIN, SwarmControlEvent};

    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let swarm_id = "swarm-log-tail";

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let make_member = |session_id: &str, role: &str, status: &str| SwarmMember {
        session_id: session_id.to_string(),
        event_tx: event_tx.clone(),
        event_txs: HashMap::new(),
        working_dir: None,
        swarm_id: Some(swarm_id.to_string()),
        swarm_enabled: true,
        status: status.to_string(),
        detail: None,
        task_label: None,
        subagent_type: None,
        friendly_name: Some(session_id.to_string()),
        report_back_to_session_id: None,
        initial_prompt_delivered: None,
        latest_completion_report: None,
        role: role.to_string(),
        joined_at: Instant::now(),
        last_status_change: Instant::now(),
        is_headless: false,
        output_tail: None,
        todo_progress: None,
        todo_items: Vec::new(),
        runtime: crate::protocol::SwarmMemberRuntime::default(),
    };
    let members = vec![
        make_member("coord-1", "coordinator", "ready"),
        make_member("worker-1", "agent", "ready"),
    ];

    let plan = VersionedPlan {
        items: vec![crate::plan::PlanItem {
            content: "task one".to_string(),
            status: "queued".to_string(),
            priority: "high".to_string(),
            id: "t1".to_string(),
            subsystem: None,
            file_scope: Vec::new(),
            blocked_by: Vec::new(),
            assigned_to: None,
        }],
        version: 1,
        participants: ["coord-1".to_string()].into_iter().collect(),
        task_progress: HashMap::new(),
        mode: "light".to_string(),
        node_meta: HashMap::new(),
        max_nodes: None,
        frozen: false,
            safety_ledger: None,
    };

    // Write the log prefix the snapshot will cover.
    let log_path = control_log_path(swarm_id);
    let mut writer = ControlLogWriter::open(&log_path, swarm_id, LOCAL_ORIGIN).expect("open log");
    for event in [
        SwarmControlEvent::MemberJoined {
            session_id: "coord-1".to_string(),
            friendly_name: Some("coord-1".to_string()),
            role: "coordinator".to_string(),
        },
        SwarmControlEvent::MemberJoined {
            session_id: "worker-1".to_string(),
            friendly_name: Some("worker-1".to_string()),
            role: "agent".to_string(),
        },
        SwarmControlEvent::TaskAssigned {
            task_id: "t1".to_string(),
            assigned_to: None,
        },
        SwarmControlEvent::TaskStatusChanged {
            task_id: "t1".to_string(),
            status: "queued".to_string(),
        },
    ] {
        writer.append(event).expect("append prefix");
    }
    let covered_offset = std::fs::metadata(&log_path).expect("log meta").len();

    // Snapshot covering exactly that prefix.
    persist_swarm_state(
        swarm_id,
        Some(&plan),
        Some("coord-1"),
        &members,
        covered_offset,
    );

    // Post-snapshot events that never reached a snapshot write.
    for event in [
        SwarmControlEvent::TaskAssigned {
            task_id: "t1".to_string(),
            assigned_to: Some("worker-1".to_string()),
        },
        SwarmControlEvent::TaskStatusChanged {
            task_id: "t1".to_string(),
            status: "completed".to_string(),
        },
        SwarmControlEvent::MemberStatusChanged {
            session_id: "worker-1".to_string(),
            status: "completed".to_string(),
        },
        SwarmControlEvent::RoleChanged {
            session_id: "worker-1".to_string(),
            role: "coordinator".to_string(),
        },
        SwarmControlEvent::RoleChanged {
            session_id: "coord-1".to_string(),
            role: "agent".to_string(),
        },
    ] {
        writer.append(event).expect("append tail");
    }

    let loaded = load_runtime_state();

    // Task tail replayed over the snapshot plan.
    let plan = loaded.plans.get(swarm_id).expect("plan restored");
    let item = plan.items.iter().find(|item| item.id == "t1").expect("t1");
    assert_eq!(
        item.assigned_to.as_deref(),
        Some("worker-1"),
        "post-snapshot assignment must survive restart"
    );
    assert_eq!(
        item.status, "completed",
        "post-snapshot task completion must survive restart"
    );

    // Member tail replayed: role handoff visible after restart.
    assert_eq!(
        loaded.members.get("worker-1").map(|m| m.role.as_str()),
        Some("coordinator"),
        "post-snapshot role handoff must survive restart"
    );
    assert_eq!(
        loaded.members.get("coord-1").map(|m| m.role.as_str()),
        Some("agent"),
        "post-snapshot demotion must survive restart"
    );
    assert_eq!(
        loaded
            .members
            .get("worker-1")
            .map(|m| jcode_swarm_core::MemberLifecycleState::from_compatibility_status(&m.status)),
        Some(jcode_swarm_core::MemberLifecycleState::Succeeded),
        "post-snapshot terminal status must survive restart (terminal states \
         are exempt from the crash-recovery rewrite); the control log still \
         spells this \"completed\" and reads back as the same state"
    );
}

/// Pre-W1 snapshots have no covered offset (serde default 0): the whole log
/// replays over them, which must be idempotent, not corrupting.
#[test]
fn recovery_with_legacy_snapshot_replays_whole_log_idempotently() {
    use jcode_swarm_core::control_log::{ControlLogWriter, LOCAL_ORIGIN, SwarmControlEvent};

    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let swarm_id = "swarm-legacy";

    let (event_tx, _event_rx) = tokio::sync::mpsc::unbounded_channel();
    let members = vec![SwarmMember {
        session_id: "coord-1".to_string(),
        event_tx,
        event_txs: HashMap::new(),
        working_dir: None,
        swarm_id: Some(swarm_id.to_string()),
        swarm_enabled: true,
        status: "ready".to_string(),
        detail: None,
        task_label: None,
        subagent_type: None,
        friendly_name: Some("owl".to_string()),
        report_back_to_session_id: None,
        initial_prompt_delivered: None,
        latest_completion_report: None,
        role: "coordinator".to_string(),
        joined_at: Instant::now(),
        last_status_change: Instant::now(),
        is_headless: false,
        output_tail: None,
        todo_progress: None,
        todo_items: Vec::new(),
        runtime: crate::protocol::SwarmMemberRuntime::default(),
    }];

    // Log fully agrees with the snapshot (dual-write in steady state)...
    let log_path = control_log_path(swarm_id);
    let mut writer = ControlLogWriter::open(&log_path, swarm_id, LOCAL_ORIGIN).expect("open log");
    writer
        .append(SwarmControlEvent::MemberJoined {
            session_id: "coord-1".to_string(),
            friendly_name: Some("owl".to_string()),
            role: "coordinator".to_string(),
        })
        .expect("append");
    // ...but the snapshot predates W1 and carries offset 0.
    persist_swarm_state(swarm_id, None, Some("coord-1"), &members, 0);

    let loaded = load_runtime_state();
    assert_eq!(
        loaded.members.get("coord-1").map(|m| m.role.as_str()),
        Some("coordinator")
    );
    assert_eq!(
        loaded
            .members
            .get("coord-1")
            .and_then(|m| m.friendly_name.as_deref()),
        Some("owl")
    );
    assert_eq!(loaded.members.len(), 1, "replay must not duplicate members");
}
