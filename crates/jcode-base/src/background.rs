//! Background task execution manager
//!
//! Allows tools to run in the background and notify the agent when complete.
//! Uses file-based storage for crash resilience + event channel for real-time notifications.

use crate::bus::{
    BackgroundTaskCompleted, BackgroundTaskProgress, BackgroundTaskProgressEvent,
    BackgroundTaskProgressSource, BackgroundTaskStatus, Bus, BusEvent,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio::sync::{RwLock, watch};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant as TokioInstant, MissedTickBehavior};

mod model;
mod store;

pub use model::{
    BackgroundCleanupResult, BackgroundTaskEventKind, BackgroundTaskEventRecord,
    BackgroundTaskInfo, BackgroundTaskWaitReason, BackgroundTaskWaitResult,
    RunningBackgroundProgress, TaskResult, TaskStatusFile, format_progress_display,
    format_progress_summary, render_progress_bar,
};
use model::{
    EXIT_MARKER_PREFIX, RunningTask, normalize_delivery, progress_equivalent,
    progress_event_record, progress_wait_reason, push_task_event, task_dir, terminal_event_record,
};

/// Env override for the live background-task cap.
pub const MAX_BACKGROUND_TASKS_ENV: &str = "JCODE_MAX_BACKGROUND_TASKS";

/// Default max live (running) background tasks for the whole process.
const DEFAULT_MAX_BACKGROUND_TASKS: usize = 64;

/// Effective live-task cap (env-overridable, default 64).
fn max_background_tasks() -> usize {
    std::env::var(MAX_BACKGROUND_TASKS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|cap| *cap > 0)
        .unwrap_or(DEFAULT_MAX_BACKGROUND_TASKS)
}

/// Point-in-time `{current, cap}` reading of the live-task budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundCapacitySnapshot {
    pub current: usize,
    pub cap: usize,
}

/// Manages background task execution
pub struct BackgroundTaskManager {
    tasks: Arc<RwLock<HashMap<String, RunningTask>>>,
    output_dir: PathBuf,
    /// Crash-durable serialized status persistence (F04/F05). Every status-file
    /// write goes through this store: fsynced same-directory temp + rename +
    /// parent fsync, cross-instance per-task serialization, terminal
    /// precedence, and surfaced errors.
    store: Arc<store::TaskStatusStore>,
    /// Activity-lease authority (F01 C5): non-detached tasks hold a lease
    /// for their tracked lifetime so the daemon cannot idle-exit while they
    /// run. Defaults to no-op; the daemon injects its real authority at
    /// startup via [`Self::set_activity_authority`] (composition-root
    /// injection, keeping this crate free of server types).
    activity: std::sync::RwLock<Arc<dyn jcode_core::activity::ActivityLeaseAuthority>>,
    /// Test-only cap override (0 = unset, use env/default). Avoids mutating
    /// process-global env vars from parallel tests.
    cap_override: std::sync::atomic::AtomicUsize,
    /// In-flight spawn reservations (F12 review important-2): incremented
    /// before the cap check, released when the task lands in the live map or
    /// the spawn is refused, so concurrent spawns cannot all pass one free
    /// slot (check-then-act TOCTOU).
    in_flight_spawns: Arc<std::sync::atomic::AtomicUsize>,
}

/// RAII reservation of one live-task slot during spawn setup. Dropped (and
/// the counter released) either on refusal or after the task is inserted
/// into the live map, at which point the map itself carries the count.
struct SpawnSlot {
    counter: Arc<std::sync::atomic::AtomicUsize>,
}

impl SpawnSlot {
    fn reserve(counter: &Arc<std::sync::atomic::AtomicUsize>) -> Self {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            counter: Arc::clone(counter),
        }
    }
}

