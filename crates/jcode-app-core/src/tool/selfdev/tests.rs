use super::*;
use crate::bus::BackgroundTaskStatus;
use std::ffi::OsStr;
use std::process::Command;

pub(super) fn lock_env() -> crate::storage::TestEnvLease {
    crate::storage::lock_test_env()
}

pub(super) struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    pub(super) fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let original = std::env::var_os(key);
        crate::env::set_var(key, value);
        Self { key, original }
    }

    pub(super) fn remove(key: &'static str) -> Self {
        let original = std::env::var_os(key);
        crate::env::remove_var(key);
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => crate::env::set_var(self.key, value),
            None => crate::env::remove_var(self.key),
        }
    }
}

pub(super) fn create_test_context(
    session_id: &str,
    working_dir: Option<std::path::PathBuf>,
) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        message_id: "test-message".to_string(),
        tool_call_id: "test-tool-call".to_string(),
        working_dir,
        stdin_request_tx: None,
        graceful_shutdown_signal: None,
        execution_mode: crate::tool::ToolExecutionMode::Direct,
    }
}

pub(super) fn create_repo_fixture() -> tempfile::TempDir {
    let temp = tempfile::TempDir::new().expect("temp repo");
    std::fs::write(temp.path().join(".gitignore"), "target/\n").expect("gitignore");
    std::fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname = \"jcode\"\nversion = \"0.1.0\"\n",
    )
    .expect("cargo toml");
    Command::new("git")
        .args(["init", "-q"])
        .current_dir(temp.path())
        .status()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.email", "jcode@example.com"])
        .current_dir(temp.path())
        .status()
        .expect("git config user.email");
    Command::new("git")
        .args(["config", "user.name", "Jcode Tests"])
        .current_dir(temp.path())
        .status()
        .expect("git config user.name");
    Command::new("git")
        .args(["add", ".gitignore", "Cargo.toml"])
        .current_dir(temp.path())
        .status()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-q", "-m", "fixture"])
        .current_dir(temp.path())
        .status()
        .expect("git commit");
    temp
}

pub(super) fn test_source_state(repo_dir: &std::path::Path) -> build::SourceState {
    build::SourceState {
        repo_scope: "test-repo-scope".to_string(),
        worktree_scope: build::worktree_scope_key(repo_dir)
            .unwrap_or_else(|_| "test-worktree".to_string()),
        short_hash: "test-build".to_string(),
        full_hash: "test-build-full".to_string(),
        dirty: true,
        fingerprint: "test-fingerprint".to_string(),
        version_label: "test-build".to_string(),
        changed_paths: 0,
    }
}

#[test]
fn build_lock_is_removed_on_drop_and_can_be_reacquired() {
    let _env_lock = lock_env();
    let temp = tempfile::tempdir().expect("temp jcode home");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());
    let scope = format!("lock-drop-{}", std::process::id());
    let path = SelfDevTool::build_lock_path(&scope).expect("lock path");

    let first = SelfDevTool::try_acquire_build_lock(&scope)
        .expect("first lock attempt")
        .expect("first lock acquired");
    assert!(path.exists(), "lock file should exist while held");
    drop(first);
    assert!(!path.exists(), "lock file should be removed on drop");

    let second = SelfDevTool::try_acquire_build_lock(&scope)
        .expect("second lock attempt")
        .expect("lock should be reacquirable after drop");
    drop(second);
    assert!(!path.exists(), "reacquired lock should also clean up");
}

