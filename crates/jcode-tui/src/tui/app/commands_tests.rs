use super::ensure_swarm_prompt_edit_path;
use super::parse_diff_mode_name;
use super::parse_manual_subagent_spec;

#[test]
fn parse_diff_mode_name_maps_known_aliases() {
    use crate::config::DiffDisplayMode;
    assert_eq!(parse_diff_mode_name("off"), Some(DiffDisplayMode::Off));
    assert_eq!(parse_diff_mode_name("none"), Some(DiffDisplayMode::Off));
    assert_eq!(
        parse_diff_mode_name("inline"),
        Some(DiffDisplayMode::Inline)
    );
    assert_eq!(parse_diff_mode_name("on"), Some(DiffDisplayMode::Inline));
    assert_eq!(
        parse_diff_mode_name("full"),
        Some(DiffDisplayMode::FullInline)
    );
    assert_eq!(
        parse_diff_mode_name("pinned"),
        Some(DiffDisplayMode::Pinned)
    );
    assert_eq!(parse_diff_mode_name("file"), Some(DiffDisplayMode::File));
}

#[test]
fn parse_diff_mode_name_is_case_insensitive_and_trims() {
    use crate::config::DiffDisplayMode;
    assert_eq!(
        parse_diff_mode_name("  PINNED "),
        Some(DiffDisplayMode::Pinned)
    );
}

#[test]
fn parse_diff_mode_name_rejects_unknown() {
    assert_eq!(parse_diff_mode_name("sidebyside"), None);
    assert_eq!(parse_diff_mode_name(""), None);
}

