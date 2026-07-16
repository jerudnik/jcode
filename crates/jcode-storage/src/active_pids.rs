//! Tracking of active session process IDs under `~/.jcode/active_pids`.
//!
//! This is pure filesystem state keyed by session ID, used to discover which
//! sessions are currently running (and to map a PID back to its session). It
//! lives in the storage crate because it only needs [`jcode_dir`] and is a
//! low-level concern shared by session management, dictation, and crash
//! recovery, none of which should pull the full `session` module into scope.

use crate::jcode_dir;
use std::fs::{File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const PID_MARKER_LOCK_FILE: &str = ".pid-markers.lock";
const PID_MARKER_TEMP_PREFIX: &str = ".pid-marker-";
const PID_MARKER_LOCK_ATTEMPTS: usize = 64;

struct PidMarkerLock {
    file: File,
}

impl PidMarkerLock {
    fn open_lock_file() -> Option<File> {
        let home = jcode_dir().ok()?;
        std::fs::create_dir_all(&home).ok()?;
        OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(home.join(PID_MARKER_LOCK_FILE))
            .ok()
    }

    fn acquire_writer() -> Option<Self> {
        let file = Self::open_lock_file()?;
        fs2::FileExt::lock_exclusive(&file).ok()?;
        Some(Self { file })
    }

    fn acquire_bounded() -> Option<Self> {
        let file = Self::open_lock_file()?;
        for _ in 0..PID_MARKER_LOCK_ATTEMPTS {
            match fs2::FileExt::try_lock_exclusive(&file) {
                Ok(()) => return Some(Self { file }),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::yield_now();
                }
                Err(_) => return None,
            }
        }
        None
    }
}

impl Drop for PidMarkerLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

/// Directory holding one file per active session ID (`~/.jcode/active_pids`).
pub fn active_pids_dir() -> Option<PathBuf> {
    jcode_dir().ok().map(|d| d.join("active_pids"))
}

/// Directory holding per-session "currently streaming" markers. A marker file
/// exists only while a session is actively generating a model response. The
/// file content is the owning process PID so stale markers (from crashed
/// processes) can be detected and ignored.
pub fn streaming_pids_dir() -> Option<std::path::PathBuf> {
    jcode_dir().ok().map(|d| d.join("streaming_pids"))
}

/// Record that `session_id` is owned by process `pid`.
pub fn register_active_pid(session_id: &str, pid: u32) {
    let Some(_lock) = PidMarkerLock::acquire_writer() else {
        return;
    };
    let Some(dir) = active_pids_dir() else {
        return;
    };
    let _ = write_pid_marker(&dir.join(session_id), pid);
}