pub(super) async fn wait_for_task_completion(task_id: &str) -> background::TaskStatusFile {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(status) = background::global().status(task_id).await
            && status.status != BackgroundTaskStatus::Running
        {
            return status;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for background task {}",
            task_id
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[test]
fn test_reload_context_serialization() {
    // Create test context with task info
    let ctx = ReloadContext {
        task_context: Some("Testing the reload feature".to_string()),
        version_before: "v0.1.100".to_string(),
        version_after: "abc1234".to_string(),
        session_id: "test-session-123".to_string(),
        timestamp: "2025-01-20T00:00:00Z".to_string(),
    };

    // Serialize and deserialize
    let json = serde_json::to_string(&ctx).unwrap();
    let loaded: ReloadContext = serde_json::from_str(&json).unwrap();

    assert_eq!(
        loaded.task_context,
        Some("Testing the reload feature".to_string())
    );
    assert_eq!(loaded.version_before, "v0.1.100");
    assert_eq!(loaded.version_after, "abc1234");
    assert_eq!(loaded.session_id, "test-session-123");
}

#[test]
fn test_reload_context_path() {
    // Just verify the session-scoped path function works
    let path = ReloadContext::path_for_session("test-session-123");
    assert!(path.is_ok());
    let path = path.unwrap();
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("reload-context-test-session-123.json"));
}

#[test]
fn test_reload_context_save_and_load_for_session_uses_session_scoped_file() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());

    let ctx = ReloadContext {
        task_context: Some("Testing scoped reload context".to_string()),
        version_before: "v0.1.100".to_string(),
        version_after: "abc1234".to_string(),
        session_id: "test-session-123".to_string(),
        timestamp: "2025-01-20T00:00:00Z".to_string(),
    };

    ctx.save().expect("save reload context");

    let path = ReloadContext::path_for_session("test-session-123").expect("context path");
    assert!(
        path.exists(),
        "session-scoped reload context file should exist"
    );

    let peeked = ReloadContext::peek_for_session("test-session-123")
        .expect("peek should succeed")
        .expect("context should exist");
    assert_eq!(peeked.session_id, "test-session-123");

    let loaded = ReloadContext::load_for_session("test-session-123")
        .expect("load should succeed")
        .expect("context should exist");
    assert_eq!(loaded.session_id, "test-session-123");
    assert!(
        !path.exists(),
        "load_for_session should consume the context file"
    );
}

#[test]
fn test_recovery_directive_prefers_reload_context_when_present() {
    let ctx = ReloadContext {
        task_context: Some("Resume a self-dev reload".to_string()),
        version_before: "old-build".to_string(),
        version_after: "new-build".to_string(),
        session_id: "session-123".to_string(),
        timestamp: "2026-04-19T00:00:00Z".to_string(),
    };

    let directive = ReloadContext::recovery_directive(
        Some(&ctx),
        true,
        "\nPersisted background task(s) detected.",
        Some(12),
    )
    .expect("directive should exist");

    assert_eq!(
        directive.reconnect_notice.as_deref(),
        Some("Reloaded with build new-build")
    );
    assert!(directive.continuation_message.contains("Reload succeeded"));
    assert!(
        directive
            .continuation_message
            .contains("Persisted background task(s)")
    );
    assert!(
        directive
            .continuation_message
            .contains("Session restored with 12 turns")
    );
}

#[test]
fn test_recovery_directive_uses_interrupted_message_without_reload_context() {
    let directive = ReloadContext::recovery_directive(None, true, "", None)
        .expect("interrupted sessions should get a directive");

    assert!(directive.reconnect_notice.is_none());
    assert!(
        directive
            .continuation_message
            .contains("interrupted by a server reload while a tool was running")
    );
}

#[test]
fn test_recovery_directive_returns_none_when_no_reload_recovery_needed() {
    assert!(ReloadContext::recovery_directive(None, false, "", None).is_none());
}

#[test]
fn reload_timeout_secs_defaults_to_15() {
    let _lock = lock_env();
    let _guard = EnvVarGuard::remove("JCODE_SELFDEV_RELOAD_TIMEOUT_SECS");
    assert_eq!(SelfDevTool::reload_timeout_secs(), 15);
}

#[test]
fn reload_timeout_secs_honors_valid_env_override() {
    let _lock = lock_env();
    let _guard = EnvVarGuard::set("JCODE_SELFDEV_RELOAD_TIMEOUT_SECS", "27");
    assert_eq!(SelfDevTool::reload_timeout_secs(), 27);
}

