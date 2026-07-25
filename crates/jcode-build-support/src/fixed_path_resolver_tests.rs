//! F20b/F20c resolver tests: the single fixed reload target
//! (`~/.jcode/current/jcode`) is the one thing both the client and the daemon
//! resolve to, and it is exactly where the real publish primitive writes.
//!
//! F20b introduced the fixed path alongside the legacy channels; F20c deleted
//! those channels, so what remains to prove is the end-to-end agreement between
//! [`publish_current_fixed`] (the writer) and the two resolvers (the readers).
//! Kept in a dedicated file so tests.rs stays under the test-size budget. Uses
//! the SAME process-global env lock as tests.rs (via
//! `super::tests::with_temp_jcode_home`) so JCODE_HOME mutation is serialized
//! across ALL home-dependent tests in this crate under multithreaded runs.
#![cfg(test)]

use super::tests::with_temp_jcode_home;
use super::*;

#[test]
fn the_real_publish_primitive_lands_where_both_resolvers_look() {
    with_temp_jcode_home(|| {
        let published = publish_current_fixed(std::env::current_exe().as_ref().unwrap())
            .expect("publish fixed");
        let canonical = std::fs::canonicalize(&published).expect("canonical fixed");

        for is_selfdev in [false, true] {
            let (client, client_label) =
                client_update_candidate(is_selfdev).expect("client candidate");
            assert_eq!(client_label, "current-fixed");
            assert_eq!(
                std::fs::canonicalize(client).expect("canonical client"),
                canonical,
                "client must read the path publish_current_fixed writes (is_selfdev={is_selfdev})"
            );

            let (server, server_label) =
                shared_server_update_candidate(is_selfdev).expect("daemon candidate");
            assert_eq!(server_label, "current-fixed");
            assert_eq!(
                std::fs::canonicalize(server).expect("canonical server"),
                canonical,
                "daemon must read the path publish_current_fixed writes (is_selfdev={is_selfdev})"
            );
        }
    });
}

#[test]
fn republishing_replaces_the_binary_in_place_at_the_same_fixed_path() {
    // There is exactly one publish target, so a second publish must overwrite
    // the first rather than creating a second addressable build.
    with_temp_jcode_home(|| {
        let first = publish_current_fixed(std::env::current_exe().as_ref().unwrap())
            .expect("first publish");
        let second = publish_current_fixed(std::env::current_exe().as_ref().unwrap())
            .expect("second publish");

        assert_eq!(first, second);
        assert_eq!(
            first,
            current_fixed_binary_path().expect("fixed binary path")
        );
    });
}
