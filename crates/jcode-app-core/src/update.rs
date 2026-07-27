//! Nix-owned update guidance and source-rebuild helpers.
//!
//! F20a made jcode nix-native: a packaged binary lives in the read-only Nix
//! store and is updated by the package manager, never by jcode itself. F20b
//! collapsed self-dev publication to one atomic fixed path. F20c therefore
//! retired the GitHub-release acquisition subsystem (download/resume/checksum
//! verify/install-into-a-version-store) entirely: there is no longer any code
//! path in which jcode fetches a release tarball and installs it over itself.
//!
//! End-user installation and updates are owned exclusively by the repository's
//! Nix flake. `jcode update` and `/update` only show package-manager guidance.
//! The remaining git helper is restricted to the explicit source-development
//! rebuild flow in [`crate::session_rebuild`]; it is not a distribution path.

use anyhow::{Context, Result};
use std::path::Path;

pub const NIX_UPDATE_GUIDANCE: &str = "Jcode is distributed and updated through Nix.\n\
Update it the way you installed it:\n\
  Home Manager: rebuild your Home Manager generation\n\
  nix profile:  nix profile upgrade jcode  (or your flake reference)\n\
  flake input:  nix flake update jcode  then rebuild";

/// Summary emitted when `git pull` cannot reconcile the local and tracked
/// histories on its own (diverged branches, non-fast-forward, unrelated
/// histories). Callers use this to recognize a divergence and offer a merge
/// affordance instead of a generic failure.
pub const GIT_PULL_DIVERGED_SUMMARY: &str =
    "The local and tracked branches have diverged, so the source rebuild could not fast-forward.";

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

/// Fast-forward the source checkout at `repo_dir`.
///
/// This developer-only source-rebuild primitive runs the
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
        return "git pull failed: current branch has no remote tracking branch".to_string();
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

/// Whether `git pull` stderr indicates the local and tracked branches have
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

#[cfg(test)]
mod tests {
    use super::*;

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
            "git pull failed: current branch has no remote tracking branch"
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
