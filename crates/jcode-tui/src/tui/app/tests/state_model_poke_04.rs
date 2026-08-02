// Todo completion-gate pump tests.
//
// Split out of state_model_poke_03.rs to keep that file under the test-size
// budget. Included by tests.rs alongside its siblings.

/// The empty-dispatch pump, as a bounded-work property.
///
/// The original defect was not that the gate queued a reminder; it was that it
/// queued the *same* reminder on every turn, because the reminder cannot change
/// the todo list it is judged against. That dispatches `Request::Message` with
/// empty content at model round-trip speed, unbounded: 361 blank sends in one
/// observed session, 538 in another.
///
/// Asserting on a single repeat would pass against a gate that merely fired
/// every other turn, so this drives many turns and demands a constant.
/// See docs/fork/ideal-base/human-noticed-issues/BLANK_CONTINUATION_TURN.md.
#[test]
fn test_todo_confidence_gate_does_not_pump_across_many_turns() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        // Calibrated-but-below-threshold scores: exactly the shape observed at
        // the onset of the real runaway sessions.
        crate::todo::save_todos(
            &app.session.id,
            &[
                crate::todo::TodoItem {
                    group: None,
                    id: "todo-1".to_string(),
                    content: "Land the risky path".to_string(),
                    status: "completed".to_string(),
                    priority: "high".to_string(),
                    blocked_by: Vec::new(),
                    assigned_to: None,
                    confidence: Some(90),
                    completion_confidence: Some(90),
                    confidence_history: Vec::new(),
                },
                crate::todo::TodoItem {
                    group: None,
                    id: "todo-2".to_string(),
                    content: "Document it".to_string(),
                    status: "completed".to_string(),
                    priority: "medium".to_string(),
                    blocked_by: Vec::new(),
                    assigned_to: None,
                    confidence: Some(93),
                    completion_confidence: Some(93),
                    confidence_history: Vec::new(),
                },
            ],
        )
        .expect("save todos");

        app.auto_poke_incomplete_todos = true;

        // Model the real fixpoint: the model answers in prose and never calls
        // the todo tool, so every turn re-reads a byte-identical list.
        let mut queued_total = 0usize;
        for _ in 0..50 {
            app.hidden_queued_system_messages.clear();
            app.pending_queued_dispatch = false;
            app.is_processing = true;
            super::local::finish_turn(&mut app);
            queued_total += app.hidden_queued_system_messages.len();
        }

        assert_eq!(
            queued_total, 1,
            "the gate must queue once for an unchanged todo list, not once per turn"
        );

        // A genuine todo revision re-arms it: the gate is suppressed, not disabled.
        let mut revised = crate::todo::load_todos(&app.session.id).expect("load todos");
        revised.push(crate::todo::TodoItem {
            group: None,
            id: "todo-3".to_string(),
            content: "Newly discovered work".to_string(),
            status: "completed".to_string(),
            priority: "low".to_string(),
            blocked_by: Vec::new(),
            assigned_to: None,
            confidence: Some(91),
            completion_confidence: Some(91),
            confidence_history: Vec::new(),
        });
        crate::todo::save_todos(&app.session.id, &revised).expect("save revised todos");

        app.hidden_queued_system_messages.clear();
        app.pending_queued_dispatch = false;
        app.is_processing = true;
        super::local::finish_turn(&mut app);
        assert_eq!(
            app.hidden_queued_system_messages.len(),
            1,
            "a real todo revision must re-arm the gate"
        );

        // ...and the revised list is likewise gated only once.
        let mut after_revision = 0usize;
        for _ in 0..20 {
            app.hidden_queued_system_messages.clear();
            app.pending_queued_dispatch = false;
            app.is_processing = true;
            super::local::finish_turn(&mut app);
            after_revision += app.hidden_queued_system_messages.len();
        }
        assert_eq!(after_revision, 0, "the revised list must not pump either");
    });
}