#[test]
fn reload_timeout_secs_ignores_empty_invalid_and_zero_values() {
    let _lock = lock_env();
    let _guard = EnvVarGuard::set("JCODE_SELFDEV_RELOAD_TIMEOUT_SECS", "   ");
    assert_eq!(SelfDevTool::reload_timeout_secs(), 15);
    drop(_guard);

    let _guard = EnvVarGuard::set("JCODE_SELFDEV_RELOAD_TIMEOUT_SECS", "abc");
    assert_eq!(SelfDevTool::reload_timeout_secs(), 15);
    drop(_guard);

    let _guard = EnvVarGuard::set("JCODE_SELFDEV_RELOAD_TIMEOUT_SECS", "0");
    assert_eq!(SelfDevTool::reload_timeout_secs(), 15);
}

#[test]
fn schema_only_advertises_core_selfdev_fields() {
    // The full (self-dev) schema exposes the build/test/reload surface.
    let schema = SelfDevTool::schema_for(true);
    let props = schema["properties"]
        .as_object()
        .expect("selfdev schema should have properties");

    assert!(props.contains_key("action"));
    assert!(props.contains_key("prompt"));
    assert!(props.contains_key("context"));
    assert!(props.contains_key("reason"));
    assert!(props.contains_key("target"));
    assert!(props.contains_key("command"));
    assert!(props.contains_key("request_id"));
    assert!(props.contains_key("task_id"));
    assert!(!props.contains_key("notify"));
    assert!(!props.contains_key("wake"));

    let actions: Vec<&str> = schema["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for expected in [
        "enter",
        "setup",
        "build",
        "build-reload",
        "test",
        "cancel-build",
        "reload",
        "status",
        "find-config",
        "socket-info",
        "socket-help",
    ] {
        assert!(actions.contains(&expected), "missing action {expected}");
    }
}

#[test]
fn non_selfdev_schema_only_exposes_onramp_actions() {
    // The default schema (what a regular session advertises) is the on-ramp
    // surface: no build/test/socket actions, only enter/setup/reload/status/
    // find-config.
    let default_schema = SelfDevTool::new().parameters_schema();
    let onramp_schema = SelfDevTool::schema_for(false);
    assert_eq!(default_schema, onramp_schema);

    let props = onramp_schema["properties"]
        .as_object()
        .expect("schema properties");
    assert!(props.contains_key("action"));
    assert!(props.contains_key("prompt"));
    // Build/test-only fields are hidden outside self-dev mode.
    assert!(!props.contains_key("reason"));
    assert!(!props.contains_key("target"));
    assert!(!props.contains_key("command"));
    assert!(!props.contains_key("request_id"));
    assert!(!props.contains_key("task_id"));

    let actions: Vec<&str> = onramp_schema["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    let mut sorted = actions.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        vec!["enter", "find-config", "reload", "setup", "status"]
    );
    for hidden in [
        "build",
        "build-reload",
        "test",
        "cancel-build",
        "socket-info",
        "socket-help",
    ] {
        assert!(
            !actions.contains(&hidden),
            "on-ramp schema should not expose {hidden}"
        );
    }
}

#[tokio::test]
async fn test_action_queues_command_in_test_mode() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    let _test_guard = EnvVarGuard::set("JCODE_TEST_SESSION", "1");
    let repo = create_repo_fixture();

    let tool = SelfDevTool::new();
    let ctx = create_test_context(
        "session-selfdev-test-action",
        Some(repo.path().to_path_buf()),
    );
    let output = tool
        .execute(
            json!({
                "action": "test",
                "command": "cargo test -p jcode selfdev_build_command",
                "reason": "verify selfdev test queue"
            }),
            ctx,
        )
        .await
        .expect("selfdev test should queue");

    assert!(output.output.contains("Self-dev test queued"));
    assert!(
        output
            .output
            .contains("cargo test -p jcode selfdev_build_command")
    );
}

#[tokio::test]
async fn do_reload_returns_after_ack_in_direct_mode() {
    let request_id = server::send_reload_signal("direct-hash".to_string(), None, true);
    let waiter = tokio::spawn({
        let request_id = request_id.clone();
        async move { server::wait_for_reload_ack(&request_id, std::time::Duration::from_secs(1)).await }
    });

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    server::acknowledge_reload_signal(&crate::server::ReloadSignal {
        hash: "direct-hash".to_string(),
        triggering_session: None,
        prefer_selfdev_binary: true,
        request_id: "ignored".to_string(),
        runtime_identity: None,
    });

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    server::acknowledge_reload_signal(&crate::server::ReloadSignal {
        hash: "direct-hash".to_string(),
        triggering_session: None,
        prefer_selfdev_binary: true,
        request_id,
        runtime_identity: None,
    });

    let ack = waiter
        .await
        .expect("waiter task should complete")
        .expect("ack should be received");
    assert_eq!(ack.hash, "direct-hash");
}

