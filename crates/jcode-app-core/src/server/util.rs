pub(crate) mod binary_freshness;

use crate::build;
use binary_freshness::{newer_binary_available, process_start_time};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::OnceCell;

/// Default embedding idle unload threshold (15 minutes).
const EMBEDDING_IDLE_UNLOAD_DEFAULT_SECS: u64 = 15 * 60;

pub(crate) fn debug_control_allowed() -> bool {
    // Check config file setting
    if crate::config::config().display.debug_socket {
        return true;
    }
    if std::env::var("JCODE_DEBUG_CONTROL")
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
    {
        return true;
    }
    // Check for file-based toggle (allows enabling without restart)
    if let Ok(jcode_dir) = crate::storage::jcode_dir()
        && jcode_dir.join("debug_control").exists()
    {
        return true;
    }
    false
}

pub(crate) fn embedding_idle_unload_secs() -> u64 {
    std::env::var("JCODE_EMBEDDING_IDLE_UNLOAD_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(EMBEDDING_IDLE_UNLOAD_DEFAULT_SECS)
}

pub(crate) async fn get_shared_mcp_pool(
    cell: &OnceCell<Arc<crate::mcp::SharedMcpPool>>,
) -> Arc<crate::mcp::SharedMcpPool> {
    cell.get_or_init(|| async {
        // Composition root (F01 design 3.0): the pool receives the server's
        // activity-lease authority so in-flight MCP calls pin the daemon.
        Arc::new(
            crate::mcp::SharedMcpPool::from_default_config_with_activity(
                super::shutdown::activity_authority(),
            ),
        )
    })
    .await
    .clone()
}

pub(crate) fn server_update_candidate(is_selfdev_session: bool) -> Option<(PathBuf, &'static str)> {
    build::shared_server_update_candidate(is_selfdev_session)
}

/// Resolve the binary the reload should actually exec into, with a hard
/// no-downgrade guard.
///
/// `server_update_candidate` can legitimately return an *older* binary (e.g. a
/// `shared-server` channel that an update never advanced, or a leftover self-dev
/// promotion synced from another machine). A forced reload bypasses
/// `server_has_newer_binary`, so without this guard it would silently exec into
/// that older binary and downgrade every connected client.
///
/// We never block a same-or-newer candidate (so self-dev builds, which are
/// freshly written and therefore newer by mtime, still apply). When the
/// candidate is *strictly older* than the running executable we refuse it and
/// re-exec into the current executable instead: same code, fresh process and
/// socket handoff, but no downgrade. Any mtime uncertainty is treated as "do
/// not downgrade".
///
/// Crucially, the candidate is the *newest* reload candidate across BOTH
/// self-dev flavors, not just the one matching `is_selfdev_session`. This keeps
/// the reload target consistent with `server_has_newer_binary`, which also scans
/// both flavors. Without this, a self-dev/canary daemon whose `shared-server`
/// channel is pinned to an *old* self-dev build would advertise
/// `server_has_update = true` (the normal-flavor probe self-heals to the freshly
/// installed release) yet reload into that same old pinned build -> the server
/// reports an update it can never apply, so the client upgrades while the server
/// stays stale and the auto-reload loops until it is suppressed. Selecting the
/// newest candidate across flavors still preserves a deliberately-pinned self-dev
/// build whenever that build is the freshest one on disk (the case the pin is
/// meant to protect).
pub(crate) fn reload_exec_target(
    is_selfdev_session: bool,
    force: bool,
) -> Option<(PathBuf, &'static str)> {
    let resolution = resolve_reload_target(is_selfdev_session, force);
    resolution.log_decision("reload_exec_target");
    if let Some(refusal) = resolution.refusal_message() {
        crate::logging::error(&format!(
            "reload target resolution refused before exec: {refusal}"
        ));
        return None;
    }
    resolution.chosen_target()
}

/// Resolve the binary the server should reload into and preserve enough context
/// for callers to either exec it or refuse/skips actionably.
///
/// This is the shared target-selection API for reload callers. `reload_exec_target`
/// keeps the legacy `Option<(PathBuf, label)>` shape for the reload worker and
/// debug/selfdev call sites, while stateful request handlers can inspect the
/// structured result first and return a client-visible refusal before shutdown.
pub(crate) fn resolve_reload_target(
    is_selfdev_session: bool,
    force: bool,
) -> ReloadTargetResolution {
    let current_exe = std::env::current_exe().ok().map(strip_deleted_suffix);
    let current_canonical = current_exe
        .as_ref()
        .map(|p| build::resolve_binary_payload(p));
    let current_mtime = current_canonical.as_deref().and_then(binary_mtime);

    let candidates = collect_reload_target_candidates(is_selfdev_session, current_exe.as_deref());
    resolve_reload_target_from_candidates(
        force,
        current_exe,
        current_canonical,
        current_mtime,
        candidates,
    )
}

fn resolve_reload_target_from_candidates(
    force: bool,
    current_exe: Option<PathBuf>,
    current_canonical: Option<PathBuf>,
    current_mtime: Option<SystemTime>,
    candidates: Vec<ReloadTargetCandidate>,
) -> ReloadTargetResolution {
    let candidate = pick_newest_target_candidate(
        candidates
            .iter()
            .filter(|candidate| candidate.exec_candidate)
            .cloned(),
    );

    let mut rejection_reasons = Vec::new();
    let mut refused = None;
    let mut chosen = None;
    let mut chosen_payload = None;

    if force && let Some(reason) = forced_stale_shared_server_refusal(&candidates) {
        rejection_reasons.push(reason.clone());
        refused = Some(reason);
    }

    if refused.is_none() {
        if let Some(candidate) = candidate {
            // Identity/mtime comparisons must look through release wrapper scripts to
            // the payload that actually runs (see `build::resolve_binary_payload`):
            // the running exe is the `.bin` payload while channel candidates are tiny
            // wrapper scripts, and comparing wrapper-vs-payload mtimes turned every
            // release install into a phantom "downgrade"/"update". The exec target
            // stays the original candidate path (the wrapper), which is what sets up
            // `LD_LIBRARY_PATH` correctly.
            let decision = guarded_reload_target(
                (candidate.path.clone(), candidate.label),
                candidate.payload.as_path(),
                current_exe.as_deref(),
                current_canonical.as_deref(),
                current_mtime,
                candidate.mtime,
            );
            match decision {
                ReloadTargetDecision::UseCandidate(target) => {
                    chosen_payload = Some(candidate.payload.clone());
                    chosen = Some(target);
                }
                ReloadTargetDecision::DowngradeBlockedUseCurrent(target) => {
                    // Never strand clients by re-execing a binary that is gone from disk.
                    // If the running exe was unlinked (e.g. an in-place rebuild) but the
                    // candidate still exists, prefer the candidate over refusing to
                    // reload. The candidate may be older, but a live downgrade beats a
                    // dead server with no replacement.
                    if !target.0.exists() && candidate.payload.exists() {
                        crate::logging::warn(&format!(
                            "reload downgrade guard: current binary {:?} is missing on disk; falling back to candidate {:?} to avoid stranding clients",
                            target.0, candidate.path,
                        ));
                        chosen_payload = Some(candidate.payload.clone());
                        chosen = Some((candidate.path.clone(), candidate.label));
                    } else {
                        crate::logging::warn(&format!(
                            "reload downgrade guard: refusing to exec into older candidate; re-execing current binary {:?} instead",
                            target.0,
                        ));
                        chosen_payload = current_canonical.clone();
                        chosen = Some(target);
                    }
                }
                ReloadTargetDecision::DowngradeUnverifiable(target) => {
                    crate::logging::warn(&format!(
                        "reload downgrade guard: older candidate {:?} detected but current exe is unavailable; proceeding with candidate",
                        target.0,
                    ));
                    chosen_payload = Some(candidate.payload.clone());
                    chosen = Some(target);
                }
            }
        } else {
            rejection_reasons.push("no reload target candidates were found".to_string());
        }
    }

    ReloadTargetResolution {
        force,
        current_exe,
        current_payload: current_canonical,
        current_mtime,
        chosen,
        chosen_payload,
        candidates,
        rejection_reasons,
        refused,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ReloadTargetResolution {
    force: bool,
    current_exe: Option<PathBuf>,
    current_payload: Option<PathBuf>,
    current_mtime: Option<SystemTime>,
    chosen: Option<(PathBuf, &'static str)>,
    chosen_payload: Option<PathBuf>,
    candidates: Vec<ReloadTargetCandidate>,
    rejection_reasons: Vec<String>,
    refused: Option<String>,
}

impl ReloadTargetResolution {
    pub(crate) fn chosen_target(&self) -> Option<(PathBuf, &'static str)> {
        self.chosen.clone()
    }

    pub(crate) fn refusal_message(&self) -> Option<&str> {
        self.refused.as_deref()
    }

    pub(crate) fn has_strictly_newer_candidate_than_current(&self) -> bool {
        newer_binary_available(
            self.current_mtime,
            self.current_payload.as_deref(),
            process_start_time(),
            self.candidates
                .iter()
                .filter(|candidate| candidate.exec_candidate)
                .map(|candidate| (candidate.payload.clone(), candidate.mtime)),
        )
    }

    pub(crate) fn no_update_message(&self) -> String {
        format!(
            "Server reload skipped: no strictly newer approved reload target by mtime. {}{}",
            self.candidate_summary(),
            self.rejection_summary_suffix()
        )
    }

    pub(crate) fn log_decision(&self, context: &str) {
        let candidates = self
            .candidates
            .iter()
            .map(|candidate| {
                serde_json::json!({
                    "purpose": candidate.purpose,
                    "label": candidate.label,
                    "path": candidate.path.display().to_string(),
                    "payload": candidate.payload.display().to_string(),
                    "mtime_unix_ms": system_time_unix_ms(candidate.mtime),
                    "exists": candidate.path.exists() || candidate.payload.exists(),
                    "exec_candidate": candidate.exec_candidate,
                    "rejection_reasons": candidate.rejection_reasons,
                })
            })
            .collect::<Vec<_>>();
        crate::logging::info(&format!(
            "RELOAD_TARGET_DECISION {}",
            serde_json::json!({
                "context": context,
                "force": self.force,
                "chosen_path": self.chosen.as_ref().map(|(path, _)| path.display().to_string()),
                "chosen_label": self.chosen.as_ref().map(|(_, label)| *label),
                "chosen_payload": self.chosen_payload.as_ref().map(|path| path.display().to_string()),
                "current_exe": self.current_exe.as_ref().map(|path| path.display().to_string()),
                "current_payload": self.current_payload.as_ref().map(|path| path.display().to_string()),
                "current_mtime_unix_ms": system_time_unix_ms(self.current_mtime),
                "candidates": candidates,
                "rejection_reasons": self.rejection_reasons,
                "refused": self.refused,
            })
        ));
    }

    fn candidate_summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(current) = self.current_payload.as_ref() {
            parts.push(format!(
                "current-exe={} mtime={}",
                current.display(),
                format_mtime(self.current_mtime)
            ));
        }
        parts.extend(self.candidates.iter().map(|candidate| {
            format!(
                "{}:{}={} payload={} mtime={}",
                candidate.purpose,
                candidate.label,
                candidate.path.display(),
                candidate.payload.display(),
                format_mtime(candidate.mtime)
            )
        }));
        format!("Candidates: {}.", parts.join("; "))
    }

    fn rejection_summary_suffix(&self) -> String {
        if self.rejection_reasons.is_empty() {
            String::new()
        } else {
            format!(" Rejections: {}.", self.rejection_reasons.join("; "))
        }
    }
}

#[derive(Debug, Clone)]
struct ReloadTargetCandidate {
    purpose: &'static str,
    label: &'static str,
    path: PathBuf,
    payload: PathBuf,
    mtime: Option<SystemTime>,
    exec_candidate: bool,
    rejection_reasons: Vec<String>,
}

fn collect_reload_target_candidates(
    is_selfdev_session: bool,
    current_exe: Option<&Path>,
) -> Vec<ReloadTargetCandidate> {
    let mut candidates = Vec::new();
    let mut seen_purposes = HashSet::new();

    for (purpose, candidate) in [
        ("preferred", server_update_candidate(is_selfdev_session)),
        ("alternate", server_update_candidate(!is_selfdev_session)),
    ] {
        if let Some((path, label)) = candidate
            && seen_purposes.insert((purpose, path.clone()))
        {
            candidates.push(target_candidate(purpose, label, path, true, Vec::new()));
        }
    }

    // F20c: the stable/shared-server channels are gone; the only published
    // binary is the single fixed target.
    if let Ok(path) = build::current_fixed_binary_path()
        && path.exists()
    {
        candidates.push(target_candidate(
            "published",
            "current-fixed",
            path,
            false,
            Vec::new(),
        ));
    }
    if let Some(path) = build::get_repo_dir()
        .and_then(|repo| build::find_dev_binary(&repo))
        .filter(|path| path.exists())
    {
        candidates.push(target_candidate(
            "candidate",
            "dev",
            path,
            false,
            Vec::new(),
        ));
    }
    if let Some(path) = current_exe {
        candidates.push(target_candidate(
            "current",
            "current-exe",
            path.to_path_buf(),
            false,
            Vec::new(),
        ));
    }

    annotate_reload_candidates(candidates)
}

fn target_candidate(
    purpose: &'static str,
    label: &'static str,
    path: PathBuf,
    exec_candidate: bool,
    rejection_reasons: Vec<String>,
) -> ReloadTargetCandidate {
    let payload = build::resolve_binary_payload(&path);
    let mtime = binary_mtime(payload.as_path());
    ReloadTargetCandidate {
        purpose,
        label,
        path,
        payload,
        mtime,
        exec_candidate,
        rejection_reasons,
    }
}

fn annotate_reload_candidates(
    mut candidates: Vec<ReloadTargetCandidate>,
) -> Vec<ReloadTargetCandidate> {
    let newest_mtime = candidates
        .iter()
        .filter_map(|candidate| candidate.mtime)
        .max();
    let payload_counts = candidates.iter().fold(
        std::collections::HashMap::<PathBuf, usize>::new(),
        |mut counts, candidate| {
            *counts.entry(candidate.payload.clone()).or_default() += 1;
            counts
        },
    );

    for candidate in &mut candidates {
        if !candidate.path.exists() && !candidate.payload.exists() {
            candidate
                .rejection_reasons
                .push("path and resolved payload are missing".to_string());
        }
        if candidate.mtime.is_none() {
            candidate
                .rejection_reasons
                .push("resolved payload mtime is unavailable".to_string());
        }
        if let (Some(newest), Some(mtime)) = (newest_mtime, candidate.mtime)
            && mtime < newest
        {
            candidate.rejection_reasons.push(format!(
                "older than another candidate by mtime (candidate={}, newest={})",
                format_mtime(Some(mtime)),
                format_mtime(Some(newest))
            ));
        }
        if payload_counts.get(&candidate.payload).copied().unwrap_or(0) > 1 {
            candidate
                .rejection_reasons
                .push("duplicates another candidate after payload resolution".to_string());
        }
    }

    candidates
}

fn pick_newest_target_candidate(
    candidates: impl IntoIterator<Item = ReloadTargetCandidate>,
) -> Option<ReloadTargetCandidate> {
    let mut best: Option<ReloadTargetCandidate> = None;
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.payload.clone()) {
            continue;
        }
        let replace = match (&best, candidate.mtime) {
            (None, _) => true,
            (Some(best), Some(new_mtime)) => best.mtime.is_none_or(|best| new_mtime > best),
            (Some(_), None) => false,
        };
        if replace {
            best = Some(candidate);
        }
    }
    best
}

fn forced_stale_shared_server_refusal(candidates: &[ReloadTargetCandidate]) -> Option<String> {
    let shared = candidates
        .iter()
        .filter(|candidate| candidate.label == "shared-server")
        .filter(|candidate| candidate.path.exists() || candidate.payload.exists())
        .filter_map(|candidate| candidate.mtime.map(|mtime| (candidate, mtime)))
        .min_by_key(|(_, mtime)| *mtime)?;

    let newer = candidates
        .iter()
        .filter(|candidate| candidate.payload != shared.0.payload)
        .filter(|candidate| candidate.path.exists() || candidate.payload.exists())
        .filter_map(|candidate| candidate.mtime.map(|mtime| (candidate, mtime)))
        .filter(|(_, mtime)| *mtime > shared.1)
        .max_by_key(|(_, mtime)| *mtime)?;

    Some(format!(
        "forced reload refused: shared-server target {} (payload {}, mtime {}) is stale while {} target {} (payload {}, mtime {}) is strictly newer. Publish or promote the intended build to shared-server, or run a non-forced reload to select an approved newer target.",
        shared.0.path.display(),
        shared.0.payload.display(),
        format_mtime(Some(shared.1)),
        newer.0.label,
        newer.0.path.display(),
        newer.0.payload.display(),
        format_mtime(Some(newer.1)),
    ))
}

fn system_time_unix_ms(time: Option<SystemTime>) -> Option<u64> {
    time.and_then(|time| {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
    })
}

fn format_mtime(time: Option<SystemTime>) -> String {
    system_time_unix_ms(time)
        .map(|millis| format!("{millis}ms-since-epoch"))
        .unwrap_or_else(|| "unavailable".to_string())
}

#[cfg(test)]
fn newest_reload_candidate(is_selfdev_session: bool) -> Option<(PathBuf, &'static str)> {
    newest_reload_candidate_inner(is_selfdev_session)
}

fn binary_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

/// Pick the newest reload candidate across BOTH self-dev flavors.
///
/// The session's own flavor (`is_selfdev_session`) is evaluated first so it wins
/// any exact-mtime tie, preserving self-dev semantics: a deliberately-pinned
/// self-dev `shared-server` build is honored whenever it is at least as fresh as
/// the other flavor's candidate. The other flavor only wins when it is
/// *strictly newer*, which is exactly the situation that makes
/// `server_has_newer_binary` report an update (e.g. `/update` installed a newer
/// release while the self-dev pin stayed on an older build).
#[cfg(test)]
fn newest_reload_candidate_inner(is_selfdev_session: bool) -> Option<(PathBuf, &'static str)> {
    let ordered = [
        server_update_candidate(is_selfdev_session),
        server_update_candidate(!is_selfdev_session),
    ];
    let with_mtimes = ordered.into_iter().flatten().map(|candidate| {
        // Compare payloads, not release wrapper scripts (whose mtimes carry no
        // version information). Dedup also happens on the payload so a wrapper
        // and its payload never count as two distinct candidates.
        let canonical = build::resolve_binary_payload(&candidate.0);
        let mtime = binary_mtime(canonical.as_path());
        (candidate, canonical, mtime)
    });
    pick_newest_candidate(with_mtimes)
}

/// Pure, order-sensitive "newest candidate" selection used by
/// [`newest_reload_candidate`]. Candidates are provided in *preference order*
/// (the session's own flavor first). A later candidate only displaces an earlier
/// one when it is provably, strictly newer by mtime, so equal/unknown mtimes
/// never demote the higher-preference flavor (protecting a self-dev pin on a
/// tie). Canonical-path duplicates are collapsed to the first occurrence.
#[cfg(test)]
fn pick_newest_candidate(
    candidates: impl IntoIterator<
        Item = (
            (PathBuf, &'static str),
            PathBuf,
            Option<std::time::SystemTime>,
        ),
    >,
) -> Option<(PathBuf, &'static str)> {
    let mut best: Option<((PathBuf, &'static str), Option<std::time::SystemTime>)> = None;
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for (candidate, canonical, mtime) in candidates {
        if !seen.insert(canonical) {
            continue;
        }
        let replace = match (&best, mtime) {
            (None, _) => true,
            (Some((_, Some(best_mtime))), Some(new_mtime)) => new_mtime > *best_mtime,
            (Some((_, None)), Some(_)) => true,
            (Some(_), None) => false,
        };
        if replace {
            best = Some((candidate, mtime));
        }
    }
    best.map(|(candidate, _)| candidate)
}

#[derive(Debug)]
enum ReloadTargetDecision {
    UseCandidate((PathBuf, &'static str)),
    DowngradeBlockedUseCurrent((PathBuf, &'static str)),
    DowngradeUnverifiable((PathBuf, &'static str)),
}

/// Pure no-downgrade decision used by [`reload_exec_target`]. A candidate is
/// accepted unless it is strictly older than (or not provably as new as) the
/// running executable, in which case we prefer re-execing the current binary.
fn guarded_reload_target(
    candidate: (PathBuf, &'static str),
    candidate_canonical: &Path,
    current_exe: Option<&Path>,
    current_canonical: Option<&Path>,
    current_mtime: Option<std::time::SystemTime>,
    candidate_mtime: Option<std::time::SystemTime>,
) -> ReloadTargetDecision {
    // Reloading into the same binary is always fine; no version question.
    if current_canonical == Some(candidate_canonical) {
        return ReloadTargetDecision::UseCandidate(candidate);
    }

    let candidate_is_strictly_older = match (current_mtime, candidate_mtime) {
        (Some(current), Some(cand)) => cand < current,
        // Unknown mtimes: be conservative and treat as a potential downgrade so
        // we never silently swap to an unverifiable binary on a forced reload.
        _ => true,
    };

    if !candidate_is_strictly_older {
        return ReloadTargetDecision::UseCandidate(candidate);
    }

    match current_exe {
        Some(current_exe) => ReloadTargetDecision::DowngradeBlockedUseCurrent((
            current_exe.to_path_buf(),
            "current-exe (downgrade-guard)",
        )),
        None => ReloadTargetDecision::DowngradeUnverifiable(candidate),
    }
}

fn canonicalize_or(path: PathBuf) -> PathBuf {
    std::fs::canonicalize(&path).unwrap_or(path)
}

/// Strip the Linux `/proc/self/exe` " (deleted)" marker that appears when the
/// running binary has been unlinked or replaced in place. The marker is part of
/// the readlink target, not the real filename, so removing it recovers the path
/// that may now point at the freshly written replacement binary.
fn strip_deleted_suffix(path: PathBuf) -> PathBuf {
    const DELETED_MARKER: &str = " (deleted)";
    if let Some(stripped) = path.to_str().and_then(|s| s.strip_suffix(DELETED_MARKER)) {
        return PathBuf::from(stripped);
    }
    path
}

pub(crate) fn git_common_dir_for(path: &Path) -> Option<PathBuf> {
    let mut current = Some(path);
    while let Some(dir) = current {
        let dotgit = dir.join(".git");
        if dotgit.is_dir() {
            return Some(canonicalize_or(dotgit));
        }
        if dotgit.is_file() {
            let content = std::fs::read_to_string(&dotgit).ok()?;
            let gitdir_line = content
                .lines()
                .find(|line| line.trim_start().starts_with("gitdir:"))?;
            let raw = gitdir_line
                .trim_start()
                .trim_start_matches("gitdir:")
                .trim();
            if raw.is_empty() {
                return None;
            }
            let gitdir = if Path::new(raw).is_absolute() {
                PathBuf::from(raw)
            } else {
                dir.join(raw)
            };
            let gitdir = canonicalize_or(gitdir);
            // Worktree gitdir looks like: <repo>/.git/worktrees/<name>
            if let Some(parent) = gitdir.parent()
                && parent.file_name().and_then(|s| s.to_str()) == Some("worktrees")
                && let Some(common) = parent.parent()
            {
                return Some(canonicalize_or(common.to_path_buf()));
            }
            return Some(gitdir);
        }
        current = dir.parent();
    }
    None
}

pub(crate) fn swarm_id_for_dir(dir: Option<PathBuf>) -> Option<String> {
    if let Ok(sw_id) = std::env::var("JCODE_SWARM_ID") {
        let trimmed = sw_id.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let dir = dir?;
    if let Some(git_common) = git_common_dir_for(&dir) {
        return Some(git_common.to_string_lossy().to_string());
    }
    Some(dir.to_string_lossy().to_string())
}

pub(crate) fn server_has_newer_binary() -> bool {
    // Directional check only: report an update solely when a reload *candidate*
    // binary is strictly newer than the binary we are running.
    //
    // We deliberately do NOT treat "my version differs from the installed
    // channel markers" as "I am outdated". That conflated *different* with
    // *older* and caused a real regression (issue #291): a newer self-dev /
    // shared-server daemon (e.g. v0.17.23-dev) running alongside an older
    // release client would be told to "reload" and downgrade itself, because
    // its git hash no longer matched the `current`/`stable` channel markers
    // after a release build moved them. It also fed the reload-loop family from
    // issue #277, since a server that merely "differs" can never make the
    // difference go away by reloading.
    //
    // `UPDATE_SEMVER` is the base Cargo version for every dev build, so it
    // cannot order two dev builds; binary mtime is the only robust, directional
    // signal we have. `newer_binary_available` compares candidate mtimes against
    // the running binary, excludes reloading into ourselves, and treats any
    // uncertainty (unreadable mtime) as "no update".
    //
    // Strip the Linux " (deleted)" marker (see `strip_deleted_suffix`) so an
    // in-place rebuild does not make the running binary's mtime unreadable and
    // suppress a legitimate update signal.
    //
    // All paths are resolved through `build::resolve_binary_payload` so release
    // installs (channel symlink -> wrapper script -> `.bin` payload) compare the
    // payload that actually runs. Comparing the wrapper script against the
    // running payload compared two different files with unrelated mtimes, which
    // could report a phantom update forever and wedge clients into an infinite
    // reload loop right after `/update`.
    let current_exe = std::env::current_exe().ok().map(strip_deleted_suffix);
    let current_canonical = current_exe
        .as_ref()
        .map(|path| build::resolve_binary_payload(path));
    let current_mtime = current_canonical
        .as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok());

    let mut candidates = HashSet::new();
    for is_selfdev_session in [false, true] {
        if let Some((candidate, _label)) = server_update_candidate(is_selfdev_session) {
            candidates.insert(build::resolve_binary_payload(&candidate));
        }
    }

    let candidates_with_mtimes = candidates.into_iter().map(|candidate| {
        let candidate_mtime = std::fs::metadata(&candidate)
            .ok()
            .and_then(|m| m.modified().ok());
        (candidate, candidate_mtime)
    });

    newer_binary_available(
        current_mtime,
        current_canonical.as_deref(),
        process_start_time(),
        candidates_with_mtimes,
    )
}

/// Server identity for multi-server support
#[derive(Debug, Clone)]
pub struct ServerIdentity {
    /// Full server ID (e.g., "server_blazing_1705012345678")
    pub id: String,
    /// Short name (e.g., "blazing")
    pub name: String,
    /// Icon for display (e.g., "🔥")
    pub icon: String,
    /// Git hash of the binary
    pub git_hash: String,
    /// Version string (e.g., "v0.1.123")
    pub version: String,
}

impl ServerIdentity {
    /// Display name with icon (e.g., "🔥 blazing")
    pub fn display_name(&self) -> String {
        format!("{} {}", self.icon, self.name)
    }
}

pub(crate) fn startup_headless_recovery_test_delay() -> Option<std::time::Duration> {
    let raw = std::env::var("JCODE_TEST_HEADLESS_STARTUP_RECOVERY_DELAY_MS").ok()?;
    let delay_ms = raw.trim().parse::<u64>().ok()?;
    (delay_ms > 0).then(|| std::time::Duration::from_millis(delay_ms))
}

#[cfg(test)]
mod reload_target_tests {
    use super::{ReloadTargetDecision, guarded_reload_target};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    fn t(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn candidate(path: &str) -> (PathBuf, &'static str) {
        (PathBuf::from(path), "shared-server")
    }

    #[test]
    fn same_binary_is_always_used() {
        // Reloading into ourselves never raises a version question, even with an
        // older mtime reading.
        let decision = guarded_reload_target(
            candidate("/x/current/jcode"),
            Path::new("/x/current/jcode"),
            Some(Path::new("/x/current/jcode")),
            Some(Path::new("/x/current/jcode")),
            Some(t(200)),
            Some(t(100)),
        );
        assert!(matches!(decision, ReloadTargetDecision::UseCandidate(_)));
    }

    #[test]
    fn newer_candidate_is_used() {
        // The self-dev case: a freshly written candidate is newer, so apply it.
        let decision = guarded_reload_target(
            candidate("/x/shared-server/jcode"),
            Path::new("/x/builds/versions/new/jcode"),
            Some(Path::new("/x/builds/versions/old/jcode")),
            Some(Path::new("/x/builds/versions/old/jcode")),
            Some(t(100)),
            Some(t(200)),
        );
        match decision {
            ReloadTargetDecision::UseCandidate((path, _)) => {
                assert_eq!(path, PathBuf::from("/x/shared-server/jcode"));
            }
            other => panic!("expected candidate to be used, got {other:?}"),
        }
    }

    #[test]
    fn equal_mtime_candidate_is_used() {
        // Same mtime is not a downgrade.
        let decision = guarded_reload_target(
            candidate("/x/shared-server/jcode"),
            Path::new("/x/builds/versions/same/jcode"),
            Some(Path::new("/x/builds/versions/current/jcode")),
            Some(Path::new("/x/builds/versions/current/jcode")),
            Some(t(100)),
            Some(t(100)),
        );
        assert!(matches!(decision, ReloadTargetDecision::UseCandidate(_)));
    }

    #[test]
    fn strictly_older_candidate_is_blocked_and_uses_current_exe() {
        // The reported bug: shared-server channel points at an older build than
        // the running client. Force reload must NOT downgrade; it re-execs the
        // current binary instead.
        let decision = guarded_reload_target(
            candidate("/x/shared-server/jcode"),
            Path::new("/x/builds/versions/old-0.14.3/jcode"),
            Some(Path::new("/x/builds/versions/new/jcode")),
            Some(Path::new("/x/builds/versions/new/jcode")),
            Some(t(300)),
            Some(t(100)),
        );
        match decision {
            ReloadTargetDecision::DowngradeBlockedUseCurrent((path, _)) => {
                assert_eq!(path, PathBuf::from("/x/builds/versions/new/jcode"));
            }
            other => panic!("expected downgrade to be blocked, got {other:?}"),
        }
    }

    #[test]
    fn unreadable_candidate_mtime_is_treated_as_downgrade() {
        let decision = guarded_reload_target(
            candidate("/x/shared-server/jcode"),
            Path::new("/x/builds/versions/unknown/jcode"),
            Some(Path::new("/x/builds/versions/new/jcode")),
            Some(Path::new("/x/builds/versions/new/jcode")),
            Some(t(300)),
            None,
        );
        assert!(matches!(
            decision,
            ReloadTargetDecision::DowngradeBlockedUseCurrent(_)
        ));
    }

    #[test]
    fn downgrade_without_current_exe_falls_back_to_candidate() {
        // If we cannot identify the running exe we cannot re-exec it, so we have
        // to proceed with the candidate rather than refuse to reload entirely.
        let decision = guarded_reload_target(
            candidate("/x/shared-server/jcode"),
            Path::new("/x/builds/versions/old/jcode"),
            None,
            None,
            None,
            Some(t(100)),
        );
        assert!(matches!(
            decision,
            ReloadTargetDecision::DowngradeUnverifiable(_)
        ));
    }
}

#[cfg(test)]
mod target_resolution_tests {
    use super::{
        annotate_reload_candidates, forced_stale_shared_server_refusal,
        resolve_reload_target_from_candidates, target_candidate,
    };
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    fn t(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn write_binary(path: &Path, mtime: SystemTime) {
        std::fs::create_dir_all(path.parent().expect("parent dir")).expect("create parent");
        std::fs::write(path, "binary").expect("write binary");
        std::fs::File::open(path)
            .expect("open binary")
            .set_modified(mtime)
            .expect("set mtime");
    }

    #[test]
    fn target_resolution_refuses_forced_stale_shared_server_when_newer_dev_exists() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let shared = temp.path().join("versions/old-shared/jcode");
        let dev = temp.path().join("target/selfdev/jcode");
        write_binary(&shared, t(100));
        write_binary(&dev, t(300));

        let candidates = annotate_reload_candidates(vec![
            target_candidate("channel", "shared-server", shared.clone(), true, Vec::new()),
            target_candidate("candidate", "dev", dev.clone(), false, Vec::new()),
        ]);

        let refusal = forced_stale_shared_server_refusal(&candidates).expect("stale refusal");
        assert!(refusal.contains("forced reload refused"));
        assert!(refusal.contains("shared-server"));
        assert!(refusal.contains(shared.to_string_lossy().as_ref()));
        assert!(refusal.contains(dev.to_string_lossy().as_ref()));
        assert!(refusal.contains("strictly newer"));
    }

    #[test]
    fn target_resolution_allows_forced_shared_server_when_it_is_freshest() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let shared = temp.path().join("versions/new-shared/jcode");
        let stable = temp.path().join("versions/old-stable/jcode");
        write_binary(&shared, t(300));
        write_binary(&stable, t(100));

        let candidates = annotate_reload_candidates(vec![
            target_candidate("channel", "shared-server", shared, true, Vec::new()),
            target_candidate("channel", "stable", stable, false, Vec::new()),
        ]);

        assert!(forced_stale_shared_server_refusal(&candidates).is_none());
    }

    #[test]
    fn non_forced_stale_shared_server_with_newer_candidate_resolves_exec_target() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let current = temp.path().join("running/jcode");
        let shared = temp.path().join("versions/old-shared/jcode");
        let stable = temp.path().join("versions/new-stable/jcode");
        let dev = temp.path().join("target/selfdev/jcode");
        write_binary(&current, t(100));
        write_binary(&shared, t(100));
        write_binary(&stable, t(300));
        write_binary(&dev, t(400));

        let candidates = annotate_reload_candidates(vec![
            target_candidate("channel", "shared-server", shared, false, Vec::new()),
            target_candidate("candidate", "stable", stable.clone(), true, Vec::new()),
            target_candidate("candidate", "dev", dev, false, Vec::new()),
        ]);

        let resolution = resolve_reload_target_from_candidates(
            false,
            Some(current.clone()),
            Some(current),
            Some(t(100)),
            candidates,
        );

        assert!(resolution.refusal_message().is_none());
        assert!(resolution.has_strictly_newer_candidate_than_current());
        assert_eq!(
            resolution.chosen_target(),
            Some((stable, "stable")),
            "the exec stage must receive a target instead of entering reload_exec_failed"
        );
    }
}

#[cfg(test)]
mod pick_newest_candidate_tests {
    use super::pick_newest_candidate;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn t(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn entry(
        path: &str,
        label: &'static str,
        mtime: Option<SystemTime>,
    ) -> ((PathBuf, &'static str), PathBuf, Option<SystemTime>) {
        let p = PathBuf::from(path);
        ((p.clone(), label), p, mtime)
    }

    #[test]
    fn other_flavor_wins_when_strictly_newer() {
        // The /update bug: the session's own (self-dev) flavor is pinned to an
        // OLD build, but the other (normal) flavor self-healed to a NEWER
        // release. The reload target must follow the newer release so the daemon
        // can actually apply the update it advertises.
        let chosen = pick_newest_candidate([
            entry(
                "/x/versions/old-selfdev/jcode",
                "shared-server",
                Some(t(100)),
            ),
            entry("/x/versions/new-release/jcode", "stable", Some(t(200))),
        ])
        .expect("a candidate");
        assert_eq!(chosen.0, PathBuf::from("/x/versions/new-release/jcode"));
    }

    #[test]
    fn own_flavor_wins_on_tie() {
        // A deliberately-pinned self-dev build that is at least as fresh as the
        // other flavor must be preserved (self-dev pin protection).
        let chosen = pick_newest_candidate([
            entry("/x/versions/selfdev/jcode", "shared-server", Some(t(200))),
            entry("/x/versions/release/jcode", "stable", Some(t(200))),
        ])
        .expect("a candidate");
        assert_eq!(chosen.0, PathBuf::from("/x/versions/selfdev/jcode"));
    }

    #[test]
    fn own_flavor_wins_when_strictly_newer() {
        let chosen = pick_newest_candidate([
            entry(
                "/x/versions/fresh-selfdev/jcode",
                "shared-server",
                Some(t(300)),
            ),
            entry("/x/versions/release/jcode", "stable", Some(t(200))),
        ])
        .expect("a candidate");
        assert_eq!(chosen.0, PathBuf::from("/x/versions/fresh-selfdev/jcode"));
    }

    #[test]
    fn unknown_other_mtime_never_displaces_preferred() {
        // An unreadable mtime on the other flavor must not let it win, so we
        // never swap to an unverifiable binary.
        let chosen = pick_newest_candidate([
            entry("/x/versions/selfdev/jcode", "shared-server", Some(t(100))),
            entry("/x/versions/release/jcode", "stable", None),
        ])
        .expect("a candidate");
        assert_eq!(chosen.0, PathBuf::from("/x/versions/selfdev/jcode"));
    }

    #[test]
    fn duplicate_canonical_paths_collapse() {
        // Both flavors resolving to the same binary must not double-count; the
        // first (preferred) occurrence wins.
        let chosen = pick_newest_candidate([
            entry("/x/versions/same/jcode", "shared-server", Some(t(100))),
            entry("/x/versions/same/jcode", "stable", Some(t(999))),
        ])
        .expect("a candidate");
        assert_eq!(chosen.1, "shared-server");
    }

    #[test]
    fn empty_is_none() {
        assert!(pick_newest_candidate(std::iter::empty()).is_none());
    }
}

#[cfg(test)]
mod newest_reload_candidate_integration_tests {
    //! End-to-end-ish coverage that drives `newest_reload_candidate` through the
    //! REAL resolver (`build::shared_server_update_candidate`) against a temp
    //! `JCODE_HOME`.
    //!
    //! F20c collapsed the stable/current/shared-server channel matrix onto a
    //! single fixed publish target, which deletes the whole class of
    //! "daemon pinned to a stale channel" bugs these tests used to reproduce.
    //! What still needs guarding is the part that survived: a daemon must pick
    //! up a newly published binary, and must NOT report a phantom update
    //! against its own install when the candidate is a wrapper whose payload is
    //! what it is actually running.
    use super::newest_reload_candidate;
    use crate::build;
    use crate::server::util::binary_freshness::newer_binary_available;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    struct HomeGuard<L> {
        _temp: tempfile::TempDir,
        prev: Option<std::ffi::OsString>,
        _lock: L,
    }

    impl<L> HomeGuard<L> {
        fn with_lock(lock: L) -> Self {
            let temp = tempfile::TempDir::new().expect("temp dir");
            let prev = std::env::var_os("JCODE_HOME");
            crate::env::set_var("JCODE_HOME", temp.path());
            Self {
                _temp: temp,
                prev,
                _lock: lock,
            }
        }
    }

    impl<L> Drop for HomeGuard<L> {
        fn drop(&mut self) {
            match self.prev.take() {
                Some(prev) => crate::env::set_var("JCODE_HOME", prev),
                None => crate::env::remove_var("JCODE_HOME"),
            }
        }
    }

    /// Write the single fixed publish target with a controlled mtime.
    fn publish_fixed_binary(contents: &str, mtime: SystemTime) -> PathBuf {
        let path = build::current_fixed_binary_path().expect("fixed path");
        std::fs::create_dir_all(path.parent().expect("fixed dir")).expect("create fixed dir");
        std::fs::write(&path, contents).expect("write published binary");
        std::fs::File::open(&path)
            .expect("open published binary")
            .set_modified(mtime)
            .expect("set mtime");
        path
    }

    /// Re-implements `server_has_newer_binary`'s decision against an *injected*
    /// running-daemon path + mtime, so a test can model "the daemon is still the
    /// OLD binary" without spawning a real process. It scans the exact same
    /// candidate set (both flavors) and uses the same `newer_binary_available`
    /// core the production function uses, including the wrapper->payload
    /// resolution.
    fn daemon_reports_update(running: &Path, running_mtime: SystemTime) -> bool {
        let running_canonical = build::resolve_binary_payload(running);
        let mut candidates = std::collections::HashSet::new();
        for is_selfdev in [false, true] {
            if let Some((candidate, _label)) = super::server_update_candidate(is_selfdev) {
                candidates.insert(build::resolve_binary_payload(&candidate));
            }
        }
        let with_mtimes = candidates.into_iter().map(|candidate| {
            let m = std::fs::metadata(&candidate)
                .ok()
                .and_then(|m| m.modified().ok());
            (candidate, m)
        });
        newer_binary_available(
            Some(running_mtime),
            Some(running_canonical.as_path()),
            None,
            with_mtimes,
        )
    }

    /// The question that matters for users: after a publish (self-dev build or
    /// `/update` rebuild), does the long-lived daemon advertise the upgrade and
    /// resolve its reload target to the newly published binary?
    #[test]
    fn daemon_detects_and_targets_a_newly_published_binary() {
        let _home = HomeGuard::with_lock(crate::storage::lock_test_env());

        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        // The running daemon is an older binary living somewhere else on disk.
        let old_dir = tempfile::TempDir::new().expect("old dir");
        let old_path = old_dir.path().join(build::binary_name());
        std::fs::write(&old_path, "old daemon binary").expect("write old binary");
        std::fs::File::open(&old_path)
            .expect("open old binary")
            .set_modified(base)
            .expect("set mtime");

        let published =
            publish_fixed_binary("new published binary", base + Duration::from_secs(60));

        assert!(
            daemon_reports_update(&old_path, base),
            "daemon should report an update once a newer binary is published"
        );
        for is_selfdev in [false, true] {
            let (target, _label) = newest_reload_candidate(is_selfdev).expect("reload candidate");
            assert_eq!(
                std::fs::canonicalize(&target).expect("canonical target"),
                std::fs::canonicalize(&published).expect("canonical published"),
                "reload target must be the single fixed publish path (is_selfdev={is_selfdev})"
            );
        }
    }

    /// Regression test for the post-update infinite reload loop: when the
    /// published target is a wrapper script that execs a payload, the running
    /// daemon's `current_exe()` is the payload while the candidate resolves to
    /// the wrapper. Comparing wrapper-vs-payload mtimes made a freshly updated
    /// daemon report "newer binary available" against ITS OWN install forever,
    /// so the client force-reloaded the server in a loop and the session never
    /// attached. F20b's wrapper->payload resolution is what prevents this.
    #[test]
    fn freshly_published_daemon_reports_no_phantom_update() {
        let _home = HomeGuard::with_lock(crate::storage::lock_test_env());

        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let fixed_dir = build::current_fixed_binary_path()
            .expect("fixed path")
            .parent()
            .expect("fixed dir")
            .to_path_buf();
        std::fs::create_dir_all(&fixed_dir).expect("create fixed dir");
        let payload = fixed_dir.join("jcode-linux-x86_64.bin");
        std::fs::write(&payload, "payload").expect("write payload");
        std::fs::File::open(&payload)
            .expect("open payload")
            .set_modified(base)
            .expect("set payload mtime");
        // Wrapper written strictly AFTER the payload (the bad copy order).
        let wrapper = publish_fixed_binary(
            "#!/usr/bin/env sh\nexec ./jcode-linux-x86_64.bin \"$@\"\n",
            base + Duration::from_secs(5),
        );

        let payload_mtime = std::fs::metadata(&payload)
            .expect("payload metadata")
            .modified()
            .expect("payload mtime");
        assert!(
            !daemon_reports_update(&payload, payload_mtime),
            "a freshly updated daemon must not report an update against its own install"
        );
        // Sanity: the wrapper IS strictly newer than the payload on disk, so a
        // naive wrapper-vs-payload comparison would have reported a phantom
        // update (the bug this guards against).
        let wrapper_mtime = std::fs::metadata(&wrapper)
            .expect("wrapper metadata")
            .modified()
            .expect("wrapper mtime");
        assert!(wrapper_mtime > payload_mtime);
    }
}

#[cfg(test)]
mod deleted_suffix_tests {
    use super::strip_deleted_suffix;
    use std::path::PathBuf;

    #[test]
    fn strips_linux_deleted_marker() {
        let p = PathBuf::from("/home/u/.jcode/builds/versions/abc/jcode (deleted)");
        assert_eq!(
            strip_deleted_suffix(p),
            PathBuf::from("/home/u/.jcode/builds/versions/abc/jcode")
        );
    }

    #[test]
    fn leaves_normal_paths_untouched() {
        let p = PathBuf::from("/home/u/.jcode/builds/versions/abc/jcode");
        assert_eq!(strip_deleted_suffix(p.clone()), p);
    }

    #[test]
    fn only_strips_trailing_marker() {
        // A path that merely contains the substring must not be altered.
        let p = PathBuf::from("/home/u/jcode (deleted)/jcode");
        assert_eq!(strip_deleted_suffix(p.clone()), p);
    }
}