/// Remove the active-PID record for `session_id`, if present.
pub fn unregister_active_pid(session_id: &str) {
    let Some(_lock) = PidMarkerLock::acquire_writer() else {
        return;
    };
    if let Some(dir) = active_pids_dir() {
        let _ = std::fs::remove_file(dir.join(session_id));
    }
    // A closed session is never streaming.
    if let Some(dir) = streaming_pids_dir() {
        let _ = std::fs::remove_file(dir.join(session_id));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PidMarkerIdentity {
    len: u64,
    modified: Option<std::time::SystemTime>,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

impl PidMarkerIdentity {
    // `.ok()` is intentionally avoided because it is one of the repository's
    // frozen swallowed-error ratchet patterns. Preserve the accepted count.
    #[allow(clippy::manual_ok_err)]
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified: match metadata.modified() {
                Ok(modified) => Some(modified),
                Err(_) => None,
            },
            #[cfg(unix)]
            dev: metadata.dev(),
            #[cfg(unix)]
            ino: metadata.ino(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidMarkerObservation {
    contents: Vec<u8>,
    identity: PidMarkerIdentity,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionPidMarkerObservations {
    pub active: Option<PidMarkerObservation>,
    pub streaming: Option<PidMarkerObservation>,
}

impl SessionPidMarkerObservations {
    pub fn active_pid(&self) -> Option<u32> {
        self.active
            .as_ref()
            .and_then(|observation| pid_from_marker_contents(&observation.contents))
    }

    pub fn active_marker_is_live(&self) -> bool {
        self.active_pid()
            .is_some_and(jcode_core::process::is_running)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionPidMarkerRemoval {
    pub active_removed: bool,
    pub streaming_removed: bool,
}

fn observe_pid_marker(path: &Path) -> Option<PidMarkerObservation> {
    let metadata_before = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return None,
    };
    let contents = match std::fs::read(path) {
        Ok(contents) => contents,
        Err(_) => return None,
    };
    maybe_replace_marker_after_observation_content_read(path, &contents);
    let metadata_after = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return None,
    };
    let identity = PidMarkerIdentity::from_metadata(&metadata_before);
    if identity != PidMarkerIdentity::from_metadata(&metadata_after) {
        return None;
    }
    Some(PidMarkerObservation { contents, identity })
}

#[cfg(not(test))]
fn maybe_replace_marker_after_observation_content_read(_path: &Path, _contents: &[u8]) {}

#[cfg(test)]
fn maybe_replace_marker_after_observation_content_read(path: &Path, contents: &[u8]) {
    const PATH_VAR: &str = "JCODE_TEST_REPLACE_MARKER_AFTER_OBSERVE_CONTENT_PATH";
    let Ok(target) = std::env::var(PATH_VAR) else {
        return;
    };
    if target != path.to_string_lossy() {
        return;
    }
    let Some(pid) = pid_from_marker_contents(contents) else {
        return;
    };
    let _ = write_pid_marker(path, pid);
}

/// Capture the current active and streaming marker bytes plus file identity for
/// a terminal transition. The observation is intentionally made before session
/// persistence; later cleanup removes only the same marker file, so a reconnect
/// that replaces the marker in between cannot be unlinked by stale cleanup.
pub fn observe_session_pid_markers(session_id: &str) -> SessionPidMarkerObservations {
    if !is_single_path_component(session_id) {
        return SessionPidMarkerObservations::default();
    }
    let Some(_lock) = PidMarkerLock::acquire_bounded() else {
        return SessionPidMarkerObservations::default();
    };
    SessionPidMarkerObservations {
        active: active_pids_dir()
            .as_deref()
            .and_then(|dir| observe_pid_marker(&dir.join(session_id))),
        streaming: streaming_pids_dir()
            .as_deref()
            .and_then(|dir| observe_pid_marker(&dir.join(session_id))),
    }
}

/// Remove terminal markers only when they are still the exact files observed
/// before durable session persistence. Unlike stale-marker sweeping, this may
/// remove a live PID owned by the current process after it has persisted a
/// terminal state, but it will not remove a marker replaced by another owner.
pub fn remove_session_pid_markers_if_unchanged(
    session_id: &str,
    observed: &SessionPidMarkerObservations,
) -> SessionPidMarkerRemoval {
    if !is_single_path_component(session_id) {
        return SessionPidMarkerRemoval::default();
    }
    let Some(_lock) = PidMarkerLock::acquire_bounded() else {
        return SessionPidMarkerRemoval::default();
    };
    SessionPidMarkerRemoval {
        active_removed: active_pids_dir()
            .as_deref()
            .zip(observed.active.as_ref())
            .is_some_and(|(dir, observation)| {
                remove_marker_if_unchanged(&dir.join(session_id), observation)
            }),
        streaming_removed: streaming_pids_dir()
            .as_deref()
            .zip(observed.streaming.as_ref())
            .is_some_and(|(dir, observation)| {
                remove_marker_if_unchanged(&dir.join(session_id), observation)
            }),
    }
}

/// Mark a session as actively streaming a model response.
pub fn mark_streaming(session_id: &str) {
    let Some(_lock) = PidMarkerLock::acquire_writer() else {
        return;
    };
    let Some(dir) = streaming_pids_dir() else {
        return;
    };
    let _ = write_pid_marker(&dir.join(session_id), std::process::id());
}

/// Clear the streaming marker for a session (turn finished or interrupted).
pub fn unmark_streaming(session_id: &str) {
    let Some(_lock) = PidMarkerLock::acquire_writer() else {
        return;
    };
    if let Some(dir) = streaming_pids_dir() {
        let _ = std::fs::remove_file(dir.join(session_id));
    }
}

fn write_pid_marker(path: &Path, pid: u32) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "PID marker path has no parent",
        ));
    };
    std::fs::create_dir_all(parent)?;

    // The temp file lives in JCODE_HOME, on the same filesystem as both marker
    // directories, so persist performs an atomic rename/replace without exposing
    // a partially-written PID to readers or the sweeper.
    let home = jcode_dir().map_err(std::io::Error::other)?;
    let mut temp = tempfile::Builder::new()
        .prefix(PID_MARKER_TEMP_PREFIX)
        .tempfile_in(home)?;
    write!(temp, "{pid}")?;
    temp.flush()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// RAII guard that marks a session as streaming for its lifetime and clears the
/// marker on drop. This guarantees the marker is cleared on every exit path
/// (normal return, `?` propagation, interrupt, or panic) so the menu bar count
/// never gets stuck showing a phantom streaming session.
pub struct StreamingGuard {
    session_id: String,
}

impl StreamingGuard {
    pub fn new(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        mark_streaming(&session_id);
        Self { session_id }
    }
}

impl Drop for StreamingGuard {
    fn drop(&mut self) {
        unmark_streaming(&self.session_id);
    }
}

/// Counts of stale PID marker files removed by [`sweep_stale_pid_markers`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PidMarkerSweep {
    pub active_removed: usize,
    pub streaming_removed: usize,
    pub temp_removed: usize,
}

impl PidMarkerSweep {
    pub fn total_removed(self) -> usize {
        self.active_removed + self.streaming_removed + self.temp_removed
    }
}

/// Remove an active marker only if it is still stale and its bytes exactly
/// match a caller's earlier observation.
///
/// Crash recovery reads marker and session state before deciding what to do.
/// This conditional operation closes the gap between that read and deletion:
/// it reacquires the common marker lock and refuses to unlink a marker that a
/// live owner replaced in the meantime.
pub fn remove_active_pid_marker_if_stale_and_matches(session_id: &str, observed: &[u8]) -> bool {
    if !is_single_path_component(session_id) {
        return false;
    }
    let Some(_lock) = PidMarkerLock::acquire_bounded() else {
        return false;
    };
    let Some(dir) = active_pids_dir() else {
        return false;
    };
    remove_marker_if_stale_and_matches(&dir.join(session_id), observed)
}

/// Delete malformed markers and markers whose owning process is no longer live.
///
/// This sweep is deliberately independent of session persistence: a missing or
/// corrupt session JSON file must not make a dead PID marker immortal. Atomic
/// write temp files left by a crashed writer are reclaimed as well. Registration,
/// unregistration, streaming guard drops, conditional removal, and this sweep share a
/// cross-process advisory lock. The lock remains held through final unlink, so a
/// coordinated live owner cannot be removed between inspection and deletion. If
/// the lock cannot be acquired, the sweep removes nothing. Removal is best-effort
/// and idempotent; unreadable entries and non-files are left untouched.
pub fn sweep_stale_pid_markers() -> PidMarkerSweep {
    let Some(_lock) = PidMarkerLock::acquire_bounded() else {
        return PidMarkerSweep::default();
    };
    PidMarkerSweep {
        active_removed: active_pids_dir()
            .as_deref()
            .map(sweep_stale_pid_marker_dir)
            .unwrap_or_default(),
        streaming_removed: streaming_pids_dir()
            .as_deref()
            .map(sweep_stale_pid_marker_dir)
            .unwrap_or_default(),
        temp_removed: jcode_dir()
            .ok()
            .as_deref()
            .map(sweep_pid_marker_temp_files)
            .unwrap_or_default(),
    }
}

fn sweep_pid_marker_temp_files(home: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(home) else {
        return 0;
    };

    // Every coordinated writer holds PidMarkerLock from temp creation through
    // persist. Consequently, a temp file visible while this sweep owns the lock
    // cannot belong to a live coordinated writer and is crash residue.
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(PID_MARKER_TEMP_PREFIX))
        })
        .filter(|entry| remove_file_if_present(&entry.path()))
        .count()
}

