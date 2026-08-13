use super::{
    ambient_widget_data_from, build_resume_command, effort_display_label,
    extract_bracketed_system_message, format_countdown_until, inferred_reasoning_efforts,
    partition_queued_messages, pretty_model_display_name, resume_invocation_args,
};
use crate::ambient::{Priority, ScheduleTarget, ScheduledItem};
use crate::terminal_launch::{detected_resume_terminal, shell_command};
use crate::tui::session_picker::ResumeTarget;
use chrono::{Duration as ChronoDuration, Utc};

use crate::storage::EnvVarGuard;
use crate::tui::app::test_support::with_temp_jcode_home;

#[test]
fn extract_bracketed_system_message_strips_wrapper() {
    let parsed = extract_bracketed_system_message(
        "[SYSTEM: Your session was interrupted. Continue immediately.]",
    );
    assert_eq!(
        parsed.as_deref(),
        Some("Your session was interrupted. Continue immediately.")
    );
}

#[test]
fn partition_queued_messages_moves_system_messages_into_reminders() {
    let (user_messages, reminder, display_system_messages) = partition_queued_messages(
        vec![
            "[SYSTEM: Continue where you left off.]".to_string(),
            "normal user input".to_string(),
        ],
        vec!["hidden reminder".to_string()],
    );

    assert_eq!(user_messages, vec!["normal user input"]);
    assert_eq!(
        display_system_messages,
        vec!["Continue where you left off."]
    );
    assert_eq!(
        reminder.as_deref(),
        Some("hidden reminder\n\nContinue where you left off.")
    );
}

#[test]
fn inferred_reasoning_efforts_use_provider_specific_order_and_max_semantics() {
    assert_eq!(
        inferred_reasoning_efforts(Some("anthropic"), Some("claude-sonnet-4-6")),
        vec![
            "none",
            "low",
            "medium",
            "high",
            "max",
            "swarm",
            "swarm-deep"
        ]
    );
    assert_eq!(
        inferred_reasoning_efforts(Some("anthropic"), Some("claude-opus-4-7")),
        vec![
            "none",
            "low",
            "medium",
            "high",
            "xhigh",
            "max",
            "swarm",
            "swarm-deep"
        ]
    );
    assert_eq!(
        inferred_reasoning_efforts(Some("openrouter"), Some("anthropic/claude-sonnet-4.6")),
        vec![
            "none",
            "low",
            "medium",
            "high",
            "xhigh",
            "swarm",
            "swarm-deep"
        ]
    );
    assert_eq!(
        inferred_reasoning_efforts(Some("openrouter"), Some("deepseek/deepseek-r1")),
        vec![
            "none",
            "low",
            "medium",
            "high",
            "xhigh",
            "swarm",
            "swarm-deep"
        ],
        "OpenRouter uses unified reasoning where max is only an alias, not a cycle level"
    );
    assert_eq!(
        inferred_reasoning_efforts(Some("deepseek"), Some("deepseek-v4-pro")),
        vec![
            "none",
            "low",
            "medium",
            "high",
            "max",
            "swarm",
            "swarm-deep"
        ],
        "DeepSeek direct keeps max as a real provider level"
    );
    assert!(inferred_reasoning_efforts(Some("ollama"), Some("llama3")).is_empty());
}

#[test]
fn swarm_effort_display_labels_are_marked_beta() {
    assert_eq!(
        effort_display_label("swarm"),
        "Swarm (light fan-out) [Beta]"
    );
    assert_eq!(
        effort_display_label("swarm-deep"),
        "Swarm Deep (Max + task graph) [Beta]"
    );
    assert_eq!(effort_display_label("high"), "High");
}

#[cfg(unix)]
#[test]
fn detected_resume_terminal_recognizes_handterm_term_program() {
    with_temp_jcode_home(|| {
        let _guard = EnvVarGuard::set("TERM_PROGRAM", "handterm");
        assert_eq!(detected_resume_terminal().as_deref(), Some("handterm"));
    });
}

#[cfg(unix)]
#[test]
fn shell_command_quotes_single_quotes_for_handterm_exec() {
    let command = shell_command(&[
        "/tmp/jcode binary".to_string(),
        "--resume".to_string(),
        "session'quote".to_string(),
    ]);
    assert_eq!(
        command,
        "'/tmp/jcode binary' '--resume' 'session'\"'\"'quote'"
    );
}

