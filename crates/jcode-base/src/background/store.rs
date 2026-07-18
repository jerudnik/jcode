//! Crash-durable, atomic persistence for background task status files (F04/F05).
//!
//! Every status-file write in the manager goes through this store. It
//! guarantees, per the ideal-base acceptance standard A2:
//!
//! - **Atomic durability**: writes go to a unique same-directory temp file,
//!   fsync that file, rename it into place, then fsync the parent directory, so
//!   readers never observe torn JSON and successful writes survive crashes.
//! - **Serialization**: all reads-for-write and writes for one task across
//!   every store instance in this process share a path-keyed async mutex, so
//!   concurrent progress/delivery/completion cycles cannot lose each other.
//! - **Terminal precedence**: once a task's persisted status is terminal
//!   (anything but `Running`), no later mutation can resurrect `Running` or
//!   replace the terminal truth. The first terminal write wins; subsequent
//!   mutations may only touch non-terminal fields (delivery flags, event
//!   history).
//! - **Surfaced failures**: serialization and IO errors are returned as
//!   `Result`s, never silently dropped. Terminal writes retry before
//!   reporting failure because losing a terminal state is the worst outcome
//!   (the file would claim `Running` forever within this process lifetime).

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};
use tokio::io::AsyncWriteExt;

use super::model::TaskStatusFile;
use jcode_background_types::BackgroundTaskStatus;

/// Fields that constitute the terminal truth of a task. Once persisted
/// terminal, these are immutable (first terminal write wins).
fn is_terminal(status: &TaskStatusFile) -> bool {
    status.status != BackgroundTaskStatus::Running
}

/// Outcome of a terminal write attempt.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TerminalWriteOutcome {
    /// This call persisted the terminal state.
    Written,
    /// A terminal state was already persisted; it was preserved untouched.
    AlreadyTerminal,
}

/// Outcome of a running-state mutation.
#[derive(Debug)]
pub(super) enum MutateOutcome {
    /// The mutation was applied and persisted.
    Applied(TaskStatusFile),
    /// The persisted state is terminal; terminal fields were preserved.
    /// Non-terminal fields from the mutation (delivery flags, events) were
    /// still applied and persisted.
    TerminalPreserved(TaskStatusFile),
    /// The closure declined (returned false): nothing was written. The
    /// carried state is the PERSISTED state, untouched by the closure's
    /// in-memory mutations.
    Unchanged(TaskStatusFile),
    /// No status file exists for this task.
    Missing,
}

pub(super) struct TaskStatusStore {
    dir: PathBuf,
}

/// Process-wide locks are keyed by final status path, not store instance, so
/// independent `BackgroundTaskManager`s sharing a directory serialize the
/// same task's read-modify-write cycles too.
static TASK_LOCKS: OnceLock<std::sync::Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
    OnceLock::new();
const STALE_TEMP_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

impl TaskStatusStore {
    pub(super) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub(super) fn status_path(&self, task_id: &str) -> PathBuf {
        self.dir.join(format!("{task_id}.status.json"))
    }

