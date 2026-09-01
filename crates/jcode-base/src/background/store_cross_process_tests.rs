use super::model::TaskStatusFile;
use super::store::{TaskStatusStore, TerminalWriteOutcome};
use crate::bus::{
    BackgroundTaskProgress, BackgroundTaskProgressKind, BackgroundTaskProgressSource,
    BackgroundTaskStatus,
};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const STALE_WRITER_PROBE: &str =
    "background::store_cross_process_tests::stale_running_writer_probe";
const ABORTING_LOCK_HOLDER_PROBE: &str =
    "background::store_cross_process_tests::aborting_lock_holder_probe";
const SLEEPING_LOCK_HOLDER_PROBE: &str =
    "background::store_cross_process_tests::sleeping_lock_holder_probe";

fn running_status(task_id: &str) -> TaskStatusFile {
    TaskStatusFile {
        task_id: task_id.to_string(),
        tool_name: "bash".into(),
        display_name: None,
        session_id: "w6-test".into(),
        status: BackgroundTaskStatus::Running,
        exit_code: None,
        error: None,
        started_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
        duration_secs: None,
        pid: None,
        owner_pid: None,
        owner_instance: None,
        detached: false,
        notify: false,
        wake: false,
        progress: None,
        event_history: Vec::new(),
    }
}

fn completed_status(existing: Option<TaskStatusFile>, task_id: &str) -> TaskStatusFile {
    let mut status = existing.unwrap_or_else(|| running_status(task_id));
    status.status = BackgroundTaskStatus::Completed;
    status.exit_code = Some(0);
    status.completed_at = Some(chrono::Utc::now().to_rfc3339());
    status
}

