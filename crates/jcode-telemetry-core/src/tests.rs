use super::*;
use std::sync::{Mutex, OnceLock};

// All of these tests mutate process-global state: the env-var opt-out tests
// flip `JCODE_NO_TELEMETRY` / `DO_NOT_TRACK`, while the session tests drive the
// global `SESSION_STATE`. They must be serialized against *each other* with a
// single shared lock. Using two separate locks previously let an env test
// disable telemetry (`is_enabled() == false`) while a session test was calling
// `begin_session_with_mode`, which then returned early and left `SESSION_STATE`
// as `None`; the session test's `expect(...)` panicked while holding the
// `SESSION_STATE` lock and poisoned it, cascading into `PoisonError` failures
// in every other session test.
fn global_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn permanent_telemetry_statuses_trip_the_process_breaker() {
    assert!(telemetry_status_is_permanent(400));
    assert!(telemetry_status_is_permanent(401));
    assert!(telemetry_status_is_permanent(404));
    assert!(!telemetry_status_is_permanent(408));
    assert!(!telemetry_status_is_permanent(425));
    assert!(!telemetry_status_is_permanent(429));
    assert!(!telemetry_status_is_permanent(500));
}

fn lock_test_env() -> std::sync::MutexGuard<'static, ()> {
    global_test_lock()
}

/// Serializes session tests *and* isolates them from the developer's real
/// jcode home and environment.
///
/// `begin_session_with_mode` is a no-op when `is_enabled()` is false, and
/// `is_enabled()` consults `JCODE_NO_TELEMETRY`, `DO_NOT_TRACK`, and a
/// `~/.jcode/no_telemetry` marker file. On a machine that has opted out (a
/// perfectly normal state for a developer) every session test silently
/// recorded nothing, then panicked while holding `SESSION_STATE`, poisoning
/// the lock and cascading `PoisonError` into the rest of the suite. Tests must
/// not depend on whether the person running them opted out of telemetry.
struct TelemetrySessionTestEnv {
    _lock: std::sync::MutexGuard<'static, ()>,
    _home: tempfile::TempDir,
    prev_home: Option<std::ffi::OsString>,
    prev_no_telemetry: Option<std::ffi::OsString>,
    prev_do_not_track: Option<std::ffi::OsString>,
    prev_opt_in: Option<std::ffi::OsString>,
}

impl Drop for TelemetrySessionTestEnv {
    fn drop(&mut self) {
        restore_env("JCODE_HOME", self.prev_home.take());
        restore_env("JCODE_NO_TELEMETRY", self.prev_no_telemetry.take());
        restore_env("DO_NOT_TRACK", self.prev_do_not_track.take());
        restore_env("JCODE_TELEMETRY", self.prev_opt_in.take());
    }
}

fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => jcode_core::env::set_var(key, value),
        None => jcode_core::env::remove_var(key),
    }
}

fn lock_telemetry_test_state() -> TelemetrySessionTestEnv {
    let env = isolated_telemetry_env();
    // Telemetry is off by default in this fork, so session tests must opt in
    // explicitly rather than inheriting an ambient default.
    jcode_core::env::set_var("JCODE_TELEMETRY", "1");
    assert!(
        is_enabled(),
        "session tests require telemetry enabled inside the isolated home"
    );
    env
}

/// Isolates `is_enabled()`'s entire input surface: a scratch `JCODE_HOME` with
/// no marker files, and all three telemetry env vars cleared. Unlike
/// [`lock_telemetry_test_state`] it asserts nothing, so tests can observe the
/// default rather than a state the helper forced.
fn isolated_telemetry_env() -> TelemetrySessionTestEnv {
    let lock = global_test_lock();
    let home = tempfile::TempDir::new().expect("create temp JCODE_HOME");
    let env = TelemetrySessionTestEnv {
        prev_home: std::env::var_os("JCODE_HOME"),
        prev_no_telemetry: std::env::var_os("JCODE_NO_TELEMETRY"),
        prev_do_not_track: std::env::var_os("DO_NOT_TRACK"),
        prev_opt_in: std::env::var_os("JCODE_TELEMETRY"),
        _lock: lock,
        _home: home,
    };
    jcode_core::env::set_var("JCODE_HOME", env._home.path());
    jcode_core::env::remove_var("JCODE_NO_TELEMETRY");
    jcode_core::env::remove_var("DO_NOT_TRACK");
    jcode_core::env::remove_var("JCODE_TELEMETRY");
    env
}