impl Drop for SpawnSlot {
    fn drop(&mut self) {
        self.counter
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl BackgroundTaskManager {
    /// Create a manager rooted at a specific output directory.
    ///
    /// Primarily for tests; production code should use [`global`].
    pub fn with_output_dir(output_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&output_dir).ok();
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            store: Arc::new(store::TaskStatusStore::new(output_dir.clone())),
            output_dir,
            activity: std::sync::RwLock::new(jcode_core::activity::noop_activity_authority()),
            cap_override: std::sync::atomic::AtomicUsize::new(0),
            in_flight_spawns: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Effective live-task cap for this manager.
    fn live_task_cap(&self) -> usize {
        match self.cap_override.load(std::sync::atomic::Ordering::Relaxed) {
            0 => max_background_tasks(),
            cap => cap,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_cap_for_tests(&self, cap: usize) {
        self.cap_override
            .store(cap, std::sync::atomic::Ordering::Relaxed);
    }

    /// Inject the daemon's activity-lease authority (F01 C5). Called once by
    /// the server at startup, before it accepts work.
    pub fn set_activity_authority(
        &self,
        authority: Arc<dyn jcode_core::activity::ActivityLeaseAuthority>,
    ) {
        *self
            .activity
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = authority;
    }

    fn acquire_task_lease(
        &self,
        task_id: &str,
        tool_name: &str,
    ) -> Result<jcode_core::activity::ActivityLeaseGuard, jcode_core::activity::ActivityLeaseError>
    {
        let authority = self
            .activity
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        jcode_core::activity::ActivityLeaseGuard::acquire(
            &authority,
            jcode_core::activity::ActivityClass::BackgroundTask,
            &format!("{tool_name}/{task_id}"),
        )
    }

    /// Fail-closed refusal path (F02-B4 / F01 I6): the task never runs.
    /// Writes a terminal Failed status so callers polling the status file
    /// observe a definite outcome instead of silent unleased execution.
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors persisted status fields at existing call sites"
    )]
    async fn write_refused_status(
        &self,
        task_id: &str,
        tool_name: &str,
        display_name: Option<String>,
        session_id: &str,
        status_path: &std::path::Path,
        notify: bool,
        wake: bool,
        reason: &str,
    ) {
        let mut refused_status = TaskStatusFile {
            task_id: task_id.to_string(),
            tool_name: tool_name.to_string(),
            display_name,
            session_id: session_id.to_string(),
            status: BackgroundTaskStatus::Failed,
            exit_code: None,
            error: Some(reason.to_string()),
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
            duration_secs: Some(0.0),
            pid: None,
            owner_pid: Some(std::process::id()),
            owner_instance: Some(model::process_instance_token().to_string()),
            detached: false,
            notify,
            wake,
            progress: None,
            event_history: Vec::new(),
        };
        let event_status = refused_status.status.clone();
        push_task_event(
            &mut refused_status,
            terminal_event_record(event_status, None, Some(reason)),
        );
        let _ = status_path; // path derives from task_id inside the store
        if let Err(error) = self
            .store
            .write_terminal(task_id, move |_| refused_status)
            .await
        {
            crate::logging::error(&format!("Refused-status persistence failed: {error:#}"));
        }
    }

    /// Create a new background task manager
    pub fn new() -> Self {
        let output_dir = task_dir();
        Self::with_output_dir(output_dir)
    }

    /// Generate a short, unique task ID
    fn generate_task_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        const TASK_ID_ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        // Use last 6 digits of timestamp + 4 random chars
        let rand_part: String = (0..4)
            .map(|_| {
                let idx = (rand::random::<u8>() as usize) % TASK_ID_ALPHABET.len();
                TASK_ID_ALPHABET[idx] as char
            })
            .collect();
        format!(
            "{}{}",
            &timestamp.to_string()[timestamp.to_string().len().saturating_sub(6)..],
            rand_part
        )
    }

    pub fn output_path_for(&self, task_id: &str) -> PathBuf {
        self.output_dir.join(format!("{}.output", task_id))
    }

    pub fn status_path_for(&self, task_id: &str) -> PathBuf {
        self.output_dir.join(format!("{}.status.json", task_id))
    }

    fn publish_task_started_activity(
        task_id: &str,
        tool_name: &str,
        display_name: Option<&str>,
        session_id: &str,
        notify: bool,
    ) {
        if !notify {
            return;
        }
        let label = crate::message::background_task_display_label(tool_name, display_name);
        let safe_label = label.replace('`', "'");
        Bus::global().publish(BusEvent::UiActivity(crate::bus::UiActivity::background(
            Some(session_id.to_string()),
            format!(
                "**Background task started** `{}` · `{}`\n\nJcode is running this in the background. Progress, checkpoints, and completion will appear here.",
                task_id, safe_label
            ),
            Some(format!("Background task started · {}", label)),
        )));
    }

    fn status_duration_secs(started_at: &str, completed_at: DateTime<Utc>) -> Option<f64> {
        DateTime::parse_from_rfc3339(started_at)
            .ok()
            .and_then(|started| (completed_at - started.with_timezone(&Utc)).to_std().ok())
            .map(|duration| duration.as_secs_f64())
    }

    fn parse_exit_code_from_output(output: &str) -> Option<i32> {
        output.lines().rev().find_map(|line| {
            let trimmed = line.trim();
            let suffix = trimmed.strip_prefix(EXIT_MARKER_PREFIX)?;
            let suffix = suffix.strip_suffix(" ---")?;
            suffix.trim().parse::<i32>().ok()
        })
    }

    /// Lenient path read for directory sweeps: malformed files are logged
    /// (never silently ignored, F04) and yield None so a sweep can continue.
    async fn read_status_file(&self, path: &std::path::Path) -> Option<TaskStatusFile> {
        match self.store.read_path(path).await {
            Ok(status) => status,
            Err(error) => {
                crate::logging::warn(&format!("Background status sweep read failed: {error:#}"));
                None
            }
        }
    }

    async fn finalize_detached_status_if_needed(
        &self,
        mut status: TaskStatusFile,
        status_path: &std::path::Path,
    ) -> TaskStatusFile {
        if status.status != BackgroundTaskStatus::Running || !status.detached {
            return status;
        }

        let Some(pid) = status.pid else {
            return status;
        };

        let reaped_exit = crate::platform::try_reap_child_process(pid).ok().flatten();

        if reaped_exit.is_none() && crate::platform::is_process_running(pid) {
            return status;
        }

        let output_path = self.output_path_for(&status.task_id);
        let output = fs::read_to_string(&output_path).await.unwrap_or_default();
        let exit_code = reaped_exit.or_else(|| Self::parse_exit_code_from_output(&output));
        let completed_at = Utc::now();
        let duration_secs = Self::status_duration_secs(&status.started_at, completed_at);
        let final_status = if matches!(exit_code, Some(0)) {
            BackgroundTaskStatus::Completed
        } else {
            BackgroundTaskStatus::Failed
        };
        let final_error = if matches!(final_status, BackgroundTaskStatus::Failed) {
            Some(match exit_code {
                Some(code) => format!("Command exited with code {}", code),
                None => "Detached command exited without a readable exit code".to_string(),
            })
        } else {
            None
        };

        status.status = final_status.clone();
        status.exit_code = exit_code;
        status.error = final_error.clone();
        status.completed_at = Some(completed_at.to_rfc3339());
        status.duration_secs = duration_secs;
        status.pid = Some(pid);
        push_task_event(
            &mut status,
            terminal_event_record(final_status.clone(), exit_code, final_error.as_deref()),
        );

        let _ = status_path;
        let terminal_status = status.clone();
        if let Err(error) = self
            .store
            .write_terminal(&status.task_id, move |_| terminal_status)
            .await
        {
            crate::logging::error(&format!("Detached finalize persistence failed: {error:#}"));
        }

        let output_preview = if output.len() > 500 {
            format!("{}...", crate::util::truncate_str(&output, 500))
        } else {
            output
        };
        Bus::global().publish(BusEvent::BackgroundTaskCompleted(BackgroundTaskCompleted {
            task_id: status.task_id.clone(),
            tool_name: status.tool_name.clone(),
            display_name: status.display_name.clone(),
            session_id: status.session_id.clone(),
            status: final_status,
            exit_code,
            output_preview,
            output_file: output_path,
            duration_secs: duration_secs.unwrap_or_default(),
            notify: status.notify,
            wake: status.wake,
        }));

        status
    }

    /// True when a non-detached `Running` status file provably belongs to a
    /// process image that no longer exists, so no future can ever finalize it.
    ///
    /// Rules, deliberately conservative because the task dir is shared by
    /// every jcode process on the machine:
    /// - Terminal, detached, or pid-bearing files are never orphans here
    ///   (detached reconciliation is `finalize_detached_status_if_needed`).
    /// - Files owned by this exact process image are never orphans: the
    ///   initial status file is written before the task lands in the live
    ///   map, so "Running + not in map + my instance" can simply mean the
    ///   task is still bootstrapping.
    /// - Files owned by this PID but a different instance token are orphans:
    ///   an exec-based reload replaced the process image, so the owning
    ///   future is gone even though the PID matches.
    /// - Files owned by another PID are orphans only once that process is
    ///   dead.
    /// - Files without owner metadata (written by older builds) are left
    ///   alone; only the explicit startup sweep in
    ///   [`Self::reconcile_orphaned_tasks`] handles those.
    fn status_is_reconcilable_orphan(status: &TaskStatusFile) -> bool {
        if status.status != BackgroundTaskStatus::Running || status.detached || status.pid.is_some()
        {
            return false;
        }
        let Some(owner_pid) = status.owner_pid else {
            return false;
        };
        if status.owner_instance.as_deref() == Some(model::process_instance_token()) {
            return false;
        }
        if owner_pid == std::process::id() {
            return true;
        }
        !crate::platform::is_process_running(owner_pid)
    }

    /// Finalize an orphaned non-detached `Running` status file as `Failed`.
    ///
    /// The owning process's task future died with the process (crash or
    /// exec-based server reload), so without this the file reads `Running`
    /// forever: `bg list`/`bg status` show a phantom task and `bg wait`
    /// blocks until its timeout.
    async fn finalize_orphaned_status_if_needed(
        &self,
        mut status: TaskStatusFile,
        status_path: &std::path::Path,
    ) -> TaskStatusFile {
        if !Self::status_is_reconcilable_orphan(&status) {
            return status;
        }
        // Belt and braces: never rewrite a task this process is executing.
        if self.is_live_task(&status.task_id) {
            return status;
        }

        let completed_at = Utc::now();
        let duration_secs = Self::status_duration_secs(&status.started_at, completed_at);
        let error =
            "Task orphaned: the owning server process exited (reloaded or crashed) before the task finished"
                .to_string();
        status.status = BackgroundTaskStatus::Failed;
        status.exit_code = None;
        status.error = Some(error.clone());
        status.completed_at = Some(completed_at.to_rfc3339());
        status.duration_secs = duration_secs;
        push_task_event(
            &mut status,
            terminal_event_record(BackgroundTaskStatus::Failed, None, Some(&error)),
        );
        let _ = status_path;
        let terminal_status = status.clone();
        if let Err(error) = self
            .store
            .write_terminal(&status.task_id, move |_| terminal_status)
            .await
        {
            crate::logging::error(&format!("Orphan finalize persistence failed: {error:#}"));
        }

        let output_path = self.output_path_for(&status.task_id);
        let output = fs::read_to_string(&output_path).await.unwrap_or_default();
        let output_preview = if output.len() > 500 {
            format!("{}...", crate::util::truncate_str(&output, 500))
        } else {
            output
        };
        Bus::global().publish(BusEvent::BackgroundTaskCompleted(BackgroundTaskCompleted {
            task_id: status.task_id.clone(),
            tool_name: status.tool_name.clone(),
            display_name: status.display_name.clone(),
            session_id: status.session_id.clone(),
            status: BackgroundTaskStatus::Failed,
            exit_code: None,
            output_preview,
            output_file: output_path,
            duration_secs: duration_secs.unwrap_or_default(),
            notify: status.notify,
            wake: status.wake,
        }));

        status
    }

    /// Startup/reload sweep: remove stale interrupted-write temp files, then
    /// mark orphaned non-detached `Running` status files as `Failed` with a
    /// "server reloaded" note.
    ///
    /// Only owner-tagged files are considered, using the liveness rules of
    /// [`Self::status_is_reconcilable_orphan`]. Files without owner metadata
    /// (written by older builds, or by processes that legitimately still run
    /// them) are left untouched: the task dir is shared machine-wide, so
    /// without owner metadata there is no safe way to distinguish a phantom
    /// from another live process's task. Returns how many files were
    /// reconciled.
    pub async fn reconcile_orphaned_tasks(&self) -> usize {
        self.store.cleanup_stale_temp_files().await;
        let mut reconciled = 0;
        let Ok(mut entries) = fs::read_dir(&self.output_dir).await else {
            return reconciled;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(status) = self.read_status_file(&path).await else {
                continue;
            };
            if !Self::status_is_reconcilable_orphan(&status) {
                continue;
            }
            if self.tasks.read().await.contains_key(&status.task_id) {
                continue;
            }
            self.finalize_orphaned_status_if_needed(status, &path).await;
            reconciled += 1;
        }
        reconciled
    }

    /// Shutdown finalize (F01 design 3.4 step 6): atomically mark every LIVE
    /// non-detached task as failed-by-shutdown and abort its future.
    ///
    /// Called by the daemon's shutdown coordinator during cleanup so that a
    /// voluntary exit leaves no `Running` status files behind and the
    /// next-boot orphan reconcile ([`Self::reconcile_orphaned_tasks`]) is
    /// unnecessary for voluntary exits. Detached tasks are untouched: they
    /// outlive the daemon by design. Returns how many tasks were finalized.
    pub async fn finalize_non_detached(&self, reason: &str) -> usize {
        let drained: Vec<RunningTask> = {
            let mut tasks = self.tasks.write().await;
            let ids: Vec<String> = tasks.keys().cloned().collect();
            ids.into_iter().filter_map(|id| tasks.remove(&id)).collect()
        };
        let count = drained.len();
        for task in drained {
            if let Some(original) = &task.original_abort {
                original.abort();
            }
            task.handle.abort();
            let (notify_flag, wake_flag) = *task.delivery_flags.borrow();
            let mut final_status = TaskStatusFile {
                task_id: task.task_id,
                tool_name: task.tool_name,
                display_name: task.display_name,
                session_id: task.session_id,
                status: BackgroundTaskStatus::Failed,
                exit_code: None,
                error: Some(format!("Daemon shutdown ({reason})")),
                started_at: task.started_at_rfc3339,
                completed_at: Some(chrono::Utc::now().to_rfc3339()),
                duration_secs: Some(task.started_at.elapsed().as_secs_f64()),
                pid: None,
                owner_pid: Some(std::process::id()),
                owner_instance: Some(model::process_instance_token().to_string()),
                detached: false,
                notify: notify_flag,
                wake: wake_flag,
                progress: None,
                event_history: Vec::new(),
            };
            let event_status = final_status.status.clone();
            let event_exit_code = final_status.exit_code;
            let event_error = final_status.error.clone();
            push_task_event(
                &mut final_status,
                terminal_event_record(event_status, event_exit_code, event_error.as_deref()),
            );
            let task_id_for_write = final_status.task_id.clone();
            if let Err(error) = self
                .store
                .write_terminal(&task_id_for_write, move |_| final_status)
                .await
            {
                // Explicit failure policy (F04-R2-B1): with a durable
                // Running record on disk, the next-boot orphan sweep is the
                // recovery authority and this log suffices (the daemon is
                // exiting; no retry loop can outlive it). Without one (an
                // adopted task whose permitted initial write failed AND
                // whose terminal write now failed while storage is broken),
                // the record is irrecoverably lost; state that loudly as
                // accepted data loss rather than pretending otherwise.
                if task.durable_record {
                    crate::logging::error(&format!(
                        "Shutdown finalize persistence failed for {task_id_for_write} \
                         (initial Running record exists; next-boot orphan sweep will reconcile): {error:#}"
                    ));
                } else {
                    crate::logging::error(&format!(
                        "DATA LOSS: shutdown finalize persistence failed for adopted task \
                         {task_id_for_write} with no durable record (initial write also failed); \
                         the task outcome cannot be recovered: {error:#}"
                    ));
                }
            }
        }
        count
    }

    /// Observability: current live-task count against the global cap.
    pub async fn capacity_snapshot(&self) -> BackgroundCapacitySnapshot {
        BackgroundCapacitySnapshot {
            current: self.tasks.read().await.len(),
            cap: self.live_task_cap(),
        }
    }

    pub fn reserve_task_info(&self) -> BackgroundTaskInfo {
        let task_id = Self::generate_task_id();
        let output_file = self.output_path_for(&task_id);
        let status_file = self.status_path_for(&task_id);
        BackgroundTaskInfo {
            task_id,
            output_file,
            status_file,
            refused: None,
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Detached task registration mirrors persisted status fields and existing call sites"
    )]
    pub async fn register_detached_task(
        &self,
        info: &BackgroundTaskInfo,
        tool_name: &str,
        display_name: Option<String>,
        session_id: &str,
        pid: u32,
        started_at: &str,
        notify: bool,
        wake: bool,
    ) {
        let (notify, wake) = normalize_delivery(notify, wake);
        let status = TaskStatusFile {
            task_id: info.task_id.clone(),
            tool_name: tool_name.to_string(),
            display_name,
            session_id: session_id.to_string(),
            status: BackgroundTaskStatus::Running,
            exit_code: None,
            error: None,
            started_at: started_at.to_string(),
            completed_at: None,
            duration_secs: None,
            pid: Some(pid),
            // Detached processes outlive this server, so no in-process owner:
            // reconciliation must never clobber them.
            owner_pid: None,
            owner_instance: None,
            detached: true,
            notify,
            wake,
            progress: None,
            event_history: Vec::new(),
        };
        if let Err(error) = self.store.write_initial(&status).await {
            crate::logging::error(&format!(
                "Detached task initial status persistence failed: {error:#}"
            ));
        }
        Self::publish_task_started_activity(
            &info.task_id,
            tool_name,
            status.display_name.as_deref(),
            session_id,
            notify,
        );
    }

    /// Spawn a background task
    ///
    /// The `execute_fn` receives the output file path and should write output there.
    /// It returns a TaskResult with exit code and optional error.
    pub async fn spawn<F, Fut>(
        &self,
        tool_name: &str,
        session_id: &str,
        execute_fn: F,
    ) -> BackgroundTaskInfo
    where
        F: FnOnce(PathBuf) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<TaskResult>> + Send,
    {
        self.spawn_with_notify(tool_name, None, session_id, true, false, execute_fn)
            .await
    }

    /// Spawn a background task with explicit notify flag
    pub async fn spawn_with_notify<F, Fut>(
        &self,
        tool_name: &str,
        display_name: Option<String>,
        session_id: &str,
        notify: bool,
        wake: bool,
        execute_fn: F,
    ) -> BackgroundTaskInfo
    where
        F: FnOnce(PathBuf) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<TaskResult>> + Send,
    {
        let (notify, wake) = normalize_delivery(notify, wake);
        let task_id = Self::generate_task_id();
        let output_path = self.output_dir.join(format!("{}.output", task_id));
        let status_path = self.output_dir.join(format!("{}.status.json", task_id));
        // Global live-task cap: live-map tasks plus in-flight spawn
        // reservations count (F12 review important-2: a bare map read lets
        // concurrent spawns all pass one free slot). The RAII slot is held
        // until the task lands in the live map. Terminal pruning
        // (completed/failed/cancelled) releases capacity. Fail closed like
        // the drain refusal below: the task NEVER runs and the status file
        // records an explicit self-documenting refusal.
        let spawn_slot = SpawnSlot::reserve(&self.in_flight_spawns);
        {
            // Load the reservation counter BEFORE the live map (F12
            // re-review): a completing spawner inserts into `tasks` before
            // dropping its slot, so counter-first ordering sees it in at
            // least one place; map-first could miss it in both and share one
            // free slot between two spawners. Double-counting merely refuses
            // conservatively.
            let reserved = self
                .in_flight_spawns
                .load(std::sync::atomic::Ordering::SeqCst)
                .saturating_sub(1); // exclude our own reservation
            let live = self.tasks.read().await.len() + reserved;
            let cap = self.live_task_cap();
            if live >= cap {
                let reason = format!(
                    "Refused: background task cap reached ({live}/{cap} live tasks). \
                     Raise via {MAX_BACKGROUND_TASKS_ENV}."
                );
                crate::logging::warn(&format!(
                    "Background task {tool_name}/{task_id} refused: {reason}"
                ));
                self.write_refused_status(
                    &task_id,
                    tool_name,
                    display_name.clone(),
                    session_id,
                    &status_path,
                    notify,
                    wake,
                    &reason,
                )
                .await;
                return BackgroundTaskInfo {
                    task_id,
                    output_file: output_path,
                    status_file: status_path,
                    refused: Some(reason),
                };
            }
        }
        // Activity lease (F01 C5 / F02-B4): acquired at method entry, BEFORE
        // the future is spawned, so it exists before the task can execute.
        // Moved into the RunningTask record below, dropped at terminal
        // pruning. A ShuttingDown refusal fails closed: the task NEVER runs
        // (I6: no new work starts silently during drain).
        let activity_lease = match self.acquire_task_lease(&task_id, tool_name) {
            Ok(lease) => Some(lease),
            Err(refused) => {
                crate::logging::warn(&format!(
                    "Background task {tool_name}/{task_id} refused during shutdown drain: {refused}"
                ));
                self.write_refused_status(
                    &task_id,
                    tool_name,
                    display_name.clone(),
                    session_id,
                    &status_path,
                    notify,
                    wake,
                    "Refused: daemon is shutting down",
                )
                .await;
                return BackgroundTaskInfo {
                    task_id,
                    output_file: output_path,
                    status_file: status_path,
                    refused: Some("Refused: daemon is shutting down".to_string()),
                };
            }
        };
        let started_at_rfc3339 = chrono::Utc::now().to_rfc3339();

        // Write initial status file
        let initial_status = TaskStatusFile {
            task_id: task_id.clone(),
            tool_name: tool_name.to_string(),
            display_name: display_name.clone(),
            session_id: session_id.to_string(),
            status: BackgroundTaskStatus::Running,
            exit_code: None,
            error: None,
            started_at: started_at_rfc3339.clone(),
            completed_at: None,
            duration_secs: None,
            pid: None,
            owner_pid: Some(std::process::id()),
            owner_instance: Some(model::process_instance_token().to_string()),
            detached: false,
            notify,
            wake,
            progress: None,
            event_history: Vec::new(),
        };
        // F04-B1: successful initial persistence is a PREREQUISITE for
        // starting non-detached work. Without a durable `Running` file, a
        // process death mid-task would leave no record for the next-boot
        // orphan sweep to reconcile. Fail closed: the task never runs.
        if let Err(error) = self.store.write_initial(&initial_status).await {
            crate::logging::error(&format!(
                "Initial status persistence failed for task {task_id}; refusing to start: {error:#}"
            ));
            return BackgroundTaskInfo {
                task_id,
                output_file: output_path,
                status_file: status_path,
                // F12 re-review: this task never runs; callers must not
                // report a started task (same contract as cap refusal).
                refused: Some(format!(
                    "Refused: initial status persistence failed: {error:#}"
                )),
            };
        }
        Self::publish_task_started_activity(
            &task_id,
            tool_name,
            display_name.as_deref(),
            session_id,
            notify,
        );

        let output_path_clone = output_path.clone();
        let status_path_clone = status_path.clone();
        let task_id_clone = task_id.clone();
        let task_id_clone2 = task_id.clone();
        let task_id_clone3 = task_id.clone();
        let tool_name_owned = tool_name.to_string();
        let display_name_owned = display_name.clone();
        let session_id_owned = session_id.to_string();
        let started_at = Instant::now();
        let started_at_rfc3339_for_task = started_at_rfc3339.clone();
        let (delivery_flags_tx, delivery_flags_rx) = watch::channel((notify, wake));
        let tasks_for_prune = Arc::clone(&self.tasks);
        let store_for_task = Arc::clone(&self.store);
        let (registered_tx, registered_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn the background task
        let handle = tokio::spawn(async move {
            let result = execute_fn(output_path_clone.clone()).await;

            let duration_secs = started_at.elapsed().as_secs_f64();
            let (status, exit_code, error) = match &result {
                Ok(task_result) => {
                    let status = task_result.status.clone().unwrap_or_else(|| {
                        if task_result.error.is_some() {
                            BackgroundTaskStatus::Failed
                        } else {
                            BackgroundTaskStatus::Completed
                        }
                    });
                    (status, task_result.exit_code, task_result.error.clone())
                }
                Err(e) => (BackgroundTaskStatus::Failed, None, Some(e.to_string())),
            };

            let (notify_flag, wake_flag) = *delivery_flags_rx.borrow();
            // Terminal write through the store with in-process recovery
            // (F04/F04-B1): serialized, atomic, first-terminal-wins. On
            // success the live-map entry is pruned; on persistence failure
            // the entry is retained as a visible tombstone and a detached
            // retry loop prunes once the write finally lands. Awaiting
            // registration first means an instantly finishing task cannot
            // race the insert below and leave a permanent phantom entry.
            let _ = registered_rx.await;
            let _ = &status_path_clone;
            let _ = task_id_clone3;
            persist_terminal_with_recovery(
                store_for_task,
                tasks_for_prune,
                TerminalSpec {
                    task_id: task_id_clone2,
                    tool_name: tool_name_owned.clone(),
                    display_name: display_name_owned.clone(),
                    session_id: session_id_owned.clone(),
                    status: status.clone(),
                    exit_code,
                    error: error.clone(),
                    started_at: started_at_rfc3339_for_task,
                    completed_at: chrono::Utc::now().to_rfc3339(),
                    duration_secs,
                    notify: notify_flag,
                    wake: wake_flag,
                },
            )
            .await;

            // Read output preview for notification
            let output_preview = tokio::fs::read_to_string(&output_path_clone)
                .await
                .map(|s| {
                    if s.len() > 500 {
                        format!("{}...", crate::util::truncate_str(&s, 500))
                    } else {
                        s
                    }
                })
                .unwrap_or_default();

            // Publish completion event to the bus
            Bus::global().publish(BusEvent::BackgroundTaskCompleted(BackgroundTaskCompleted {
                task_id: task_id_clone,
                tool_name: tool_name_owned,
                display_name: display_name_owned,
                session_id: session_id_owned,
                status,
                exit_code,
                output_preview,
                output_file: output_path_clone,
                duration_secs,
                notify: notify_flag,
                wake: wake_flag,
            }));

            result
        });

        // Track the running task
        let running_task = RunningTask {
            task_id: task_id.clone(),
            tool_name: tool_name.to_string(),
            display_name,
            session_id: session_id.to_string(),
            status_path: status_path.clone(),
            started_at,
            started_at_rfc3339,
            delivery_flags: delivery_flags_tx,
            handle,
            original_abort: None,
            activity_lease,
            durable_record: true,
        };

        self.tasks
            .write()
            .await
            .insert(task_id.clone(), running_task);
        // The live map now carries the count; release the spawn reservation.
        drop(spawn_slot);
        let _ = registered_tx.send(());

        BackgroundTaskInfo {
            task_id,
            output_file: output_path,
            status_file: status_path,
            refused: None,
        }
    }

    /// Adopt an already-spawned task as a background task.
    /// Used when the user moves a currently-executing tool to background via Alt+B.
    /// The `handle` is an already-running tokio task; we just register it for tracking
    /// and wire up completion notifications.
    pub async fn adopt(
        &self,
        tool_name: &str,
        session_id: &str,
        handle: JoinHandle<Result<jcode_tool_types::ToolOutput>>,
    ) -> BackgroundTaskInfo {
        self.adopt_with_options(tool_name, None, session_id, true, false, handle)
            .await
    }

    /// Adopt an already-spawned task as a background task, with explicit display
    /// name and delivery flags. Used both for user-initiated handoff (Alt+B) and
    /// for promoting a foreground command that exceeded its timeout but is still
    /// running, so it keeps running and surfaces as a background-task card.
    pub async fn adopt_with_options(
        &self,
        tool_name: &str,
        display_name: Option<String>,
        session_id: &str,
        notify: bool,
        wake: bool,
        handle: JoinHandle<Result<jcode_tool_types::ToolOutput>>,
    ) -> BackgroundTaskInfo {
        let (notify, wake) = normalize_delivery(notify, wake);
        let task_id = Self::generate_task_id();
        let output_path = self.output_dir.join(format!("{}.output", task_id));
        let status_path = self.output_dir.join(format!("{}.status.json", task_id));

        let initial_status = TaskStatusFile {
            task_id: task_id.clone(),
            tool_name: tool_name.to_string(),
            display_name: display_name.clone(),
            session_id: session_id.to_string(),
            status: BackgroundTaskStatus::Running,
            exit_code: None,
            error: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            duration_secs: None,
            pid: None,
            owner_pid: Some(std::process::id()),
            owner_instance: Some(model::process_instance_token().to_string()),
            detached: false,
            notify,
            wake,
            progress: None,
            event_history: Vec::new(),
        };
        // Adoption cannot fail closed on initial-persistence failure: the
        // future is ALREADY running (a promoted foreground command). Track
        // it regardless so shutdown finalize can abort it; the terminal
        // recovery loop (persist_terminal_with_recovery) will eventually
        // produce a durable record. Only a process death in the window
        // between a failed initial write and terminal recovery loses the
        // record, which is no worse than the pre-adoption foreground state.
        let durable_record = match self.store.write_initial(&initial_status).await {
            Ok(()) => true,
            Err(error) => {
                crate::logging::error(&format!(
                    "Initial status persistence failed for adopted task {task_id} (tracking anyway): {error:#}"
                ));
                false
            }
        };
        Self::publish_task_started_activity(
            &task_id,
            tool_name,
            display_name.as_deref(),
            session_id,
            notify,
        );

        let output_path_clone = output_path.clone();
        let status_path_clone = status_path.clone();
        let task_id_clone = task_id.clone();
        let task_id_clone2 = task_id.clone();
        let tool_name_owned = tool_name.to_string();
        let session_id_owned = session_id.to_string();
        let started_at = Instant::now();
        let started_at_rfc3339 = initial_status.started_at.clone();
        let display_name_owned = initial_status.display_name.clone();
        let (delivery_flags_tx, delivery_flags_rx) = watch::channel((notify, wake));
        let tasks_for_prune = Arc::clone(&self.tasks);
        let store_for_task = Arc::clone(&self.store);
        let (registered_tx, registered_rx) = tokio::sync::oneshot::channel::<()>();

        // F02-R2-B2: keep abort authority over the ORIGINAL future. Aborting
        // only the wrapper would drop-detach this JoinHandle and leave the
        // underlying work running.
        let original_abort = handle.abort_handle();
        let wrapper_handle = tokio::spawn(async move {
            let tool_result = handle.await;
            let duration_secs = started_at.elapsed().as_secs_f64();

            let (status, exit_code, error, output_text) = match tool_result {
                Ok(Ok(output)) => (
                    BackgroundTaskStatus::Completed,
                    Some(0),
                    None,
                    output.output,
                ),
                Ok(Err(e)) => (
                    BackgroundTaskStatus::Failed,
                    None,
                    Some(e.to_string()),
                    e.to_string(),
                ),
                Err(e) => (
                    BackgroundTaskStatus::Failed,
                    None,
                    Some(e.to_string()),
                    format!("Task panicked: {}", e),
                ),
            };

            if let Ok(mut file) = File::create(&output_path_clone).await {
                let _ = file.write_all(output_text.as_bytes()).await;
            }

            let (notify_flag, wake_flag) = *delivery_flags_rx.borrow();
            // Terminal write via store with in-process recovery (F04-B1);
            // prune only after persistence succeeds (helper), registration
            // awaited first so instant completions cannot leave phantoms.
            let _ = registered_rx.await;
            let _ = &status_path_clone;
            persist_terminal_with_recovery(
                store_for_task,
                tasks_for_prune,
                TerminalSpec {
                    task_id: task_id_clone2,
                    tool_name: tool_name_owned.clone(),
                    display_name: display_name_owned.clone(),
                    session_id: session_id_owned.clone(),
                    status: status.clone(),
                    exit_code,
                    error: error.clone(),
                    started_at: started_at_rfc3339,
                    completed_at: chrono::Utc::now().to_rfc3339(),
                    duration_secs,
                    notify: notify_flag,
                    wake: wake_flag,
                },
            )
            .await;

            let output_preview = if output_text.len() > 500 {
                format!("{}...", crate::util::truncate_str(&output_text, 500))
            } else {
                output_text
            };

            Bus::global().publish(BusEvent::BackgroundTaskCompleted(BackgroundTaskCompleted {
                task_id: task_id_clone,
                tool_name: tool_name_owned,
                display_name: display_name_owned,
                session_id: session_id_owned,
                status: status.clone(),
                exit_code,
                output_preview,
                output_file: output_path_clone,
                duration_secs,
                notify: notify_flag,
                wake: wake_flag,
            }));

            Ok(TaskResult {
                exit_code,
                error,
                status: Some(status),
            })
        });

        let running_task = RunningTask {
            task_id: task_id.clone(),
            tool_name: tool_name.to_string(),
            display_name: None,
            session_id: session_id.to_string(),
            status_path: status_path.clone(),
            started_at,
            started_at_rfc3339: initial_status.started_at.clone(),
            delivery_flags: delivery_flags_tx,
            handle: wrapper_handle,
            original_abort: Some(original_abort),
            // Acquired at adoption: pre-adoption execution was covered by
            // the foreground owner's own lease (F01 design 3.3.3 C5).
            // Refusal during drain is NOT silent unleased new work (F02-B4):
            // the future already runs, and adopting it into the live map is
            // what lets the shutdown cleanup's finalize_non_detached abort
            // and finalize it. The refusal is logged; the task holds no
            // lease and cannot pin the daemon.
            activity_lease: match self.acquire_task_lease(&task_id, tool_name) {
                Ok(lease) => Some(lease),
                Err(refused) => {
                    crate::logging::warn(&format!(
                        "Adopted task {tool_name}/{task_id} unleased during shutdown drain \
                         ({refused}); shutdown finalize will abort it."
                    ));
                    None
                }
            },
            durable_record,
        };

        self.tasks
            .write()
            .await
            .insert(task_id.clone(), running_task);
        let _ = registered_tx.send(());

        BackgroundTaskInfo {
            task_id,
            output_file: output_path,
            status_file: status_path,
            refused: None,
        }
    }

    /// List all tasks (both running and completed from disk)
    pub async fn list(&self) -> Vec<TaskStatusFile> {
        let mut results = Vec::new();

        // Read all status files from disk
        if let Ok(mut entries) = fs::read_dir(&self.output_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false)
                    && let Some(status) = self.read_status_file(&path).await
                {
                    let reconciled = self.finalize_detached_status_if_needed(status, &path).await;
                    let reconciled = self
                        .finalize_orphaned_status_if_needed(reconciled, &path)
                        .await;
                    results.push(reconciled);
                }
            }
        }

        // Sort by task_id (which includes timestamp)
        results.sort_by(|a, b| b.task_id.cmp(&a.task_id));
        results
    }

