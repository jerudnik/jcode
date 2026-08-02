//! Todo-list partitioning and poke-message construction.
//!
//! Split out of `commands.rs`, which had grown into a grab bag. These helpers
//! form one coherent unit: they decide which todos are outstanding, which of
//! those are actionable versus blocked, and what the poke says about each.

use super::App;
use super::commands::{is_poke_message, is_todo_confidence_summary_message};

pub(super) fn poke_todos(app: &App) -> Vec<crate::todo::TodoItem> {
    crate::todo::load_todos(&super::commands::active_session_id(app)).unwrap_or_default()
}

pub(super) fn is_incomplete_poke_todo(todo: &crate::todo::TodoItem) -> bool {
    !crate::todo::is_terminal_todo_status(&todo.status)
}

pub(super) fn incomplete_poke_todos(app: &App) -> Vec<crate::todo::TodoItem> {
    poke_todos(app)
        .into_iter()
        .filter(is_incomplete_poke_todo)
        .collect()
}

/// True when a todo names an unmet dependency.
///
/// A blocked item is outstanding but not actionable, which is a third state the
/// poke previously could not express: it partitioned todos into terminal and
/// "keep working", so blocked work was nagged with an instruction that cannot
/// be followed until something outside the model's control changes.
/// Banner shown when every outstanding todo is blocked.
///
/// Auto-poke disarms here rather than staying armed and silent: an armed poke
/// that declines every turn looks identical to a working poke from the outside,
/// so it would report readiness it does not have.
pub(super) fn auto_poke_blocked_banner(blocked: &[crate::todo::TodoItem]) -> String {
    format!(
        "⏸ Auto-poke stopped: {} outstanding todo{} blocked. Not complete, not actionable.",
        blocked.len(),
        if blocked.len() == 1 { " is" } else { "s are" },
    )
}

pub(super) fn is_blocked_poke_todo(todo: &crate::todo::TodoItem) -> bool {
    todo.blocked_by
        .iter()
        .any(|blocker| !blocker.trim().is_empty())
}

pub(super) fn build_poke_message(incomplete: &[crate::todo::TodoItem]) -> String {
    build_poke_message_with_blocked(incomplete, &[])
}

pub(super) fn build_poke_message_with_blocked(
    actionable: &[crate::todo::TodoItem],
    blocked: &[crate::todo::TodoItem],
) -> String {
    let blocked: Vec<(String, Vec<String>)> = blocked
        .iter()
        .map(|todo| (todo.content.clone(), todo.blocked_by.clone()))
        .collect();
    crate::todo::build_auto_poke_message_with_blocked(actionable.len(), &blocked)
}

/// Re-derive a queued poke's text from the todo list as it stands *now*.
///
/// A poke's count is rendered into a `String` when the poke is scheduled, but
/// the queue is drained later and forwards that string verbatim. Any todo
/// update landing in between is invisible to the message the model reads, so
/// the poke can assert a superseded count in the present tense. Since telling
/// the model what remains is the poke's whole purpose, that is the mechanism
/// misreporting the one fact it exists to report.
///
/// Returns `Some(text)` with a freshly counted message for pokes, `Some(text)`
/// unchanged for anything that is not a poke, and `None` when the list has been
/// fully resolved in the meantime, since a poke reading "0 incomplete todos"
/// would be a new piece of nonsense rather than a fix for the old one.
pub(super) fn refresh_poke_message_for_dispatch(app: &App, message: &str) -> Option<String> {
    if !is_poke_message(message) {
        return Some(message.to_string());
    }
    // Confidence-summary continuations are also "poke messages" but carry no
    // count, so re-deriving an incomplete-count message for them would replace
    // their content rather than refresh it.
    if is_todo_confidence_summary_message(message) {
        return Some(message.to_string());
    }
    let incomplete = incomplete_poke_todos(app);
    if incomplete.is_empty() {
        return None;
    }
    Some(build_poke_message(&incomplete))
}

/// How the poke sees a todo list: finished, blocked, or with work to do.
///
/// Three states, not two. Blocked work is outstanding but not actionable, so it
/// must neither be nagged with "continue working" nor routed into the
/// completion gate: an empty *actionable* partition is not a finished list, and
/// announcing "Todos complete" over blocked work would trade one false report
/// for a worse one.
pub(super) enum PokeDisposition {
    Settled(Vec<crate::todo::TodoItem>),
    AllBlocked(Vec<crate::todo::TodoItem>),
    Actionable {
        actionable: Vec<crate::todo::TodoItem>,
        blocked: Vec<crate::todo::TodoItem>,
    },
}

pub(super) fn classify_poke_todos(app: &App) -> PokeDisposition {
    let todos = poke_todos(app);
    let outstanding: Vec<_> = todos
        .iter()
        .filter(|todo| is_incomplete_poke_todo(todo))
        .cloned()
        .collect();
    if outstanding.is_empty() {
        return PokeDisposition::Settled(todos);
    }
    let (blocked, actionable): (Vec<_>, Vec<_>) =
        outstanding.into_iter().partition(is_blocked_poke_todo);
    if actionable.is_empty() {
        return PokeDisposition::AllBlocked(blocked);
    }
    PokeDisposition::Actionable {
        actionable,
        blocked,
    }
}

/// Decide whether this turn should poke, and queue the poke if so.
///
/// Lives here rather than on `App` so the whole decision (partition, backstop,
/// message) reads as one unit.
pub(super) fn schedule_auto_poke_followup_if_needed(app: &mut App) -> bool {
    if !app.auto_poke_incomplete_todos
        || app.pending_queued_dispatch
        || app.pending_turn
        || app.has_queued_followups()
    {
        return false;
    }

    let (actionable, blocked) = match classify_poke_todos(app) {
        PokeDisposition::Settled(todos) => return app.settle_completed_todo_list(&todos),
        PokeDisposition::AllBlocked(blocked) => {
            app.auto_poke_incomplete_todos = false;
            app.push_display_message(crate::tui::DisplayMessage::system(
                auto_poke_blocked_banner(&blocked),
            ));
            return false;
        }
        PokeDisposition::Actionable {
            actionable,
            blocked,
        } => (actionable, blocked),
    };

    // Backstop for a list that never terminates. After the completion check so
    // a finished list still settles, and after the partition so declined pokes
    // are not charged.
    if super::commands::spend_auto_poke_budget(app) {
        return false;
    }

    app.push_display_message(crate::tui::DisplayMessage::system(
        super::commands::auto_poking_banner(actionable.len()),
    ));
    app.queued_messages
        .push(build_poke_message_with_blocked(&actionable, &blocked));
    app.pending_queued_dispatch = true;
    true
}
