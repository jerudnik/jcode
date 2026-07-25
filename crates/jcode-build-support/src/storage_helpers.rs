use anyhow::Result;
use jcode_storage as storage;
use std::path::PathBuf;

/// Path to the build manifest (self-dev build history).
///
/// F20c: this deliberately does NOT live under `~/.jcode/builds/`. That
/// directory is the retired distribution layout, and `jcode doctor
/// --clean-retired-layout` exists to delete it; keeping live state inside it
/// would mean the cleanup destroys data that is still in use, and would
/// recreate the retired directory on every run.
pub fn manifest_path() -> Result<PathBuf> {
    Ok(storage::jcode_dir()?.join("build-manifest.json"))
}

/// The pre-F20c manifest location, inside the retired `builds/` directory.
///
/// Only used to migrate history forward once; nothing writes it.
pub fn legacy_manifest_path() -> Result<PathBuf> {
    Ok(storage::jcode_dir()?.join("builds").join("manifest.json"))
}

/// Get path to build log file
pub fn build_log_path() -> Result<PathBuf> {
    Ok(storage::jcode_dir()?.join("build.log"))
}

/// Get path to build progress file (for TUI to watch)
pub fn build_progress_path() -> Result<PathBuf> {
    Ok(storage::jcode_dir()?.join("build-progress"))
}

/// Write current build progress (for TUI to display)
pub fn write_build_progress(status: &str) -> Result<()> {
    let path = build_progress_path()?;
    std::fs::write(&path, status)?;
    invalidate_build_progress_cache();
    Ok(())
}

/// Process-local cache for `read_build_progress`. Stores the last-read value
/// alongside the time it was read so per-frame TUI calls can be served without
/// a disk hit.
static BUILD_PROGRESS_CACHE: std::sync::Mutex<Option<(std::time::Instant, Option<String>)>> =
    std::sync::Mutex::new(None);

const BUILD_PROGRESS_TTL: std::time::Duration = std::time::Duration::from_millis(100);

fn invalidate_build_progress_cache() {
    if let Ok(mut guard) = BUILD_PROGRESS_CACHE.lock() {
        *guard = None;
    }
}

/// Read current build progress.
///
/// The TUI calls this from its per-frame redraw scheduler (several times per
/// frame, across every connected client), so a naive implementation performs a
/// synchronous disk read on every render tick even when no build is running.
/// Build progress is a purely cosmetic status string, so we cache the result
/// for a short window. The cache is invalidated immediately on
/// `write_build_progress`/`clear_build_progress` so progress still updates
/// promptly when a build is driven from the same process; cross-process updates
/// become visible within the TTL.
pub fn read_build_progress() -> Option<String> {
    if let Ok(guard) = BUILD_PROGRESS_CACHE.lock()
        && let Some((at, ref value)) = *guard
        && at.elapsed() < BUILD_PROGRESS_TTL
    {
        return value.clone();
    }

    let value = read_build_progress_uncached();

    if let Ok(mut guard) = BUILD_PROGRESS_CACHE.lock() {
        *guard = Some((std::time::Instant::now(), value.clone()));
    }

    value
}

fn read_build_progress_uncached() -> Option<String> {
    build_progress_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Clear build progress
pub fn clear_build_progress() -> Result<()> {
    let path = build_progress_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    invalidate_build_progress_cache();
    Ok(())
}