#[test]
fn reload_environment_uses_working_dir_when_primary_detection_fails() {
    let repo = create_repo_fixture();
    let nested = repo.path().join("crates").join("jcode-build-support");
    std::fs::create_dir_all(&nested).expect("nested dir");

    let resolved = reload::resolve_selfdev_reload_repo_dir_from(None, Some(&nested));
    assert_eq!(resolved.as_deref(), Some(repo.path()));
}

#[tokio::test]
async fn reload_environment_rejects_missing_repo_for_real_sessions() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());

    let mut session = session::Session::create(None, Some("Missing repo".to_string()));
    session.set_canary("self-dev");
    session.save().expect("save session");

    let tool = SelfDevTool::new();
    let missing_repo = tempfile::TempDir::new().expect("missing repo dir");
    let err = tool
        .execute(
            json!({"action": "reload"}),
            create_test_context(&session.id, Some(missing_repo.path().to_path_buf())),
        )
        .await
        .expect_err("reload should fail without a repo");
    assert!(err.to_string().contains("Could not find jcode repository directory"));
}

#[tokio::test]
async fn reload_environment_rejects_missing_binary_for_real_sessions() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    let repo = create_repo_fixture();

    let mut session = session::Session::create(None, Some("Missing binary".to_string()));
    session.set_canary("self-dev");
    session.save().expect("save session");

    let tool = SelfDevTool::new();
    let err = tool
        .execute(
            json!({"action": "reload"}),
            create_test_context(&session.id, Some(repo.path().to_path_buf())),
        )
        .await
        .expect_err("reload should fail without a binary");
    assert!(err.to_string().contains("No binary found at"));
}

#[tokio::test]
async fn enter_creates_selfdev_session_in_test_mode() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    let _test_guard = EnvVarGuard::set("JCODE_TEST_SESSION", "1");
    let repo = create_repo_fixture();

    let mut parent = session::Session::create(None, Some("Origin Session".to_string()));
    parent.working_dir = Some("/tmp/origin-project".to_string());
    parent.model = Some("gpt-test".to_string());
    parent.provider_key = Some("openai".to_string());
    parent.subagent_model = Some("gpt-subagent".to_string());
    parent.add_message(
        crate::message::Role::User,
        vec![crate::message::ContentBlock::Text {
            text: "hello from parent".to_string(),
            cache_control: None,
        }],
    );
    parent.compaction = Some(session::StoredCompactionState {
        summary_text: "summary".to_string(),
        openai_encrypted_content: None,
        covers_up_to_turn: 1,
        original_turn_count: 1,
        compacted_count: 1,
    });
    parent.record_replay_display_message("system", None, "remember this context");
    parent.save().expect("save parent session");

    let tool = SelfDevTool::new();
    let ctx = create_test_context(&parent.id, Some(repo.path().to_path_buf()));
    let output = tool
        .execute(
            json!({"action": "enter", "prompt": "Work on jcode itself"}),
            ctx,
        )
        .await
        .expect("selfdev enter should succeed in test mode");

    assert!(output.output.contains("Created self-dev session"));
    assert!(
        output
            .output
            .contains("Test mode skipped launching a new terminal")
    );
    assert!(
        output.output.contains("Seed prompt captured"),
        "test-mode enter should still report captured prompt"
    );

    let metadata = output.metadata.expect("metadata");
    let session_id = metadata["session_id"]
        .as_str()
        .expect("session id metadata");
    assert_eq!(metadata["inherited_context"].as_bool(), Some(true));
    let session = session::Session::load(session_id).expect("load spawned session");
    assert!(
        session.is_canary,
        "spawned session should be canary/self-dev"
    );
    assert_eq!(session.testing_build.as_deref(), Some("self-dev"));
    assert_eq!(
        session.working_dir.as_deref(),
        Some(repo.path().to_string_lossy().as_ref())
    );
    assert_eq!(session.parent_id.as_deref(), Some(parent.id.as_str()));
    assert_eq!(session.messages.len(), parent.messages.len());
    assert_eq!(session.messages[0].content_preview(), "hello from parent");
    assert_eq!(session.compaction, parent.compaction);
    assert_eq!(session.model, parent.model);
    assert_eq!(session.provider_key, parent.provider_key);
    assert_eq!(session.subagent_model, parent.subagent_model);
    assert_eq!(session.replay_events, parent.replay_events);
}

