#![cfg(test)]

//! Shared, scoped support for TUI app tests that touch process-global state.
//!
//! `JCODE_HOME`, authentication state, configuration caches, and several render
//! caches are process-global. App tests retain a shared read lease for their
//! lifetime. Tests that mutate the environment take an exclusive write lease.

use super::App;
use crate::provider::Provider;
use crate::tool::Registry;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

const EXPLICIT_SCOPED_ENV_KEYS: &[&str] = &[
    "HOME",
    "XDG_CONFIG_HOME",
    "HANDTERM_NATIVE_SCROLL_SOCKET",
    "AZURE_OPENAI_USE_ENTRA",
    "AWS_BEARER_TOKEN_BEDROCK",
    "COMPOSIO_GMAIL_CONNECTED_ACCOUNT_ID",
    "COMPOSIO_GMAIL_USER_ID",
    "COMPOSIO_USER_ID",
    "COMPOSIO_GMAIL_AUTH_CONFIG_ID",
    "JADE_RELAY_TOKEN_ID",
    "JADE_RELAY_SESSION_ID",
    "JADE_RELAY_USER_ID",
    "PI_OPENAI_KEY",
];

const SCOPED_PROVIDER_ENV_SUFFIXES: &[&str] = &[
    "_API_KEY",
    "_TOKEN",
    "_SECRET",
    "_BASE_URL",
    "_API_BASE",
    "_API_ENDPOINT",
    "_ENDPOINT",
    "_CLIENT_ID",
    "_CLIENT_SECRET",
    "_PROJECT",
    "_PROJECT_ID",
    "_REGION",
    "_PROFILE",
    "_API_VERSION",
    "_DEPLOYMENT",
    "_MODEL",
    "_CREDENTIALS",
    "_CREDENTIALS_FILE",
    "_CONFIG_FILE",
    "_TOKEN_FILE",
    "_RELATIVE_URI",
    "_FULL_URI",
    "_ACCESS_KEY_ID",
    "_SECRET_ACCESS_KEY",
];

fn test_env_scope_clears_key(key: &OsStr) -> bool {
    let Some(key) = key.to_str() else {
        return false;
    };

    if matches!(
        key,
        "PATH" | "TMPDIR" | "NIX_PATH" | "NIX_PROFILE" | "IN_NIX_SHELL"
    ) || key.starts_with("NIX_")
        || key.starts_with("CARGO_")
        || key.starts_with("RUST")
    {
        return false;
    }

    key.starts_with("JCODE_")
        || SCOPED_PROVIDER_ENV_SUFFIXES
            .iter()
            .any(|suffix| key.ends_with(suffix))
        || EXPLICIT_SCOPED_ENV_KEYS.contains(&key)
}

/// Acquire the exclusive environment lease for a test that mutates process
/// environment variables or their dependent caches.
pub(crate) fn lock_test_env() -> TestEnvWriteScope {
    TestEnvWriteScope::new()
}

/// Lock the process-global renderer along with the environment that configures
/// it. Render tests must use this instead of private per-module locks because
/// layout snapshots, Mermaid state, and frame histories are shared across the
/// entire TUI test crate.
///
/// The environment lease is intentionally acquired first, preserving the
/// global `environment -> renderer` lock order.
pub(crate) fn lock_test_render_state() -> TestRenderScope {
    static RENDER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    let env_lock = lock_test_env();
    reset_tui_test_globals();
    let render_lock = RENDER_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    TestRenderScope {
        _render: render_lock,
        _env: env_lock,
    }
}

/// Scoped exclusive render access for tests that use process-global layout or
/// Mermaid state. Fields drop in declaration order, releasing the renderer
/// before the environment lease.
pub(crate) struct TestRenderScope {
    _render: MutexGuard<'static, ()>,
    _env: TestEnvWriteScope,
}

thread_local! {
    static TEST_CLIPBOARD_RESULT: Cell<Option<bool>> = const { Cell::new(None) };
}

pub(crate) struct TestClipboardScope {
    previous: Option<bool>,
}

pub(crate) fn scoped_test_clipboard_result(result: bool) -> TestClipboardScope {
    let previous = TEST_CLIPBOARD_RESULT.with(|slot| slot.replace(Some(result)));
    TestClipboardScope { previous }
}

pub(crate) fn test_clipboard_result() -> Option<bool> {
    TEST_CLIPBOARD_RESULT.with(Cell::get)
}

impl Drop for TestClipboardScope {
    fn drop(&mut self) {
        TEST_CLIPBOARD_RESULT.with(|slot| slot.set(self.previous));
    }
}

