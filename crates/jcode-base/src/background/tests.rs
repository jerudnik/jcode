use super::*;
use crate::bus::{BackgroundTaskProgressKind, BackgroundTaskProgressSource, BusEvent};
use anyhow::anyhow;
use tempfile::tempdir;
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn spawn_with_notify_emits_started_ui_activity() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());
    let mut bus_rx = Bus::global().subscribe();

    let info = manager
        .spawn_with_notify(
            "bash",
            Some("checks".to_string()),
            "session-started",
            true,
            false,
            |_output_path| async move {
                sleep(Duration::from_millis(10)).await;
                Ok(TaskResult::completed(Some(0)))
            },
        )
        .await;

    for _ in 0..20 {
        let event = tokio::time::timeout(Duration::from_millis(200), bus_rx.recv())
            .await
            .map_err(|err| anyhow!("timed out waiting for UI activity event: {err}"))?
            .map_err(|err| anyhow!("bus should stay open: {err}"))?;
        if let BusEvent::UiActivity(activity) = event
            && activity.session_id.as_deref() == Some("session-started")
            && activity.message.contains(&info.task_id)
        {
            assert_eq!(activity.kind, crate::bus::UiActivityKind::Background);
            assert!(activity.message.contains("Background task started"));
            assert!(activity.message.contains("checks"));
            assert_eq!(
                activity.status_notice.as_deref(),
                Some("Background task started · checks")
            );
            return Ok(());
        }
    }

    Err(anyhow!(
        "started UI activity event for task {} not received",
        info.task_id
    ))
}

#[tokio::test]
async fn update_delivery_applies_to_running_task_completion() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    let info = manager
        .spawn_with_notify(
            "bash",
            None,
            "session-test",
            false,
            false,
            |output_path| async move {
                sleep(Duration::from_millis(25)).await;
                tokio::fs::write(&output_path, "hello").await?;
                Ok(TaskResult::completed(Some(0)))
            },
        )
        .await;

    let updated = manager
        .update_delivery(&info.task_id, true, true)
        .await
        .map_err(|err| anyhow!("update delivery should succeed: {err}"))?
        .ok_or_else(|| anyhow!("task should exist"))?;
    assert!(updated.notify);
    assert!(updated.wake);

    for _ in 0..200 {
        let status = manager
            .status(&info.task_id)
            .await
            .ok_or_else(|| anyhow!("status should exist"))?;
        if status.status != BackgroundTaskStatus::Running {
            assert!(status.notify);
            assert!(status.wake);
            assert_eq!(status.status, BackgroundTaskStatus::Completed);
            return Ok(());
        }
        sleep(Duration::from_millis(10)).await;
    }

    Err(anyhow!("background task did not complete in time"))
}

#[tokio::test]
async fn update_progress_persists_status_and_emits_bus_event() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    let info = manager
        .spawn_with_notify(
            "bash",
            None,
            "session-progress",
            false,
            false,
            |_output_path| async move {
                sleep(Duration::from_millis(50)).await;
                Ok(TaskResult::completed(Some(0)))
            },
        )
        .await;

    let progress = BackgroundTaskProgress {
        kind: BackgroundTaskProgressKind::Determinate,
        percent: Some(42.0),
        message: Some("Running checks".to_string()),
        current: Some(21),
        total: Some(50),
        unit: Some("tests".to_string()),
        eta_seconds: Some(8),
        updated_at: Utc::now().to_rfc3339(),
        source: BackgroundTaskProgressSource::Reported,
    };

    let mut bus_rx = Bus::global().subscribe();
    let updated = manager
        .update_progress(&info.task_id, progress.clone())
        .await
        .map_err(|err| anyhow!("update progress should succeed: {err}"))?
        .ok_or_else(|| anyhow!("task should exist"))?;

    assert_eq!(updated.progress, Some(progress.clone().normalize()));

    for _ in 0..20 {
        let event = tokio::time::timeout(Duration::from_millis(200), bus_rx.recv())
            .await
            .map_err(|err| anyhow!("timed out waiting for progress event: {err}"))?
            .map_err(|err| anyhow!("bus should stay open: {err}"))?;
        if let BusEvent::BackgroundTaskProgress(event) = event
            && event.task_id == info.task_id
        {
            assert_eq!(event.session_id, "session-progress");
            assert_eq!(event.progress, progress.normalize());
            return Ok(());
        }
    }

    Err(anyhow!(
        "progress event for task {} not received",
        info.task_id
    ))
}

