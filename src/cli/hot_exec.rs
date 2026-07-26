use anyhow::Result;
use std::process::Command as ProcessCommand;

pub use crate::session_rebuild::{hot_rebuild, spawn_background_session_rebuild};

use crate::{build, tui::RunResult, update};

pub fn has_requested_action(run_result: &RunResult) -> bool {
    run_result.reload_session.is_some()
        || run_result.rebuild_session.is_some()
        || run_result.update_session.is_some()
        || run_result.restart_session.is_some()
}

pub fn execute_requested_action(run_result: &RunResult) -> Result<()> {
    if let Some(ref reload_session_id) = run_result.reload_session {
        hot_reload(reload_session_id)?;
    }

    if let Some(ref rebuild_session_id) = run_result.rebuild_session {
        hot_rebuild(rebuild_session_id)?;
    }

    if let Some(ref update_session_id) = run_result.update_session {
        hot_update(update_session_id)?;
    }

    if let Some(ref restart_session_id) = run_result.restart_session {
        hot_restart(restart_session_id)?;
    }

    Ok(())
}

pub fn hot_restart(session_id: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let exe = std::env::current_exe()?;
    let is_selfdev = crate::cli::selfdev::client_selfdev_requested();

    crate::logging::info(&format!("Restarting with current binary: {:?}", exe));

    crate::env::set_var("JCODE_RESUMING", "1");

    let mut cmd = ProcessCommand::new(&exe);
    if is_selfdev {
        cmd.arg("self-dev");
    }
    cmd.arg("--resume").arg(session_id).current_dir(&cwd);
    let err = crate::platform::replace_process(&mut cmd);

    Err(anyhow::anyhow!("Failed to exec {:?}: {}", exe, err))
}

/// Resolve the explicit migrate target for the next reload exec, if any.
///
/// `JCODE_MIGRATE_BINARY` may name a binary directly. When it names a path that
/// no longer exists (the case F20c created by retiring the `stable` channel that
/// used to populate it), fall back to the nix-managed binary so the escape hatch
/// still lands on a real, package-manager-owned generation instead of silently
/// doing nothing. Returns `None` when there is no usable target, so the caller
/// falls through to normal reload resolution.
fn migrate_target() -> Option<std::path::PathBuf> {
    let requested = std::env::var_os("JCODE_MIGRATE_BINARY")?;
    let requested = std::path::PathBuf::from(requested.to_string_lossy().trim());
    if requested.as_os_str().is_empty() {
        return None;
    }
    if requested.exists() {
        return Some(requested);
    }

    match build::nix_managed_fallback_binary() {
        Some(fallback) => {
            crate::logging::warn(&format!(
                "Migration binary {:?} not found; using nix-managed binary {:?}",
                requested, fallback
            ));
            Some(fallback)
        }
        None => {
            crate::logging::warn(&format!(
                "Migration binary {:?} not found and no nix-managed binary available; \
                 falling back to normal reload resolution",
                requested
            ));
            None
        }
    }
}

pub fn hot_reload(session_id: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;

    crate::env::set_var("JCODE_RESUMING", "1");

    // Escape hatch: `JCODE_MIGRATE_BINARY` pins the next exec to an explicit
    // binary. F20c retired the `stable` channel that used to populate it, so the
    // fallback is now the nix-managed binary (the package manager's generation,
    // rolled back with `home-manager`/`nix profile rollback`). An unset or
    // missing target simply falls through to normal reload resolution.
    if let Some(binary_path) = migrate_target() {
        crate::logging::info(&format!("Migrating to binary {:?}...", binary_path));
        let mut cmd = ProcessCommand::new(&binary_path);
        cmd.arg("--resume")
            .arg(session_id)
            .arg("--no-update")
            .env_remove("JCODE_MIGRATE_BINARY")
            .current_dir(&cwd);
        let err = crate::platform::replace_process(&mut cmd);
        return Err(anyhow::anyhow!("Failed to exec {:?}: {}", binary_path, err));
    }

    let is_selfdev = crate::cli::selfdev::client_selfdev_requested();
    let (exe, _label) = build::preferred_reload_candidate(is_selfdev)
        .ok_or_else(|| anyhow::anyhow!("No reloadable binary found"))?;

    if let Ok(metadata) = std::fs::metadata(&exe) {
        let age = metadata
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .map(|d| {
                let secs = d.as_secs();
                if secs < 60 {
                    format!("{} seconds ago", secs)
                } else if secs < 3600 {
                    format!("{} minutes ago", secs / 60)
                } else {
                    format!("{} hours ago", secs / 3600)
                }
            })
            .unwrap_or_else(|| "unknown".to_string());
        crate::logging::info(&format!("Reloading with binary built {}...", age));
    }

    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            if !exe.exists() {
                continue;
            }
        }
        let mut cmd = ProcessCommand::new(&exe);
        if is_selfdev {
            cmd.arg("self-dev");
        }
        cmd.arg("--resume").arg(session_id).current_dir(&cwd);
        let err = crate::platform::replace_process(&mut cmd);

        if err.kind() == std::io::ErrorKind::NotFound && attempt < 2 {
            crate::logging::warn(&format!(
                "exec attempt {} failed (ENOENT) for {:?}, retrying...",
                attempt + 1,
                exe
            ));
            continue;
        }
        return Err(anyhow::anyhow!("Failed to exec {:?}: {}", exe, err));
    }
    Err(anyhow::anyhow!(
        "Failed to exec {:?}: binary not found after retries",
        exe
    ))
}

