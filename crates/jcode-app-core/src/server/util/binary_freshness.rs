//! Binary freshness: is a reload candidate provably newer than the running
//! process?
//!
//! Extracted from `server/util.rs` (which is over the code-size budget) when
//! the same-path in-place replacement signal was added; see the function docs
//! for the full history (#277 reload loops, #291 downgrade regression).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Best-effort start time of this process, cached on first read.
///
/// Same-path in-place binary replacement (the canonical publish flow writes the
/// new build over `~/.jcode/current/jcode`) makes the running exe and the
/// reload candidate the *same canonical file*. Comparing "candidate mtime vs
/// current-exe mtime" is then comparing the file against itself, which can
/// never be strictly newer, so a genuinely newer published build was invisible
/// to `server_has_newer_binary` and non-forced reloads. The process start time
/// is the correct baseline for that case: if the file on disk is newer than
/// the moment we started executing it, we are running stale code.
pub(crate) fn process_start_time() -> Option<SystemTime> {
    use std::sync::OnceLock;
    static START: OnceLock<Option<SystemTime>> = OnceLock::new();
    *START.get_or_init(process_start_time_uncached)
}

#[cfg(target_os = "macos")]
fn process_start_time_uncached() -> Option<SystemTime> {
    // proc_pidinfo(PROC_PIDTBSDINFO) -> proc_bsdinfo.pbi_start_tv{sec,usec}.
    // Same libproc surface (and FFI style) as jcode-core's stdin_detect probe;
    // needs no extra privileges for our own pid.
    const PROC_PIDTBSDINFO: libc::c_int = 3;

    #[repr(C)]
    struct ProcBsdInfo {
        pbi_flags: u32,
        pbi_status: u32,
        pbi_xstatus: u32,
        pbi_pid: u32,
        pbi_ppid: u32,
        pbi_uid: u32,
        pbi_gid: u32,
        pbi_ruid: u32,
        pbi_rgid: u32,
        pbi_svuid: u32,
        pbi_svgid: u32,
        rfu_1: u32,
        pbi_comm: [u8; 16],
        pbi_name: [u8; 32],
        pbi_nfiles: u32,
        pbi_pgid: u32,
        pbi_pjobc: u32,
        e_tdev: u32,
        e_tpgid: u32,
        pbi_nice: i32,
        pbi_start_tvsec: u64,
        pbi_start_tvusec: u64,
    }

    unsafe extern "C" {
        fn proc_pidinfo(
            pid: libc::c_int,
            flavor: libc::c_int,
            arg: u64,
            buffer: *mut libc::c_void,
            buffersize: libc::c_int,
        ) -> libc::c_int;
    }

    let mut info: ProcBsdInfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<ProcBsdInfo>() as libc::c_int;
    let rc = unsafe {
        proc_pidinfo(
            std::process::id() as libc::c_int,
            PROC_PIDTBSDINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        )
    };
    if rc != size || info.pbi_start_tvsec == 0 {
        return None;
    }
    Some(
        SystemTime::UNIX_EPOCH
            + std::time::Duration::new(
                info.pbi_start_tvsec,
                (info.pbi_start_tvusec as u32).saturating_mul(1000),
            ),
    )
}