/// The fork's headline policy change: with nothing configured either way,
/// nothing is sent. Upstream returned `true` here.
#[test]
fn telemetry_is_off_by_default() {
    let env = isolated_telemetry_env();
    assert!(
        !is_enabled(),
        "telemetry must be off unless the user explicitly opts in"
    );
    drop(env);
}

#[test]
fn opt_in_env_var_enables_telemetry() {
    let env = isolated_telemetry_env();
    jcode_core::env::set_var("JCODE_TELEMETRY", "1");
    assert!(is_enabled());
    for truthy in ["true", "YES", " on "] {
        jcode_core::env::set_var("JCODE_TELEMETRY", truthy);
        assert!(is_enabled(), "{truthy:?} should read as opt-in");
    }
    drop(env);
}

/// `JCODE_TELEMETRY=0` is a user saying "no". Testing mere presence of the
/// variable would turn that into a "yes".
#[test]
fn opt_in_env_var_requires_an_affirmative_value() {
    let env = isolated_telemetry_env();
    for falsy in ["0", "false", "no", "off", ""] {
        jcode_core::env::set_var("JCODE_TELEMETRY", falsy);
        assert!(!is_enabled(), "{falsy:?} must not read as opt-in");
    }
    drop(env);
}

#[test]
fn opt_in_marker_file_enables_telemetry() {
    let env = isolated_telemetry_env();
    let dir = crate::storage::jcode_dir().expect("isolated jcode dir");
    std::fs::create_dir_all(&dir).expect("create jcode dir");
    std::fs::write(dir.join(crate::consent::TELEMETRY_OPT_IN_MARKER), "")
        .expect("write opt-in marker");
    assert!(is_enabled());
    drop(env);
}

/// An explicit "off" must beat an explicit "on" regardless of which mechanism
/// carries it, so a user who opted out cannot be re-enabled by a stray env var
/// (or a leftover marker) from a script or a shared shell profile.
#[test]
fn explicit_opt_out_overrides_opt_in() {
    let env = isolated_telemetry_env();
    let dir = crate::storage::jcode_dir().expect("isolated jcode dir");
    std::fs::create_dir_all(&dir).expect("create jcode dir");
    std::fs::write(dir.join(crate::consent::TELEMETRY_OPT_IN_MARKER), "")
        .expect("write opt-in marker");
    jcode_core::env::set_var("JCODE_TELEMETRY", "1");
    assert!(is_enabled(), "precondition: both opt-in paths active");

    jcode_core::env::set_var("JCODE_NO_TELEMETRY", "1");
    assert!(!is_enabled(), "JCODE_NO_TELEMETRY must win");
    jcode_core::env::remove_var("JCODE_NO_TELEMETRY");

    jcode_core::env::set_var("DO_NOT_TRACK", "1");
    assert!(!is_enabled(), "DO_NOT_TRACK must win");
    jcode_core::env::remove_var("DO_NOT_TRACK");

    std::fs::write(dir.join("no_telemetry"), "").expect("write opt-out marker");
    assert!(!is_enabled(), "no_telemetry marker must win");
    drop(env);
}

#[test]
fn test_opt_out_env_var() {
    let _guard = lock_test_env();
    jcode_core::env::set_var("JCODE_NO_TELEMETRY", "1");
    assert!(!is_enabled());
    jcode_core::env::remove_var("JCODE_NO_TELEMETRY");
}

#[test]
fn test_do_not_track() {
    let _guard = lock_test_env();
    jcode_core::env::set_var("DO_NOT_TRACK", "1");
    assert!(!is_enabled());
    jcode_core::env::remove_var("DO_NOT_TRACK");
}

#[test]
fn test_is_ci_detects_ci_env() {
    let _guard = lock_test_env();
    // Clear any inherited CI markers so the baseline is deterministic.
    for key in [
        "CI",
        "GITHUB_ACTIONS",
        "BUILDKITE",
        "JENKINS_URL",
        "GITLAB_CI",
        "CIRCLECI",
    ] {
        jcode_core::env::remove_var(key);
    }
    assert!(
        !is_ci(),
        "expected non-CI baseline after clearing CI markers"
    );
    jcode_core::env::set_var("CI", "true");
    assert!(
        is_ci(),
        "CI env var should mark the run as CI (gates install skip)"
    );
    jcode_core::env::remove_var("CI");
    assert!(!is_ci());
}