/// Thread-bound TUI writer scope.
pub(crate) struct TestEnvWriteScope {
    _lease: crate::storage::TestEnvWriteLease,
}

impl TestEnvWriteScope {
    fn new() -> Self {
        let lease = crate::storage::lock_test_env_write();
        Self { _lease: lease }
    }
}

/// Acquire the shared environment lease retained by an App that only reads
/// environment-derived configuration and auth state.
pub(crate) fn lock_test_env_read() -> crate::storage::TestEnvReadLease {
    crate::storage::lock_test_env_read()
}

/// Acquire the transferable environment lease retained by a test App. Under a
/// same-thread writer this becomes a child lease that safely keeps exclusive
/// exclusion alive if the App escapes or is moved into background work.
pub(crate) fn test_app_env_lease() -> Option<crate::storage::TestEnvFixtureLease> {
    Some(crate::storage::lock_test_env_fixture())
}

/// Reset global caches whose contents depend on process-global environment
/// state. Call only while holding the exclusive environment write lease.
pub(crate) fn reset_tui_test_globals() {
    // Force synchronous mermaid rendering for the whole test process: the
    // detached deferred worker otherwise calls register_active_diagram from a
    // background thread that can run after a render test resets ACTIVE_DIAGRAMS,
    // polluting the next test regardless of --test-threads.
    crate::tui::mermaid::set_synchronous_render_mode(true);
    crate::config::invalidate_config_cache();
    crate::auth::claude::set_active_account_override(None);
    crate::auth::codex::set_active_account_override(None);
    crate::auth::AuthStatus::invalidate_cache();
    crate::tui::app::helpers::clear_ambient_info_cache_for_tests();
    crate::tui::app::helpers::clear_todos_cache_for_tests();
    crate::tui::ui::clear_test_render_state_for_tests();
    crate::tui::ui::clear_slow_frame_history_for_tests();
    crate::tui::ui::clear_flicker_frame_history_for_tests();
    crate::tui::clear_side_panel_render_caches();
    crate::tui::theme_detect::reset_detected_theme_for_tests();
    crate::tui::mermaid::clear_active_diagrams();
    crate::tui::mermaid::clear_streaming_preview_diagram();
    crate::tui::mermaid::clear_image_state();
    crate::tui::info_widget::clear_widget_placements_for_tests();
}

/// Construct a test app with environment exclusion retained for the returned
/// App's full lifetime. Under an enclosing temporary-home writer, the App keeps
/// a transferable writer-child lease rather than a shared read lease.
pub(crate) fn create_test_app_with(
    provider: Arc<dyn Provider>,
    configure: impl FnOnce(&mut App),
) -> App {
    let env_lock = test_app_env_lease();
    // Test workers are reused. A render test that finishes mid tail-catchup can
    // otherwise leave this thread-local animation flag set for the next app
    // fixture, making an idle app appear to have live redraw work.
    crate::tui::ui::set_tail_catchup_active(false);
    // Force synchronous mermaid rendering so the detached deferred worker never
    // registers a diagram after a sibling test resets ACTIVE_DIAGRAMS. Set here
    // too (not only in the render lock) so render tests that build an app
    // without the render lock are covered.
    crate::tui::mermaid::set_synchronous_render_mode(true);

    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    let registry = runtime.block_on(Registry::new(provider.clone()));
    let mut app = App::new_for_test_harness(provider, registry);
    configure(&mut app);
    app._test_env_lock = env_lock;
    app
}

/// Run a closure with a fresh, isolated JCODE_HOME while retaining the shared
/// write lease. Scoped configuration is cleared on entry, and the complete
/// process environment is restored even when the closure panics.
pub(crate) fn with_temp_jcode_home<T>(f: impl FnOnce() -> T) -> T {
    let _scope = TestEnvScope::new();
    f()
}

pub(crate) struct TestEnvScope {
    _env_lock: TestEnvWriteScope,
    _temp: tempfile::TempDir,
    saved_env: BTreeMap<OsString, OsString>,
}

impl TestEnvScope {
    pub(crate) fn new() -> Self {
        let env_lock = lock_test_env();
        let saved_env = std::env::vars_os().collect();
        let temp = tempfile::tempdir().expect("temp JCODE_HOME");

        // Construct the guard before mutating globals so an unexpected panic
        // during reset still restores the original environment and depth.
        let scope = Self {
            _env_lock: env_lock,
            _temp: temp,
            saved_env,
        };

        let current_keys: Vec<OsString> = std::env::vars_os().map(|(key, _)| key).collect();
        for key in current_keys {
            if key != "HOME" && key != "JCODE_HOME" && test_env_scope_clears_key(&key) {
                crate::env::remove_var(&key);
            }
        }
        crate::env::set_var("HOME", scope._temp.path());
        crate::env::set_var("JCODE_HOME", scope._temp.path());
        reset_tui_test_globals();
        scope
    }
}

