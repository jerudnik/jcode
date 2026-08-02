use crate::storage;
use anyhow::Result;
use std::path::PathBuf;

pub use jcode_task_types::{TodoGoal, TodoItem, is_terminal_todo_status};

/// Minimum passing score for 0-100 quality assessments. Scores below this do
/// not provide enough evidence to clear their respective quality gate.
pub const QUALITY_GATE_THRESHOLD: u8 = 96;

/// Goals with a hill-climbability score strictly below this are considered
/// low: no credible metric to iterate against. The todo tool nudges the model
/// on every applicable write to reframe the objective into something
/// quantifiable and verifiable.
pub const LOW_HILL_CLIMBABILITY: u8 = QUALITY_GATE_THRESHOLD;

/// Model-facing continuation for the private hill-climbability check. Names the
/// assessment category without disclosing the score or threshold.
pub const TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE: &str = "Your todo update was saved. Your hill-climbability is not high enough, so this is a nudge, not a rejection: nothing was discarded and you do not need to resend the same content. Improve the goal's objective and feedback loop so progress can be measured across iterations, then call the todo tool again with the revised goal before continuing the task. The goal is to create a strong feedback loop you can iterate against.";

/// Model-facing continuation for the private end-to-end ownership check. Names
/// the assessment category without disclosing the score or threshold.
pub const TODO_OWNERSHIP_CONTINUATION_MESSAGE: &str = "Your end-to-end ownership is not high enough to complete this goal. Take ownership of the full user outcome, not just the immediate implementation. Follow the work through every relevant integration and runtime path, resolve consequential gaps, validate the complete workflow, and finish the necessary follow-through.";

/// Model-facing continuation for private completion-confidence checks. Names
/// the assessment category without disclosing scores, items, or thresholds.
pub const TODO_COMPLETION_CONTINUATION_MESSAGE: &str = "Your completion confidence is missing or not high enough. Validate the completed result more thoroughly, address any remaining issues, and then reassess whether the work is ready to finalize.";
const LEGACY_TODO_CONFIDENCE_SUMMARY_PREFIX: &str = "All todos are done. Todo confidence summary:";

fn normalized_group(group: Option<&str>) -> Option<String> {
    group
        .map(str::trim)
        .filter(|group| !group.is_empty())
        .map(str::to_string)
}

fn group_is_complete(todos: &[TodoItem], group: &Option<String>) -> bool {
    let mut matching = todos
        .iter()
        .filter(|todo| normalized_group(todo.group.as_deref()) == *group)
        .peekable();
    matching.peek().is_some() && matching.all(|todo| todo.status == "completed")
}

/// Why a newly completed group failed the ownership gate.
///
/// The gate used to answer `bool`, so all three faults had to share one
/// sentence -- and that sentence described only the third. Telling a model its
/// ownership is "not high enough" when the real fault is a mistyped group label
/// sends it to re-score work that was already scored, which cannot ever clear
/// the gate. Naming the fault is what makes the nudge actionable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipFaultKind {
    /// No goal carries this group's label. Usually a label mismatch, so the
    /// assessment exists but is filed under a name nothing matches.
    NoGoalForGroup,
    /// A goal matches, but it carries no ownership assessment at all.
    NoOwnershipScore,
    /// Assessed, and found wanting. The only fault the old sentence described.
    BelowThreshold,
}

/// A single group's ownership fault, carrying enough to name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnershipFault {
    /// The group that failed, or `None` for the ungrouped list.
    pub group: Option<String>,
    pub kind: OwnershipFaultKind,
}

/// Shared lead-in for ownership nudges.
///
/// Keeps two promises the old wording broke: the write survived, and the
/// specific fault follows. `is_auto_poke_message` matches on this so the
/// continuation is still recognized as machine-initiated rather than typed.
pub const TODO_OWNERSHIP_FAULT_PREFIX: &str = "Your todo update was saved. This is a nudge about end-to-end ownership, not a rejection: nothing was discarded.";

impl OwnershipFault {
    fn label(&self) -> String {
        match &self.group {
            Some(group) => format!("\"{group}\""),
            None => "the ungrouped list".to_string(),
        }
    }