#[test]
fn test_error_counters() {
    let _guard = lock_telemetry_test_state();
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
    begin_session_with_mode("openai", "gpt-5.4", None, false);
    record_error(ErrorCategory::ProviderTimeout);
    record_error(ErrorCategory::ProviderTimeout);
    record_error(ErrorCategory::ToolError);
    {
        let guard = SESSION_STATE.lock().unwrap();
        let state = guard.as_ref().expect("session telemetry state");
        assert_eq!(state.error_provider_timeout, 2);
        assert_eq!(state.error_tool_error, 1);
        let errors = current_error_counts(state);
        assert_eq!(errors.provider_timeout, 2);
        assert_eq!(errors.tool_error, 1);
    }
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
}

#[test]
fn test_error_counter_caps_per_session() {
    let _guard = lock_telemetry_test_state();
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
    begin_session_with_mode("openai", "gpt-5.4", None, false);
    // A runaway retry loop once logged 18k+ auth failures in one session and
    // distorted daily aggregates. The counter must saturate at the cap.
    for _ in 0..600 {
        record_error(ErrorCategory::AuthFailed);
    }
    {
        let guard = SESSION_STATE.lock().unwrap();
        let state = guard.as_ref().expect("session telemetry state");
        assert_eq!(state.error_auth_failed, 500);
    }
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
}

#[test]
fn test_error_counters_no_session_is_noop() {
    let _guard = lock_telemetry_test_state();
    // Errors recorded with no active session must not bump any counter that a
    // future session could observe (issue #394: counts drifting across the
    // session boundary).
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
    record_error(ErrorCategory::AuthFailed);
    record_provider_switch();
    record_model_switch();
    begin_session_with_mode("openai", "gpt-5.4", None, false);
    {
        let guard = SESSION_STATE.lock().unwrap();
        let state = guard.as_ref().expect("session telemetry state");
        assert_eq!(state.error_auth_failed, 0);
        assert_eq!(state.provider_switches, 0);
        assert_eq!(state.model_switches, 0);
    }
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
}

#[test]
fn test_session_reason_labels() {
    assert_eq!(SessionEndReason::NormalExit.as_str(), "normal_exit");
    assert_eq!(SessionEndReason::Disconnect.as_str(), "disconnect");
}

#[test]
fn test_session_start_event_serialization() {
    let event = SessionStartEvent {
        event_id: "event-1".to_string(),
        id: "test-uuid".to_string(),
        session_id: "session-1".to_string(),
        event: "session_start",
        version: "0.6.1".to_string(),
        os: "linux",
        arch: "x86_64",
        provider_start: "claude".to_string(),
        model_start: "claude-sonnet-4".to_string(),
        resumed_session: true,
        session_start_hour_utc: 13,
        session_start_weekday_utc: 2,
        previous_session_gap_secs: Some(3600),
        sessions_started_24h: 3,
        sessions_started_7d: 8,
        active_sessions_at_start: 2,
        other_active_sessions_at_start: 1,
        schema_version: TELEMETRY_SCHEMA_VERSION,
        build_channel: "release".to_string(),
        is_git_checkout: false,
        is_ci: false,
        ran_from_cargo: false,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "session_start");
    assert_eq!(json["resumed_session"], true);
    assert_eq!(json["session_id"], "session-1");
    assert_eq!(json["sessions_started_24h"], 3);
}

#[test]
fn test_discovery_event_serialization_excludes_free_text() {
    let event = DiscoveryEvent {
        event_id: "event-discovery-1".to_string(),
        id: "test-uuid".to_string(),
        session_id: Some("session-1".to_string()),
        event: "discovery",
        version: "0.41.0".to_string(),
        os: "linux",
        arch: "x86_64",
        request_id: "request-1".to_string(),
        phase: "select".to_string(),
        category: Some("payments".to_string()),
        selected_tool: Some("agentcard".to_string()),
        outcome: "success".to_string(),
        failure_reason: None,
        http_status: Some(200),
        latency_ms: 123,
        response_bytes: Some(456),
        result_count: Some(1),
        query_present: true,
        reason_present: true,
        benchmark_run: true,
        custom_endpoint: false,
        schema_version: TELEMETRY_SCHEMA_VERSION,
        build_channel: "release".to_string(),
        is_git_checkout: false,
        is_ci: false,
        ran_from_cargo: false,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "discovery");
    assert_eq!(json["request_id"], "request-1");
    assert_eq!(json["phase"], "select");
    assert_eq!(json["selected_tool"], "agentcard");
    assert_eq!(json["latency_ms"], 123);
    assert_eq!(json["benchmark_run"], true);
    assert!(json.get("query").is_none());
    assert!(json.get("reason").is_none());
}