#[tokio::test]
async fn enter_falls_back_to_fresh_session_when_parent_missing() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    let _test_guard = EnvVarGuard::set("JCODE_TEST_SESSION", "1");
    let repo = create_repo_fixture();

    let tool = SelfDevTool::new();
    let ctx = create_test_context("missing-parent", Some(repo.path().to_path_buf()));
    let output = tool
        .execute(json!({"action": "enter"}), ctx)
        .await
        .expect("selfdev enter should succeed without a persisted parent session");

    let metadata = output.metadata.expect("metadata");
    let session_id = metadata["session_id"]
        .as_str()
        .expect("session id metadata");
    assert_eq!(metadata["inherited_context"].as_bool(), Some(false));

    let session = session::Session::load(session_id).expect("load spawned session");
    assert!(session.messages.is_empty());
    assert!(session.parent_id.is_none());
    assert_eq!(
        session.working_dir.as_deref(),
        Some(repo.path().to_string_lossy().as_ref())
    );
}

#[tokio::test]
async fn reload_in_non_selfdev_session_is_upgrade_in_place() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    assert!(
        !crate::server::server_has_newer_binary(),
        "the empty test home should not advertise a newer server binary"
    );

    let mut session = session::Session::create(None, Some("Normal Session".to_string()));
    session.save().expect("save session");

    let tool = SelfDevTool::new();
    let ctx = create_test_context(&session.id, session.working_dir.clone().map(Into::into));
    let output = tool
        .execute(json!({"action": "reload"}), ctx)
        .await
        .expect("reload should route to upgrade-in-place");

    // It must NOT be the old "only available inside a self-dev session" error;
    // a regular session can still take the strict newest-binary path.
    assert!(
        !output
            .output
            .contains("only available inside a self-dev session")
    );
    assert!(
        output
            .output
            .contains("Already running the newest installed jcode build; no reload needed."),
        "unexpected output: {}",
        output.output
    );
}

#[tokio::test]
async fn socket_actions_require_selfdev_session() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());

    let mut session = session::Session::create(None, Some("Normal Session".to_string()));
    session.save().expect("save session");

    let tool = SelfDevTool::new();
    for action in ["socket-info", "socket-help"] {
        let ctx = create_test_context(&session.id, session.working_dir.clone().map(Into::into));
        let output = tool
            .execute(json!({"action": action}), ctx)
            .await
            .expect("socket action should return guidance instead of failing");
        assert!(
            output
                .output
                .contains("only available inside a self-dev session"),
            "{action} should be gated"
        );
        assert!(output.output.contains("selfdev enter"));
    }
}

#[tokio::test]
async fn find_config_reports_key_paths() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());

    let mut session = session::Session::create(None, Some("Normal Session".to_string()));
    session.save().expect("save session");

    let tool = SelfDevTool::new();
    let ctx = create_test_context(&session.id, None);
    let output = tool
        .execute(json!({"action": "find-config"}), ctx)
        .await
        .expect("find-config should succeed");

    assert!(output.output.contains("Config file:"));
    assert!(output.output.contains("config.toml"));
    // F20c: find-config reports the ONE published binary, not a channel list.
    assert!(output.output.contains("### Binaries"));
    assert!(output.output.contains("**published (current):**"));
    assert!(
        !output.output.contains("Build channels"),
        "the retired channel matrix must not be advertised: {}",
        output.output
    );
    let metadata = output.metadata.expect("find-config metadata");
    assert!(metadata["config_path"].as_str().is_some());
}