/// `/update` from inside a live session.
///
/// F20c removed the GitHub-release download path, so for a source checkout an
/// update *is* a pull + rebuild: delegate to the rebuild path, which already
/// pulls, builds, publishes, and re-execs into the fresh binary. Nix-managed
/// installs are inert here (the package manager owns the binary), so we print
/// the honest guidance and resume the session instead of pretending to update.
pub fn hot_update(session_id: &str) -> Result<()> {
    if build::is_externally_managed() {
        print_nix_managed_update_guidance();
        return resume_session_with_current_binary(session_id);
    }

    if get_repo_dir().is_none() {
        update::print_centered("No jcode source checkout found; nothing to update.");
        update::print_centered("Install jcode with nix to receive updates.");
        return resume_session_with_current_binary(session_id);
    }

    hot_rebuild(session_id)
}

/// Re-exec the *current* binary to resume `session_id` unchanged. Used when an
/// update turns out to be a no-op (nix-managed, or no source checkout) so the
/// session still comes back instead of dropping the user at a shell.
fn resume_session_with_current_binary(session_id: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    crate::env::set_var("JCODE_RESUMING", "1");
    let exe = std::env::current_exe()?;
    let is_selfdev = crate::cli::selfdev::client_selfdev_requested();
    let mut cmd = ProcessCommand::new(&exe);
    if is_selfdev {
        cmd.arg("self-dev");
    }
    cmd.arg("--resume")
        .arg(session_id)
        .arg("--no-update")
        .current_dir(&cwd);
    let err = crate::platform::replace_process(&mut cmd);
    Err(anyhow::anyhow!("Failed to exec {:?}: {}", exe, err))
}

fn print_nix_managed_update_guidance() {
    update::print_centered("jcode is managed by nix; self-update is disabled.");
    update::print_centered("Update it the way you installed it:");
    update::print_centered("  home-manager:  rebuild your home-manager generation");
    update::print_centered("  nix profile:   nix profile upgrade jcode  (or your flake ref)");
    update::print_centered("  flake input:   nix flake update jcode  then rebuild");
}

pub fn get_repo_dir() -> Option<std::path::PathBuf> {
    build::get_repo_dir()
}

/// Minimum interval between `git fetch` update probes across all jcode
/// processes. Every source-build client spawn used to fetch unconditionally,
/// so spawning N clients at once ran N concurrent `git fetch` + ssh sessions
/// against the remote. One probe per interval per machine is plenty; a marker
/// file's mtime coordinates it (same pattern as the session-backup pruner).
const UPDATE_FETCH_INTERVAL_SECS: u64 = 15 * 60;

fn claim_update_fetch_slot() -> bool {
    let Ok(base) = crate::storage::jcode_dir() else {
        // Cannot coordinate without a home dir; fall back to probing.
        return true;
    };
    let marker = base.join("update-fetch.stamp");
    if let Ok(metadata) = std::fs::metadata(&marker)
        && let Ok(modified) = metadata.modified()
        && let Ok(age) = std::time::SystemTime::now().duration_since(modified)
        && age.as_secs() < UPDATE_FETCH_INTERVAL_SECS
    {
        return false;
    }
    // Touch before fetching so a spawn burst collapses to ~one fetch.
    std::fs::write(&marker, b"").is_ok()
}

pub fn check_for_updates() -> Option<bool> {
    let repo_dir = get_repo_dir()?;

    if claim_update_fetch_slot() {
        let fetch = ProcessCommand::new("git")
            .args(["fetch", "-q"])
            .current_dir(&repo_dir)
            .output()
            .ok()?;

        if !fetch.status.success() {
            return None;
        }
    }
    // When the fetch slot was claimed by another recent process, still answer
    // from the (fresh enough) local refs instead of skipping the check.

    let behind = ProcessCommand::new("git")
        .args(["rev-list", "--count", "HEAD..@{u}"])
        .current_dir(&repo_dir)
        .output()
        .ok()?;

    if behind.status.success() {
        let count: u32 = String::from_utf8_lossy(&behind.stdout)
            .trim()
            .parse()
            .unwrap_or(0);
        Some(count > 0)
    } else {
        None
    }
}