#[test]
fn test_session_end_event_serialization() {
    let event = SessionLifecycleEvent {
        event_id: "event-2".to_string(),
        id: "test-uuid".to_string(),
        session_id: "session-2".to_string(),
        event: "session_end",
        version: "0.6.1".to_string(),
        os: "linux",
        arch: "x86_64",
        provider_start: "claude".to_string(),
        provider_end: "openrouter".to_string(),
        model_start: "claude-sonnet-4-20250514".to_string(),
        model_end: "anthropic/claude-sonnet-4".to_string(),
        provider_switches: 1,
        model_switches: 2,
        duration_mins: 45,
        duration_secs: 2700,
        turns: 23,
        had_user_prompt: true,
        had_assistant_response: true,
        assistant_responses: 3,
        first_assistant_response_ms: Some(1200),
        first_tool_call_ms: Some(900),
        first_tool_success_ms: Some(1500),
        first_file_edit_ms: Some(2200),
        first_test_pass_ms: Some(4100),
        tool_calls: 4,
        tool_failures: 1,
        executed_tool_calls: 5,
        executed_tool_successes: 4,
        executed_tool_failures: 1,
        tool_latency_total_ms: 3200,
        tool_latency_max_ms: 1400,
        file_write_calls: 2,
        tests_run: 1,
        tests_passed: 1,
        input_tokens: 1234,
        output_tokens: 567,
        cache_read_input_tokens: 890,
        cache_creation_input_tokens: 12,
        total_tokens: 2703,
        feature_memory_used: true,
        feature_swarm_used: false,
        feature_web_used: true,
        feature_email_used: false,
        feature_mcp_used: true,
        feature_side_panel_used: true,
        feature_goal_used: false,
        feature_selfdev_used: false,
        feature_background_used: false,
        feature_subagent_used: true,
        unique_mcp_servers: 2,
        session_success: true,
        abandoned_before_response: false,
        session_stop_reason: "completed_successfully",
        agent_role: "foreground",
        parent_session_id: None,
        agent_active_ms_total: 180_000,
        agent_model_ms_total: 120_000,
        agent_tool_ms_total: 60_000,
        session_idle_ms_total: 30_000,
        agent_blocked_ms_total: 0,
        time_to_first_agent_action_ms: Some(900),
        time_to_first_useful_action_ms: Some(1500),
        spawned_agent_count: 3,
        background_task_count: 1,
        background_task_completed_count: 1,
        subagent_task_count: 1,
        subagent_success_count: 1,
        swarm_task_count: 1,
        swarm_success_count: 0,
        user_cancelled_count: 1,
        transport_https: 2,
        transport_persistent_ws_fresh: 1,
        transport_persistent_ws_reuse: 5,
        transport_cli_subprocess: 0,
        transport_native_http2: 0,
        transport_other: 0,
        tool_cat_read_search: 2,
        tool_cat_write: 2,
        tool_cat_shell: 1,
        tool_cat_web: 1,
        tool_cat_memory: 1,
        tool_cat_subagent: 1,
        tool_cat_swarm: 0,
        tool_cat_email: 0,
        tool_cat_side_panel: 1,
        tool_cat_goal: 0,
        tool_cat_mcp: 1,
        tool_cat_other: 0,
        command_login_used: false,
        command_model_used: true,
        command_usage_used: false,
        command_resume_used: false,
        command_memory_used: true,
        command_swarm_used: false,
        command_goal_used: false,
        command_selfdev_used: false,
        command_feedback_used: false,
        command_other_used: false,
        workflow_chat_only: false,
        workflow_coding_used: true,
        workflow_research_used: true,
        workflow_tests_used: true,
        workflow_background_used: false,
        workflow_subagent_used: true,
        workflow_swarm_used: false,
        project_repo_present: true,
        project_lang_rust: true,
        project_lang_js_ts: false,
        project_lang_python: false,
        project_lang_go: false,
        project_lang_markdown: true,
        project_lang_mixed: true,
        days_since_install: Some(12),
        active_days_7d: 4,
        active_days_30d: 9,
        session_start_hour_utc: 13,
        session_start_weekday_utc: 2,
        session_end_hour_utc: 14,
        session_end_weekday_utc: 2,
        previous_session_gap_secs: Some(1800),
        sessions_started_24h: 5,
        sessions_started_7d: 12,
        active_sessions_at_start: 2,
        other_active_sessions_at_start: 1,
        max_concurrent_sessions: 3,
        multi_sessioned: true,
        resumed_session: false,
        end_reason: "normal_exit",
        schema_version: TELEMETRY_SCHEMA_VERSION,
        build_channel: "release".to_string(),
        is_git_checkout: false,
        is_ci: false,
        ran_from_cargo: false,
        errors: ErrorCounts {
            provider_timeout: 2,
            auth_failed: 0,
            tool_error: 1,
            mcp_error: 0,
            rate_limited: 0,
        },
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "session_end");
    assert_eq!(json["assistant_responses"], 3);
    assert_eq!(json["duration_secs"], 2700);
    assert_eq!(json["executed_tool_calls"], 5);
    assert_eq!(json["transport_https"], 2);
    assert_eq!(json["tool_cat_write"], 2);
    assert_eq!(json["workflow_coding_used"], true);
    assert_eq!(json["active_days_30d"], 9);
    assert_eq!(json["transport_persistent_ws_reuse"], 5);
    assert_eq!(json["multi_sessioned"], true);
    assert_eq!(json["end_reason"], "normal_exit");
    assert_eq!(json["input_tokens"], 1234);
    assert_eq!(json["output_tokens"], 567);
    assert_eq!(json["cache_read_input_tokens"], 890);
    assert_eq!(json["cache_creation_input_tokens"], 12);
    assert_eq!(json["total_tokens"], 2703);
    assert_eq!(json["errors"]["provider_timeout"], 2);
    assert_eq!(json["session_stop_reason"], "completed_successfully");
    assert_eq!(json["agent_active_ms_total"], 180_000);
    assert_eq!(json["time_to_first_useful_action_ms"], 1500);
    assert_eq!(json["subagent_task_count"], 1);
    assert_eq!(json["user_cancelled_count"], 1);
}

