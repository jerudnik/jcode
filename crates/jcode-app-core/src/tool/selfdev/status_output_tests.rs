//! Status-output pruning and sidecar-reporting tests, split from tests.rs
//! to keep it under the test-size ratchet threshold.

use super::tests::{EnvVarGuard, lock_env, test_source_state};
use super::*;

#[test]
fn status_output_prunes_stale_pending_requests() {
    let _lock = lock_env();
    let temp_home = crate::storage::RuntimePaths::test_root("jcode-selfdev-home-");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());

    let mut session = session::Session::create(None, Some("Stale Build".to_string()));
    session.short_name = Some("ghost".to_string());
    session.save().expect("save session");

    let stale_status_path = temp_home.path().join("missing-selfdev.status.json");
    let source = test_source_state(std::path::Path::new("/tmp/jcode"));
    let request = BuildRequest {
        request_id: "stale-request".to_string(),
        background_task_id: Some("missing-task".to_string()),
        session_id: session.id.clone(),
        session_short_name: session.short_name.clone(),
        session_title: Some("Stale Build".to_string()),
        reason: "stale reason".to_string(),
        repo_dir: "/tmp/jcode".to_string(),
        repo_scope: source.repo_scope.clone(),
        worktree_scope: source.worktree_scope.clone(),
        command: "scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode".to_string(),
        // Outside the bootstrap grace window: a request with a missing status
        // file is only pruned once it is old enough that the queue handler
        // cannot still be mid-spawn.
        requested_at: (Utc::now() - chrono::Duration::minutes(10)).to_rfc3339(),
        started_at: Some(Utc::now().to_rfc3339()),
        completed_at: None,
        state: BuildRequestState::Building,
        version: Some("stale-build".to_string()),
        dedupe_key: Some("stale-dedupe".to_string()),
        requested_source: Some(source),
        built_source: None,
        published_version: None,
        last_progress: Some("building".to_string()),
        validated: false,
        error: None,
        output_file: None,
        status_file: Some(stale_status_path.display().to_string()),
        attached_to_request_id: None,
    };
    request.save().expect("save stale request");

    let status_output = selfdev_status_output(None).expect("status output");
    assert!(
        !status_output.output.contains("stale reason"),
        "stale request should be pruned from queue output"
    );

    let request = BuildRequest::load("stale-request")
        .expect("load stale request")
        .expect("stale request exists");
    assert_eq!(request.state, BuildRequestState::Failed);
    assert!(
        request
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("pruning stale self-dev build request"),
        "stale request should record why it was pruned"
    );
}

#[test]
fn status_output_reports_the_published_build_from_its_source_sidecar() {
    // F20c: the channel markers this used to read are gone. The status view now
    // reports the single published binary, and its identity must come from the
    // sidecar written next to that binary at publish time (not from manifest
    // state that can go stale).
    let _lock = lock_env();
    let temp_home = crate::storage::RuntimePaths::test_root("jcode-selfdev-home-");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());

    let published = build::current_fixed_binary_path().expect("fixed path");
    std::fs::create_dir_all(published.parent().expect("fixed dir")).expect("create fixed dir");
    std::fs::write(&published, "published binary").expect("write published binary");
    let source = test_source_state(std::path::Path::new("/tmp/jcode"));
    build::write_dev_binary_source_metadata(&published, &source).expect("write sidecar");

    let status_output = selfdev_status_output(None).expect("status output");
    assert!(
        status_output
            .output
            .contains(&format!("**Version:** {}", source.version_label)),
        "status should report the published build's version from its sidecar: {}",
        status_output.output
    );
    assert!(
        status_output
            .output
            .contains(&format!("**Source fingerprint:** `{}`", source.fingerprint))
    );
}