impl Drop for TestEnvScope {
    fn drop(&mut self) {
        let current_env: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
        for key in current_env.keys() {
            if !self.saved_env.contains_key(key) {
                crate::env::remove_var(key);
            }
        }

        for (key, value) in &self.saved_env {
            if current_env.get(key) != Some(value) {
                crate::env::set_var(key, value);
            }
        }
        reset_tui_test_globals();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopProvider;

    #[async_trait::async_trait]
    impl Provider for NoopProvider {
        async fn complete(
            &self,
            _messages: &[crate::message::Message],
            _tools: &[crate::message::ToolDefinition],
            _system: &str,
            _resume_session_id: Option<&str>,
        ) -> anyhow::Result<crate::provider::EventStream> {
            Err(anyhow::anyhow!(
                "NoopProvider should not stream completions in test_support tests"
            ))
        }

        fn name(&self) -> &str {
            "noop"
        }

        fn fork(&self) -> Arc<dyn Provider> {
            Arc::new(Self)
        }
    }
    #[test]
    fn temp_jcode_home_clears_and_restores_explicit_non_prefix_environment() {
        assert!(EXPLICIT_SCOPED_ENV_KEYS.contains(&"HOME"));
        for &key in EXPLICIT_SCOPED_ENV_KEYS {
            assert!(test_env_scope_clears_key(OsStr::new(key)), "{key}");
        }

        let outer_scope = TestEnvScope::new();
        for &key in EXPLICIT_SCOPED_ENV_KEYS
            .iter()
            .filter(|&&key| key != "HOME")
        {
            crate::env::set_var(key, "outer-sentinel");
        }

        with_temp_jcode_home(|| {
            for &key in EXPLICIT_SCOPED_ENV_KEYS
                .iter()
                .filter(|&&key| key != "HOME")
            {
                assert_eq!(std::env::var_os(key), None, "{key} was not cleared");
            }
        });

        for &key in EXPLICIT_SCOPED_ENV_KEYS
            .iter()
            .filter(|&&key| key != "HOME")
        {
            assert_eq!(
                std::env::var(key).as_deref(),
                Ok("outer-sentinel"),
                "{key} was not restored"
            );
        }
        drop(outer_scope);
    }

    #[test]
    fn temp_jcode_home_clears_provider_environment_and_preserves_build_environment() {
        const PROVIDER_KEYS: &[&str] = &[
            "JCODE_OPENROUTER_MAX_TOKENS",
            "JCODE_OPENROUTER_THINKING",
            "JCODE_OPENAI_EXTRA_BODY",
            "JCODE_TEST_DYNAMIC_PREFIX",
            "F17_TEST_DYNAMIC_API_KEY",
            "OPENAI_BASE_URL",
            "OPENAI_API_BASE",
            "GEMINI_API_ENDPOINT",
            "GEMINI_CLIENT_ID",
            "GEMINI_CLIENT_SECRET",
            "GOOGLE_CLOUD_PROJECT",
            "GOOGLE_CLOUD_PROJECT_ID",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "COMPOSIO_BASE_URL",
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_PROFILE",
            "AWS_REGION",
            "AWS_WEB_IDENTITY_TOKEN_FILE",
            "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
            "AWS_CONTAINER_CREDENTIALS_FULL_URI",
            "AWS_SHARED_CREDENTIALS_FILE",
            "AWS_CONFIG_FILE",
            "CURSOR_ACCESS_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CODE_ASSIST_API_VERSION",
            "AZURE_OPENAI_DEPLOYMENT",
            "AZURE_OPENAI_MODEL",
        ];
        const BUILD_KEYS: &[&str] = &[
            "PATH",
            "TMPDIR",
            "NIX_PATH",
            "NIX_PROFILE",
            "NIX_SSL_CERT_FILE",
            "NIX_REMOTE",
            "IN_NIX_SHELL",
            "CARGO_HOME",
            "CARGO_TARGET_DIR",
            "CARGO_REGISTRIES_CRATES_IO_TOKEN",
            "RUST_LOG",
            "RUST_BACKTRACE",
            "RUSTFLAGS",
            "RUSTUP_HOME",
            "RUSTC_WRAPPER",
            "SSL_CERT_FILE",
        ];
        const VISIBLE_BUILD_KEYS: &[&str] = &[
            "NIX_REMOTE",
            "CARGO_REGISTRIES_CRATES_IO_TOKEN",
            "RUST_LOG",
            "SSL_CERT_FILE",
        ];

        for &key in PROVIDER_KEYS {
            assert!(!EXPLICIT_SCOPED_ENV_KEYS.contains(&key), "{key}");
            assert!(test_env_scope_clears_key(OsStr::new(key)), "{key}");
        }
        for &key in BUILD_KEYS {
            assert!(!EXPLICIT_SCOPED_ENV_KEYS.contains(&key), "{key}");
            assert!(!test_env_scope_clears_key(OsStr::new(key)), "{key}");
        }

        let outer_scope = TestEnvScope::new();
        for &key in PROVIDER_KEYS.iter().chain(VISIBLE_BUILD_KEYS) {
            crate::env::set_var(key, "outer-sentinel");
        }

        with_temp_jcode_home(|| {
            for &key in PROVIDER_KEYS {
                assert_eq!(std::env::var_os(key), None, "{key} was not cleared");
            }
            for &key in VISIBLE_BUILD_KEYS {
                assert_eq!(
                    std::env::var(key).as_deref(),
                    Ok("outer-sentinel"),
                    "{key} was unexpectedly cleared"
                );
            }
        });

        for &key in PROVIDER_KEYS.iter().chain(VISIBLE_BUILD_KEYS) {
            assert_eq!(
                std::env::var(key).as_deref(),
                Ok("outer-sentinel"),
                "{key} was not restored"
            );
        }
        drop(outer_scope);
    }

    #[test]
    fn temp_jcode_home_leaves_unrelated_unlisted_environment_visible_then_restores_snapshot() {
        const RESTORED_KEY: &str = "F17_TEST_UNRELATED_VISIBLE";
        const REMOVED_KEY: &str = "F17_TEST_UNRELATED_CREATED";

        assert!(!EXPLICIT_SCOPED_ENV_KEYS.contains(&RESTORED_KEY));
        assert!(!EXPLICIT_SCOPED_ENV_KEYS.contains(&REMOVED_KEY));
        assert!(!test_env_scope_clears_key(OsStr::new(RESTORED_KEY)));
        assert!(!test_env_scope_clears_key(OsStr::new(REMOVED_KEY)));

        let outer_scope = TestEnvScope::new();
        crate::env::set_var(RESTORED_KEY, "outer-sentinel");
        crate::env::remove_var(REMOVED_KEY);

        with_temp_jcode_home(|| {
            assert_eq!(std::env::var(RESTORED_KEY).as_deref(), Ok("outer-sentinel"));
            assert_eq!(std::env::var_os(REMOVED_KEY), None);

            crate::env::set_var(RESTORED_KEY, "inner-overwrite");
            crate::env::set_var(REMOVED_KEY, "inner-created");

            assert_eq!(
                std::env::var(RESTORED_KEY).as_deref(),
                Ok("inner-overwrite")
            );
            assert_eq!(std::env::var(REMOVED_KEY).as_deref(), Ok("inner-created"));
        });

        assert_eq!(std::env::var(RESTORED_KEY).as_deref(), Ok("outer-sentinel"));
        assert_eq!(std::env::var_os(REMOVED_KEY), None);
        drop(outer_scope);
    }

    #[test]
    fn create_test_app_with_does_not_register_active_pid_marker() {
        with_temp_jcode_home(|| {
            let app = create_test_app_with(Arc::new(NoopProvider), |_| {});
            let jcode_home = crate::storage::jcode_dir().expect("temporary JCODE_HOME path");
            let active_pids_dir =
                crate::storage::active_pids_dir().expect("active PID marker directory path");
            let marker_path = active_pids_dir.join(&app.session.id);

            assert!(
                !marker_path.exists(),
                "test harness app unexpectedly created active PID marker at {}",
                marker_path.display()
            );
            assert!(
                !crate::storage::active_session_ids().contains(&app.session.id),
                "test harness session {} leaked through active_session_ids",
                app.session.id
            );
            assert!(
                !jcode_home.join(".pid-markers.lock").exists(),
                "test harness app unexpectedly touched the PID marker lock file"
            );
            assert!(
                !active_pids_dir.exists(),
                "test harness app unexpectedly created the active PID marker directory"
            );
        });
    }
}