#[tokio::test]
async fn wait_returns_when_task_finishes() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    let info = manager
        .spawn_with_notify(
            "bash",
            None,
            "session-wait-finish",
            false,
            false,
            |output_path| async move {
                sleep(Duration::from_millis(25)).await;
                tokio::fs::write(&output_path, "done").await?;
                Ok(TaskResult::completed(Some(0)))
            },
        )
        .await;

    let wait_result = manager
        .wait(&info.task_id, Duration::from_secs(2), true)
        .await
        .ok_or_else(|| anyhow!("task should exist"))?;

    assert_eq!(wait_result.reason, BackgroundTaskWaitReason::Finished);
    assert_eq!(wait_result.task.status, BackgroundTaskStatus::Completed);
    assert_eq!(wait_result.task.exit_code, Some(0));
    Ok(())
}

#[tokio::test]
async fn wait_returns_on_progress_checkpoint() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    let info = manager
        .spawn_with_notify(
            "bash",
            None,
            "session-wait-progress",
            false,
            false,
            |_output_path| async move {
                sleep(Duration::from_secs(2)).await;
                Ok(TaskResult::completed(Some(0)))
            },
        )
        .await;

    let progress = BackgroundTaskProgress {
        kind: BackgroundTaskProgressKind::Determinate,
        percent: Some(25.0),
        message: Some("checkpoint".to_string()),
        current: Some(1),
        total: Some(4),
        unit: Some("steps".to_string()),
        eta_seconds: Some(3),
        updated_at: Utc::now().to_rfc3339(),
        source: BackgroundTaskProgressSource::Reported,
    };

    let waiter = manager.wait(&info.task_id, Duration::from_secs(2), true);
    let updater = async {
        sleep(Duration::from_millis(25)).await;
        manager
            .update_progress(&info.task_id, progress.clone())
            .await
            .map_err(|err| anyhow!("progress update should succeed: {err}"))?
            .ok_or_else(|| anyhow!("task should exist"))?;
        Result::<()>::Ok(())
    };
    let (wait_result, updater_result) = tokio::join!(waiter, updater);
    updater_result?;
    let wait_result = wait_result.ok_or_else(|| anyhow!("task should exist"))?;

    assert_eq!(wait_result.reason, BackgroundTaskWaitReason::Progress);
    assert_eq!(wait_result.task.status, BackgroundTaskStatus::Running);
    assert_eq!(wait_result.task.progress, Some(progress.normalize()));
    assert!(wait_result.progress_event.is_some());
    Ok(())
}

#[tokio::test]
async fn wait_returns_on_timeout() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    let info = manager
        .spawn_with_notify(
            "bash",
            None,
            "session-wait-timeout",
            false,
            false,
            |_output_path| async move {
                sleep(Duration::from_millis(250)).await;
                Ok(TaskResult::completed(Some(0)))
            },
        )
        .await;

    let wait_result = manager
        .wait(&info.task_id, Duration::from_millis(25), true)
        .await
        .ok_or_else(|| anyhow!("task should exist"))?;

    assert_eq!(wait_result.reason, BackgroundTaskWaitReason::Timeout);
    assert_eq!(wait_result.task.status, BackgroundTaskStatus::Running);
    Ok(())
}