#[test]
fn resume_invocation_args_includes_socket_when_present() {
    let args = resume_invocation_args("ses_123", Some("/tmp/jcode-test.sock"));
    assert_eq!(
        args,
        vec![
            "--fresh-spawn".to_string(),
            "--resume".to_string(),
            "ses_123".to_string(),
            "--socket".to_string(),
            "/tmp/jcode-test.sock".to_string()
        ]
    );
}

#[test]
fn resume_invocation_args_omits_blank_socket() {
    let args = resume_invocation_args("ses_123", Some("   "));
    assert_eq!(
        args,
        vec![
            "--fresh-spawn".to_string(),
            "--resume".to_string(),
            "ses_123".to_string()
        ]
    );
}

/// Pin JCODE_HOME to a tempdir containing a published `current/jcode` binary so
/// `launch_client_executable()` resolves deterministically, independent of
/// whether the developer machine has a local published build and of other tests
/// mutating JCODE_HOME in parallel. Returns the guards that keep the
/// environment pinned for the duration of the test.
///
/// F20c: the fixture must write the SINGLE fixed publish path
/// (`$JCODE_HOME/current/jcode`); the old `builds/current` channel is no longer
/// read by any resolver, so a fixture writing it would silently stop pinning.
fn pinned_resume_test_home() -> (
    EnvVarGuard,
    tempfile::TempDir,
    crate::tui::app::test_support::TestEnvWriteScope,
) {
    let env_lock = crate::tui::app::test_support::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let current = temp.path().join("current");
    std::fs::create_dir_all(&current).expect("create current dir");
    std::fs::write(current.join("jcode"), b"#!/bin/sh\n").expect("write fake jcode binary");
    let home = EnvVarGuard::set("JCODE_HOME", temp.path());
    // Tuple fields drop in DECLARATION order, so the lease must come last: it
    // has to outlive the `EnvVarGuard` that restores `JCODE_HOME`. Returned the
    // other way round, this fixture released the lease first and then restored
    // the variable, leaving a window in which the next test to take the lease
    // had its `JCODE_HOME` overwritten by this one's teardown. That is exactly
    // how `build_resume_command_uses_imported_jcode_session_for_codex` failed
    // on Linux CI: resolution fell through to `current_exe()` and the assert
    // saw the test binary (`jcode_tui-<hash>`) instead of `jcode`.
    (home, temp, env_lock)
}

#[test]
fn build_resume_command_uses_imported_jcode_session_for_claude_code() {
    let _pinned = pinned_resume_test_home();
    let (program, args, title) = build_resume_command(
        &ResumeTarget::ClaudeCodeSession {
            session_id: "claude-session-123".to_string(),
            session_path: "/tmp/claude-session-123.jsonl".to_string(),
        },
        None,
    );

    assert_eq!(
        program.file_name().and_then(|name| name.to_str()),
        Some("jcode")
    );
    assert_eq!(
        args,
        vec![
            "--fresh-spawn".to_string(),
            "--resume".to_string(),
            crate::import::imported_claude_code_session_id("claude-session-123")
        ]
    );
    assert!(title.contains("Claude Code"));
    assert!(title.contains("claude-s"));
}

#[test]
fn build_resume_command_uses_imported_jcode_session_for_codex() {
    let _pinned = pinned_resume_test_home();
    let (program, args, title) = build_resume_command(
        &ResumeTarget::CodexSession {
            session_id: "codex-session-123".to_string(),
            session_path: "/tmp/codex-session-123.jsonl".to_string(),
        },
        None,
    );

    assert_eq!(
        program.file_name().and_then(|name| name.to_str()),
        Some("jcode")
    );
    assert_eq!(
        args,
        vec![
            "--fresh-spawn".to_string(),
            "--resume".to_string(),
            crate::import::imported_codex_session_id("codex-session-123")
        ]
    );
    assert!(title.contains("Codex"));
}

#[test]
fn format_countdown_until_handles_subminute_and_minutes() {
    let soon = Utc::now() + ChronoDuration::seconds(25);
    let medium = Utc::now() + ChronoDuration::minutes(2) + ChronoDuration::seconds(15);

    let soon_text = format_countdown_until(soon);
    let medium_text = format_countdown_until(medium);

    assert!(soon_text.starts_with("in "));
    assert!(soon_text.ends_with('s'));
    assert!(medium_text.starts_with("in 2m"));
}