#[test]
fn test_record_token_usage_aggregates_session_and_turn() {
    let _guard = lock_telemetry_test_state();
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
    begin_session_with_mode("openai", "gpt-5.4", None, false);
    record_turn();
    record_token_usage(100, 25, Some(200), Some(10));
    record_token_usage(50, 5, None, Some(2));

    {
        let guard = SESSION_STATE.lock().unwrap();
        let state = guard.as_ref().expect("session telemetry state");
        assert_eq!(state.input_tokens, 150);
        assert_eq!(state.output_tokens, 30);
        assert_eq!(state.cache_read_input_tokens, 200);
        assert_eq!(state.cache_creation_input_tokens, 12);
        assert_eq!(state.total_tokens, 392);
        let turn = state.current_turn.as_ref().expect("current turn");
        assert_eq!(turn.input_tokens, 150);
        assert_eq!(turn.output_tokens, 30);
        assert_eq!(turn.cache_read_input_tokens, 200);
        assert_eq!(turn.cache_creation_input_tokens, 12);
        assert_eq!(turn.total_tokens, 392);
    }
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
}

#[test]
fn test_record_connection_type_buckets_transport() {
    let _guard = lock_telemetry_test_state();
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
    begin_session_with_mode("openai", "gpt-5.4", None, false);
    record_connection_type("websocket/persistent-fresh");
    record_connection_type("websocket/persistent-reuse");
    record_connection_type("https/sse");
    record_connection_type("native http2");
    record_connection_type("cli subprocess");
    record_connection_type("weird-transport");

    {
        let guard = SESSION_STATE.lock().unwrap();
        let state = guard.as_ref().expect("session telemetry state");
        assert_eq!(state.transport_persistent_ws_fresh, 1);
        assert_eq!(state.transport_persistent_ws_reuse, 1);
        assert_eq!(state.transport_https, 1);
        assert_eq!(state.transport_native_http2, 1);
        assert_eq!(state.transport_cli_subprocess, 1);
        assert_eq!(state.transport_other, 1);
    }
    if let Ok(mut session) = SESSION_STATE.lock() {
        *session = None;
    }
}