fn running_status_fixture(task_id: &str, session_id: &str) -> TaskStatusFile {
    TaskStatusFile {
        task_id: task_id.to_string(),
        tool_name: "swarm".to_string(),
        display_name: None,
        session_id: session_id.to_string(),
        status: BackgroundTaskStatus::Running,
        exit_code: None,
        error: None,
        started_at: Utc::now().to_rfc3339(),
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

async fn write_status_fixture(manager: &BackgroundTaskManager, status: &TaskStatusFile) {
    let path = manager.status_path_for(&status.task_id);
    let json = serde_json::to_string_pretty(status).expect("serialize status fixture");
    tokio::fs::write(&path, json).await.expect("write fixture");
}

#[tokio::test]
async fn tasks_map_prunes_entry_after_natural_completion() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    let info = manager
        .spawn_with_notify(
            "bash",
            None,
            "session-prune",
            false,
            false,
            |_output_path| async move {
                sleep(Duration::from_millis(10)).await;
                Ok(TaskResult::completed(Some(0)))
            },
        )
        .await;
    assert!(
        manager.is_live_task(&info.task_id),
        "task should be live right after spawn"
    );

    for _ in 0..200 {
        let status = manager
            .status(&info.task_id)
            .await
            .ok_or_else(|| anyhow!("status should exist"))?;
        if status.status != BackgroundTaskStatus::Running && !manager.is_live_task(&info.task_id) {
            // Pruned only after the status file was finalized, so the live
            // map never claims a task whose status file is already terminal.
            let (running_count, labels, _) = manager.running_snapshot();
            assert_eq!(running_count, 0, "snapshot should not count finished tasks");
            assert!(labels.is_empty());
            return Ok(());
        }
        sleep(Duration::from_millis(10)).await;
    }

    Err(anyhow!(
        "task {} was not pruned from the live map after completion",
        info.task_id
    ))
}

#[tokio::test]
async fn reconcile_marks_orphan_from_reloaded_process_failed() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    // Same PID, different instance token: exactly what an exec-based server
    // reload leaves behind.
    let mut orphan = running_status_fixture("orphan1aaaa", "session-orphan");
    orphan.owner_pid = Some(std::process::id());
    orphan.owner_instance = Some("previous-process-image".to_string());
    write_status_fixture(&manager, &orphan).await;

    let reconciled = manager.reconcile_orphaned_tasks().await;
    assert_eq!(reconciled, 1);

    let status = manager
        .status("orphan1aaaa")
        .await
        .ok_or_else(|| anyhow!("status should exist"))?;
    assert_eq!(status.status, BackgroundTaskStatus::Failed);
    let error = status.error.unwrap_or_default();
    assert!(
        error.contains("orphaned") && error.contains("reloaded"),
        "error should explain the reload orphaning, got: {error}"
    );
    assert!(status.completed_at.is_some());
    Ok(())
}

#[tokio::test]
async fn reconcile_marks_orphan_from_dead_process_failed() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    // A child process that has already exited and been reaped gives us a PID
    // that is provably not running.
    let mut child = std::process::Command::new("true")
        .spawn()
        .map_err(|err| anyhow!("spawn child: {err}"))?;
    let dead_pid = child.id();
    child.wait().map_err(|err| anyhow!("wait child: {err}"))?;

    let mut orphan = running_status_fixture("orphan2bbbb", "session-orphan-dead");
    orphan.owner_pid = Some(dead_pid);
    orphan.owner_instance = Some("some-dead-instance".to_string());
    write_status_fixture(&manager, &orphan).await;

    let reconciled = manager.reconcile_orphaned_tasks().await;
    assert_eq!(reconciled, 1);
    let status = manager
        .status("orphan2bbbb")
        .await
        .ok_or_else(|| anyhow!("status should exist"))?;
    assert_eq!(status.status, BackgroundTaskStatus::Failed);
    Ok(())
}

#[tokio::test]
async fn reconcile_leaves_non_orphans_alone() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    // Owned by this exact process image: could still be bootstrapping.
    let mut own = running_status_fixture("keep1aaaa", "session-keep");
    own.owner_pid = Some(std::process::id());
    own.owner_instance = Some(model::process_instance_token().to_string());
    write_status_fixture(&manager, &own).await;

    // Legacy file without owner metadata: no safe liveness signal, leave it.
    let legacy = running_status_fixture("keep2bbbb", "session-keep");
    write_status_fixture(&manager, &legacy).await;

    // Owned by a live foreign process (PID 1 is always alive on Unix).
    let mut foreign = running_status_fixture("keep3cccc", "session-keep");
    foreign.owner_pid = Some(1);
    foreign.owner_instance = Some("init-instance".to_string());
    write_status_fixture(&manager, &foreign).await;

    // Detached with a live pid: reconciled by the detached path, not this one.
    let mut detached = running_status_fixture("keep4dddd", "session-keep");
    detached.detached = true;
    detached.pid = Some(std::process::id());
    write_status_fixture(&manager, &detached).await;

    let reconciled = manager.reconcile_orphaned_tasks().await;
    assert_eq!(reconciled, 0);

    for task_id in ["keep1aaaa", "keep2bbbb", "keep3cccc", "keep4dddd"] {
        let status = manager
            .status(task_id)
            .await
            .ok_or_else(|| anyhow!("status for {task_id} should exist"))?;
        assert_eq!(
            status.status,
            BackgroundTaskStatus::Running,
            "{task_id} should not be reconciled"
        );
    }
    Ok(())
}