    /// The model-facing nudge. Names the group and the specific fault while
    /// keeping the private calibration (score and threshold) undisclosed.
    pub fn message(&self) -> String {
        let label = self.label();
        let detail = match self.kind {
            OwnershipFaultKind::NoGoalForGroup => format!(
                "The group {label} is now complete, but no goal assessment carries that exact label, so its ownership was never assessed. Check that the goal's group matches the todos' group, or add a goal for {label}."
            ),
            OwnershipFaultKind::NoOwnershipScore => format!(
                "The group {label} is now complete and has a goal, but that goal carries no end_to_end_ownership assessment. Assess it before finalizing."
            ),
            OwnershipFaultKind::BelowThreshold => {
                format!("The group {label} is now complete. {TODO_OWNERSHIP_CONTINUATION_MESSAGE}")
            }
        };
        format!("{TODO_OWNERSHIP_FAULT_PREFIX} {detail}")
    }
}

/// The first ownership fault among the groups this update newly closed, if any.
///
/// Groups completed before this check was introduced are intentionally
/// grandfathered so existing sessions stay writable.
pub fn ownership_fault(
    previous: &[TodoItem],
    incoming: &[TodoItem],
    goals: &[TodoGoal],
) -> Option<OwnershipFault> {
    let mut groups: Vec<Option<String>> = Vec::new();
    for todo in incoming {
        let group = normalized_group(todo.group.as_deref());
        if !groups.contains(&group) {
            groups.push(group);
        }
    }

    groups.into_iter().find_map(|group| {
        if !group_is_complete(incoming, &group) || group_is_complete(previous, &group) {
            return None;
        }
        let kind = match goals
            .iter()
            .find(|goal| normalized_group(goal.group.as_deref()) == group)
        {
            None => OwnershipFaultKind::NoGoalForGroup,
            Some(goal) => match goal.end_to_end_ownership {
                None => OwnershipFaultKind::NoOwnershipScore,
                Some(score) if score < QUALITY_GATE_THRESHOLD => OwnershipFaultKind::BelowThreshold,
                Some(_) => return None,
            },
        };
        Some(OwnershipFault { group, kind })
    })
}

/// Whether every group newly closed by this update has a sufficient assessment
/// of ownership over its full outcome.
pub fn newly_completed_groups_have_sufficient_ownership(
    previous: &[TodoItem],
    incoming: &[TodoItem],
    goals: &[TodoGoal],
) -> bool {
    ownership_fault(previous, incoming, goals).is_none()
}

/// Build the synthetic auto-poke continuation prompt sent when the model
/// stops with incomplete todos. Kept here so every producer (TUI auto-poke,
/// `jcode run` auto-poke) and the transcript renderer agree on the exact text.
pub fn build_auto_poke_message(incomplete_count: usize) -> String {
    build_auto_poke_message_with_blocked(incomplete_count, &[])
}

/// Build the auto-poke prompt, naming any work that is blocked rather than
/// actionable.
///
/// A blocked item cannot be advanced by "continue working", so folding it into
/// the incomplete count produces an instruction the model cannot follow, and
/// omitting it silently makes the remaining count look like the whole picture.
/// Naming it separately keeps the count honest about what is actionable while
/// still disclosing that the list is not finished.
pub fn build_auto_poke_message_with_blocked(
    incomplete_count: usize,
    blocked: &[(String, Vec<String>)],
) -> String {
    let mut message = format!(
        "You have {} incomplete todo{}.",
        incomplete_count,
        if incomplete_count == 1 { "" } else { "s" },
    );
    if blocked.is_empty() {
        // Byte-identical to the long-standing single-line poke, which is
        // persisted in existing sessions and matched by the renderer.
        message.push(' ');
    } else {
        message.push_str(&format!(
            "\n\n{} other todo{} blocked and not counted above:",
            blocked.len(),
            if blocked.len() == 1 { " is" } else { "s are" },
        ));
        for (content, blockers) in blocked {
            message.push_str(&format!(
                "\n- {} (blocked by: {})",
                content,
                blockers.join(", ")
            ));
        }
        message.push_str("\n\n");
    }
    // The trailing sentence is load-bearing: `is_auto_poke_message` anchors on
    // it to tell a synthetic poke from a real user turn, so the blocked
    // disclosure goes above it rather than after it.
    message.push_str("Continue working, or update the todo tool.");
    message
}