#[test]
fn gather_ambient_info_filters_to_session_reminders_when_ambient_disabled() {
    // `ambient_widget_data_from` is pure over the slice it is handed, so this
    // regression builds the items directly instead of routing through
    // `AmbientManager`.
    //
    // The manager resolves its queue path from `JCODE_HOME` at construction
    // time and loads `ambient/queue.json` from disk. That made the observed
    // count depend on state this test does not own: it failed once with
    // `queue_count == 8` after scheduling three items, and 8 = 5 loaded + 3
    // scheduled is unreachable from the fresh temp home the test sets up. The
    // manager had therefore resolved some other home, which a fresh tempdir
    // cannot explain and which no assertion here could diagnose.
    //
    // Constructing the items removes the filesystem from a test about queue
    // filtering entirely.
    fn item(id: &str, minutes: i64, description: &str, target: ScheduleTarget) -> ScheduledItem {
        ScheduledItem {
            id: id.to_string(),
            scheduled_for: Utc::now() + ChronoDuration::minutes(minutes),
            context: format!("{description} context"),
            priority: Priority::Normal,
            target,
            created_by_session: "session_1".to_string(),
            created_at: Utc::now(),
            working_dir: None,
            task_description: Some(description.to_string()),
            relevant_files: Vec::new(),
            git_branch: None,
            additional_context: None,
        }
    }

    let session = || ScheduleTarget::Session {
        session_id: "session_1".to_string(),
    };
    let items = vec![
        item("sched_ambient", 5, "ambient work", ScheduleTarget::Ambient),
        item("sched_first", 5, "first reminder", session()),
        item("sched_second", 10, "second reminder", session()),
    ];

    let info = ambient_widget_data_from(crate::ambient::AmbientState::default(), &items, false)
        .expect("ambient info");

    // Ambient is disabled, so the widget must show only the two directly
    // delivered session reminders while still counting the whole queue.
    assert!(info.show_widget);
    assert_eq!(info.queue_count, 3);
    assert_eq!(info.reminder_count, 2);
    assert_eq!(
        info.next_reminder_preview.as_deref(),
        Some("first reminder")
    );
    assert!(
        info.next_reminder_wake
            .as_deref()
            .is_some_and(|text| text.starts_with("in 4m") || text.starts_with("in 5m"))
    );
}

#[test]
fn pretty_model_display_name_formats_common_models() {
    assert_eq!(pretty_model_display_name("gpt-5.5"), "GPT-5.5");
    assert_eq!(pretty_model_display_name("gpt-5.1-codex"), "GPT-5.1-codex");
    assert_eq!(
        pretty_model_display_name("claude-opus-4-8"),
        "Claude Opus 4.8"
    );
    assert_eq!(
        pretty_model_display_name("claude-sonnet-4-5"),
        "Claude Sonnet 4.5"
    );
    assert_eq!(
        pretty_model_display_name("claude-opus-4-8[1m]"),
        "Claude Opus 4.8 (1M)"
    );
    assert_eq!(
        pretty_model_display_name("gemini-2.5-pro"),
        "Gemini 2.5 Pro"
    );
}

#[test]
fn pretty_model_display_name_handles_empty_and_unknown() {
    assert_eq!(pretty_model_display_name(""), "your default model");
    assert_eq!(pretty_model_display_name("   "), "your default model");
    // Unknown shapes fall back to a title-cased dashed rendering.
    assert_eq!(
        pretty_model_display_name("some-new-model"),
        "Some New Model"
    );
}