    fn lock_for(&self, task_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let path = self.status_path(task_id);
        let mut locks = TASK_LOCKS
            .get_or_init(|| std::sync::Mutex::new(HashMap::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Arc::clone(
            locks
                .entry(path)
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
        )
    }

    /// Crash-durable atomic replacement: serialize, write to a unique
    /// same-directory temp file, fsync the temp file, rename it into place,
    /// then fsync the parent directory. A concurrent reader sees the old file
    /// or the new file, never a torn mix; after a successful return the file
    /// contents and directory entry have both crossed the durability boundary.
    async fn write_atomic(&self, path: &Path, status: &TaskStatusFile) -> Result<()> {
        let json = serde_json::to_string_pretty(status)
            .with_context(|| format!("serialize status for task {}", status.task_id))?;
        let tmp = path.with_extension(format!(
            "json.tmp.{}.{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .await
            .with_context(|| format!("create temp status file {}", tmp.display()))?;
        if let Err(error) = async {
            file.write_all(json.as_bytes()).await?;
            file.sync_all().await
        }
        .await
        {
            drop(file);
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(anyhow::Error::from(error))
                .with_context(|| format!("write and fsync temp status file {}", tmp.display()));
        }
        drop(file);
        // Rename is atomic on POSIX within the same filesystem (the temp file
        // is a sibling). On failure, clean up the temp file.
        if let Err(error) = tokio::fs::rename(&tmp, path).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(anyhow::Error::from(error))
                .with_context(|| format!("rename status file into place at {}", path.display()));
        }
        let parent = path
            .parent()
            .with_context(|| format!("status path has no parent: {}", path.display()))?;
        let parent_dir = tokio::fs::File::open(parent)
            .await
            .with_context(|| format!("open status parent directory {}", parent.display()))?;
        parent_dir
            .sync_all()
            .await
            .with_context(|| format!("fsync status parent directory {}", parent.display()))?;
        Ok(())
    }

    /// Remove abandoned `*.json.tmp.*` siblings left by interrupted writers.
    /// A temp file is eligible when its encoded owner PID is dead or when its
    /// modification time is older than the conservative age bound. Unknown
    /// suffixes are only removed by age, never merely by name.
    pub(super) async fn cleanup_stale_temp_files(&self) -> usize {
        let mut removed = 0;
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return removed;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(suffix) = name.split_once(".json.tmp.").map(|(_, suffix)| suffix) else {
                continue;
            };
            let owner_dead = suffix
                .split('.')
                .next()
                .and_then(|pid| pid.parse::<u32>().ok())
                .is_some_and(|pid| {
                    pid != std::process::id() && !crate::platform::is_process_running(pid)
                });
            let age_expired = entry
                .metadata()
                .await
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                .is_some_and(|age| age >= STALE_TEMP_MAX_AGE);
            if !owner_dead && !age_expired {
                continue;
            }
            match tokio::fs::remove_file(&path).await {
                Ok(()) => removed += 1,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => crate::logging::warn(&format!(
                    "Failed to remove stale background status temp {}: {error}",
                    path.display()
                )),
            }
        }
        removed
    }

    /// Read a status file. Distinguishes missing (Ok(None)) from malformed
    /// or unreadable (Err), so corruption is surfaced instead of treated as
    /// absence.
    pub(super) async fn read(&self, task_id: &str) -> Result<Option<TaskStatusFile>> {
        let path = self.status_path(task_id);
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(anyhow::Error::from(error))
                    .with_context(|| format!("read status file {}", path.display()));
            }
        };
        let status: TaskStatusFile = serde_json::from_str(&content)
            .with_context(|| format!("malformed status file {}", path.display()))?;
        Ok(Some(status))
    }

