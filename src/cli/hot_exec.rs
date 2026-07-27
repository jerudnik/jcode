use anyhow::Result;
use std::process::Command as ProcessCommand;

pub use crate::session_rebuild::{hot_rebuild, spawn_background_session_rebuild};

use crate::{build, tui::RunResult, update};

pub fn has_requested_action(run_result: &RunResult) -> bool {
    run_result.reload_session.is_some()
        || run_result.rebuild_session.is_some()
        || run_result.restart_session.is_some()
}

pub fn execute_requested_action(run_result: &RunResult) -> Result<()> {
    if let Some(ref reload_session_id) = run_result.reload_session {
        hot_reload(reload_session_id)?;
    }

    if let Some(ref rebuild_session_id) = run_result.rebuild_session {
        hot_rebuild(rebuild_session_id)?;
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

/// Show the package-manager-owned update path without modifying the running binary.
pub fn run_update() -> Result<()> {
    update::print_centered(update::NIX_UPDATE_GUIDANCE);
    Ok(())
}
