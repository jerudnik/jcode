//! Source-only update surface.
//!
//! F20a made jcode nix-native: a packaged binary lives in the read-only Nix
//! store and is updated by the package manager, never by jcode itself. F20b
//! collapsed self-dev publication to one atomic fixed path. F20c therefore
//! retired the GitHub-release acquisition subsystem (download/resume/checksum
//! verify/install-into-a-version-store) entirely: there is no longer any code
//! path in which jcode fetches a release tarball and installs it over itself.
//!
//! What remains is the honest set of update mechanisms for this fork:
//!
//! * **nix-managed installs** -- `jcode update` prints the package-manager path
//!   (`home-manager` rebuild / `nix profile upgrade`) and changes nothing.
//! * **source checkouts** -- `git pull --ff-only` plus a local `cargo build`,
//!   driven by [`crate::session_rebuild`] and `src/cli/hot_exec.rs`.
//!
//! Everything here is deterministic and offline except [`run_git_pull_ff_only`],
//! which shells out to the user's own `git` against their own remote.

use anyhow::{Context, Result};
use std::path::Path;

/// Summary emitted when `git pull` cannot reconcile the local and upstream
/// histories on its own (diverged branches, non-fast-forward, unrelated
/// histories). Callers use this to recognize a divergence and offer a merge
/// affordance instead of a generic failure.
pub const GIT_PULL_DIVERGED_SUMMARY: &str =
    "Local and upstream have diverged, so the update could not fast-forward.";

pub fn print_centered(msg: &str) {
    let width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    for line in msg.lines() {
        let visible_len = unicode_display_width(line);
        if visible_len >= width {
            println!("{}", line);
        } else {
            let pad = (width - visible_len) / 2;
            println!("{:>pad$}{}", "", line, pad = pad);
        }
    }
}

fn unicode_display_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthChar;
    let mut w = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if in_escape {
            if c == 'm' {
                in_escape = false;
            }
            continue;
        }
        if c == '\x1b' {
            in_escape = true;
            continue;
        }
        w += UnicodeWidthChar::width(c).unwrap_or(0);
    }
    w
}

pub fn is_release_build() -> bool {
    jcode_build_meta::is_release_build()
}

/// Fast-forward the source checkout at `repo_dir`.
///
/// This is the only remaining network-touching update primitive: it runs the
/// user's own `git` against their own remote. A non-fast-forward outcome is
/// summarized (not swallowed) so the TUI can offer a merge affordance.
pub fn run_git_pull_ff_only(repo_dir: &Path, quiet: bool) -> Result<()> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("pull").arg("--ff-only");
    if quiet {
        cmd.arg("-q");
    }
    let output = cmd
        .current_dir(repo_dir)
        .output()
        .context("Failed to run git pull")?;

    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("{}", summarize_git_pull_failure(&output.stderr));
    }
}

/// Reduce `git pull` stderr to one actionable line, mapping every divergence
/// dialect onto the single [`GIT_PULL_DIVERGED_SUMMARY`] sentinel so callers can
/// branch on it with [`summary_is_divergence`] instead of re-parsing git output.
pub fn summarize_git_pull_failure(stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let text = stderr.trim();
    if text.is_empty() {
        return "git pull failed".to_string();
    }

    if git_pull_failure_is_divergence(text) {
        return GIT_PULL_DIVERGED_SUMMARY.to_string();
    }

    if text.contains("There is no tracking information for the current branch") {
        return "git pull failed: current branch has no upstream tracking branch".to_string();
    }

    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("hint:"))
        .unwrap_or("git pull failed");
    let line = line.strip_prefix("fatal: ").unwrap_or(line);
    if line.eq_ignore_ascii_case("git pull failed") {
        "git pull failed".to_string()
    } else {
        format!("git pull failed: {}", line)
    }
}

/// Whether `git pull` stderr indicates the local and upstream branches have
/// diverged (and therefore need a manual merge/rebase, not a fast-forward).
pub fn git_pull_failure_is_divergence(stderr: &str) -> bool {
    stderr.contains("Need to specify how to reconcile divergent branches")
        || stderr.contains("Not possible to fast-forward")
        || stderr.contains("refusing to merge unrelated histories")
        || stderr.contains("have diverged")
}

/// Whether a [`summarize_git_pull_failure`] summary describes a divergence.
pub fn summary_is_divergence(summary: &str) -> bool {
    summary == GIT_PULL_DIVERGED_SUMMARY
}

/// Start a background source update for `session_id`.
///
/// Source checkouts update by pull + rebuild, which is exactly what
/// [`crate::session_rebuild::spawn_background_session_rebuild`] already does
/// (with test gating and reload publication). Since F20c removed the release
/// download path, "update" and "rebuild" are the same operation for every
/// non-nix install, so this delegates rather than maintaining a second,
/// subtly-different pipeline.
pub fn spawn_background_session_update(session_id: String) {
    crate::session_rebuild::spawn_background_session_rebuild(session_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_release_build() {
        assert!(!is_release_build());
    }

    #[test]
    fn test_summarize_git_pull_failure_diverged() {
        let stderr = b"hint: You have divergent branches and need to specify how to reconcile them.\nfatal: Need to specify how to reconcile divergent branches.\n";
        assert_eq!(
            summarize_git_pull_failure(stderr),
            GIT_PULL_DIVERGED_SUMMARY
        );
        assert!(summary_is_divergence(&summarize_git_pull_failure(stderr)));
    }

    #[test]
    fn test_summarize_git_pull_failure_no_tracking_branch() {
        let stderr = b"There is no tracking information for the current branch.\n";
        assert_eq!(
            summarize_git_pull_failure(stderr),
            "git pull failed: current branch has no upstream tracking branch"
        );
    }

    #[test]
    fn test_summarize_git_pull_failure_uses_first_non_hint_line() {
        let stderr = b"hint: test hint\nfatal: repository not found\n";
        assert_eq!(
            summarize_git_pull_failure(stderr),
            "git pull failed: repository not found"
        );
    }

    #[test]
    fn test_summarize_git_pull_failure_empty_stderr() {
        assert_eq!(summarize_git_pull_failure(b""), "git pull failed");
    }

    #[test]
    fn test_non_divergence_summary_is_not_divergence() {
        assert!(!summary_is_divergence(&summarize_git_pull_failure(
            b"fatal: repository not found\n"
        )));
    }
}