#[test]
fn test_finish_turn_auto_poke_queues_confidence_summary_when_todos_done() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[
                crate::todo::TodoItem {
                    group: None,
                    id: "todo-1".to_string(),
                    content: "Finish risky provider path".to_string(),
                    status: "completed".to_string(),
                    priority: "high".to_string(),
                    blocked_by: Vec::new(),
                    assigned_to: None,
                    confidence: Some(70),
                    completion_confidence: Some(80),
                    confidence_history: Vec::new(),
                },
                crate::todo::TodoItem {
                    group: None,
                    id: "todo-2".to_string(),
                    content: "Document straightforward behavior".to_string(),
                    status: "completed".to_string(),
                    priority: "medium".to_string(),
                    blocked_by: Vec::new(),
                    assigned_to: None,
                    confidence: Some(90),
                    completion_confidence: Some(95),
                    confidence_history: Vec::new(),
                },
            ],
        )
        .expect("save todos");

        app.auto_poke_incomplete_todos = true;
        app.is_processing = true;
        super::local::finish_turn(&mut app);

        assert!(app.auto_poke_incomplete_todos);
        assert!(app.pending_queued_dispatch);
        assert!(app.queued_messages().is_empty());
        assert_eq!(app.hidden_queued_system_messages.len(), 1);
        let summary = &app.hidden_queued_system_messages[0];
        assert!(super::commands::is_poke_message(summary));
        assert!(super::commands::is_todo_confidence_summary_message(summary));
        assert_eq!(summary, crate::todo::TODO_COMPLETION_CONTINUATION_MESSAGE);
        assert!(!summary.chars().any(|ch| ch.is_ascii_digit()));
        assert!(summary.contains("completion confidence"));
        assert!(!summary.to_ascii_lowercase().contains("gate"));
        assert!(!summary.to_ascii_lowercase().contains("threshold"));
        assert!(!summary.contains("Finish risky provider path"));
        assert!(
            app.display_messages()
                .iter()
                .any(|msg| msg.content.contains(
                    "Todo completion gate: completion confidence needs stronger validation."
                ))
        );

        // Finishing another turn without touching the todo list must NOT queue
        // the reminder again. Nothing about answering it changes the todos, so
        // re-firing re-evaluates identical state and re-queues the identical
        // reminder every turn: an unbounded empty-content send loop at model
        // round-trip speed, observed as 361 blank turns in one session.
        // See docs/fork/ideal-base/human-noticed-issues/BLANK_CONTINUATION_TURN.md.
        app.hidden_queued_system_messages.clear();
        app.pending_queued_dispatch = false;
        app.is_processing = true;
        super::local::finish_turn(&mut app);
        assert!(
            app.hidden_queued_system_messages.is_empty(),
            "the gate must fire at most once per todo-list revision"
        );
        assert!(!app.pending_queued_dispatch);
        // Auto-poke stays armed so a later genuine todo revision is still gated.
        assert!(app.auto_poke_incomplete_todos);

        // Once the model records sufficient completion confidence through the
        // todo tool, the next completion check passes and disarms auto-poke.
        let mut validated = crate::todo::load_todos(&app.session.id).expect("load todos");
        for todo in &mut validated {
            todo.completion_confidence = Some(100);
        }
        crate::todo::save_todos(&app.session.id, &validated).expect("save validated todos");
        app.hidden_queued_system_messages.clear();
        app.pending_queued_dispatch = false;
        app.is_processing = true;
        super::local::finish_turn(&mut app);
        assert!(!app.auto_poke_incomplete_todos);
        assert!(!app.pending_queued_dispatch);
        assert!(app.hidden_queued_system_messages.is_empty());
        assert!(app.display_messages().iter().any(|msg| {
            msg.content
                .contains("Todos complete. Completion confidence: 100%.")
        }));
    });
}

/// A poke must state the todo count as it is when the poke is *dispatched*,
/// not as it was when the poke was *queued*.
///
/// `build_poke_message` renders the count into a `String` at schedule time, and
/// `process_queued_messages` later forwards that string verbatim via
/// `std::mem::take`. Nothing recomputes in between, so any todo update landing
/// between queue and dispatch is invisible to the message the model reads.
///
/// That matters because telling the model what remains is the poke's entire
/// job. A stale count is the mechanism misreporting the one fact it exists to
/// report, and asserting it in the present tense.
///
/// Live precedent: session_badger 2026-08-02T06:58:22Z announced "4 incomplete
/// todos" against a list that had held 2 for 18.2 seconds. That specific
/// instance was never root-caused (seven hypotheses, seven falsifications; see
/// R08(h) in TODO_POKE_TERMINAL_STATES.md), and this test does not claim to
/// reproduce it. It closes the structural gap that would produce the same
/// symptom, which is worth doing on its own merits.
#[test]
fn poke_message_is_rebuilt_from_the_todo_list_at_dispatch_time() {
    with_temp_jcode_home(|| {
        let make = |id: &str, status: &str| crate::todo::TodoItem {
            group: None,
            id: id.to_string(),
            content: format!("task {id}"),
            status: status.to_string(),
            priority: "high".to_string(),
            blocked_by: Vec::new(),
            assigned_to: None,
            confidence: Some(90),
            completion_confidence: None,
            confidence_history: Vec::new(),
        };
        let app = create_test_app();

        // Four open todos when the poke text is built.
        crate::todo::save_todos(
            &app.session.id,
            &[
                make("a", "pending"),
                make("b", "pending"),
                make("c", "pending"),
                make("d", "in_progress"),
            ],
        )
        .expect("save todos");
        let queued =
            super::commands::build_poke_message(&super::commands::incomplete_poke_todos(&app));
        assert!(
            queued.contains("4 incomplete todos"),
            "precondition: queued text should reflect the list at queue time, got {queued:?}"
        );

        // The model finishes two of them before the queued poke is dispatched.
        // A turn that ends by updating todos is exactly when a poke is pending,
        // so this is the ordinary case rather than a contrived race.
        let mut updated = crate::todo::load_todos(&app.session.id).expect("load todos");
        for item in updated.iter_mut().filter(|t| t.id == "a" || t.id == "b") {
            item.status = "completed".to_string();
        }
        crate::todo::save_todos(&app.session.id, &updated).expect("save updated todos");

        let dispatched = super::commands::refresh_poke_message_for_dispatch(&app, &queued);
        assert_eq!(
            dispatched,
            Some(crate::todo::build_auto_poke_message(2)),
            "dispatch must re-read the list and report 2, not the queued 4"
        );
    });
}