fn sweep_stale_pid_marker_dir(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| remove_marker_if_stale(&entry.path()))
        .count()
}

fn remove_marker_if_stale(path: &Path) -> bool {
    let Ok(observed) = std::fs::read(path) else {
        return false;
    };
    remove_marker_if_stale_and_matches(path, &observed)
}

fn remove_marker_if_stale_and_matches(path: &Path, observed: &[u8]) -> bool {
    // Re-read under the marker lock. This protects callers that made their
    // observation before acquiring the lock and also rejects uncoordinated
    // replacements that happen while a sweep is inspecting an entry.
    let Ok(current) = std::fs::read(path) else {
        return false;
    };
    if current != observed || marker_contents_are_live(&current) {
        return false;
    }

    remove_file_if_present(path)
}

fn remove_marker_if_unchanged(path: &Path, observed: &PidMarkerObservation) -> bool {
    // Re-read contents and metadata under the marker lock. Matching contents
    // alone are not enough because a successor may register the same PID bytes
    // via atomic replacement before stale terminal cleanup runs.
    let Ok(current) = std::fs::read(path) else {
        return false;
    };
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if current != observed.contents
        || PidMarkerIdentity::from_metadata(&metadata) != observed.identity
    {
        return false;
    }

    remove_file_if_present(path)
}

