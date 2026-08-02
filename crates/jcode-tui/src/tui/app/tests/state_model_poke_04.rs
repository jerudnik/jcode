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