#[tokio::test]
async fn setup_reports_dependency_checks() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    // Test mode avoids attempting a real git clone when no repo is detected.
    let _test_guard = EnvVarGuard::set("JCODE_TEST_SESSION", "1");
    let repo = create_repo_fixture();

    let mut session = session::Session::create(None, Some("Normal Session".to_string()));
    session.save().expect("save session");

    let tool = SelfDevTool::new();
    let ctx = create_test_context(&session.id, Some(repo.path().to_path_buf()));
    let output = tool
        .execute(json!({"action": "setup"}), ctx)
        .await
        .expect("setup should succeed");

    assert!(output.output.contains("Self-dev setup"));
    assert!(output.output.contains("**cargo**") || output.output.contains("cargo"));
    assert!(output.output.contains("repository"));
    let metadata = output.metadata.expect("setup metadata");
    assert!(metadata["checks"].as_array().is_some());
    // The fixture repo should be detected as the repository.
    assert_eq!(
        metadata["repo_dir"].as_str(),
        Some(repo.path().to_string_lossy().as_ref())
    );
}

#[tokio::test]
async fn build_requires_reason() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    let _test_guard = EnvVarGuard::set("JCODE_TEST_SESSION", "1");
    let repo = create_repo_fixture();

    let tool = SelfDevTool::new();
    let ctx = create_test_context("build-session", Some(repo.path().to_path_buf()));
    let err = tool
        .execute(json!({"action": "build"}), ctx)
        .await
        .expect_err("build without reason should fail");

    assert!(err.to_string().contains("requires a non-empty `reason`"));
}

#[tokio::test]
async fn build_queues_background_tasks_and_reports_queue_status() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    let _test_guard = EnvVarGuard::set("JCODE_TEST_SESSION", "1");
    let repo = create_repo_fixture();

    let mut session_one = session::Session::create(None, Some("First build session".to_string()));
    session_one.short_name = Some("alpha".to_string());
    session_one.save().expect("save session one");

    let mut session_two = session::Session::create(None, Some("Second build session".to_string()));
    session_two.short_name = Some("beta".to_string());
    session_two.save().expect("save session two");

    let tool = SelfDevTool::new();
    let first = tool
        .execute(
            json!({"action": "build", "reason": "first reason"}),
            create_test_context(&session_one.id, Some(repo.path().to_path_buf())),
        )
        .await
        .expect("first build should queue");
    let second = tool
        .execute(
            json!({"action": "build", "reason": "second reason"}),
            create_test_context(&session_two.id, Some(repo.path().to_path_buf())),
        )
        .await
        .expect("second build should queue");

    let first_meta = first.metadata.expect("first metadata");
    let second_meta = second.metadata.expect("second metadata");
    let first_task_id = first_meta["task_id"].as_str().expect("first task id");
    let second_task_id = second_meta["task_id"].as_str().expect("second task id");

    assert_eq!(first_meta["queue_position"].as_u64(), Some(1));
    assert_eq!(second_meta["deduped"].as_bool(), Some(true));
    assert!(
        second
            .output
            .contains("attached instead of spawning a duplicate build")
    );

    let status_output = selfdev_status_output().expect("status output");
    assert!(status_output.output.contains("## Build Queue"));
    assert!(status_output.output.contains("first reason"));
    assert!(status_output.output.contains("Attached watchers: 1"));
    assert!(
        status_output
            .output
            .contains("Target version: `test-build`")
    );

    let first_status = wait_for_task_completion(first_task_id).await;
    let second_status = wait_for_task_completion(second_task_id).await;
    assert_eq!(first_status.status, BackgroundTaskStatus::Completed);
    assert_eq!(second_status.status, BackgroundTaskStatus::Completed);

    let request_one =
        BuildRequest::load(first_meta["request_id"].as_str().expect("first request id"))
            .expect("load request one")
            .expect("request one exists");
    let request_two = BuildRequest::load(
        second_meta["request_id"]
            .as_str()
            .expect("second request id"),
    )
    .expect("load request two")
    .expect("request two exists");
    assert_eq!(request_one.state, BuildRequestState::Completed);
    assert_eq!(request_two.state, BuildRequestState::Completed);
}