fn remove_file_if_present(path: &Path) -> bool {
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => false,
    }
}

fn is_single_path_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}

fn marker_contents_are_live(contents: &[u8]) -> bool {
    pid_from_marker_contents(contents).is_some_and(jcode_core::process::is_running)
}

fn pid_from_marker_contents(contents: &[u8]) -> Option<u32> {
    std::str::from_utf8(contents)
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
}

/// Find the active session ID currently owned by the given process ID.
pub fn find_active_session_id_by_pid(pid: u32) -> Option<String> {
    let dir = active_pids_dir()?;
    for entry in std::fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let session_id = entry.file_name().to_string_lossy().to_string();
        let stored = std::fs::read_to_string(entry.path()).ok()?;
        if stored.trim().parse::<u32>().ok()? == pid {
            return Some(session_id);
        }
    }
    None
}

/// List active session IDs currently tracked in `~/.jcode/active_pids`.
pub fn active_session_ids() -> Vec<String> {
    let Some(dir) = active_pids_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect()
}

/// Live snapshot of how many jcode sessions are running, and how many of those
/// are actively streaming a model response right now. Used by the menu bar
/// indicator (`jcode menubar`) and any other presence UI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionCounts {
    /// Number of live sessions (registered PID is still running).
    pub total: usize,
    /// Number of live sessions currently streaming a model response.
    pub streaming: usize,
}

/// Live presence info for one running session, derived from the active-pid
/// registry and the streaming markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPresence {
    /// Session ID, e.g. `session_fox_1234567890_deadbeef`.
    pub session_id: String,
    /// PID of the process that owns the session.
    pub pid: u32,
    /// Whether the session is actively streaming a model response right now.
    pub streaming: bool,
    /// When the current streaming turn started (streaming marker mtime), if
    /// the session is streaming. Lets presence UIs show "working for 2m".
    pub streaming_since: Option<std::time::SystemTime>,
}

/// Snapshot per-session presence by scanning the active-pid registry and
/// streaming markers, skipping any entries whose owning process is no longer
/// alive. This is a cheap O(n) scan over a handful of tiny files; used by the
/// menu bar indicator and other presence UI.
pub fn session_presence() -> Vec<SessionPresence> {
    let Some(active_dir) = active_pids_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&active_dir) else {
        return Vec::new();
    };

    let streaming_dir = streaming_pids_dir();
    let mut sessions = Vec::new();

    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        let session_id = entry.file_name().to_string_lossy().to_string();
        let Some(pid) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
        else {
            continue;
        };
        if !jcode_core::process::is_running(pid) {
            continue;
        }

        let marker_path = streaming_dir.as_ref().map(|dir| dir.join(&session_id));
        let streaming = marker_path.as_ref().is_some_and(|marker| {
            std::fs::read_to_string(marker)
                .ok()
                .and_then(|raw| raw.trim().parse::<u32>().ok())
                .is_some_and(jcode_core::process::is_running)
        });
        let streaming_since = if streaming {
            marker_path
                .as_ref()
                .and_then(|marker| std::fs::metadata(marker).ok())
                .and_then(|meta| meta.modified().ok())
        } else {
            None
        };

        sessions.push(SessionPresence {
            session_id,
            pid,
            streaming,
            streaming_since,
        });
    }

    sessions
}