#[tokio::test]
async fn status_read_self_heals_orphaned_task() -> Result<()> {
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    let mut orphan = running_status_fixture("orphan3cccc", "session-orphan-read");
    orphan.owner_pid = Some(std::process::id());
    orphan.owner_instance = Some("previous-process-image".to_string());
    write_status_fixture(&manager, &orphan).await;

    // A plain status read (used by bg status / bg wait) heals the phantom
    // without waiting for the startup sweep.
    let status = manager
        .status("orphan3cccc")
        .await
        .ok_or_else(|| anyhow!("status should exist"))?;
    assert_eq!(status.status, BackgroundTaskStatus::Failed);

    // And wait() returns immediately instead of blocking to timeout.
    let wait_result = manager
        .wait("orphan3cccc", Duration::from_secs(5), false)
        .await
        .ok_or_else(|| anyhow!("wait should find the task"))?;
    assert_eq!(
        wait_result.reason,
        BackgroundTaskWaitReason::AlreadyFinished
    );
    Ok(())
}

#[tokio::test]
async fn finalize_non_detached_aborts_adopted_original_future() -> Result<()> {
    // F02-R2-B2: aborting only the adoption wrapper drop-detaches the
    // original JoinHandle and the underlying work keeps running. Cleanup
    // must abort the ORIGINAL future via its stored AbortHandle.
    let tmp = tempdir()?;
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());

    // The original future signals if it survives past the abort window.
    let survived = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let survived_flag = std::sync::Arc::clone(&survived);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel::<()>();
    let original = tokio::spawn(async move {
        let _ = started_tx.send(());
        sleep(Duration::from_millis(300)).await;
        survived_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(jcode_tool_types::ToolOutput {
            output: "should never complete".to_string(),
            title: None,
            metadata: None,
            images: Vec::new(),
        })
    });
    started_rx.await.expect("original task should start");

    let info = manager
        .adopt_with_options("bash", None, "session-adopt-abort", false, false, original)
        .await;
    assert!(manager.is_live_task(&info.task_id));

    let finalized = manager.finalize_non_detached("test-shutdown").await;
    assert_eq!(finalized, 1, "the adopted task must be finalized");

    // Give a detached-but-alive original ample time to reach its flag.
    sleep(Duration::from_millis(400)).await;
    assert!(
        !survived.load(std::sync::atomic::Ordering::SeqCst),
        "the ORIGINAL adopted future must be aborted, not detached"
    );

    let status = manager
        .status(&info.task_id)
        .await
        .ok_or_else(|| anyhow!("status should exist"))?;
    assert_eq!(status.status, BackgroundTaskStatus::Failed);
    Ok(())
}

#[tokio::test]
async fn live_map_prunes_only_after_terminal_persistence(){
    // F04 gate 2: the live-map entry must outlive the moment the status file
    // still claims Running. We poll aggressively during a short task's
    // lifetime and assert the invariant "status file Running => task is in
    // the live map" never breaks (the converse - terminal file while briefly
    // still mapped - is allowed and unobservable as a phantom).
    let tmp = tempdir().unwrap();
    let manager = BackgroundTaskManager::with_output_dir(tmp.path().to_path_buf());
    let info = manager
        .spawn_with_notify("bash", None, "session-order", false, false, |_p| async move {
            sleep(Duration::from_millis(50)).await;
            Ok(TaskResult::completed(Some(0)))
        })
        .await;

    for _ in 0..400 {
        let status = manager.status(&info.task_id).await;
        let live = manager.is_live_task(&info.task_id);
        if let Some(status) = status {
            if status.status == BackgroundTaskStatus::Running {
                assert!(
                    live,
                    "status file claims Running but the task is not in the live map (pruned before terminal persistence)"
                );
            } else if !live {
                return; // terminal + pruned: done, invariant held throughout
            }
        }
        sleep(Duration::from_millis(2)).await;
    }
    panic!("task never reached terminal+pruned within the poll budget");
}

