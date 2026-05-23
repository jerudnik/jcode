use super::*;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

fn lock_env() -> crate::storage::TestEnvLockGuard {
    crate::storage::lock_test_env()
}

/// RAII guard that restores the process current working directory on
/// drop, even when the protected scope panics.
///
/// Used by tests that need to `set_current_dir` to a tempdir and then
/// assert on cwd-derived state. Without this, an in-scope `assert!`
/// panic leaves the process cwd dangling at a deleted tempdir, which
/// poisons every subsequent test in the binary that calls
/// `current_dir()` (e.g. `tool::communicate::*`).
pub(crate) struct CwdGuard {
    prev: PathBuf,
}

impl CwdGuard {
    /// Capture `current_dir()` and arrange for it to be restored on
    /// drop. Caller is responsible for any subsequent `set_current_dir`
    /// inside the guarded scope.
    pub(crate) fn capture() -> std::io::Result<Self> {
        let prev = std::env::current_dir()?;
        Ok(Self { prev })
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        // Best-effort: if the original cwd is gone too, ignore - the
        // subsequent test that itself calls `current_dir()` will fail
        // loudly anyway, which is the right diagnostic.
        let _ = std::env::set_current_dir(&self.prev);
    }
}

/// Macos-friendly canonicalization for tempdir paths.
///
/// `tempfile::TempDir::path()` returns the path as constructed (e.g.
/// `/tmp/...`), but after `std::env::set_current_dir(temp.path())`
/// `current_dir()` returns the canonical, symlink-resolved form
/// (`/private/tmp/...` on macOS). Tests that assert "Working directory:
/// <temp.path()>" against a message built from `current_dir()` therefore
/// flake on macOS. Use this helper on the LHS of such asserts.
pub(crate) fn canonical_display(path: &Path) -> String {
    std::fs::canonicalize(path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

struct EnvVarGuard {
    key: &'static str,
    prev: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let prev = std::env::var_os(key);
        crate::env::set_var(key, value);
        Self { key, prev }
    }

    fn remove(key: &'static str) -> Self {
        let prev = std::env::var_os(key);
        crate::env::remove_var(key);
        Self { key, prev }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(prev) = &self.prev {
            crate::env::set_var(self.key, prev);
        } else {
            crate::env::remove_var(self.key);
        }
    }
}

#[path = "cases.rs"]
mod cases;
