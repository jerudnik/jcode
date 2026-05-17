//! Thin wrapper around the `open` crate for opening URLs and files in the
//! user's default handler (browser, file viewer, etc).
//!
//! Centralizing these calls lets us:
//!
//! 1. Disable browser-opening globally via `JCODE_DISABLE_BROWSER_OPEN=1`
//!    (the test harness sets this by default in `TestJcodeHome::acquire`).
//! 2. Audit every place jcode shells out to the OS opener.
//! 3. Optionally swap in a different backend in the future without touching
//!    every call site.
//!
//! The env-var guard is checked at call time (not at process start), so a
//! test scope that wants to verify the real open path can clear the var
//! within its guard.

use std::ffi::OsStr;
use std::io;

/// Environment variable that, when set to a non-empty value, causes every
/// `open_url` / `open_path` call to become a no-op success. The standard
/// test harness (`TestJcodeHome::acquire`) sets this to `1` so that test
/// runs cannot accidentally pop browser windows for OAuth flows when a
/// pristine `JCODE_HOME` lacks cached credentials.
pub const DISABLE_ENV_VAR: &str = "JCODE_DISABLE_BROWSER_OPEN";

fn is_disabled() -> bool {
    std::env::var_os(DISABLE_ENV_VAR)
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false)
}

/// Open a URL in the user's default browser. Returns the underlying
/// `io::Result` so callers can decide how to surface failures (most
/// production sites do `.is_ok()` or `let _ =`).
///
/// If `JCODE_DISABLE_BROWSER_OPEN` is set, returns `Ok(())` without
/// invoking the OS opener.
pub fn open_url<S: AsRef<OsStr>>(url: S) -> io::Result<()> {
    if is_disabled() {
        return Ok(());
    }
    open::that(url)
}

/// Open a URL or file path in a detached child process (does not wait
/// for the opener to exit). Matches the semantics of `open::that_detached`.
///
/// If `JCODE_DISABLE_BROWSER_OPEN` is set, returns `Ok(())` without
/// invoking the OS opener.
pub fn open_detached<S: AsRef<OsStr>>(target: S) -> io::Result<()> {
    if is_disabled() {
        return Ok(());
    }
    open::that_detached(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: with the disable var set, `open_url` reports success
    /// without trying to spawn `/usr/bin/open` (which would fail in CI
    /// or hang on a missing display).
    #[test]
    fn open_url_is_noop_when_disabled() {
        // Use a scoped env mutation so we don't pollute sibling tests.
        let _guard = crate::storage::lock_test_env();
        let prev = std::env::var_os(DISABLE_ENV_VAR);
        crate::env::set_var(DISABLE_ENV_VAR, "1");

        let result = open_url("https://example.invalid/should-not-open");

        if let Some(prev) = prev {
            crate::env::set_var(DISABLE_ENV_VAR, prev);
        } else {
            crate::env::remove_var(DISABLE_ENV_VAR);
        }

        assert!(result.is_ok(), "open_url should succeed silently when disabled");
    }

    #[test]
    fn open_detached_is_noop_when_disabled() {
        let _guard = crate::storage::lock_test_env();
        let prev = std::env::var_os(DISABLE_ENV_VAR);
        crate::env::set_var(DISABLE_ENV_VAR, "1");

        let result = open_detached("https://example.invalid/should-not-open");

        if let Some(prev) = prev {
            crate::env::set_var(DISABLE_ENV_VAR, prev);
        } else {
            crate::env::remove_var(DISABLE_ENV_VAR);
        }

        assert!(result.is_ok());
    }

    #[test]
    fn empty_or_zero_does_not_disable() {
        let _guard = crate::storage::lock_test_env();
        let prev = std::env::var_os(DISABLE_ENV_VAR);

        crate::env::set_var(DISABLE_ENV_VAR, "");
        assert!(!is_disabled(), "empty value should not disable");

        crate::env::set_var(DISABLE_ENV_VAR, "0");
        assert!(!is_disabled(), "literal 0 should not disable");

        crate::env::set_var(DISABLE_ENV_VAR, "1");
        assert!(is_disabled(), "1 should disable");

        if let Some(prev) = prev {
            crate::env::set_var(DISABLE_ENV_VAR, prev);
        } else {
            crate::env::remove_var(DISABLE_ENV_VAR);
        }
    }
}