/// Compute the current session counts from [`session_presence`].
pub fn session_counts() -> SessionCounts {
    let sessions = session_presence();
    SessionCounts {
        total: sessions.len(),
        streaming: sessions.iter().filter(|s| s.streaming).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize tests that mutate `JCODE_HOME`.
    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(unix)]
    fn exited_child_pid() -> u32 {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn short-lived child");
        let pid = child.id();
        child.wait().expect("wait for short-lived child");
        assert!(
            !jcode_core::process::is_running(pid),
            "test child must have exited"
        );
        pid
    }

    #[cfg(unix)]
    #[test]
    fn stale_marker_sweep_removes_dead_and_invalid_but_preserves_live() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        let live_pid = std::process::id();
        let dead_pid = exited_child_pid();
        let active_dir = active_pids_dir().expect("active marker dir");
        let streaming_dir = streaming_pids_dir().expect("streaming marker dir");
        std::fs::create_dir_all(&active_dir).expect("create active marker dir");
        std::fs::create_dir_all(&streaming_dir).expect("create streaming marker dir");

        std::fs::write(active_dir.join("live"), live_pid.to_string()).expect("write live marker");
        std::fs::write(active_dir.join("dead"), dead_pid.to_string()).expect("write dead marker");
        std::fs::write(active_dir.join("invalid"), "not-a-pid").expect("write invalid marker");
        std::fs::write(active_dir.join("out-of-range"), u32::MAX.to_string())
            .expect("write out-of-range Unix PID marker");
        std::fs::write(streaming_dir.join("live"), live_pid.to_string())
            .expect("write live streaming marker");
        std::fs::write(streaming_dir.join("dead"), dead_pid.to_string())
            .expect("write dead streaming marker");
        std::fs::write(streaming_dir.join("invalid"), [0xff])
            .expect("write invalid streaming marker");

        let first = sweep_stale_pid_markers();
        assert_eq!(
            first,
            PidMarkerSweep {
                active_removed: 3,
                streaming_removed: 2,
                temp_removed: 0,
            }
        );
        assert_eq!(first.total_removed(), 5);
        assert!(active_dir.join("live").exists());
        assert!(streaming_dir.join("live").exists());
        assert!(!active_dir.join("dead").exists());
        assert!(!active_dir.join("invalid").exists());
        assert!(!active_dir.join("out-of-range").exists());
        assert!(!streaming_dir.join("dead").exists());
        assert!(!streaming_dir.join("invalid").exists());

        assert_eq!(
            sweep_stale_pid_markers(),
            PidMarkerSweep::default(),
            "repeating the sweep must be idempotent"
        );

        jcode_core::env::remove_var("JCODE_HOME");
    }

    #[cfg(unix)]
    #[test]
    fn conditional_cleanup_preserves_a_replaced_live_marker() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        let session_id = "session_replaced_during_crash_scan";
        let dead_pid = exited_child_pid();
        register_active_pid(session_id, dead_pid);
        let marker = active_pids_dir()
            .expect("active marker dir")
            .join(session_id);
        let observed = std::fs::read(&marker).expect("read dead marker observation");

        register_active_pid(session_id, std::process::id());
        assert!(
            !remove_active_pid_marker_if_stale_and_matches(session_id, &observed),
            "cleanup must reject an observation replaced before lock acquisition"
        );
        assert_eq!(
            std::fs::read_to_string(&marker).expect("replacement marker remains"),
            std::process::id().to_string()
        );

        jcode_core::env::remove_var("JCODE_HOME");
    }

    #[cfg(unix)]
    #[test]
    fn observation_rejects_marker_replaced_between_content_and_metadata_read() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        let session_id = "session_replaced_during_observation";
        register_active_pid(session_id, std::process::id());
        let marker = active_pids_dir()
            .expect("active marker dir")
            .join(session_id);
        jcode_core::env::set_var(
            "JCODE_TEST_REPLACE_MARKER_AFTER_OBSERVE_CONTENT_PATH",
            &marker,
        );

        let observed = observe_session_pid_markers(session_id);
        assert!(
            observed.active.is_none(),
            "unstable content/metadata observations must be rejected"
        );
        let removal = remove_session_pid_markers_if_unchanged(session_id, &observed);
        assert!(!removal.active_removed);
        assert!(marker.exists(), "replaced marker must survive cleanup");

        jcode_core::env::remove_var("JCODE_TEST_REPLACE_MARKER_AFTER_OBSERVE_CONTENT_PATH");
        jcode_core::env::remove_var("JCODE_HOME");
    }

    #[test]
    fn sweep_reclaims_atomic_write_temp_residue_idempotently() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        let residue = temp.path().join(".pid-marker-crash-residue");
        let similarly_named_directory = temp.path().join(".pid-marker-directory");
        std::fs::write(&residue, "partial").expect("write simulated temp residue");
        std::fs::create_dir(&similarly_named_directory).expect("create similarly named directory");

        let first = sweep_stale_pid_markers();
        assert_eq!(first.temp_removed, 1);
        assert_eq!(first.total_removed(), 1);
        assert!(!residue.exists());
        assert!(
            similarly_named_directory.exists(),
            "only regular temp files are eligible for reclamation"
        );
        assert_eq!(
            sweep_stale_pid_markers(),
            PidMarkerSweep::default(),
            "repeating temp reclamation must be idempotent"
        );

        jcode_core::env::remove_var("JCODE_HOME");
    }

    #[test]
    fn lock_failure_leaves_marker_state_untouched() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        let active_dir = active_pids_dir().expect("active marker dir");
        std::fs::create_dir_all(&active_dir).expect("create active marker dir");
        let existing = active_dir.join("must-stay");
        std::fs::write(&existing, "invalid").expect("write existing marker");
        std::fs::create_dir(temp.path().join(PID_MARKER_LOCK_FILE))
            .expect("make lock path unopenable as a file");

        register_active_pid("not-written", std::process::id());
        unregister_active_pid("must-stay");
        assert_eq!(sweep_stale_pid_markers(), PidMarkerSweep::default());
        assert!(existing.exists(), "lock failure must prevent deletion");
        assert!(
            !active_dir.join("not-written").exists(),
            "lock failure must prevent an uncoordinated write"
        );

        jcode_core::env::remove_var("JCODE_HOME");
    }

    #[test]
    fn held_marker_lock_is_bounded_and_fail_closed_without_sleeping() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());
        let _held = PidMarkerLock::acquire_writer().expect("hold marker lock");
        let active_dir = active_pids_dir().expect("active marker dir");
        std::fs::create_dir_all(&active_dir).expect("create active marker dir");
        let marker = active_dir.join("must-stay-held-lock");
        std::fs::write(&marker, "not-a-pid").expect("write stale marker");

        assert_eq!(
            sweep_stale_pid_markers(),
            PidMarkerSweep::default(),
            "a contended marker lock must return a bounded fail-closed outcome"
        );
        assert!(marker.exists(), "contended cleanup must not delete markers");

        jcode_core::env::remove_var("JCODE_HOME");
    }

    #[cfg(unix)]
    #[test]
    fn conditional_session_marker_cleanup_reports_exact_partial_removals() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        let session_id = "session_partial_marker_cleanup";
        let pid = std::process::id();
        let active_dir = active_pids_dir().expect("active marker dir");
        let streaming_dir = streaming_pids_dir().expect("streaming marker dir");

        register_active_pid(session_id, pid);
        mark_streaming(session_id);
        let observed = observe_session_pid_markers(session_id);
        write_pid_marker(&active_dir.join(session_id), pid).expect("replace active marker");
        let removal = remove_session_pid_markers_if_unchanged(session_id, &observed);
        assert_eq!(
            removal,
            SessionPidMarkerRemoval {
                active_removed: false,
                streaming_removed: true,
            }
        );
        assert!(active_dir.join(session_id).exists());
        assert!(!streaming_dir.join(session_id).exists());

        register_active_pid(session_id, pid);
        mark_streaming(session_id);
        let observed = observe_session_pid_markers(session_id);
        write_pid_marker(&streaming_dir.join(session_id), pid).expect("replace streaming marker");
        let removal = remove_session_pid_markers_if_unchanged(session_id, &observed);
        assert_eq!(
            removal,
            SessionPidMarkerRemoval {
                active_removed: true,
                streaming_removed: false,
            }
        );
        assert!(!active_dir.join(session_id).exists());
        assert!(streaming_dir.join(session_id).exists());

        register_active_pid(session_id, pid);
        mark_streaming(session_id);
        let observed = observe_session_pid_markers(session_id);
        write_pid_marker(&active_dir.join(session_id), pid).expect("replace active marker again");
        write_pid_marker(&streaming_dir.join(session_id), pid)
            .expect("replace streaming marker again");
        let removal = remove_session_pid_markers_if_unchanged(session_id, &observed);
        assert_eq!(
            removal,
            SessionPidMarkerRemoval {
                active_removed: false,
                streaming_removed: false,
            }
        );
        assert!(active_dir.join(session_id).exists());
        assert!(streaming_dir.join(session_id).exists());

        jcode_core::env::remove_var("JCODE_HOME");
    }

    #[cfg(unix)]
    #[test]
    fn explicit_sweep_removes_dead_marker_without_session_data() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        let dead_pid = exited_child_pid();
        register_active_pid("session_missing_from_store", dead_pid);
        let marker = active_pids_dir()
            .expect("active marker dir")
            .join("session_missing_from_store");
        assert!(marker.exists());
        assert!(
            !temp.path().join("sessions").exists(),
            "fixture intentionally has no persisted session data"
        );

        assert_eq!(sweep_stale_pid_markers().active_removed, 1);
        assert!(
            !marker.exists(),
            "storage sweep must delete the dead marker even when session data is missing"
        );

        jcode_core::env::remove_var("JCODE_HOME");
    }

    #[test]
    fn session_counts_counts_live_and_streaming_only() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        let live = std::process::id();
        // Pick a PID that is almost certainly dead.
        let dead = 999_999u32;

        // live + streaming
        register_active_pid("session_alpha", live);
        mark_streaming("session_alpha");
        // live + not streaming
        register_active_pid("session_beta", live);
        // dead session (should be ignored entirely)
        register_active_pid("session_gamma", dead);
        // live session whose streaming marker points at a dead pid (ignored for streaming)
        register_active_pid("session_delta", live);
        if let Some(dir) = streaming_pids_dir() {
            let _ = std::fs::write(dir.join("session_delta"), dead.to_string());
        }

        let counts = session_counts();
        assert_eq!(counts.total, 3, "three live sessions expected");
        assert_eq!(
            counts.streaming, 1,
            "only one live streaming session expected"
        );

        // Per-session presence reports the same view, keyed by session.
        let sessions = session_presence();
        assert_eq!(sessions.len(), 3);
        let by_id = |id: &str| {
            sessions
                .iter()
                .find(|s| s.session_id == id)
                .unwrap_or_else(|| panic!("{id} should be present"))
        };
        assert!(by_id("session_alpha").streaming);
        assert!(!by_id("session_beta").streaming);
        assert!(!by_id("session_delta").streaming);
        assert_eq!(by_id("session_alpha").pid, live);
        assert!(!sessions.iter().any(|s| s.session_id == "session_gamma"));

        // Clearing the streaming marker drops the streaming count.
        unmark_streaming("session_alpha");
        assert_eq!(session_counts().streaming, 0);

        // Unregistering also clears any leftover streaming marker.
        register_active_pid("session_epsilon", live);
        mark_streaming("session_epsilon");
        assert_eq!(session_counts().streaming, 1);
        unregister_active_pid("session_epsilon");
        assert_eq!(session_counts().streaming, 0);

        jcode_core::env::remove_var("JCODE_HOME");
    }

    #[test]
    fn streaming_guard_marks_and_clears_on_drop() {
        let _guard = lock_env();
        let temp = tempfile::tempdir().expect("tempdir");
        jcode_core::env::set_var("JCODE_HOME", temp.path());

        register_active_pid("session_guard", std::process::id());
        assert_eq!(session_counts().streaming, 0);
        {
            let _streaming = StreamingGuard::new("session_guard");
            assert_eq!(session_counts().streaming, 1);
        }
        assert_eq!(session_counts().streaming, 0);

        jcode_core::env::remove_var("JCODE_HOME");
    }
}