#[tokio::test]
async fn progress_and_delivery_survive_concurrent_terminal_completion() {
    // F04: a terminal completion racing progress/delivery mutations must
    // preserve the terminal truth while the mutations either apply to
    // non-terminal fields or are cleanly superseded - never resurrect
    // Running, never tear the file.
    let tmp = tempdir().unwrap();
    let manager = std::sync::Arc::new(BackgroundTaskManager::with_output_dir(
        tmp.path().to_path_buf(),
    ));
    let info = manager
        .spawn_with_notify("bash", None, "session-race2", false, false, |_p| async move {
            sleep(Duration::from_millis(30)).await;
            Ok(TaskResult::completed(Some(0)))
        })
        .await;

    let mut mutators = Vec::new();
    for i in 0..4u64 {
        let manager = std::sync::Arc::clone(&manager);
        let task_id = info.task_id.clone();
        mutators.push(tokio::spawn(async move {
            for j in 0..20u64 {
                let _ = manager
                    .update_progress(
                        &task_id,
                        crate::bus::BackgroundTaskProgress {
                            kind: crate::bus::BackgroundTaskProgressKind::Determinate,
                            percent: Some(((i * 20 + j) % 100) as f32),
                            message: Some(format!("m{i}-{j}")),
                            current: None,
                            total: None,
                            unit: None,
                            eta_seconds: None,
                            updated_at: chrono::Utc::now().to_rfc3339(),
                            source: crate::bus::BackgroundTaskProgressSource::Reported,
                        },
                    )
                    .await;
                let _ = manager.update_delivery(&task_id, j % 2 == 0, false).await;
            }
        }));
    }
    for mutator in mutators {
        mutator.await.unwrap();
    }

    // Wait for terminal state and assert it is the completion, not a
    // mutation artifact.
    for _ in 0..300 {
        if let Some(status) = manager.status(&info.task_id).await
            && status.status != BackgroundTaskStatus::Running
        {
            assert_eq!(status.status, BackgroundTaskStatus::Completed);
            assert_eq!(status.exit_code, Some(0));
            return;
        }
        sleep(Duration::from_millis(5)).await;
    }
    panic!("task never reached terminal state");
}

#[tokio::test]
async fn spawn_refuses_to_start_when_initial_persistence_fails() {
    // F04-B1: successful initial persistence is a prerequisite for starting
    // non-detached work. Make the output dir unwritable-as-a-dir by using a
    // file path, so the store's temp write fails deterministically.
    let tmp = tempdir().unwrap();
    let bogus_dir = tmp.path().join("not-a-dir");
    tokio::fs::write(&bogus_dir, b"file").await.unwrap();
    let manager = BackgroundTaskManager::with_output_dir(bogus_dir);

    let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ran_flag = std::sync::Arc::clone(&ran);
    let info = manager
        .spawn_with_notify("bash", None, "session-noinit", false, false, move |_p| {
            let ran_flag = ran_flag;
            async move {
                ran_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(TaskResult::completed(Some(0)))
            }
        })
        .await;

    sleep(Duration::from_millis(100)).await;
    assert!(
        !ran.load(std::sync::atomic::Ordering::SeqCst),
        "task must NOT run when the initial status write fails"
    );
    assert!(
        !manager.is_live_task(&info.task_id),
        "refused task must not be tracked"
    );
}

#[tokio::test]
async fn terminal_persistence_failure_retains_tombstone_then_recovers() {
    // F04-B1: on terminal persistence failure the live-map entry is
    // RETAINED (visible tombstone, no phantom prune) and the detached
    // retry loop prunes once persistence recovers. We simulate failure by
    // replacing the status dir with a file after the initial write, then
    // restore it and watch recovery land.
    let tmp = tempdir().unwrap();
    let dir = tmp.path().join("tasks");
    let manager = BackgroundTaskManager::with_output_dir(dir.clone());

    let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
    let info = manager
        .spawn_with_notify("bash", None, "session-tomb", false, false, move |_p| {
            async move {
                let _ = release_rx.await;
                Ok(TaskResult::completed(Some(0)))
            }
        })
        .await;
    assert!(manager.is_live_task(&info.task_id));

    // Break the directory: swap it for a file so temp writes fail.
    let saved = tmp.path().join("saved-tasks");
    tokio::fs::rename(&dir, &saved).await.unwrap();
    tokio::fs::write(&dir, b"block").await.unwrap();

    // Let the task complete; its terminal write must fail.
    let _ = release_tx.send(());
    sleep(Duration::from_millis(300)).await;
    assert!(
        manager.is_live_task(&info.task_id),
        "task must remain in the live map (tombstone) while terminal persistence fails"
    );

    // Heal the directory; the retry loop (250ms initial backoff, doubling)
    // must persist the terminal state and prune.
    tokio::fs::remove_file(&dir).await.unwrap();
    tokio::fs::rename(&saved, &dir).await.unwrap();

    for _ in 0..100 {
        if !manager.is_live_task(&info.task_id) {
            let status = manager
                .status(&info.task_id)
                .await
                .expect("terminal status must exist after recovery");
            assert_ne!(status.status, BackgroundTaskStatus::Running);
            return;
        }
        sleep(Duration::from_millis(50)).await;
    }
    panic!("terminal persistence never recovered / task never pruned");
}