#[tokio::test]
async fn build_reload_waits_for_build_then_reloads() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    let _test_guard = EnvVarGuard::set("JCODE_TEST_SESSION", "1");
    let repo = create_repo_fixture();
    let source = test_source_state(repo.path());
    let target_binary = repo.path().join("target").join("selfdev").join("jcode");
    let _reload_env_guard = reload::override_reload_environment_for_tests(reload::ReloadEnvironment {
        repo_dir: repo.path().to_path_buf(),
        target_binary: target_binary.clone(),
        version_before: jcode_build_meta::VERSION.to_string(),
        version_after: source.version_label.clone(),
        source: source.clone(),
        runtime_identity: source.runtime_identity_projection("selfdev", target_binary),
        wait_mode: reload::ReloadWaitMode::AcknowledgeOnly {
            message: format!(
                "Reload acknowledged for build {}. Server is restarting now.",
                source.version_label
            ),
        },
    });

    let mut session = session::Session::create(None, Some("Build+reload session".to_string()));
    session.is_canary = true;
    session.short_name = Some("gamma".to_string());
    session.save().expect("save session");

    // The reload phase blocks on a server ack. Spawn a watcher that mirrors the
    // server: it observes reload signals and acknowledges them so the inline
    // reload can complete deterministically in test mode. It keeps acking every
    // signal it sees (the RELOAD_SIGNAL channel is a process-global shared by
    // parallel tests, and `wait_for_reload_ack` matches by request id, so acking
    // unrelated/stale signals is harmless).
    let mut signal_rx = server::subscribe_reload_signal_for_tests();
    let acker = tokio::spawn(async move {
        if let Some(signal) = signal_rx.borrow_and_update().clone() {
            server::acknowledge_reload_signal(&signal);
        }
        while signal_rx.changed().await.is_ok() {
            if let Some(signal) = signal_rx.borrow_and_update().clone() {
                server::acknowledge_reload_signal(&signal);
            }
        }
    });

    let tool = SelfDevTool::new();
    let output = tool
        .execute(
            json!({"action": "build-reload", "reason": "combined build and reload"}),
            create_test_context(&session.id, Some(repo.path().to_path_buf())),
        )
        .await
        .expect("build-reload should succeed");

    acker.abort();

    assert!(
        output.output.contains("Build completed successfully"),
        "unexpected output: {}",
        output.output
    );
    let meta = output.metadata.expect("build-reload metadata");
    assert_eq!(meta["phase"].as_str(), Some("reload"));
    assert_eq!(meta["build_finished"].as_bool(), Some(true));
    assert_eq!(meta["build_succeeded"].as_bool(), Some(true));

    let request_id = meta["request_id"].as_str().expect("request id in metadata");
    let request = BuildRequest::load(request_id)
        .expect("load request")
        .expect("request exists");
    assert_eq!(request.state, BuildRequestState::Completed);
}

#[tokio::test]
async fn build_dedupes_identical_reason_and_version_with_attached_watcher() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    let _test_guard = EnvVarGuard::set("JCODE_TEST_SESSION", "1");
    let repo = create_repo_fixture();

    let mut session_one = session::Session::create(None, Some("Build A".to_string()));
    session_one.short_name = Some("alpha".to_string());
    session_one.save().expect("save session one");

    let mut session_two = session::Session::create(None, Some("Build B".to_string()));
    session_two.short_name = Some("beta".to_string());
    session_two.save().expect("save session two");

    let tool = SelfDevTool::new();
    let first = tool
        .execute(
            json!({"action": "build", "reason": "same reason"}),
            create_test_context(&session_one.id, Some(repo.path().to_path_buf())),
        )
        .await
        .expect("first build should queue");
    let second = tool
        .execute(
            json!({"action": "build", "reason": "same reason"}),
            create_test_context(&session_two.id, Some(repo.path().to_path_buf())),
        )
        .await
        .expect("second build should attach");

    let first_meta = first.metadata.expect("first metadata");
    let second_meta = second.metadata.expect("second metadata");
    assert_eq!(second_meta["deduped"].as_bool(), Some(true));
    assert_eq!(
        second_meta["duplicate_of"]["request_id"].as_str(),
        first_meta["request_id"].as_str()
    );

    let status_output = selfdev_status_output().expect("status output");
    assert!(status_output.output.contains("Attached watchers: 1"));
    assert!(status_output.output.contains("alpha"));
    assert!(status_output.output.contains("beta"));

    let first_status = wait_for_task_completion(first_meta["task_id"].as_str().unwrap()).await;
    let second_status = wait_for_task_completion(second_meta["task_id"].as_str().unwrap()).await;
    assert_eq!(first_status.status, BackgroundTaskStatus::Completed);
    assert_eq!(second_status.status, BackgroundTaskStatus::Completed);

    let watcher_request = BuildRequest::load(second_meta["request_id"].as_str().unwrap())
        .expect("load watcher request")
        .expect("watcher request exists");
    assert_eq!(watcher_request.state, BuildRequestState::Completed);
    assert_eq!(
        watcher_request.attached_to_request_id.as_deref(),
        first_meta["request_id"].as_str()
    );
}