    /// Get status of a specific task
    pub async fn status(&self, task_id: &str) -> Option<TaskStatusFile> {
        let status_path = self.status_path_for(task_id);
        let status = self.read_status_file(&status_path).await?;
        let status = self
            .finalize_detached_status_if_needed(status, &status_path)
            .await;
        Some(
            self.finalize_orphaned_status_if_needed(status, &status_path)
                .await,
        )
    }

    /// Best-effort synchronous check for whether a task is still live in this process.
    pub fn is_live_task(&self, task_id: &str) -> bool {
        let Ok(tasks) = self.tasks.try_read() else {
            return false;
        };
        tasks.contains_key(task_id)
    }

    /// Get full output of a task
    pub async fn output(&self, task_id: &str) -> Option<String> {
        let output_path = self.output_path_for(task_id);
        fs::read_to_string(&output_path).await.ok()
    }

    /// Wait for a task to finish, emit progress, or reach the caller's maximum wait.
    ///
    /// This combines bus-driven wakeups with a light periodic status reconciliation so
    /// detached tasks, missed broadcast messages, or crash/reload edges still return no
    /// later than `max_wait` and can notice completion without active polling by the agent.
    pub async fn wait(
        &self,
        task_id: &str,
        max_wait: Duration,
        return_on_progress: bool,
    ) -> Option<BackgroundTaskWaitResult> {
        let mut bus_rx = Bus::global().subscribe();
        let initial = self.status(task_id).await?;
        if initial.status != BackgroundTaskStatus::Running {
            return Some(BackgroundTaskWaitResult {
                reason: BackgroundTaskWaitReason::AlreadyFinished,
                task: initial,
                progress_event: None,
                event_record: None,
            });
        }
        if max_wait.is_zero() {
            return Some(BackgroundTaskWaitResult {
                reason: BackgroundTaskWaitReason::Timeout,
                task: initial,
                progress_event: None,
                event_record: None,
            });
        }

        let mut last_progress = initial.progress.clone();
        let deadline = TokioInstant::now() + max_wait;
        let timeout = tokio::time::sleep_until(deadline);
        tokio::pin!(timeout);
        let mut poll = tokio::time::interval(Duration::from_secs(1));
        poll.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    let task = self.status(task_id).await?;
                    let reason = if task.status == BackgroundTaskStatus::Running {
                        BackgroundTaskWaitReason::Timeout
                    } else {
                        BackgroundTaskWaitReason::Finished
                    };
                    return Some(BackgroundTaskWaitResult {
                        reason,
                        task,
                        progress_event: None,
                        event_record: None,
                    });
                }
                _ = poll.tick() => {
                    let task = self.status(task_id).await?;
                    if task.status != BackgroundTaskStatus::Running {
                        return Some(BackgroundTaskWaitResult {
                            reason: BackgroundTaskWaitReason::Finished,
                            task,
                            progress_event: None,
                            event_record: None,
                        });
                    }
                    if return_on_progress && task.progress != last_progress {
                        let event_record = task.event_history.last().cloned();
                        return Some(BackgroundTaskWaitResult {
                            reason: progress_wait_reason(event_record.as_ref()),
                            progress_event: None,
                            task,
                            event_record,
                        });
                    }
                    last_progress = task.progress.clone();
                }
                event = bus_rx.recv() => {
                    match event {
                        Ok(BusEvent::BackgroundTaskCompleted(event)) if event.task_id == task_id => {
                            let task = self.status(task_id).await?;
                            return Some(BackgroundTaskWaitResult {
                                reason: BackgroundTaskWaitReason::Finished,
                                task,
                                progress_event: None,
                                event_record: None,
                            });
                        }
                        Ok(BusEvent::BackgroundTaskProgress(event)) if event.task_id == task_id => {
                            if return_on_progress {
                                let task = self.status(task_id).await?;
                                let event_record = task.event_history.last().cloned();
                                return Some(BackgroundTaskWaitResult {
                                    reason: progress_wait_reason(event_record.as_ref()),
                                    task,
                                    progress_event: Some(event),
                                    event_record,
                                });
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let task = self.status(task_id).await?;
                            if task.status != BackgroundTaskStatus::Running {
                                return Some(BackgroundTaskWaitResult {
                                    reason: BackgroundTaskWaitReason::Finished,
                                    task,
                                    progress_event: None,
                                    event_record: None,
                                });
                            }
                            if return_on_progress && task.progress != last_progress {
                                let event_record = task.event_history.last().cloned();
                                return Some(BackgroundTaskWaitResult {
                                    reason: progress_wait_reason(event_record.as_ref()),
                                    progress_event: None,
                                    task,
                                    event_record,
                                });
                            }
                            last_progress = task.progress.clone();
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            let task = self.status(task_id).await?;
                            let reason = if task.status == BackgroundTaskStatus::Running {
                                BackgroundTaskWaitReason::Timeout
                            } else {
                                BackgroundTaskWaitReason::Finished
                            };
                            return Some(BackgroundTaskWaitResult {
                                reason,
                                task,
                                progress_event: None,
                                event_record: None,
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Update progress for an existing background task.
    pub async fn update_progress(
        &self,
        task_id: &str,
        progress: BackgroundTaskProgress,
    ) -> Result<Option<TaskStatusFile>> {
        self.update_progress_with_event_kind(task_id, progress, BackgroundTaskEventKind::Progress)
            .await
    }

    /// Record an explicit checkpoint for an existing background task.
    pub async fn update_checkpoint(
        &self,
        task_id: &str,
        progress: BackgroundTaskProgress,
    ) -> Result<Option<TaskStatusFile>> {
        self.update_progress_with_event_kind(task_id, progress, BackgroundTaskEventKind::Checkpoint)
            .await
    }

    async fn update_progress_with_event_kind(
        &self,
        task_id: &str,
        progress: BackgroundTaskProgress,
        event_kind: BackgroundTaskEventKind,
    ) -> Result<Option<TaskStatusFile>> {
        // Serialized read-modify-write via the store (F04): a concurrent
        // terminal write cannot be lost, and two progress updates cannot
        // interleave their read/write cycles.
        let progress = progress.normalize();
        let progress_for_mutate = progress.clone();
        let outcome = self
            .store
            .mutate(task_id, move |status| {
                if let Some(existing) = status.progress.as_ref() {
                    if progress_equivalent(existing, &progress_for_mutate) {
                        return false;
                    }
                    let existing_is_more_determinate = existing.percent.is_some()
                        || matches!((existing.current, existing.total), (_, Some(total)) if total > 0);
                    let new_is_less_determinate = progress_for_mutate.percent.is_none()
                        && !matches!(
                            (progress_for_mutate.current, progress_for_mutate.total),
                            (_, Some(total)) if total > 0
                        );
                    if existing_is_more_determinate
                        && new_is_less_determinate
                        && matches!(
                            progress_for_mutate.source,
                            BackgroundTaskProgressSource::ParsedOutput
                        )
                    {
                        return false;
                    }
                }
                status.progress = Some(progress_for_mutate.clone());
                push_task_event(
                    status,
                    progress_event_record(event_kind, progress_for_mutate.clone()),
                );
                true
            })
            .await?;

        let status = match outcome {
            // Applied: the mutation persisted this progress value; publish.
            store::MutateOutcome::Applied(status) => status,
            // Unchanged (no-op skip) or terminal truth preserved: nothing
            // new happened; no bus event (F04-I1).
            store::MutateOutcome::Unchanged(status)
            | store::MutateOutcome::TerminalPreserved(status) => return Ok(Some(status)),
            store::MutateOutcome::Missing => return Ok(None),
        };

        Bus::global().publish(BusEvent::BackgroundTaskProgress(
            BackgroundTaskProgressEvent {
                task_id: status.task_id.clone(),
                tool_name: status.tool_name.clone(),
                display_name: status.display_name.clone(),
                session_id: status.session_id.clone(),
                progress,
            },
        ));

        Ok(Some(status))
    }

    /// Update delivery behavior for an existing background task.
    ///
    /// This supports retroactively enabling notify/wake after the task was already started.
    pub async fn update_delivery(
        &self,
        task_id: &str,
        notify: bool,
        wake: bool,
    ) -> Result<Option<TaskStatusFile>> {
        let (notify, wake) = normalize_delivery(notify, wake);
        // Serialized mutate (F04): delivery flags are non-terminal fields,
        // so this applies even to a task that just went terminal, without
        // being able to disturb the terminal truth.
        let outcome = self
            .store
            .mutate(task_id, move |status| {
                status.notify = notify;
                status.wake = wake;
                let event_status = status.status.clone();
                let event_exit_code = status.exit_code;
                let event_progress = status.progress.clone();
                push_task_event(
                    status,
                    BackgroundTaskEventRecord {
                        kind: BackgroundTaskEventKind::DeliveryUpdated,
                        timestamp: Utc::now().to_rfc3339(),
                        message: Some(format!("notify={}, wake={}", notify, wake)),
                        status: Some(event_status),
                        exit_code: event_exit_code,
                        progress: event_progress,
                    },
                );
                true
            })
            .await?;
        let status = match outcome {
            store::MutateOutcome::Applied(status)
            | store::MutateOutcome::TerminalPreserved(status)
            | store::MutateOutcome::Unchanged(status) => status,
            store::MutateOutcome::Missing => return Ok(None),
        };

        if let Some(task) = self.tasks.read().await.get(task_id) {
            let _ = task.delivery_flags.send((notify, wake));
        }

        Ok(Some(status))
    }

    /// Cancel a running task
    pub async fn cancel(&self, task_id: &str) -> Result<bool> {
        self.cancel_with_grace(task_id, std::time::Duration::from_millis(400))
            .await
    }

    /// Cancel a running task, allowing detached processes a configurable grace period
    /// between TERM and KILL on Unix.
    pub async fn cancel_with_grace(
        &self,
        task_id: &str,
        _graceful_timeout: std::time::Duration,
    ) -> Result<bool> {
        let tasks = self.tasks.read().await;
        if let Some(task) = tasks.get(task_id) {
            // Abort IN PLACE (F04-R2-B1): the entry STAYS in the live map as
            // a tombstone until terminal persistence succeeds, so a failed
            // write can never leave a cancelled task with neither a map
            // entry nor a durable record. The recovery helper prunes on
            // success (immediately or from its retry loop).
            if let Some(original) = &task.original_abort {
                original.abort();
            }
            task.handle.abort();
            let spec = TerminalSpec {
                task_id: task.task_id.clone(),
                tool_name: task.tool_name.clone(),
                display_name: task.display_name.clone(),
                session_id: task.session_id.clone(),
                status: BackgroundTaskStatus::Failed,
                exit_code: None,
                error: Some("Cancelled by user".to_string()),
                started_at: task.started_at_rfc3339.clone(),
                completed_at: chrono::Utc::now().to_rfc3339(),
                duration_secs: task.started_at.elapsed().as_secs_f64(),
                notify: task.delivery_flags.borrow().0,
                wake: task.delivery_flags.borrow().1,
            };
            drop(tasks);

            persist_terminal_with_recovery(Arc::clone(&self.store), Arc::clone(&self.tasks), spec)
                .await;

            Ok(true)
        } else {
            drop(tasks);

            let status_path = self.status_path_for(task_id);
            let Some(mut status) = self.read_status_file(&status_path).await else {
                return Ok(false);
            };
            status = self
                .finalize_detached_status_if_needed(status, &status_path)
                .await;
            if status.status != BackgroundTaskStatus::Running || !status.detached {
                return Ok(false);
            }

            let Some(pid) = status.pid else {
                return Ok(false);
            };

            #[cfg(unix)]
            {
                let _ = crate::platform::signal_detached_process_group(pid, libc::SIGTERM);
                tokio::time::sleep(_graceful_timeout).await;
                if crate::platform::is_process_running(pid) {
                    let _ = crate::platform::signal_detached_process_group(pid, libc::SIGKILL);
                }
            }
            #[cfg(windows)]
            {
                let _ = crate::platform::signal_detached_process_group(pid, 0);
            }

            let completed_at = Utc::now();
            status.status = BackgroundTaskStatus::Failed;
            status.exit_code = None;
            status.error = Some("Cancelled by user".to_string());
            status.completed_at = Some(completed_at.to_rfc3339());
            status.duration_secs = Self::status_duration_secs(&status.started_at, completed_at);
            let event_status = status.status.clone();
            let event_exit_code = status.exit_code;
            let event_error = status.error.clone();
            push_task_event(
                &mut status,
                terminal_event_record(event_status, event_exit_code, event_error.as_deref()),
            );
            let detached_task_id = status.task_id.clone();
            if let Err(error) = self
                .store
                .write_terminal(&detached_task_id, move |_| status)
                .await
            {
                crate::logging::error(&format!(
                    "Detached cancel terminal persistence failed: {error:#}"
                ));
            }
            Ok(true)
        }
    }

    /// Clean up old task files (older than specified hours)
    pub async fn cleanup(&self, max_age_hours: u64) -> Result<usize> {
        Ok(self
            .cleanup_filtered(max_age_hours, &std::collections::HashSet::new(), false)
            .await?
            .removed_files)
    }

    /// Clean up old task files, skipping running tasks and optionally filtering by status.
    pub async fn cleanup_filtered(
        &self,
        max_age_hours: u64,
        status_filter: &std::collections::HashSet<&str>,
        dry_run: bool,
    ) -> Result<BackgroundCleanupResult> {
        let mut result = BackgroundCleanupResult {
            matched_files: 0,
            removed_files: 0,
            skipped_running_files: 0,
        };
        let cutoff =
            std::time::SystemTime::now() - std::time::Duration::from_secs(max_age_hours * 3600);

        if let Ok(mut entries) = fs::read_dir(&self.output_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let Ok(metadata) = fs::metadata(&path).await else {
                    continue;
                };
                let Ok(modified) = metadata.modified() else {
                    continue;
                };
                if modified >= cutoff {
                    continue;
                }

                let mut associated_status = None;
                if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    associated_status = self.read_status_file(&path).await;
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("output")
                    && let Some(task_id) = path.file_stem().and_then(|stem| stem.to_str())
                {
                    associated_status = self.status(task_id).await;
                }

                if let Some(status) = associated_status.as_ref() {
                    if status.status == BackgroundTaskStatus::Running {
                        result.skipped_running_files += 1;
                        continue;
                    }
                    let status_label = match status.status {
                        BackgroundTaskStatus::Running => "running",
                        BackgroundTaskStatus::Completed => "completed",
                        BackgroundTaskStatus::Superseded => "superseded",
                        BackgroundTaskStatus::Failed => "failed",
                    };
                    if !status_filter.is_empty() && !status_filter.contains(status_label) {
                        continue;
                    }
                } else if !status_filter.is_empty() {
                    continue;
                }

                result.matched_files += 1;
                if !dry_run {
                    let _ = fs::remove_file(&path).await;
                    result.removed_files += 1;
                }
            }
        }

        if dry_run {
            result.removed_files = result.matched_files;
        }

        Ok(result)
    }

    /// Best-effort synchronous snapshot of currently running tasks.
    /// This avoids async calls in render paths.
    pub fn running_snapshot(&self) -> (usize, Vec<String>, Option<RunningBackgroundProgress>) {
        let Ok(tasks) = self.tasks.try_read() else {
            return (0, Vec::new(), None);
        };

        let mut rows: Vec<RunningBackgroundProgress> = Vec::new();
        for task in tasks.values() {
            let status = std::fs::read_to_string(&task.status_path)
                .ok()
                .and_then(|content| serde_json::from_str::<TaskStatusFile>(&content).ok());
            let progress = status.as_ref().and_then(|status| status.progress.clone());
            let label = status
                .as_ref()
                .and_then(|status| status.display_name.clone())
                .or_else(|| task.display_name.clone())
                .unwrap_or_else(|| task.tool_name.clone());

            rows.push(RunningBackgroundProgress {
                task_id: task.task_id.clone(),
                tool_name: task.tool_name.clone(),
                label,
                detail: progress.map(|progress| format_progress_display(&progress, 10)),
            });
        }

        rows.sort_by(|a, b| b.task_id.cmp(&a.task_id));
        let latest = rows.iter().find(|row| row.detail.is_some()).cloned();

        (
            tasks.len(),
            rows.iter().map(|row| row.label.clone()).collect(),
            latest,
        )
    }

    /// Best-effort synchronous lookup of detached tasks that are still running
    /// for a specific session.
    ///
    /// This is primarily used during self-dev reload recovery, where the new
    /// process needs to remind the agent that a previous `bash` command was
    /// persisted into the background instead of being interrupted.
    pub fn persisted_detached_running_tasks_for_session(
        &self,
        session_id: &str,
    ) -> Vec<TaskStatusFile> {
        let mut matches = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.output_dir) else {
            return matches;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(status) = serde_json::from_str::<TaskStatusFile>(&content) else {
                continue;
            };

            if status.session_id != session_id
                || status.status != BackgroundTaskStatus::Running
                || !status.detached
            {
                continue;
            }

            let Some(pid) = status.pid else {
                continue;
            };

            if crate::platform::is_process_running(pid) {
                matches.push(status);
            }
        }

        matches.sort_by(|a, b| a.task_id.cmp(&b.task_id));
        matches
    }
}

/// Owned terminal outcome of a task, used to (re)build the final status file
/// against freshly loaded prior state on every persistence attempt (F04-B1).
#[derive(Clone)]
struct TerminalSpec {
    task_id: String,
    tool_name: String,
    display_name: Option<String>,
    session_id: String,
    status: BackgroundTaskStatus,
    exit_code: Option<i32>,
    error: Option<String>,
    started_at: String,
    completed_at: String,
    duration_secs: f64,
    notify: bool,
    wake: bool,
}

fn build_terminal_status(prior: Option<TaskStatusFile>, spec: &TerminalSpec) -> TaskStatusFile {
    // The freshly loaded durable state is authoritative for mutable delivery
    // flags. This closes the update_delivery-vs-completion and retry-window
    // race where a stale TerminalSpec snapshot could otherwise overwrite a
    // delivery change that already persisted (F04-R3-I1 / F05).
    let (notify, wake, prior_progress, prior_event_history) = prior
        .map(|status| {
            (
                status.notify,
                status.wake,
                status.progress,
                status.event_history,
            )
        })
        .unwrap_or_else(|| (spec.notify, spec.wake, None, Vec::new()));
    let mut final_status = TaskStatusFile {
        task_id: spec.task_id.clone(),
        tool_name: spec.tool_name.clone(),
        display_name: spec.display_name.clone(),
        session_id: spec.session_id.clone(),
        status: spec.status.clone(),
        exit_code: spec.exit_code,
        error: spec.error.clone(),
        started_at: spec.started_at.clone(),
        completed_at: Some(spec.completed_at.clone()),
        duration_secs: Some(spec.duration_secs),
        pid: None,
        owner_pid: Some(std::process::id()),
        owner_instance: Some(model::process_instance_token().to_string()),
        detached: false,
        notify,
        wake,
        progress: prior_progress,
        event_history: prior_event_history,
    };
    push_task_event(
        &mut final_status,
        terminal_event_record(spec.status.clone(), spec.exit_code, spec.error.as_deref()),
    );
    final_status
}

/// Persist a terminal state with in-process durable recovery (F04-B1).
///
/// On immediate success the live-map entry is pruned. On failure the entry
/// is RETAINED (a visible tombstone, never a phantom "pruned but Running on
/// disk" state) and a detached retry loop keeps attempting persistence with
/// backoff, pruning only once the write lands. Across a process death the
/// initial `Running` file (a spawn prerequisite since F04-B1) is reconciled
/// by the next-boot orphan sweep.
async fn persist_terminal_with_recovery(
    store: Arc<store::TaskStatusStore>,
    tasks: Arc<RwLock<HashMap<String, RunningTask>>>,
    spec: TerminalSpec,
) {
    let spec_for_first = spec.clone();
    match store
        .write_terminal(&spec.task_id, move |prior| {
            build_terminal_status(prior, &spec_for_first)
        })
        .await
    {
        Ok(_) => {
            tasks.write().await.remove(&spec.task_id);
        }
        Err(error) => {
            crate::logging::error(&format!(
                "Terminal persistence failed for task {}; retaining live-map tombstone and retrying: {error:#}",
                spec.task_id
            ));
            tokio::spawn(async move {
                let mut delay = std::time::Duration::from_millis(250);
                loop {
                    tokio::time::sleep(delay).await;
                    let spec_for_retry = spec.clone();
                    match store
                        .write_terminal(&spec.task_id, move |prior| {
                            build_terminal_status(prior, &spec_for_retry)
                        })
                        .await
                    {
                        Ok(_) => {
                            crate::logging::info(&format!(
                                "Terminal persistence recovered for task {}",
                                spec.task_id
                            ));
                            tasks.write().await.remove(&spec.task_id);
                            return;
                        }
                        Err(error) => {
                            crate::logging::warn(&format!(
                                "Terminal persistence retry failed for task {}: {error:#}",
                                spec.task_id
                            ));
                            delay = (delay * 2).min(std::time::Duration::from_secs(60));
                        }
                    }
                }
            });
        }
    }
}

impl Default for BackgroundTaskManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton for background task manager
static BACKGROUND_MANAGER: std::sync::OnceLock<BackgroundTaskManager> = std::sync::OnceLock::new();

/// Get the global background task manager
pub fn global() -> &'static BackgroundTaskManager {
    BACKGROUND_MANAGER.get_or_init(BackgroundTaskManager::new)
}

#[cfg(test)]
mod tests;