/// Contrast case. When the list does not change between queue and dispatch, the
/// refreshed message must still carry the real count.
///
/// Without this, the test above would pass against a "fix" that dropped the
/// number, or blanked the message, or refused to send a poke at all. Those
/// would all end the misreporting by destroying the poke's only useful content.
#[test]
fn poke_refresh_preserves_the_real_count_when_todos_are_unchanged() {
    with_temp_jcode_home(|| {
        let make = |id: &str| crate::todo::TodoItem {
            group: None,
            id: id.to_string(),
            content: format!("task {id}"),
            status: "pending".to_string(),
            priority: "high".to_string(),
            blocked_by: Vec::new(),
            assigned_to: None,
            confidence: Some(90),
            completion_confidence: None,
            confidence_history: Vec::new(),
        };
        let app = create_test_app();
        crate::todo::save_todos(&app.session.id, &[make("a"), make("b"), make("c")])
            .expect("save todos");

        let queued =
            super::commands::build_poke_message(&super::commands::incomplete_poke_todos(&app));
        let dispatched = super::commands::refresh_poke_message_for_dispatch(&app, &queued);
        assert_eq!(
            dispatched,
            Some(crate::todo::build_auto_poke_message(3)),
            "an unchanged list must still be counted accurately"
        );
    });
}

/// Non-poke messages must pass through untouched. The refresh keys off
/// `is_poke_message`, so a user's own queued text that happens to sit next to a
/// poke must never be rewritten, and a poke whose todos all completed before
/// dispatch must not claim "0 incomplete todos".
#[test]
fn poke_refresh_leaves_user_messages_alone_and_drops_emptied_pokes() {
    with_temp_jcode_home(|| {
        let app = create_test_app();
        crate::todo::save_todos(&app.session.id, &[]).expect("save empty todos");

        let user_text = "You have thoughts about incomplete work. Continue.";
        assert_eq!(
            super::commands::refresh_poke_message_for_dispatch(&app, user_text),
            Some(user_text.to_string()),
            "a real user message must survive dispatch unmodified"
        );

        let stale_poke = crate::todo::build_auto_poke_message(4);
        assert_eq!(
            super::commands::refresh_poke_message_for_dispatch(&app, &stale_poke),
            None,
            "a poke whose todos are all done must be dropped, not sent claiming 0"
        );
    });
}

/// The refresh must live in the *drain* path, not merely exist as a helper.
///
/// This is the control for the wiring itself. The three tests above call
/// `refresh_poke_message_for_dispatch` directly, so they keep passing even if
/// the drain stops calling it. That is not hypothetical: I deleted the call
/// from `process_queued_messages` and the entire 1874-test suite stayed green.
/// This test drains the real queue through the same entry point dispatch uses,
/// so unwiring the refresh fails here.
///
/// `process_queued_messages` itself needs a live terminal and event stream,
/// which is why the drain is extracted into `take_queued_messages_for_dispatch`
/// rather than tested in place.
#[test]
fn draining_the_queue_for_dispatch_refreshes_poke_counts_in_place() {
    with_temp_jcode_home(|| {
        let make = |id: &str, status: &str| crate::todo::TodoItem {
            group: None,
            id: id.to_string(),
            content: format!("task {id}"),
            status: status.to_string(),
            priority: "high".to_string(),
            blocked_by: Vec::new(),
            assigned_to: None,
            confidence: Some(90),
            completion_confidence: None,
            confidence_history: Vec::new(),
        };
        let mut app = create_test_app();

        // A poke asserting 4, queued behind an ordinary user message.
        let queued_poke = crate::todo::build_auto_poke_message(4);
        app.queued_messages
            .push("a user message that must survive untouched".to_string());
        app.queued_messages.push(queued_poke.clone());

        // Two of the four resolve before the queue is drained.
        crate::todo::save_todos(
            &app.session.id,
            &[
                make("a", "completed"),
                make("b", "completed"),
                make("c", "pending"),
                make("d", "pending"),
            ],
        )
        .expect("save todos");

        let drained = app.take_queued_messages_for_dispatch();

        assert_eq!(
            drained.len(),
            2,
            "both messages should survive the drain: {drained:?}"
        );
        assert_eq!(
            drained[0], "a user message that must survive untouched",
            "a non-poke message must pass through the drain byte-for-byte"
        );
        assert_ne!(
            drained[1], queued_poke,
            "the poke was dispatched with its stale schedule-time text, so the \
             drain path is not refreshing it"
        );
        assert_eq!(
            drained[1],
            crate::todo::build_auto_poke_message(2),
            "the dispatched poke must report the 2 todos that are actually open"
        );
        assert!(
            app.queued_messages.is_empty(),
            "the queue must be drained, not copied"
        );
    });
}