#[test]
fn parse_manual_subagent_spec_accepts_flags_and_prompt() {
    let spec = parse_manual_subagent_spec(
        "--type research --model gpt-5.4 --continue session_123 investigate this bug",
    )
    .expect("parse manual subagent spec");

    assert_eq!(spec.subagent_type, "research");
    assert_eq!(spec.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(spec.session_id.as_deref(), Some("session_123"));
    assert_eq!(spec.prompt, "investigate this bug");
}

#[test]
fn parse_manual_subagent_spec_rejects_missing_prompt() {
    let err = parse_manual_subagent_spec("--model gpt-5.4")
        .expect_err("missing prompt should be rejected");
    assert!(err.contains("Missing prompt"));
}

#[test]
fn swarm_prompt_edit_path_prefers_nonblank_project_override() {
    let project = tempfile::tempdir().expect("project tempdir");
    let jcode_home = tempfile::tempdir().expect("jcode tempdir");
    let project_prompt = project.path().join(".jcode/swarm-prompt.md");
    std::fs::create_dir_all(project_prompt.parent().expect("prompt parent"))
        .expect("create project config dir");
    std::fs::write(&project_prompt, "project routing").expect("write project prompt");
    std::fs::write(jcode_home.path().join("swarm-prompt.md"), "global routing")
        .expect("write global prompt");

    let path = ensure_swarm_prompt_edit_path(project.path().to_str(), jcode_home.path())
        .expect("resolve prompt path");
    assert_eq!(path, project_prompt);
}

#[test]
fn swarm_prompt_edit_path_falls_back_to_nonblank_global_override() {
    let project = tempfile::tempdir().expect("project tempdir");
    let jcode_home = tempfile::tempdir().expect("jcode tempdir");
    let project_prompt = project.path().join(".jcode/swarm-prompt.md");
    std::fs::create_dir_all(project_prompt.parent().expect("prompt parent"))
        .expect("create project config dir");
    std::fs::write(&project_prompt, "  \n").expect("write blank project prompt");
    let global_prompt = jcode_home.path().join("swarm-prompt.md");
    std::fs::write(&global_prompt, "global routing").expect("write global prompt");

    let path = ensure_swarm_prompt_edit_path(project.path().to_str(), jcode_home.path())
        .expect("resolve prompt path");
    assert_eq!(path, global_prompt);
}

#[test]
fn swarm_prompt_edit_path_materializes_builtin_default_globally() {
    let project = tempfile::tempdir().expect("project tempdir");
    let jcode_home = tempfile::tempdir().expect("jcode tempdir");

    let path = ensure_swarm_prompt_edit_path(project.path().to_str(), jcode_home.path())
        .expect("create editable prompt");
    assert_eq!(path, jcode_home.path().join("swarm-prompt.md"));
    let content = std::fs::read_to_string(path).expect("read created prompt");
    assert_eq!(content.trim(), crate::prompt::DEFAULT_SWARM_PROMPT.trim());
}

#[test]
fn openrouter_402_payment_required_is_non_retryable() {
    use super::is_non_retryable_auto_poke_error;
    let err = "OpenAI-compatible chat request failed\n  endpoint: \
        https://openrouter.ai/api/v1/chat/completions\n  model: openai/gpt-5.4\n  \
        auth: OPENROUTER_API_KEY\n  status: 402 Payment Required\n  response: \
        {\"error\":{\"message\":\"This request requires more credits, or fewer max_tokens. \
        You requested up to 65536 tokens, but can only afford 34424. To increase, visit \
        https://openrouter.ai/settings/credits and add more credits\",\"code\":402}}";
    assert!(is_non_retryable_auto_poke_error(err));
}

#[test]
fn transient_server_error_remains_retryable_for_auto_poke() {
    use super::is_non_retryable_auto_poke_error;
    let err = "OpenAI-compatible chat request failed\n  status: 503 Service Unavailable";
    assert!(!is_non_retryable_auto_poke_error(err));
}

#[test]
fn volcengine_ark_unsupported_model_is_fatal_model_endpoint_error() {
    use super::{is_fatal_model_endpoint_error, is_non_retryable_auto_poke_error};
    let err = "OpenAI-compatible chat request failed\n  endpoint: \
        https://ark.cn-beijing.volces.com/api/coding/v3/chat/completions\n  model: \
        volcengine:ark-code-latest\n  auth: ARK_API_KEY\n  status: 404 Not Found\n  response: \
        {\"error\":{\"code\":\"UnsupportedModel\",\"message\":\"The requested model does not \
        support the coding plan feature.\"}}";
    // It is both a fatal model/endpoint error (fail fast, no retries) and a
    // non-retryable auto-poke error (don't keep poking).
    assert!(is_fatal_model_endpoint_error(err));
    assert!(is_non_retryable_auto_poke_error(err));
}

#[test]
fn transient_5xx_is_not_a_fatal_model_endpoint_error() {
    use super::is_fatal_model_endpoint_error;
    let err = "OpenAI-compatible chat request failed\n  status: 503 Service Unavailable";
    assert!(!is_fatal_model_endpoint_error(err));
}

#[test]
fn model_not_found_is_fatal_model_endpoint_error() {
    use super::is_fatal_model_endpoint_error;
    let err = "chat request failed: 404 model_not_found: The model `gpt-foo` does not exist";
    assert!(is_fatal_model_endpoint_error(err));
}

// The todo completion-confidence gate queues a continuation reminder asking the
// model to validate further and reassess. Nothing about answering that reminder
// changes the todo list, so the gate must not re-fire against unchanged state:
// an unguarded gate re-queues the identical reminder every turn, producing an
// unbounded empty-content send loop at model round-trip speed.
// See docs/fork/ideal-base/human-noticed-issues/BLANK_CONTINUATION_TURN.md.
#[test]
fn todo_gate_fingerprint_is_stable_for_unchanged_todos() {
    use super::super::todos_view::todo_gate_fingerprint;
    let todos = vec![gate_todo("a", "completed", "high", Some(50))];
    assert_eq!(
        todo_gate_fingerprint(&todos),
        todo_gate_fingerprint(&todos.clone()),
        "an unchanged todo list must fingerprint identically, or the gate re-fires forever"
    );
}

#[test]
fn todo_gate_fingerprint_changes_when_the_gate_verdict_could_change() {
    use super::super::todos_view::todo_gate_fingerprint;
    let base = vec![gate_todo("a", "completed", "high", Some(50))];
    let baseline = todo_gate_fingerprint(&base);

    // Every field `todo_confidence_summary` reads must re-arm the gate.
    let raised = vec![gate_todo("a", "completed", "high", Some(99))];
    assert_ne!(
        baseline,
        todo_gate_fingerprint(&raised),
        "raising completion_confidence must re-arm the gate"
    );

    let missing = vec![gate_todo("a", "completed", "high", None)];
    assert_ne!(
        baseline,
        todo_gate_fingerprint(&missing),
        "clearing completion_confidence must re-arm the gate"
    );

    let reprioritized = vec![gate_todo("a", "completed", "low", Some(50))];
    assert_ne!(
        baseline,
        todo_gate_fingerprint(&reprioritized),
        "priority changes the confidence weighting, so it must re-arm the gate"
    );

    let reopened = vec![gate_todo("a", "in_progress", "high", Some(50))];
    assert_ne!(
        baseline,
        todo_gate_fingerprint(&reopened),
        "reopening a todo must re-arm the gate"
    );

    let mut appended = base.clone();
    appended.push(gate_todo("b", "completed", "high", Some(50)));
    assert_ne!(
        baseline,
        todo_gate_fingerprint(&appended),
        "adding a todo must re-arm the gate"
    );

    let renamed = vec![gate_todo("z", "completed", "high", Some(50))];
    assert_ne!(
        baseline,
        todo_gate_fingerprint(&renamed),
        "replacing a todo must re-arm the gate"
    );
}

fn gate_todo(
    id: &str,
    status: &str,
    priority: &str,
    completion_confidence: Option<u8>,
) -> crate::todo::TodoItem {
    crate::todo::TodoItem {
        content: format!("todo {id}"),
        status: status.to_string(),
        priority: priority.to_string(),
        id: id.to_string(),
        completion_confidence,
        ..Default::default()
    }
}

/// R08(c): `needs_more_work` must distinguish "no completed items to assess"
/// from "the completed items scored badly".
///
/// `unwrap_or(true)` on an empty completed set reported the two identically, so
/// an all-cancelled list was told its completion confidence was insufficient.
/// That is a false statement about work that was never assessed, and it is what
/// sent the auto-poke into the completion gate for a list nobody had scored.
#[test]
fn all_cancelled_todos_have_nothing_to_assess_rather_than_needing_work() {
    use super::super::todos_view::todo_confidence_summary;

    let cancelled = vec![gate_todo("a", "cancelled", "high", None)];
    let summary = todo_confidence_summary(&cancelled);
    assert!(
        !summary.needs_more_work,
        "a list with no completed items has nothing to assess, so it cannot need more validation"
    );
    assert_eq!(
        summary.completion_average, None,
        "an unassessed list still has no average to report"
    );

    // The contrast case must keep failing: completed work that scored low
    // genuinely does need more validation.
    let low = vec![gate_todo("a", "completed", "high", Some(10))];
    assert!(
        todo_confidence_summary(&low).needs_more_work,
        "low-scoring completed work must still be flagged"
    );
}

/// Guards the equivalence that lets `needs_more_work` drop its `unwrap_or(true)`.
///
/// `completion_average` is `None` exactly when no completed item carries a
/// score. With at least one completed item that means every one of them is
/// missing a score, so `missing_completion_confidence > 0` already fires and
/// the `None` arm cannot change the verdict. If someone later makes the average
/// `None` for a different reason, this test fails and the arm has to come back.
#[test]
fn completed_todos_without_scores_still_need_work_without_the_none_arm() {
    use super::super::todos_view::todo_confidence_summary;

    let unscored = vec![
        gate_todo("a", "completed", "high", None),
        gate_todo("b", "completed", "low", None),
    ];
    let summary = todo_confidence_summary(&unscored);
    assert_eq!(
        summary.completion_average, None,
        "no scores means no average"
    );
    assert!(
        summary.needs_more_work,
        "completed work nobody scored must still be flagged, via the missing count"
    );
}
