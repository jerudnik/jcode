use super::swarm_persistence_tests::test_env;
use super::*;
use jcode_swarm_core::control_log::{ControlLogWriter, LOCAL_ORIGIN, SwarmControlEvent, replay};
use std::time::{Duration, Instant};

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
        std::fs::read(created).expect("new evidence"),
        b"new corrupt bytes"
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
}

#[test]
fn terminal_control_log_retention_removes_only_old_orphan_logs() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let previous_retention = std::env::var_os("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS");
    crate::env::set_var("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS", "1");
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

    if let Some(value) = previous_retention {
        crate::env::set_var("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS", value);
    } else {
        crate::env::remove_var("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS");
    }
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
    crate::env::set_var("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS", "1");

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
    crate::env::set_var("JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS", "1");

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

    let _ = load_runtime_state();
    assert_eq!(
        std::fs::read(&log).expect("pending await log must survive retention"),
        b"old log covered by a pending await cursor",
        "retention must not invalidate a persisted absolute-byte await cursor"
    );
}

#[tokio::test]
async fn deleting_swarm_state_resets_cached_control_log_before_replacement() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let _env = test_env(&dir);
    let swarm_id = "swarm-cache-reset";
    let first = member_for(swarm_id, "first", "agent", "ready");
    crate::server::control_log_sync::sync_swarm_control_log_members(swarm_id, &[first]);
    assert!(control_log_path(swarm_id).exists(), "initial log exists");
    let mut receiver = crate::server::control_log_sync::subscribe_control_log(swarm_id);
    let _current_offset = *receiver.borrow_and_update();

    remove_swarm_state(swarm_id);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), receiver.changed())
            .await
            .is_err(),
        "a live idle append notifier must remain pending, not close and hot-loop its await watcher"
    );
    crate::server::control_log_sync::sync_swarm_control_log_members(swarm_id, &[]);
    let (folded, _) = replay(&control_log_path(swarm_id)).unwrap_or_default();
    assert!(
        folded.members.is_empty(),
        "empty replacement sync must not emit cached ghost removals"
    );
}