#[test]
fn test_sanitize_telemetry_label_strips_ansi_and_controls() {
    assert_eq!(
        sanitize_telemetry_label("\u{1b}[1mclaude-opus-4-6\u{1b}[0m\n"),
        "claude-opus-4-6"
    );
}

#[test]
fn test_onboarding_step_event_serialization_includes_failure_reason() {
    let event = OnboardingStepEvent {
        event_id: "event-3".to_string(),
        id: "test-uuid".to_string(),
        session_id: None,
        event: "onboarding_step",
        version: "0.6.1".to_string(),
        os: "linux",
        arch: "x86_64",
        step: "auth_failed",
        auth_provider: Some("openai".to_string()),
        auth_method: Some("oauth".to_string()),
        auth_failure_reason: Some("callback_timeout".to_string()),
        milestone_elapsed_ms: Some(1234),
        schema_version: TELEMETRY_SCHEMA_VERSION,
        build_channel: "release".to_string(),
        is_git_checkout: false,
        is_ci: false,
        ran_from_cargo: false,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["step"], "auth_failed");
    assert_eq!(json["auth_failure_reason"], "callback_timeout");
}

#[test]
fn test_onboarding_step_milestone_key_includes_provider_and_method() {
    assert_eq!(
        onboarding_step_milestone_key("auth_success", Some("jcode"), Some("API key")),
        "auth_success_jcode_api_key"
    );
    assert_eq!(
        onboarding_step_milestone_key("login_picker_opened", None, None),
        "login_picker_opened"
    );
}

#[test]
fn test_install_marker_tracks_current_telemetry_id() {
    let _guard = lock_test_env();
    let prev_home = std::env::var_os("JCODE_HOME");
    let temp = tempfile::TempDir::new().expect("create temp dir");
    jcode_core::env::set_var("JCODE_HOME", temp.path());

    assert!(!install_recorded_for_id("id-a"));
    mark_install_recorded("id-a");
    assert!(install_recorded_for_id("id-a"));
    assert!(!install_recorded_for_id("id-b"));

    if let Some(prev_home) = prev_home {
        jcode_core::env::set_var("JCODE_HOME", prev_home);
    } else {
        jcode_core::env::remove_var("JCODE_HOME");
    }
}

/// Guards the fork's second privacy invariant: even an opted-in user sends
/// nothing unless an operator explicitly names a collector. Upstream's default
/// was a Cloudflare Worker this fork does not run.
#[test]
fn telemetry_endpoint_has_no_default() {
    let _guard = lock_test_env();
    let prev = std::env::var_os("JCODE_TELEMETRY_ENDPOINT");
    jcode_core::env::remove_var("JCODE_TELEMETRY_ENDPOINT");
    assert!(
        crate::consent::telemetry_endpoint().is_none(),
        "no endpoint may be baked in"
    );

    for blank in ["", "   "] {
        jcode_core::env::set_var("JCODE_TELEMETRY_ENDPOINT", blank);
        assert!(
            crate::consent::telemetry_endpoint().is_none(),
            "blank endpoint {blank:?} must not be treated as configured"
        );
    }

    jcode_core::env::set_var("JCODE_TELEMETRY_ENDPOINT", " https://collector.example/v1 ");
    assert_eq!(
        crate::consent::telemetry_endpoint().as_deref(),
        Some("https://collector.example/v1")
    );
    restore_env("JCODE_TELEMETRY_ENDPOINT", prev);
}

/// A source-level guard: the upstream maintainer's telemetry host must not
/// reappear anywhere in this crate, whether as a default, a fallback, or a
/// "temporary" constant. Behavioral tests would not catch a constant that is
/// only reached on a path they do not exercise.
#[test]
fn upstream_telemetry_host_is_absent_from_source() {
    // Every file in the crate, not just the one the constant used to live in:
    // the policy moved to consent.rs once already, and a guard that only knows
    // where something used to be stops guarding the moment it moves.
    for (name, source) in [
        ("lib.rs", include_str!("lib.rs")),
        ("consent.rs", include_str!("consent.rs")),
        ("lifecycle.rs", include_str!("lifecycle.rs")),
        ("state_support.rs", include_str!("state_support.rs")),
        ("tests.rs", include_str!("tests.rs")),
    ] {
        // Split so this test's own text does not match itself.
        let host = concat!("jcode-telemetry.", "jeremyhuang55555", ".workers.dev");
        assert!(
            !source.contains(host),
            "{name} must not reference the upstream telemetry host"
        );
    }
}