#[test]
fn invalidate_todos_cache_backdates_entry_so_next_gather_refetches() {
    use super::{
        clear_todos_cache_for_tests, gather_todos_and_goals_for_session, invalidate_todos_cache,
        todos_cache_entry_age_for_tests,
    };

    let _env_lock = crate::tui::app::test_support::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _home = EnvVarGuard::set("JCODE_HOME", temp.path());
    clear_todos_cache_for_tests();

    let session_id = "freshness-test-session";

    // No entry yet.
    assert_eq!(todos_cache_entry_age_for_tests(session_id), None);

    // First gather seeds the cache entry (and spawns the initial fetch). The
    // entry exists immediately, marked as actively refreshing / freshly stamped.
    let _ = gather_todos_and_goals_for_session(Some(session_id));
    let before = todos_cache_entry_age_for_tests(session_id);
    assert!(before.is_some(), "first gather must seed a cache entry");

    // Let the background fetch settle so we have a non-refreshing, fresh entry.
    std::thread::sleep(std::time::Duration::from_millis(50));
    let _ = gather_todos_and_goals_for_session(Some(session_id));
    std::thread::sleep(std::time::Duration::from_millis(50));
    let settled = todos_cache_entry_age_for_tests(session_id)
        .expect("entry should exist after gather settles");
    assert!(
        settled.0 < 5,
        "a freshly fetched entry should be recent, got age={}s",
        settled.0
    );

    // Invalidation backdates the timestamp far past the TTL and clears the
    // refreshing flag, so the next gather treats it as expired and refetches.
    invalidate_todos_cache(session_id);
    let after = todos_cache_entry_age_for_tests(session_id)
        .expect("entry should still exist after invalidation");
    assert!(
        after.0 >= 1000,
        "invalidation must backdate the entry well past the 1s TTL, got age={}s",
        after.0
    );
    assert!(
        !after.1,
        "invalidation must clear the refreshing flag so the next gather refetches"
    );
}

#[test]
fn fresh_session_command_includes_fresh_spawn_and_socket() {
    let command = super::build_fresh_session_command(Some("/tmp/test.sock"));
    assert!(command.fresh_spawn, "must hand off as a fresh spawn");
    assert_eq!(command.kind.as_deref(), Some("new-terminal"));
    assert_eq!(command.title.as_deref(), Some("jcode · new session"));
    assert_eq!(
        command.args,
        vec![
            "--fresh-spawn".to_string(),
            "--socket".to_string(),
            "/tmp/test.sock".to_string(),
        ]
    );
}

#[test]
fn fresh_session_command_omits_blank_socket() {
    let command = super::build_fresh_session_command(Some("   "));
    assert_eq!(command.args, vec!["--fresh-spawn".to_string()]);
    let command = super::build_fresh_session_command(None);
    assert_eq!(command.args, vec!["--fresh-spawn".to_string()]);
}

/// Regression for issue #424: `Instant::now() - Duration` panics with
/// "overflow when subtracting duration from instant" when the monotonic clock
/// epoch (boot time) is more recent than the backdate amount. `backdated_now`
/// must saturate instead of panicking, and still return a value in the past
/// when possible so TTL checks treat the entry as expired.
#[test]
fn backdated_now_never_panics_and_prefers_past_instants() {
    use std::time::{Duration, Instant};

    let now = Instant::now();

    // Typical case: small backdate should land in the past.
    let recent = super::backdated_now(Duration::from_millis(10));
    assert!(recent <= now, "backdated instant must not be in the future");

    // Huge backdate (longer than any plausible uptime) must not panic and
    // must still return something no later than now.
    let ancient = super::backdated_now(Duration::from_secs(60 * 60 * 24 * 365 * 100));
    assert!(
        ancient <= now,
        "saturated backdate must not be in the future"
    );

    // Zero backdate is a no-op.
    let zero = super::backdated_now(Duration::ZERO);
    assert!(zero <= Instant::now());
}

/// A queue holding only bracketed system messages leaves no user text behind.
/// Both dispatch sites in `remote.rs` gate on `!queued_messages.is_empty()` and
/// then send `messages.join("\n\n")` as the user body, so this case sends "".
/// The `auto_retry` flag they compute (`reminder.is_some() && messages.is_empty()`)
/// records that this is a real state, not a hypothetical one.
#[test]
fn partition_queued_messages_yields_no_user_text_when_queue_is_all_system() {
    let (user_messages, reminder, _display) = partition_queued_messages(
        vec![
            "[SYSTEM: Continue where you left off.]".to_string(),
            "[SYSTEM: Todo confidence needs validation.]".to_string(),
        ],
        Vec::new(),
    );

    assert!(user_messages.is_empty());
    assert!(reminder.is_some());
    assert_eq!(
        user_messages.join("\n\n"),
        "",
        "the dispatch sites send this join result as the user body"
    );
}