    /// Parse a status file at an arbitrary path (directory sweeps). Malformed
    /// content is surfaced as Err.
    pub(super) async fn read_path(&self, path: &Path) -> Result<Option<TaskStatusFile>> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(anyhow::Error::from(error))
                    .with_context(|| format!("read status file {}", path.display()));
            }
        };
        let status: TaskStatusFile = serde_json::from_str(&content)
            .with_context(|| format!("malformed status file {}", path.display()))?;
        Ok(Some(status))
    }

    /// Persist the initial `Running` status for a new task. Task IDs are
    /// create-once: any existing valid status file, including `Running`, is a
    /// collision and is rejected rather than replacing another task's owner,
    /// session, progress, or terminal truth. Malformed existing files surface
    /// as errors and are likewise never silently overwritten here.
    pub(super) async fn write_initial(&self, status: &TaskStatusFile) -> Result<()> {
        let lock = self.lock_for(&status.task_id);
        let _guard = lock.lock().await;
        if let Some(existing) = self.read(&status.task_id).await? {
            anyhow::bail!(
                "refusing initial status for task {}: a {:?} status file already exists",
                status.task_id,
                existing.status
            );
        }
        self.write_atomic(&self.status_path(&status.task_id), status)
            .await
    }

    /// Serialized read-modify-write for a (presumed) running task.
    ///
    /// The mutation closure runs on the freshly loaded state. If the
    /// persisted state is terminal, the terminal truth (status, exit_code,
    /// error, completed_at, duration_secs) is restored after the closure so
    /// no mutation can resurrect `Running` or alter the terminal outcome;
    /// remaining field changes (delivery flags, event history) still persist.
    ///
    /// Returns the persisted state, or `Missing` when no file exists. The
    /// closure may return `false` to skip persisting (no-op mutations).
    pub(super) async fn mutate<F>(&self, task_id: &str, mutate: F) -> Result<MutateOutcome>
    where
        F: FnOnce(&mut TaskStatusFile) -> bool,
    {
        let lock = self.lock_for(task_id);
        let _guard = lock.lock().await;
        let Some(existing) = self.read(task_id).await? else {
            return Ok(MutateOutcome::Missing);
        };
        let terminal_truth = is_terminal(&existing).then(|| existing.clone());
        let mut updated = existing.clone();
        if !mutate(&mut updated) {
            // Nothing written: return the PERSISTED state, not the closure's
            // discarded in-memory mutations (store contract: the returned
            // state is always persisted truth).
            return Ok(MutateOutcome::Unchanged(existing));
        }
        if let Some(truth) = terminal_truth {
            // Terminal precedence: terminal fields are immutable.
            updated.status = truth.status;
            updated.exit_code = truth.exit_code;
            updated.error = truth.error;
            updated.completed_at = truth.completed_at;
            updated.duration_secs = truth.duration_secs;
            self.write_atomic(&self.status_path(task_id), &updated)
                .await?;
            return Ok(MutateOutcome::TerminalPreserved(updated));
        }
        self.write_atomic(&self.status_path(task_id), &updated)
            .await?;
        Ok(MutateOutcome::Applied(updated))
    }

    /// Persist a terminal state. First terminal write wins: if the persisted
    /// state is already terminal, it is preserved untouched and
    /// `AlreadyTerminal` is returned. IO failures are retried (losing a
    /// terminal state means the file claims `Running` forever within this
    /// process lifetime), then surfaced.
    pub(super) async fn write_terminal<F>(
        &self,
        task_id: &str,
        build: F,
    ) -> Result<TerminalWriteOutcome>
    where
        F: FnOnce(Option<TaskStatusFile>) -> TaskStatusFile,
    {
        let lock = self.lock_for(task_id);
        let _guard = lock.lock().await;
        let existing = match self.read(task_id).await {
            Ok(existing) => existing,
            Err(error) => {
                // A malformed file must not block terminal persistence: the
                // terminal write is the recovery. Log and overwrite.
                crate::logging::warn(&format!(
                    "Background status unreadable before terminal write (overwriting): {error:#}"
                ));
                None
            }
        };
        if let Some(existing) = existing.as_ref()
            && is_terminal(existing)
        {
            return Ok(TerminalWriteOutcome::AlreadyTerminal);
        }
        let status = build(existing);
        debug_assert!(
            is_terminal(&status),
            "write_terminal must be given a terminal status"
        );
        let path = self.status_path(task_id);
        let mut last_error = None;
        for attempt in 0..3 {
            match self.write_atomic(&path, &status).await {
                Ok(()) => return Ok(TerminalWriteOutcome::Written),
                Err(error) => {
                    crate::logging::warn(&format!(
                        "Terminal status write attempt {} failed for task {}: {error:#}",
                        attempt + 1,
                        task_id
                    ));
                    last_error = Some(error);
                    tokio::time::sleep(std::time::Duration::from_millis(50 * (attempt + 1))).await;
                }
            }
        }
        Err(last_error.expect("retry loop ran").context(format!(
            "terminal status persistence failed for task {task_id} after retries; \
             the file will be reconciled at next process startup"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::{
        BackgroundTaskEventKind, BackgroundTaskEventRecord, push_task_event,
    };
    use super::*;

    fn running_status(task_id: &str) -> TaskStatusFile {
        TaskStatusFile {
            task_id: task_id.to_string(),
            tool_name: "bash".into(),
            display_name: None,
            session_id: "s".into(),
            status: BackgroundTaskStatus::Running,
            exit_code: None,
            error: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            duration_secs: None,
            pid: None,
            owner_pid: Some(std::process::id()),
            owner_instance: None,
            detached: false,
            notify: false,
            wake: false,
            progress: None,
            event_history: Vec::new(),
        }
    }

    fn terminal_from(existing: Option<TaskStatusFile>, task_id: &str) -> TaskStatusFile {
        let mut status = existing.unwrap_or_else(|| running_status(task_id));
        status.status = BackgroundTaskStatus::Completed;
        status.exit_code = Some(0);
        status.completed_at = Some(chrono::Utc::now().to_rfc3339());
        status
    }

    #[tokio::test]
    async fn read_distinguishes_missing_from_malformed() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStatusStore::new(tmp.path().to_path_buf());
        assert!(store.read("absent").await.unwrap().is_none());

        tokio::fs::write(store.status_path("broken"), b"{not json")
            .await
            .unwrap();
        let error = store.read("broken").await.expect_err("malformed is Err");
        assert!(error.to_string().contains("malformed"), "{error:#}");
    }

    #[tokio::test]
    async fn terminal_precedence_survives_concurrent_delivery_update() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStatusStore::new(tmp.path().to_path_buf());
        store.write_initial(&running_status("t1")).await.unwrap();

        // Terminal write first.
        let outcome = store
            .write_terminal("t1", |existing| terminal_from(existing, "t1"))
            .await
            .unwrap();
        assert_eq!(outcome, TerminalWriteOutcome::Written);

        // A racing delivery mutation must not resurrect Running.
        let result = store
            .mutate("t1", |status| {
                status.notify = true;
                status.wake = true;
                status.status = BackgroundTaskStatus::Running; // hostile
                status.exit_code = None;
                true
            })
            .await
            .unwrap();
        let MutateOutcome::TerminalPreserved(persisted) = result else {
            panic!("expected TerminalPreserved, got {result:?}");
        };
        assert_eq!(persisted.status, BackgroundTaskStatus::Completed);
        assert_eq!(persisted.exit_code, Some(0));
        assert!(persisted.notify, "non-terminal fields still apply");

        let on_disk = store.read("t1").await.unwrap().unwrap();
        assert_eq!(on_disk.status, BackgroundTaskStatus::Completed);
        assert!(on_disk.notify);
    }

    #[tokio::test]
    async fn first_terminal_write_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStatusStore::new(tmp.path().to_path_buf());
        store.write_initial(&running_status("t2")).await.unwrap();

        assert_eq!(
            store
                .write_terminal("t2", |existing| terminal_from(existing, "t2"))
                .await
                .unwrap(),
            TerminalWriteOutcome::Written
        );
        // Second terminal (e.g. cancel racing natural completion) is a no-op.
        assert_eq!(
            store
                .write_terminal("t2", |existing| {
                    let mut status = existing.unwrap();
                    status.status = BackgroundTaskStatus::Failed;
                    status.error = Some("cancelled".into());
                    status
                })
                .await
                .unwrap(),
            TerminalWriteOutcome::AlreadyTerminal
        );
        let on_disk = store.read("t2").await.unwrap().unwrap();
        assert_eq!(on_disk.status, BackgroundTaskStatus::Completed);
        assert!(on_disk.error.is_none());
    }

    #[tokio::test]
    async fn write_failure_is_surfaced_not_swallowed() {
        let tmp = tempfile::tempdir().unwrap();
        // Point the store at a path that is a FILE, so its strict collision
        // read fails before any write can be attempted.
        let bogus_dir = tmp.path().join("not-a-dir");
        tokio::fs::write(&bogus_dir, b"file").await.unwrap();
        let store = TaskStatusStore::new(bogus_dir);

        let error = store
            .write_initial(&running_status("t3"))
            .await
            .expect_err("write into a non-directory must fail");
        assert!(error.to_string().contains("read status file"), "{error:#}");

        let error = store
            .write_terminal("t3", |existing| terminal_from(existing, "t3"))
            .await
            .expect_err("terminal write failure must be surfaced after retries");
        assert!(
            error
                .to_string()
                .contains("terminal status persistence failed"),
            "{error:#}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn concurrent_writers_never_tear_json_or_lose_terminal() {
        // Hammer one task with concurrent progress mutations, delivery
        // mutations, and one terminal write; readers poll continuously and
        // must never observe unparseable JSON; the terminal state must win.
        let tmp = tempfile::tempdir().unwrap();
        let store = Arc::new(TaskStatusStore::new(tmp.path().to_path_buf()));
        store.write_initial(&running_status("race")).await.unwrap();

        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let path = store.status_path("race");
        let reader_stop = Arc::clone(&stop);
        let reader = tokio::spawn(async move {
            let mut observed = 0usize;
            while !reader_stop.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    serde_json::from_str::<TaskStatusFile>(&content)
                        .expect("reader must never observe torn/partial JSON");
                    observed += 1;
                }
                tokio::task::yield_now().await;
            }
            observed
        });

        let mut writers = Vec::new();
        for writer_id in 0..4 {
            let store = Arc::clone(&store);
            writers.push(tokio::spawn(async move {
                for i in 0..25 {
                    let _ = store
                        .mutate("race", |status| {
                            push_task_event(
                                status,
                                BackgroundTaskEventRecord {
                                    kind: BackgroundTaskEventKind::Progress,
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    message: Some(format!("w{writer_id} i{i}")),
                                    status: None,
                                    exit_code: None,
                                    progress: None,
                                },
                            );
                            true
                        })
                        .await;
                }
            }));
        }
        let terminal_store = Arc::clone(&store);
        let terminal = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            terminal_store
                .write_terminal("race", |existing| terminal_from(existing, "race"))
                .await
                .unwrap()
        });

        for writer in writers {
            writer.await.unwrap();
        }
        assert_eq!(terminal.await.unwrap(), TerminalWriteOutcome::Written);
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let observed = reader.await.unwrap();
        assert!(observed > 0, "reader must have sampled the file");

        let final_state = store.read("race").await.unwrap().unwrap();
        assert_eq!(
            final_state.status,
            BackgroundTaskStatus::Completed,
            "terminal truth must survive every concurrent mutation"
        );
    }

    #[tokio::test]
    async fn initial_write_refuses_to_clobber_terminal() {
        let tmp = tempfile::tempdir().unwrap();
        let store = TaskStatusStore::new(tmp.path().to_path_buf());
        store.write_initial(&running_status("t4")).await.unwrap();
        store
            .write_terminal("t4", |existing| terminal_from(existing, "t4"))
            .await
            .unwrap();
        let error = store
            .write_initial(&running_status("t4"))
            .await
            .expect_err("terminal file must not be clobbered by a new initial state");
        assert!(error.to_string().contains("already exists"), "{error:#}");
    }

    #[tokio::test]
    async fn write_initial_rejects_existing_running_collision_without_clobbering() {
        let tmp = tempfile::tempdir().unwrap();
        let first_store = TaskStatusStore::new(tmp.path().to_path_buf());
        let second_store = TaskStatusStore::new(tmp.path().to_path_buf());
        let mut first = running_status("collision");
        first.session_id = "original-session".into();
        first_store.write_initial(&first).await.unwrap();

        let mut collision = running_status("collision");
        collision.session_id = "colliding-session".into();
        let error = second_store.write_initial(&collision).await.unwrap_err();
        assert!(error.to_string().contains("already exists"), "{error:#}");

        let persisted = first_store.read("collision").await.unwrap().unwrap();
        assert_eq!(persisted.status, BackgroundTaskStatus::Running);
        assert_eq!(persisted.session_id, "original-session");
    }
}
