//! Binary freshness: is a reload candidate provably newer than the running
//! process?
//!
//! Extracted from `server/util.rs` (which is over the code-size budget) when
//! the same-path in-place replacement signal was added; see the function docs
//! for the full history (#277 reload loops, #291 downgrade regression).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

static IMAGE_BASELINE_MTIME: OnceLock<Option<SystemTime>> = OnceLock::new();

/// The mtime of the binary image this process is *currently executing*, sampled
/// once and then frozen for the lifetime of the image.
///
/// Same-path in-place binary replacement (the canonical publish flow writes the
/// new build over `~/.jcode/current/jcode`) makes the running exe and the
/// reload candidate the *same canonical file*. Comparing "candidate mtime vs
/// current-exe mtime" is then comparing the file against itself, which can
/// never be strictly newer, so a genuinely newer published build was invisible
/// to `server_has_newer_binary` and non-forced reloads.
///
/// This baseline is the correct discriminator: if the file on disk is newer
/// than it was when we loaded it, we are provably running stale code.
///
/// It deliberately replaces an earlier *process start time* baseline, which was
/// unsound. Reload re-execs via `Command::exec`, and `exec` preserves the
/// process start time (verified on macOS via `proc_pidinfo`; Linux `/proc/self`
/// btime is per-task, not per-image). A daemon whose binary was republished
/// after it started therefore kept satisfying `mtime > start` *forever*, even
/// after successfully reloading, so it advertised an update it could never
/// clear. Clients then re-entered the runtime-identity defer path on every
/// history bootstrap and never loaded a session.
///
/// A `OnceLock` is the right lifetime because `exec` replaces the process image
/// and reinitializes statics: each reloaded image re-samples, so the baseline
/// genuinely advances across reloads and the signal terminates.
///
/// Callers that care about a precise baseline (the server) should seed this at
/// boot via [`seed_image_baseline_mtime`] before any republish can race the
/// first lazy read.
pub(crate) fn image_baseline_mtime() -> Option<SystemTime> {
    *IMAGE_BASELINE_MTIME.get_or_init(read_running_image_mtime)
}

/// Sample and freeze the running image's mtime as early as possible.
///
/// Idempotent, and a no-op once the baseline has been established. Called from
/// server startup so the frozen value describes the image we booted rather than
/// whatever happens to be on disk at the first freshness query.
pub(crate) fn seed_image_baseline_mtime() {
    IMAGE_BASELINE_MTIME.get_or_init(read_running_image_mtime);
}

fn read_running_image_mtime() -> Option<SystemTime> {
    // Every failure below degrades to `None`, which the caller treats as "no
    // provable update". That is the loop-safe direction: an unreadable baseline
    // must never manufacture a reload signal.
    let Ok(exe) = std::env::current_exe() else {
        return None;
    };
    let exe = super::strip_deleted_suffix(exe);
    // Release installs run a wrapper script that execs a `.bin` payload; the
    // payload is the file that actually changes on publish, and it is what the
    // candidate side resolves to, so both sides must agree.
    let payload = crate::build::resolve_binary_payload(&exe);
    let Ok(meta) = std::fs::metadata(payload) else {
        return None;
    };
    let Ok(modified) = meta.modified() else {
        return None;
    };
    Some(modified)
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
/// `image_baseline` handles the same-canonical-path case: the canonical publish
/// flow overwrites `~/.jcode/current/jcode` in place, so the running exe and
/// the reload candidate are the same file and an mtime-vs-mtime comparison is
/// the file against itself (never strictly newer). If that shared file is newer
/// than it was when this image loaded, we are provably running stale code and
/// the candidate is a real update. When the baseline is unavailable the
/// same-path case stays "no update", preserving the historical loop-safe
/// behavior.
///
/// The baseline must describe the *image*, not the process: `exec` preserves
/// the process start time, so a start-time baseline never advanced across a
/// reload and wedged the daemon into permanently advertising an update it could
/// not clear. See [`image_baseline_mtime`].
pub(crate) fn newer_binary_available(
    current_mtime: Option<std::time::SystemTime>,
    current_canonical: Option<&Path>,
    image_baseline: Option<std::time::SystemTime>,
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
            // the file changed after this image loaded. Without a readable
            // baseline, keep the historical "never reload into ourselves".
            return match (candidate_mtime, image_baseline) {
                (Some(candidate_time), Some(baseline)) => candidate_time > baseline,
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
        // Same canonical path without a readable image baseline must never
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
        // Normal steady state: our image baseline is at (or after) the last
        // write of our own binary. No update, no loop.
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
    fn image_baseline_mtime_is_available_and_in_the_past() {
        // The same-path update signal silently degrades to "never" without a
        // readable baseline, so prove the probe actually resolves for a real
        // running binary.
        let baseline = super::image_baseline_mtime().expect("image baseline should resolve");
        let now = std::time::SystemTime::now();
        assert!(baseline <= now, "baseline must not be in the future");
    }

    #[test]
    fn image_baseline_is_stable_within_one_image() {
        // Freezing is what makes the signal meaningful: a baseline that
        // re-sampled on every read would always equal the candidate mtime and
        // could never report an in-place republish.
        super::seed_image_baseline_mtime();
        let first = super::image_baseline_mtime();
        let second = super::image_baseline_mtime();
        assert_eq!(first, second);
    }

    #[test]
    fn same_path_republish_clears_once_the_baseline_advances() {
        // Regression for the latched-update stall: a daemon whose binary was
        // republished after it loaded must report an update (so a reload is
        // offered), and the *reloaded* image -- which re-samples the baseline
        // to the republished mtime -- must then report no update.
        //
        // The old baseline was the process start time, which `exec` preserves
        // (verified on macOS via proc_pidinfo), so the second assertion failed
        // in production: the server advertised an update forever, the client
        // deferred history on every bootstrap, and sessions never loaded.
        let republished = t(500);
        let candidates = vec![(PathBuf::from("/x/current/jcode"), Some(republished))];

        // Stale image: loaded at t=400, binary rewritten at t=500.
        assert!(newer_binary_available(
            Some(republished),
            Some(std::path::Path::new("/x/current/jcode")),
            Some(t(400)),
            candidates.clone(),
        ));

        // Reloaded image: re-sampled the baseline from the same file it now runs.
        assert!(!newer_binary_available(
            Some(republished),
            Some(std::path::Path::new("/x/current/jcode")),
            Some(republished),
            candidates,
        ));
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
