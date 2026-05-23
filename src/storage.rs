#![cfg_attr(test, allow(clippy::items_after_test_module))]

pub use jcode_storage::*;

use anyhow::Result;
use serde::de::DeserializeOwned;
use std::path::Path;

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    jcode_storage::read_json_with_recovery_handler(path, |event| match event {
        jcode_storage::StorageRecoveryEvent::CorruptPrimary { path, error } => {
            crate::logging::warn(&format!(
                "Corrupt JSON at {}, trying backup: {}",
                path.display(),
                error
            ));
        }
        jcode_storage::StorageRecoveryEvent::RecoveredFromBackup { backup_path } => {
            crate::logging::info(&format!("Recovered from backup: {}", backup_path.display()));
        }
    })
}

#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Ambient environment variables scrubbed once at first
/// `lock_test_env()` so that dev shells with real credentials cannot
/// pollute tests that read provider auth via `std::env::var`.
///
/// Order matters only insofar as adding a new var here is a "default
/// deny" — tests that need the var must set + restore it explicitly.
/// Provider-config env vars (e.g. `JCODE_ACTIVE_PROVIDER`,
/// `JCODE_SOCKET`, `JCODE_NON_INTERACTIVE`) are scrubbed here too so
/// the dev shell's ambient `selfdev` / `non_interactive` state doesn't
/// flip TUI / dispatch code paths under test.
#[cfg(test)]
const TEST_ENV_SCRUB_LIST: &[&str] = &[
    // OpenAI / Codex
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    // Anthropic
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_BASE_URL",
    // OpenRouter
    "OPENROUTER_API_KEY",
    "OPENROUTER_BASE_URL",
    // Google / Gemini
    "GOOGLE_API_KEY",
    "GOOGLE_GENERATIVE_AI_API_KEY",
    "GEMINI_API_KEY",
    // GitHub (Copilot, etc.)
    "GITHUB_TOKEN",
    "GH_TOKEN",
    // AWS / Bedrock
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_REGION",
    "AWS_DEFAULT_REGION",
    "AWS_PROFILE",
    // Azure
    "AZURE_OPENAI_API_KEY",
    "AZURE_OPENAI_ENDPOINT",
    // DeepSeek / Together / etc. (generic catch)
    "DEEPSEEK_API_KEY",
    "TOGETHER_API_KEY",
    "GROQ_API_KEY",
    "MISTRAL_API_KEY",
    "PERPLEXITY_API_KEY",
    "FIREWORKS_API_KEY",
    "XAI_API_KEY",
    // Jcode runtime/dispatch state that should not leak from a live
    // self-dev shell into unit tests.
    "JCODE_SOCKET",
    "JCODE_NON_INTERACTIVE",
    "JCODE_ACTIVE_PROVIDER",
    "JCODE_ACTIVE_MODEL",
    "JCODE_FORCE_HEADLESS",
];

#[cfg(test)]
pub(crate) fn test_env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| {
        // Process-wide default-deny for side-effecting OS interactions
        // during cargo test runs. Set BEFORE any test can race past us;
        // the `OnceLock` initializer is guaranteed to run exactly once
        // before any concurrent test acquires the lock.
        //
        // Tests that genuinely want to exercise the real spawn path
        // (e.g. `cli::tui_launch::tests::spawn_*_uses_handterm_exec_mode`)
        // opt back in by setting the relevant
        // `JCODE_TEST_ALLOW_{BROWSER_OPEN,TERMINAL_SPAWN}` var within
        // their scope while holding `lock_test_env()`.
        //
        // Note: we don't gate on whether the var is already set; tests
        // that want to override must do so explicitly via the opt-out
        // pattern, not by relying on ambient env.
        crate::env::set_var(crate::browser_open::DISABLE_ENV_VAR, "1");
        crate::env::set_var(crate::terminal_launch::DISABLE_ENV_VAR, "1");

        // Scrub ambient developer / CI credentials that would otherwise
        // leak into tests that read them via `std::env::var` (e.g.
        // `auth::codex::load_credentials`, OpenRouter provider auto-detect,
        // AWS Bedrock detect, etc.). Tests that need specific provider
        // credentials must set them explicitly under `lock_test_env()`.
        //
        // We scrub at OnceLock-init time so it happens exactly once,
        // before any test thread races past us. Individual tests that
        // legitimately need one of these (rare; should use a TestJcodeHome
        // + EnvVarGuard) can set + restore it within their scope.
        for var in TEST_ENV_SCRUB_LIST {
            crate::env::remove_var(var);
        }

        Mutex::new(())
    })
}