#[cfg(target_os = "linux")]
fn process_start_time_uncached() -> Option<SystemTime> {
    // /proc/self is owned by this process; its creation time is the process
    // start. Falls back to None (and thus the historical behavior) on
    // filesystems that do not report btime.
    let meta = std::fs::metadata("/proc/self").ok()?;
    meta.created().ok().or_else(|| meta.modified().ok())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn process_start_time_uncached() -> Option<SystemTime> {
    None
}

/// Decide whether any reload candidate is *provably* newer than the running
/// server binary.
///
/// This is intentionally conservative. An earlier version reported "update
/// available" whenever the mtime comparison was inconclusive (e.g. a metadata
/// read failed) as long as the candidate path differed from the running exe.
/// On some systems that fallback fired permanently, so the client would
/// auto-reload the server, the server would exec into the candidate, and the
/// freshly-exec'd server would again report an update -> an infinite reload
/// loop that flickers the terminal (see issue #277).
///
/// We now only report an update when we can read both mtimes and the candidate
/// is strictly newer than the running binary. Any uncertainty suppresses the
/// auto-reload signal so it can never wedge the client into a loop.
///
/// `process_start` handles the same-canonical-path case: the canonical publish
/// flow overwrites `~/.jcode/current/jcode` in place, so the running exe and
/// the reload candidate are the same file and an mtime-vs-mtime comparison is
/// the file against itself (never strictly newer). If that shared file was
/// modified *after this process started executing*, we are provably running
/// stale code and the candidate is a real update. When `process_start` is
/// unavailable the same-path case stays "no update", preserving the historical
/// loop-safe behavior: the file cannot keep being newer than a start time that
/// advances with every exec, so this cannot reintroduce the #277 reload loop.
pub(crate) fn newer_binary_available(
    current_mtime: Option<std::time::SystemTime>,
    current_canonical: Option<&Path>,
    process_start: Option<std::time::SystemTime>,
    candidates: impl IntoIterator<Item = (PathBuf, Option<std::time::SystemTime>)>,
) -> bool {
    let Some(current_time) = current_mtime else {
        crate::logging::warn(
            "server_has_newer_binary: current executable mtime unavailable; suppressing auto-reload update signal",
        );
        return false;
    };

    candidates.into_iter().any(|(candidate, candidate_mtime)| {
        if current_canonical == Some(candidate.as_path()) {
            // Same canonical file: an in-place replacement is an update only if
            // it happened after this process started. Without a readable start
            // time, keep the historical "never reload into ourselves".
            return match (candidate_mtime, process_start) {
                (Some(candidate_time), Some(started)) => candidate_time > started,
                _ => false,
            };
        }

        match candidate_mtime {
            Some(candidate_time) => candidate_time > current_time,
            None => {
                crate::logging::warn(&format!(
                    "server_has_newer_binary: candidate mtime unavailable for {}; suppressing auto-reload update signal",
                    candidate.display()
                ));
                false
            }
        }
    })
}

#[cfg(test)]
mod newer_binary_tests {
    use super::newer_binary_available;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn t(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn reports_update_when_candidate_is_strictly_newer() {
        let candidates = vec![(PathBuf::from("/x/stable/jcode"), Some(t(200)))];
        assert!(newer_binary_available(
            Some(t(100)),
            Some(std::path::Path::new("/x/current/jcode")),
            None,
            candidates,
        ));
    }

    #[test]
    fn ignores_candidate_that_is_not_newer() {
        let candidates = vec![(PathBuf::from("/x/stable/jcode"), Some(t(100)))];
        assert!(!newer_binary_available(
            Some(t(100)),
            Some(std::path::Path::new("/x/current/jcode")),
            None,
            candidates,
        ));
    }

    #[test]
    fn never_reloads_into_self_even_if_paths_were_equal() {
        // Same canonical path without a readable process start time must never
        // count as an update, regardless of mtime (historical loop-safe rule).
        let candidates = vec![(PathBuf::from("/x/current/jcode"), Some(t(999)))];
        assert!(!newer_binary_available(
            Some(t(100)),
            Some(std::path::Path::new("/x/current/jcode")),
            None,
            candidates,
        ));
    }

    #[test]
    fn same_path_inplace_replacement_after_start_is_an_update() {
        // The canonical publish flow overwrites ~/.jcode/current/jcode in
        // place: same canonical path, but the file was rewritten *after* this
        // process started, so we are provably running stale code.
        let candidates = vec![(PathBuf::from("/x/current/jcode"), Some(t(500)))];
        assert!(newer_binary_available(
            Some(t(500)),
            Some(std::path::Path::new("/x/current/jcode")),
            Some(t(400)),
            candidates,
        ));
    }

    #[test]
    fn same_path_binary_older_than_start_is_not_an_update() {
        // Normal steady state: we started after (or at) the last write of our
        // own binary. No update, no loop.
        let candidates = vec![(PathBuf::from("/x/current/jcode"), Some(t(400)))];
        assert!(!newer_binary_available(
            Some(t(400)),
            Some(std::path::Path::new("/x/current/jcode")),
            Some(t(400)),
            candidates.clone(),
        ));
        assert!(!newer_binary_available(
            Some(t(400)),
            Some(std::path::Path::new("/x/current/jcode")),
            Some(t(500)),
            candidates,
        ));
    }

    #[test]
    fn process_start_time_is_available_and_in_the_past() {
        // The same-path update signal silently degrades to "never" without a
        // readable start time, so prove the platform probe actually works on
        // the OSes we ship (macOS libproc, Linux /proc).
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let started = super::process_start_time().expect("process start time should resolve");
            let now = std::time::SystemTime::now();
            assert!(started <= now, "start time must not be in the future");
            let age = now.duration_since(started).unwrap_or_default();
            assert!(
                age < std::time::Duration::from_secs(60 * 60 * 24 * 30),
                "start time implausibly old: {age:?}"
            );
        }
    }

    #[test]
    fn same_path_missing_candidate_mtime_is_not_an_update() {
        // Uncertainty stays "no update" even with a process start time.
        let candidates = vec![(PathBuf::from("/x/current/jcode"), None)];
        assert!(!newer_binary_available(
            Some(t(400)),
            Some(std::path::Path::new("/x/current/jcode")),
            Some(t(100)),
            candidates,
        ));
    }

    #[test]
    fn suppresses_update_when_current_mtime_unavailable() {
        // Regression for issue #277: an unreadable current mtime previously fell
        // through to a path-difference heuristic that could loop forever.
        let candidates = vec![(PathBuf::from("/x/stable/jcode"), Some(t(200)))];
        assert!(!newer_binary_available(
            None,
            Some(std::path::Path::new("/x/current/jcode")),
            None,
            candidates,
        ));
    }

    #[test]
    fn suppresses_update_when_candidate_mtime_unavailable() {
        // The dangerous case from issue #277: candidate path differs but its
        // mtime cannot be read. Must NOT report an update.
        let candidates = vec![(PathBuf::from("/x/stable/jcode"), None)];
        assert!(!newer_binary_available(
            Some(t(100)),
            Some(std::path::Path::new("/x/current/jcode")),
            None,
            candidates,
        ));
    }

    #[test]
    fn reports_update_if_any_candidate_is_newer() {
        let candidates = vec![
            (PathBuf::from("/x/stable/jcode"), None),
            (PathBuf::from("/x/shared/jcode"), Some(t(300))),
        ];
        assert!(newer_binary_available(
            Some(t(100)),
            Some(std::path::Path::new("/x/current/jcode")),
            None,
            candidates,
        ));
    }

    #[test]
    fn newer_server_is_not_outdated_by_older_channel_binary() {
        // Issue #291: a newer self-dev / shared-server daemon must NOT report an
        // update just because an *older* channel binary exists. Here the running
        // server (t=300) is newer than the only candidate (stable at t=100), so
        // there is no update. Previously a channel-version *mismatch* short-circuit
        // reported `true` here and told the newer server to downgrade itself.
        let candidates = vec![(PathBuf::from("/x/stable/jcode"), Some(t(100)))];
        assert!(!newer_binary_available(
            Some(t(300)),
            Some(std::path::Path::new("/x/builds/versions/dev/jcode")),
            None,
            candidates,
        ));
    }

    #[test]
    fn equal_mtime_channel_binary_is_not_an_update() {
        // A candidate with the same mtime is not strictly newer, so it must not
        // trigger a reload (avoids the differ-but-not-newer reload loop, #277).
        let candidates = vec![(PathBuf::from("/x/stable/jcode"), Some(t(100)))];
        assert!(!newer_binary_available(
            Some(t(100)),
            Some(std::path::Path::new("/x/builds/versions/dev/jcode")),
            None,
            candidates,
        ));
    }
}