/// True when `message` is a synthetic auto-poke continuation (the
/// incomplete-todos poke or the todo confidence summary) rather than a real
/// user prompt.
///
/// These are persisted as `Role::User` so the model treats them as a normal
/// continuation turn, but they are not something the user typed. The live UI
/// hides them (showing an "Auto-poking..." notice instead), and the session
/// renderer uses this to avoid re-rendering them as user prompts on
/// reload/resume/remote attach.
pub fn is_auto_poke_message(message: &str) -> bool {
    let trimmed = message.trim();
    (trimmed.starts_with("You have ")
        && trimmed.contains(" incomplete todo")
        && trimmed.ends_with("update the todo tool."))
        || trimmed.starts_with(TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE)
        || trimmed.starts_with(TODO_OWNERSHIP_CONTINUATION_MESSAGE)
        || trimmed.starts_with(TODO_OWNERSHIP_FAULT_PREFIX)
        || trimmed.starts_with(TODO_COMPLETION_CONTINUATION_MESSAGE)
        || trimmed.starts_with(LEGACY_TODO_CONFIDENCE_SUMMARY_PREFIX)
}

pub fn load_todos(session_id: &str) -> Result<Vec<TodoItem>> {
    let path = todo_path(session_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    storage::read_json(&path).or_else(|_| Ok(Vec::new()))
}

pub fn todos_exist(session_id: &str) -> Result<bool> {
    Ok(todo_path(session_id)?.exists())
}

pub fn save_todos(session_id: &str, todos: &[TodoItem]) -> Result<()> {
    let path = todo_path(session_id)?;
    storage::write_json_fast(&path, todos)
}

fn todo_path(session_id: &str) -> Result<PathBuf> {
    let base = storage::jcode_dir()?;
    Ok(base.join("todos").join(format!("{}.json", session_id)))
}

/// Goal-level assessments live beside the todo list in a separate file so the
/// todo list format (a bare `Vec<TodoItem>` array) stays readable by every
/// existing consumer.
pub fn load_goals(session_id: &str) -> Result<Vec<TodoGoal>> {
    let path = goals_path(session_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    storage::read_json(&path).or_else(|_| Ok(Vec::new()))
}

/// Derive a concise session-title hint from the todo tool's persisted plan.
///
/// Todo groups are intended to name coherent goals, so the group containing the
/// current (or latest incomplete) item is the strongest signal. Ungrouped plans
/// fall back to their measurable objective, then the item text itself.
pub fn derive_session_title(todos: &[TodoItem], goals: &[TodoGoal]) -> Option<String> {
    fn non_empty(value: Option<&str>) -> Option<String> {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    let current = todos
        .iter()
        .rev()
        .find(|todo| todo.status.eq_ignore_ascii_case("in_progress"))
        .or_else(|| {
            todos
                .iter()
                .rev()
                .find(|todo| !todo.status.eq_ignore_ascii_case("completed"))
        })
        .or_else(|| todos.last());

    if let Some(todo) = current {
        if let Some(group) = non_empty(todo.group.as_deref()) {
            return Some(group);
        }

        if let Some(objective) = goals
            .iter()
            .rev()
            .find(|goal| goal.group.is_none())
            .and_then(|goal| non_empty(goal.objective.as_deref()))
        {
            return Some(objective);
        }

        return non_empty(Some(&todo.content));
    }

    goals.iter().rev().find_map(|goal| {
        non_empty(goal.group.as_deref()).or_else(|| non_empty(goal.objective.as_deref()))
    })
}

/// Load todo state for a session and derive its best title hint.
pub fn load_session_title(session_id: &str) -> Option<String> {
    let todos = load_todos(session_id).ok()?;
    let goals = load_goals(session_id).unwrap_or_default();
    derive_session_title(&todos, &goals)
}

pub fn save_goals(session_id: &str, goals: &[TodoGoal]) -> Result<()> {
    let path = goals_path(session_id)?;
    storage::write_json_fast(&path, goals)
}

fn goals_path(session_id: &str) -> Result<PathBuf> {
    let base = storage::jcode_dir()?;
    Ok(base
        .join("todos")
        .join(format!("{}-goals.json", session_id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The unblocked poke must stay byte-identical after the blocked variant
    /// was introduced: this exact string is persisted in existing sessions and
    /// is what `is_auto_poke_message` matches, so drifting it would silently
    /// reclassify historical pokes as real user turns.
    #[test]
    fn the_unblocked_poke_text_is_unchanged() {
        assert_eq!(
            build_auto_poke_message(1),
            "You have 1 incomplete todo. Continue working, or update the todo tool."
        );
        assert_eq!(
            build_auto_poke_message(3),
            "You have 3 incomplete todos. Continue working, or update the todo tool."
        );
    }

    /// A blocked disclosure must not break poke recognition. The recognizer
    /// anchors on the trailing sentence, so the blocked list is placed above it.
    #[test]
    fn a_poke_naming_blocked_work_is_still_recognized_as_a_poke() {
        let message = build_auto_poke_message_with_blocked(
            1,
            &[("Ship it".to_string(), vec!["DBA review".to_string()])],
        );
        assert!(is_auto_poke_message(&message), "got: {message}");
        assert!(message.contains("blocked by: DBA review"), "got: {message}");
    }

    #[test]
    fn built_auto_poke_messages_are_detected() {
        assert!(is_auto_poke_message(&build_auto_poke_message(1)));
        assert!(is_auto_poke_message(&build_auto_poke_message(3)));
        assert!(is_auto_poke_message(
            TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE
        ));
        assert!(is_auto_poke_message(TODO_OWNERSHIP_CONTINUATION_MESSAGE));
        assert!(is_auto_poke_message(TODO_COMPLETION_CONTINUATION_MESSAGE));
        assert!(is_auto_poke_message(LEGACY_TODO_CONFIDENCE_SUMMARY_PREFIX));
    }

    #[test]
    fn quality_continuations_are_actionable_without_private_calibration() {
        for (message, category) in [
            (
                TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE,
                "hill-climbability",
            ),
            (TODO_OWNERSHIP_CONTINUATION_MESSAGE, "end-to-end ownership"),
            (
                TODO_COMPLETION_CONTINUATION_MESSAGE,
                "completion confidence",
            ),
        ] {
            let lower = message.to_ascii_lowercase();
            assert!(lower.contains(category));
            assert!(!message.chars().any(|ch| ch.is_ascii_digit()));
            for disclosure in ["threshold", "score", "percent", "below", "quality gate"] {
                assert!(
                    !lower.contains(disclosure),
                    "category-only continuation disclosed {disclosure}: {message}"
                );
            }
        }

        assert!(TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE.contains("strong feedback loop"));
        assert!(TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE.contains("Improve"));
        assert!(TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE.contains("call the todo tool again"));
        assert!(TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE.contains("before continuing the task"));
        // R08(a): the nudge must say the write survived. `save_todos` runs
        // unconditionally, but the old wording ("First, improve ... Then call
        // the todo tool again") read as a rejection, so the only apparent way
        // to persist was to resend or inflate the score. Observed live: six
        // identical nudges in one session, every write already saved.
        assert!(TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE.contains("was saved"));
        assert!(TODO_HILL_CLIMBABILITY_CONTINUATION_MESSAGE.contains("not a rejection"));
        assert!(TODO_OWNERSHIP_CONTINUATION_MESSAGE.contains("full user outcome"));
        assert!(TODO_OWNERSHIP_CONTINUATION_MESSAGE.contains("complete workflow"));
        assert!(TODO_OWNERSHIP_CONTINUATION_MESSAGE.contains("necessary follow-through"));
        assert!(TODO_COMPLETION_CONTINUATION_MESSAGE.contains("Validate the completed result"));
    }

    #[test]
    fn real_user_prompts_are_not_detected_as_pokes() {
        assert!(!is_auto_poke_message("fix the login bug"));
        assert!(!is_auto_poke_message(
            "You have 2 incomplete todos. Continue working, or update the todo tool.\n\nalso please fix the tests"
        ));
        assert!(!is_auto_poke_message(""));
    }

    fn todo(content: &str, status: &str, group: Option<&str>) -> TodoItem {
        TodoItem {
            content: content.to_string(),
            status: status.to_string(),
            priority: "high".to_string(),
            id: content.to_ascii_lowercase().replace(' ', "-"),
            group: group.map(str::to_string),
            confidence: None,
            completion_confidence: None,
            confidence_history: Vec::new(),
            blocked_by: Vec::new(),
            assigned_to: None,
        }
    }

    fn ownership_goal(group: Option<&str>, ownership: Option<u8>) -> TodoGoal {
        TodoGoal {
            group: group.map(str::to_string),
            end_to_end_ownership: ownership,
            ..Default::default()
        }
    }

    #[test]
    fn newly_completed_group_requires_sufficient_end_to_end_ownership() {
        let previous = vec![todo("work", "in_progress", Some("ship"))];
        let completed = vec![todo("work", "completed", Some("ship"))];

        for ownership in [None, Some(0), Some(95)] {
            assert!(!newly_completed_groups_have_sufficient_ownership(
                &previous,
                &completed,
                &[ownership_goal(Some("ship"), ownership)],
            ));
        }
        assert!(newly_completed_groups_have_sufficient_ownership(
            &previous,
            &completed,
            &[ownership_goal(Some("ship"), Some(96))],
        ));
    }

    /// R08 gate 1: "A rejected todo update names the discarded group and the
    /// specific fault, and never reports low ownership when the score exceeds
    /// the threshold."
    ///
    /// The gate returned a bare `bool`, so one sentence had to cover three
    /// different faults. The worst is the third: a goal whose label does not
    /// match its todos' group is *missing*, not low, yet the model was told its
    /// ownership was insufficient. Re-scoring cannot fix a label mismatch, so
    /// that message sends the model to work on the wrong thing.
    #[test]
    fn r08_gate1_a_label_mismatch_is_reported_as_missing_not_low() {
        let previous = vec![todo("work", "in_progress", Some("ship"))];
        let completed = vec![todo("work", "completed", Some("ship"))];
        // The model scored 100, but filed it under a different label.
        let goals = [ownership_goal(Some("shipping"), Some(100))];

        let fault = ownership_fault(&previous, &completed, &goals)
            .expect("a completed group with no matching goal must report a fault");
        assert_eq!(fault.group.as_deref(), Some("ship"));
        assert!(
            matches!(fault.kind, OwnershipFaultKind::NoGoalForGroup),
            "a label mismatch must not be reported as a low score: {fault:?}"
        );
        let message = fault.message();
        assert!(message.contains("ship"), "must name the group: {message}");
        assert!(
            !message.contains("not high enough"),
            "must not claim the score is low when none was found for this group: {message}"
        );
    }

    /// The three faults are distinguishable, and each names its group.
    #[test]
    fn r08_gate1_the_three_faults_are_distinct_and_named() {
        let previous = vec![todo("work", "in_progress", Some("ship"))];
        let completed = vec![todo("work", "completed", Some("ship"))];

        let missing_goal = ownership_fault(&previous, &completed, &[]).unwrap();
        assert!(matches!(
            missing_goal.kind,
            OwnershipFaultKind::NoGoalForGroup
        ));

        let missing_score =
            ownership_fault(&previous, &completed, &[ownership_goal(Some("ship"), None)]).unwrap();
        assert!(matches!(
            missing_score.kind,
            OwnershipFaultKind::NoOwnershipScore
        ));

        let low = ownership_fault(
            &previous,
            &completed,
            &[ownership_goal(Some("ship"), Some(10))],
        )
        .unwrap();
        assert!(matches!(low.kind, OwnershipFaultKind::BelowThreshold));

        let mut seen = vec![
            missing_goal.message(),
            missing_score.message(),
            low.message(),
        ];
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 3, "the three faults must read differently");
        for message in &seen {
            assert!(message.contains("ship"), "must name the group: {message}");
        }
    }

    /// CONTRAST: a sufficient score reports no fault at all. Without this, a
    /// fault type that always fires would satisfy every other assertion here.
    #[test]
    fn r08_gate1_a_sufficient_score_reports_no_fault() {
        let previous = vec![todo("work", "in_progress", Some("ship"))];
        let completed = vec![todo("work", "completed", Some("ship"))];
        assert!(
            ownership_fault(
                &previous,
                &completed,
                &[ownership_goal(Some("ship"), Some(96))],
            )
            .is_none()
        );
        // Ungrouped work still resolves through the implicit goal.
        let previous = vec![todo("work", "in_progress", None)];
        let completed = vec![todo("work", "completed", None)];
        assert!(
            ownership_fault(&previous, &completed, &[ownership_goal(None, Some(96))]).is_none()
        );
    }

    /// Every fault message keeps the calibration private: no score, no
    /// threshold, no digits. Naming the group must not leak the number.
    #[test]
    fn r08_gate1_fault_messages_disclose_no_calibration() {
        let previous = vec![todo("work", "in_progress", Some("ship"))];
        let completed = vec![todo("work", "completed", Some("ship"))];
        for goals in [
            Vec::new(),
            vec![ownership_goal(Some("ship"), None)],
            vec![ownership_goal(Some("ship"), Some(10))],
        ] {
            let message = ownership_fault(&previous, &completed, &goals)
                .unwrap()
                .message();
            let lower = message.to_ascii_lowercase();
            assert!(
                !message.chars().any(|ch| ch.is_ascii_digit()),
                "leaked a number: {message}"
            );
            for disclosure in ["threshold", "score", "percent", "quality gate"] {
                assert!(
                    !lower.contains(disclosure),
                    "leaked {disclosure}: {message}"
                );
            }
            assert!(
                is_auto_poke_message(&message),
                "fault messages must stay recognizable as machine continuations: {message}"
            );
        }
    }

    #[test]
    fn ownership_is_not_required_before_group_completion() {
        let previous = vec![todo("work", "pending", Some("ship"))];
        let in_progress = vec![todo("work", "in_progress", Some("ship"))];

        assert!(newly_completed_groups_have_sufficient_ownership(
            &previous,
            &in_progress,
            &[],
        ));
    }

    #[test]
    fn ownership_gate_normalizes_groups_and_supports_ungrouped_work() {
        let previous = vec![todo("work", "in_progress", Some(" ship "))];
        let completed = vec![todo("work", "completed", Some("ship"))];
        assert!(newly_completed_groups_have_sufficient_ownership(
            &previous,
            &completed,
            &[ownership_goal(Some(" ship"), Some(96))],
        ));

        let previous = vec![todo("work", "in_progress", None)];
        let completed = vec![todo("work", "completed", None)];
        assert!(newly_completed_groups_have_sufficient_ownership(
            &previous,
            &completed,
            &[ownership_goal(None, Some(96))],
        ));
    }

    #[test]
    fn ownership_gate_grandfathers_preexisting_completed_groups() {
        let completed = vec![todo("legacy", "completed", Some("legacy"))];
        assert!(newly_completed_groups_have_sufficient_ownership(
            &completed,
            &completed,
            &[],
        ));
    }

    #[test]
    fn session_title_prefers_in_progress_todo_group() {
        let todos = vec![
            todo("old task", "pending", Some("Older goal")),
            todo("current task", "in_progress", Some("Fix resume names")),
            todo("later task", "pending", Some("Later goal")),
        ];

        assert_eq!(
            derive_session_title(&todos, &[]).as_deref(),
            Some("Fix resume names")
        );
    }

    #[test]
    fn session_title_uses_latest_incomplete_group_when_nothing_is_active() {
        let todos = vec![
            todo("finished", "completed", Some("Old goal")),
            todo("next", "pending", Some("Current goal")),
        ];

        assert_eq!(
            derive_session_title(&todos, &[]).as_deref(),
            Some("Current goal")
        );
    }

    #[test]
    fn ungrouped_session_title_prefers_goal_objective_then_item_content() {
        let todos = vec![todo("Run targeted tests", "in_progress", None)];
        let goals = vec![TodoGoal {
            group: None,
            hill_climbability: Some(90),
            objective: Some("All resume naming tests pass".to_string()),
            ..Default::default()
        }];

        assert_eq!(
            derive_session_title(&todos, &goals).as_deref(),
            Some("All resume naming tests pass")
        );
        assert_eq!(
            derive_session_title(&todos, &[]).as_deref(),
            Some("Run targeted tests")
        );
    }
}