#[tokio::test]
async fn cancel_build_marks_request_cancelled_and_removes_it_from_queue() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());
    let _test_guard = EnvVarGuard::set("JCODE_TEST_SESSION", "1");
    let repo = create_repo_fixture();

    let mut session_one = session::Session::create(None, Some("Build A".to_string()));
    session_one.short_name = Some("alpha".to_string());
    session_one.save().expect("save session one");

    let mut session_two = session::Session::create(None, Some("Build B".to_string()));
    session_two.short_name = Some("beta".to_string());
    session_two.save().expect("save session two");

    let tool = SelfDevTool::new();
    let first = tool
        .execute(
            json!({"action": "build", "reason": "keep building"}),
            create_test_context(&session_one.id, Some(repo.path().to_path_buf())),
        )
        .await
        .expect("first build should queue");
    let second = tool
        .execute(
            json!({"action": "build", "reason": "cancel me"}),
            create_test_context(&session_two.id, Some(repo.path().to_path_buf())),
        )
        .await
        .expect("second build should queue");

    let second_meta = second.metadata.expect("second metadata");
    let cancel = tool
        .execute(
            json!({
                "action": "cancel-build",
                "request_id": second_meta["request_id"].as_str().unwrap()
            }),
            create_test_context(&session_two.id, Some(repo.path().to_path_buf())),
        )
        .await
        .expect("cancel should succeed");

    assert!(cancel.output.contains("Cancelled self-dev build request"));

    let second_status = wait_for_task_completion(second_meta["task_id"].as_str().unwrap()).await;
    assert_eq!(second_status.status, BackgroundTaskStatus::Failed);

    let cancelled_request = BuildRequest::load(second_meta["request_id"].as_str().unwrap())
        .expect("load cancelled request")
        .expect("cancelled request exists");
    assert_eq!(cancelled_request.state, BuildRequestState::Cancelled);
    assert!(
        !BuildRequest::pending_requests()
            .expect("pending requests")
            .iter()
            .any(|request| request.request_id == cancelled_request.request_id)
    );

    let status_output = selfdev_status_output().expect("status output");
    assert!(!status_output.output.contains("cancel me"));

    let first_meta = first.metadata.expect("first metadata");
    let first_status = wait_for_task_completion(first_meta["task_id"].as_str().unwrap()).await;
    assert_eq!(first_status.status, BackgroundTaskStatus::Completed);
}

#[test]
fn status_output_prunes_stale_pending_requests() {
    let _lock = lock_env();
    let temp_home = tempfile::TempDir::new().expect("temp home");
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

    let status_output = selfdev_status_output().expect("status output");
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
    let temp_home = tempfile::TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("JCODE_HOME", temp_home.path());

    let published = build::current_fixed_binary_path().expect("fixed path");
    std::fs::create_dir_all(published.parent().expect("fixed dir")).expect("create fixed dir");
    std::fs::write(&published, "published binary").expect("write published binary");
    let source = test_source_state(std::path::Path::new("/tmp/jcode"));
    build::write_dev_binary_source_metadata(&published, &source).expect("write sidecar");

    let status_output = selfdev_status_output().expect("status output");
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