async fn wait_for_path(path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if path.exists() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn probe_command(test_name: &str, dir: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("current test binary"));
    command
        .args(["--ignored", "--exact", test_name])
        .env("JCODE_W6_DIR", dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_survives_stale_cross_process_running_write() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let store = TaskStatusStore::new(tmp.path().to_path_buf());
    store
        .write_initial(&running_status("t1"))
        .await
        .expect("write initial status");

    let mut child = probe_command(STALE_WRITER_PROBE, tmp.path())
        .spawn()
        .expect("spawn stale writer probe");

    let read_done = tmp.path().join("read-done");
    assert!(
        wait_for_path(&read_done, Duration::from_secs(10)).await,
        "child never reached the read"
    );

    assert_eq!(
        store
            .write_terminal("t1", |existing| completed_status(existing, "t1"))
            .await
            .expect("write terminal status"),
        TerminalWriteOutcome::Written
    );
    std::fs::write(tmp.path().join("gate-open"), b"").expect("open child gate");

    let child_status = child.wait().expect("join stale writer probe");
    assert!(child_status.success(), "stale writer probe failed");

    let child_branch =
        std::fs::read_to_string(tmp.path().join("child-branch")).expect("read child branch");
    let on_disk = store
        .read("t1")
        .await
        .expect("read final status")
        .expect("final status exists");
    assert_eq!(
        on_disk.status,
        BackgroundTaskStatus::Completed,
        "terminal state was replaced; child branch: {child_branch}"
    );
    assert_eq!(
        child_branch, "timed-out",
        "post-fix parent must block until the child's bounded gate wait expires"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crash_releases_cross_process_lock() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let store = TaskStatusStore::new(tmp.path().to_path_buf());
    store
        .write_initial(&running_status("crash"))
        .await
        .expect("write initial status");

    let mut child = probe_command(ABORTING_LOCK_HOLDER_PROBE, tmp.path())
        .spawn()
        .expect("spawn aborting holder");
    assert!(
        wait_for_path(&tmp.path().join("held"), Duration::from_secs(10)).await,
        "child never acquired the lock"
    );

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        store.write_terminal("crash", |existing| completed_status(existing, "crash")),
    )
    .await;
    let outcome = match outcome {
        Ok(outcome) => outcome.expect("write terminal status"),
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("terminal write remained blocked after child death");
        }
    };
    assert_eq!(outcome, TerminalWriteOutcome::Written);
    assert!(
        !child.wait().expect("join aborting holder").success(),
        "aborting holder unexpectedly exited successfully"
    );
    assert_eq!(
        store
            .read("crash")
            .await
            .expect("read final status")
            .expect("final status exists")
            .status,
        BackgroundTaskStatus::Completed
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconciler_degrades_to_read_only_under_contention() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let store = TaskStatusStore::new(tmp.path().to_path_buf());
    let mut orphan = running_status("contended");
    orphan.owner_pid = Some(u32::MAX);
    orphan.owner_instance = Some("dead-process-instance".into());
    store
        .write_initial(&orphan)
        .await
        .expect("write orphan fixture");

    let mut child = probe_command(SLEEPING_LOCK_HOLDER_PROBE, tmp.path())
        .spawn()
        .expect("spawn sleeping holder");
    assert!(
        wait_for_path(&tmp.path().join("held"), Duration::from_secs(10)).await,
        "child never acquired the lock"
    );

    let manager = super::BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());
    let started = Instant::now();
    let statuses = tokio::time::timeout(Duration::from_millis(500), manager.list())
        .await
        .expect("list blocked on a contended reconciler lock");
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "list exceeded the contention budget"
    );
    let status = statuses
        .into_iter()
        .find(|status| status.task_id == "contended")
        .expect("contended task returned");
    assert_eq!(status.status, BackgroundTaskStatus::Running);

    assert!(
        child.wait().expect("join sleeping holder").success(),
        "sleeping holder failed"
    );
    assert_eq!(
        store
            .read("contended")
            .await
            .expect("read contended status")
            .expect("contended status exists")
            .status,
        BackgroundTaskStatus::Running,
        "read-only degradation must not finalize the task"
    );
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "helper process for terminal_survives_stale_cross_process_running_write"]
async fn stale_running_writer_probe() {
    let dir = std::env::var_os("JCODE_W6_DIR").expect("JCODE_W6_DIR");
    let dir = std::path::PathBuf::from(dir);
    let store = TaskStatusStore::new(dir.clone());
    let gate = dir.join("gate-open");

    store
        .mutate("t1", |status| {
            std::fs::write(dir.join("read-done"), b"").expect("signal snapshot read");
            let deadline = Instant::now() + Duration::from_millis(1500);
            let branch = loop {
                if gate.exists() {
                    break "gate-open";
                }
                if Instant::now() >= deadline {
                    break "timed-out";
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            std::fs::write(dir.join("child-branch"), branch).expect("record child branch");
            status.progress = Some(BackgroundTaskProgress {
                kind: BackgroundTaskProgressKind::Indeterminate,
                percent: None,
                message: Some("stale child update".into()),
                current: None,
                total: None,
                unit: None,
                eta_seconds: None,
                updated_at: chrono::Utc::now().to_rfc3339(),
                source: BackgroundTaskProgressSource::Reported,
            });
            true
        })
        .await
        .expect("persist stale running mutation");
}

#[test]
#[ignore = "helper process for crash_releases_cross_process_lock"]
fn aborting_lock_holder_probe() {
    let dir = std::path::PathBuf::from(std::env::var_os("JCODE_W6_DIR").expect("JCODE_W6_DIR"));
    let status_path = dir.join("crash.status.json");
    let _guard = super::store_lock::acquire_blocking(&status_path).expect("acquire crash lock");
    std::fs::write(dir.join("held"), b"").expect("signal held lock");
    std::process::abort();
}

#[test]
#[ignore = "helper process for reconciler_degrades_to_read_only_under_contention"]
fn sleeping_lock_holder_probe() {
    let dir = std::path::PathBuf::from(std::env::var_os("JCODE_W6_DIR").expect("JCODE_W6_DIR"));
    let status_path = dir.join("contended.status.json");
    let _guard = super::store_lock::acquire_blocking(&status_path).expect("acquire contended lock");
    std::fs::write(dir.join("held"), b"").expect("signal held lock");
    std::thread::sleep(Duration::from_secs(3));
}