pub fn run_auto_update() -> Result<()> {
    use crate::bus::{Bus, BusEvent, UpdateStatus};

    let repo_dir =
        get_repo_dir().ok_or_else(|| anyhow::anyhow!("Could not find jcode repository"))?;

    update::run_git_pull_ff_only(&repo_dir, true)?;

    crate::logging::info("Building updated source version...");
    let build_output = ProcessCommand::new("cargo")
        .args(["build", "--release"])
        .current_dir(&repo_dir)
        .output()?;

    if !build_output.status.success() {
        let stderr = String::from_utf8_lossy(&build_output.stderr);
        let stdout = String::from_utf8_lossy(&build_output.stdout);
        if !stderr.trim().is_empty() {
            crate::logging::error(&format!("auto-update cargo stderr:\n{}", stderr.trim()));
        }
        if !stdout.trim().is_empty() {
            crate::logging::info(&format!("auto-update cargo stdout:\n{}", stdout.trim()));
        }
        anyhow::bail!("cargo build failed");
    }

    if let Err(e) = build::publish_local_current_build(&repo_dir) {
        crate::logging::warn(&format!("auto-update publish failed: {}", e));
    }

    let hash = ProcessCommand::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&repo_dir)
        .output()?;
    let hash = String::from_utf8_lossy(&hash.stdout);
    let version = format!("main-{}", hash.trim());
    Bus::global().publish(BusEvent::UpdateStatus(UpdateStatus::Installed {
        version: version.clone(),
    }));
    crate::logging::info(&format!("Updated to {}. Restarting...", version));
    std::thread::sleep(std::time::Duration::from_millis(250));

    let exe = build::client_update_candidate(false)
        .map(|(p, _)| p)
        .or_else(|| std::env::current_exe().ok())
        .ok_or_else(|| anyhow::anyhow!("No executable path found after update"))?;
    let args: Vec<String> = std::env::args().skip(1).collect();

    let err =
        crate::platform::replace_process(ProcessCommand::new(&exe).args(&args).arg("--no-update"));

    Err(anyhow::anyhow!(
        "Failed to exec new binary {:?}: {}",
        exe,
        err
    ))
}

pub fn run_update() -> Result<()> {
    // Nix/externally managed installs are updated by the package manager, not by
    // jcode. Surface honest guidance instead of attempting an install that would
    // fail against the read-only store path (or drift the launcher off it).
    if build::is_externally_managed() {
        print_nix_managed_update_guidance();
        return Ok(());
    }

    // F20c: there is no release-acquisition path left. A non-nix install updates
    // from its own source checkout.
    let repo_dir =
        get_repo_dir().ok_or_else(|| anyhow::anyhow!("Could not find jcode repository"))?;

    update::print_centered(&format!("Updating jcode from {}...", repo_dir.display()));

    update::print_centered("Pulling latest changes (fast-forward only)...");
    update::run_git_pull_ff_only(&repo_dir, true)?;

    update::print_centered("Building...");
    let build_status = ProcessCommand::new("cargo")
        .args(["build", "--release"])
        .current_dir(&repo_dir)
        .status()?;

    if !build_status.success() {
        anyhow::bail!("cargo build failed");
    }

    if let Err(e) = build::publish_local_current_build(&repo_dir) {
        update::print_centered(&format!("Warning: publish failed: {}", e));
    }

    let hash = ProcessCommand::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&repo_dir)
        .output()?;

    let hash = String::from_utf8_lossy(&hash.stdout);
    update::print_centered(&format!("Successfully updated to {}", hash.trim()));
    reload_server_after_update("source update");

    Ok(())
}

fn reload_server_after_update(reason: &str) {
    let exe = build::client_update_candidate(false)
        .map(|(path, _)| path)
        .or_else(|| std::env::current_exe().ok());
    let Some(exe) = exe else {
        crate::logging::warn("update: could not find jcode binary to reload stale server");
        return;
    };

    let output = ProcessCommand::new(&exe)
        .args(["--no-update", "server", "reload", "--force"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            crate::logging::info(&format!(
                "update: requested server reload after {} via {:?}",
                reason, exe
            ));
        }
        Ok(output) => {
            crate::logging::warn(&format!(
                "update: server reload after {} failed with status {:?}: {}",
                reason,
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Err(error) => {
            crate::logging::warn(&format!(
                "update: failed to request server reload after {} via {:?}: {}",
                reason, exe, error
            ));
        }
    }
}
