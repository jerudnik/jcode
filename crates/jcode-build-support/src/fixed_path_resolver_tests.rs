//! F20b resolver tests: the single fixed reload target (~/.jcode/current/
//! jcode) must win ahead of the legacy channels for both the client and the
//! daemon. Kept in a dedicated file so tests.rs stays under the test-size
//! budget.
#![cfg(test)]

use super::*;

// Serialize JCODE_HOME mutation across the process (same discipline as the
// tests.rs env lock). A dedicated lock is fine: these tests only race each
// other on the env var, and correctness comes from holding it per test.
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn with_temp_home<T>(f: impl FnOnce() -> T) -> T {
    let _guard = env_lock();
    let temp = tempfile::tempdir().expect("tempdir");
    let prev = std::env::var_os("JCODE_HOME");
    jcode_core::env::set_var("JCODE_HOME", temp.path());
    let result = f();
    match prev {
        Some(prev) => jcode_core::env::set_var("JCODE_HOME", prev),
        None => jcode_core::env::remove_var("JCODE_HOME"),
    }
    result
}

#[test]
fn client_update_candidate_prefers_fixed_path_over_channels() {
    // F20b: the single fixed reload target (~/.jcode/current/jcode) is the
    // source of truth and must win ahead of the legacy `current` channel.
    with_temp_home(|| {
        // Publish both a legacy `current` channel AND the fixed path.
        let version = "legacy-current";
        install_binary_at_version(std::env::current_exe().as_ref().unwrap(), version)
            .expect("install version");
        update_current_symlink(version).expect("update current symlink");
        let fixed = publish_current_fixed(std::env::current_exe().as_ref().unwrap())
            .expect("publish fixed");

        let candidate = client_update_candidate(true).expect("expected a candidate");
        assert_eq!(candidate.1, "current-fixed", "fixed path must win");
        assert_eq!(
            std::fs::canonicalize(candidate.0).expect("canonical candidate"),
            std::fs::canonicalize(&fixed).expect("canonical fixed"),
        );
    });
}

#[test]
fn shared_server_candidate_prefers_fixed_path_over_channels() {
    // F20b: routing the daemon too. shared_server_update_candidate must prefer
    // the fixed path ahead of the shared-server/stable channels.
    with_temp_home(|| {
        let version = "legacy-shared";
        install_binary_at_version(std::env::current_exe().as_ref().unwrap(), version)
            .expect("install version");
        update_shared_server_symlink(version).expect("update shared-server symlink");
        let fixed = publish_current_fixed(std::env::current_exe().as_ref().unwrap())
            .expect("publish fixed");

        let candidate = shared_server_update_candidate(true).expect("expected a candidate");
        assert_eq!(
            candidate.1, "current-fixed",
            "fixed path must win for daemon"
        );
        assert_eq!(
            std::fs::canonicalize(candidate.0).expect("canonical candidate"),
            std::fs::canonicalize(&fixed).expect("canonical fixed"),
        );
    });
}
