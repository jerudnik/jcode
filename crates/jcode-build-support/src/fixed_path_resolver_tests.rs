//! F20b resolver tests: the single fixed reload target (~/.jcode/current/
//! jcode) must win ahead of the legacy channels for both the client and the
//! daemon. Kept in a dedicated file so tests.rs stays under the test-size
//! budget. Uses the SAME process-global env lock as tests.rs (via
//! `super::tests::with_temp_jcode_home`) so JCODE_HOME mutation is serialized
//! across ALL home-dependent tests in this crate under multithreaded runs.
#![cfg(test)]

use super::tests::with_temp_jcode_home;
use super::*;

#[test]
fn client_update_candidate_prefers_fixed_path_over_channels() {
    // F20b: the single fixed reload target (~/.jcode/current/jcode) is the
    // source of truth and must win ahead of the legacy `current` channel.
    with_temp_jcode_home(|| {
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
    with_temp_jcode_home(|| {
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