#[cfg(test)]
thread_local! {
    /// Re-entry depth for `lock_test_env` on the current thread. While > 0,
    /// nested calls return a sentinel guard that does not actually re-lock
    /// the mutex, preventing self-deadlock when helpers like
    /// `TestJcodeHome::acquire()` are nested inside callers that already
    /// hold `lock_test_env()`.
    static TEST_ENV_LOCK_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// RAII guard returned by [`lock_test_env`]. Wraps either a real
/// `MutexGuard<'static, ()>` (top-level acquisition) or a sentinel
/// (re-entrant acquisition on the same thread).
#[cfg(test)]
pub(crate) struct TestEnvLockGuard {
    _inner: TestEnvLockGuardInner,
}

#[cfg(test)]
#[allow(dead_code)] // Real guard is held for its Drop side effect; the field is intentionally never read.
enum TestEnvLockGuardInner {
    Real(MutexGuard<'static, ()>),
    Reentrant,
}

#[cfg(test)]
impl Drop for TestEnvLockGuard {
    fn drop(&mut self) {
        TEST_ENV_LOCK_DEPTH.with(|d| {
            let cur = d.get();
            debug_assert!(cur > 0, "TestEnvLockGuard dropped with depth=0");
            d.set(cur.saturating_sub(1));
        });
        // _inner drops naturally; if Real, releases the underlying mutex.
    }
}

#[cfg(test)]
pub(crate) fn lock_test_env() -> TestEnvLockGuard {
    let inner = TEST_ENV_LOCK_DEPTH.with(|d| {
        let cur = d.get();
        d.set(cur + 1);
        if cur > 0 {
            TestEnvLockGuardInner::Reentrant
        } else {
            let guard = test_env_lock()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            TestEnvLockGuardInner::Real(guard)
        }
    });
    TestEnvLockGuard { _inner: inner }
}

/// RAII guard providing an isolated `JCODE_HOME` for the duration of a test.
///
/// Tests historically shared a single `/tmp/jcode-test-home-<pid>` directory and
/// invalidated persisted UI state files (`ambient/queue.json`, `state.json`,
/// `directives.json`, `visible_cycle.json`) up-front, which races badly under
/// parallel `cargo test` execution. This guard gives every test its own
/// tempdir behind the global [`test_env_lock`] so tests no longer stomp on
/// each other's persisted state.
///
/// Nesting semantics: if `JCODE_HOME` is already set when the guard is
/// constructed, the guard inherits the outer environment as a passthrough
/// (no lock, no tempdir, no env restoration on drop). This preserves the
/// existing pattern where `with_temp_jcode_home(|| { create_test_app(); })`
/// works without re-entering the global mutex.
#[cfg(test)]
pub(crate) struct TestJcodeHome {
    inner: Option<TestJcodeHomeInner>,
}

#[cfg(test)]
struct TestJcodeHomeInner {
    _guard: TestEnvLockGuard,
    _temp: tempfile::TempDir,
    prev_home: Option<std::ffi::OsString>,
    prev_disable_browser: Option<std::ffi::OsString>,
    prev_disable_terminal_spawn: Option<std::ffi::OsString>,
}

#[cfg(test)]
impl TestJcodeHome {
    /// Acquire an isolated `JCODE_HOME` for the lifetime of the returned guard.
    ///
    /// If `JCODE_HOME` is already set (by an outer guard or a closure-based
    /// helper like `with_temp_jcode_home`), this is a no-op passthrough.
    ///
    /// Also sets two safety guards for the duration of the guard:
    ///
    /// * `JCODE_DISABLE_BROWSER_OPEN=1` makes
    ///   `crate::browser_open::{open_url,open_detached}` a no-op success,
    ///   so OAuth flows in a pristine `JCODE_HOME` (no cached creds)
    ///   cannot pop real browser windows.
    /// * `JCODE_DISABLE_TERMINAL_SPAWN=1` makes
    ///   `crate::terminal_launch::spawn_command_in_new_terminal` return
    ///   `Ok(false)` without spawning, so TUI flows like `/transfer`,
    ///   `/resume`, or self-dev handoff cannot pop real Ghostty / iTerm
    ///   / Terminal.app windows during test runs.
    ///
    /// Both prior values are saved and restored on drop. Tests that
    /// genuinely want to exercise either path can clear the relevant
    /// var within their scope (see `cli::tui_launch::tests` for the
    /// `spawn_*_in_new_terminal_uses_handterm_exec_mode` pattern).
    pub(crate) fn acquire() -> Self {
        if std::env::var_os("JCODE_HOME").is_some() {
            return Self { inner: None };
        }

        let guard = lock_test_env();
        // Re-check inside the lock: another thread may have set JCODE_HOME
        // between our early-return check and acquiring the mutex. If so,
        // release the lock and act as a passthrough.
        if std::env::var_os("JCODE_HOME").is_some() {
            drop(guard);
            return Self { inner: None };
        }

        let temp = tempfile::tempdir().expect("create test JCODE_HOME tempdir");
        let prev_home = std::env::var_os("JCODE_HOME");
        let prev_disable_browser = std::env::var_os(crate::browser_open::DISABLE_ENV_VAR);
        let prev_disable_terminal_spawn = std::env::var_os(crate::terminal_launch::DISABLE_ENV_VAR);
        crate::env::set_var("JCODE_HOME", temp.path());
        crate::env::set_var(crate::browser_open::DISABLE_ENV_VAR, "1");
        crate::env::set_var(crate::terminal_launch::DISABLE_ENV_VAR, "1");

        Self {
            inner: Some(TestJcodeHomeInner {
                _guard: guard,
                _temp: temp,
                prev_home,
                prev_disable_browser,
                prev_disable_terminal_spawn,
            }),
        }
    }

    /// Returns true if this guard owns the `JCODE_HOME` (vs. inheriting from
    /// an outer scope). Useful for tests that need to know whether they can
    /// safely mutate global state without disturbing a parent.
    #[allow(dead_code)]
    pub(crate) fn owns_home(&self) -> bool {
        self.inner.is_some()
    }
}

#[cfg(test)]
impl Drop for TestJcodeHome {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            if let Some(prev) = &inner.prev_home {
                crate::env::set_var("JCODE_HOME", prev);
            } else {
                crate::env::remove_var("JCODE_HOME");
            }
            if let Some(prev) = &inner.prev_disable_browser {
                crate::env::set_var(crate::browser_open::DISABLE_ENV_VAR, prev);
            } else {
                crate::env::remove_var(crate::browser_open::DISABLE_ENV_VAR);
            }
            if let Some(prev) = &inner.prev_disable_terminal_spawn {
                crate::env::set_var(crate::terminal_launch::DISABLE_ENV_VAR, prev);
            } else {
                crate::env::remove_var(crate::terminal_launch::DISABLE_ENV_VAR);
            }
            // _temp and _guard are dropped here, in that order.
        }
    }
}

#[cfg(test)]
mod tests;
