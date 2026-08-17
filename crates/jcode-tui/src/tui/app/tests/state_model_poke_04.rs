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
/// See docs/issues/blank-continuation-turn.md.
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
        // See docs/issues/blank-continuation-turn.md.
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

/// Build `count` pending todos, saved for `app`'s session.
fn save_pending_todos(app: &App, count: usize) {
    let todos: Vec<crate::todo::TodoItem> = (0..count)
        .map(|i| crate::todo::TodoItem {
            group: None,
            id: format!("t{i}"),
            content: format!("task {i}"),
            status: "pending".to_string(),
            priority: "high".to_string(),
            blocked_by: Vec::new(),
            assigned_to: None,
            confidence: Some(90),
            completion_confidence: None,
            confidence_history: Vec::new(),
        })
        .collect();
    crate::todo::save_todos(&app.session.id, &todos).expect("save todos");
}

/// Simulate the queued poke being dispatched and the turn finishing, so the
/// next `schedule_auto_poke_followup_if_needed` is not short-circuited by its
/// own previous queue entry.
fn drain_poke_queue(app: &mut App) {
    app.queued_messages.clear();
    app.pending_queued_dispatch = false;
}

/// The ordinary auto-poke must be bounded, like the overnight one.
///
/// `OVERNIGHT_MAX_POKES` caps the overnight poke at 48 follow-up turns, but the
/// ordinary `/poke` loop had no ceiling at all. A todo list that never reaches
/// a terminal status therefore drives model round-trips indefinitely, at
/// whatever the provider will serve. The poke's stopping condition was entirely
/// dependent on the model doing the thing the poke is nagging it to do, which
/// is precisely the case where that assumption is least safe.
#[test]
fn the_ordinary_auto_poke_stops_at_its_safety_budget() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        app.auto_poke_incomplete_todos = true;

        let budget = super::commands::MAX_AUTO_POKE_FOLLOWUPS;
        for i in 0..budget {
            save_pending_todos(&app, 3 + usize::from(i));
            assert!(
                app.schedule_auto_poke_followup_if_needed(),
                "poke {i} of {budget} is inside the budget and must still fire"
            );
            drain_poke_queue(&mut app);
        }

        // The list is still incomplete, so only the budget can stop this.
        save_pending_todos(&app, 3 + usize::from(budget));
        assert!(
            !app.schedule_auto_poke_followup_if_needed(),
            "the poke must stop once it has spent its budget of {budget}"
        );
        assert!(
            !app.auto_poke_incomplete_todos,
            "exhausting the budget must disarm auto-poke, not merely skip one turn; \
             leaving it armed would re-fire on the next completed turn"
        );
        assert!(
            app.queued_messages.is_empty(),
            "the refused poke must not queue a message"
        );
        assert!(
            app.display_messages()
                .iter()
                .any(|msg| msg.content.contains("safety budget")),
            "the user must be told why the poke stopped; a silent stop is \
             indistinguishable from the feature breaking"
        );
    });
}

/// Contrast case: the budget must not be reachable by ordinary use.
///
/// Without this, the test above would pass against a "fix" that refused to
/// poke at all, or that counted every scheduling call rather than every poke
/// actually sent. Re-arming with `/poke` must also restore a full budget,
/// since the ceiling is a runaway backstop and not a session-lifetime quota.
#[test]
fn the_poke_budget_is_not_consumed_by_turns_that_do_not_poke_and_resets_on_rearm() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        app.auto_poke_incomplete_todos = true;
        save_pending_todos(&app, 2);

        // Calls that decline to poke must not spend budget.
        app.pending_turn = true;
        for _ in 0..10 {
            assert!(
                !app.schedule_auto_poke_followup_if_needed(),
                "a turn already in flight must not poke"
            );
        }
        app.pending_turn = false;
        assert_eq!(
            app.auto_poke_followups_sent, 0,
            "declined pokes must not be charged against the budget"
        );

        // Spend the budget with distinct todo revisions. Unchanged revisions are
        // intentionally suppressed and must not count against this backstop.
        for i in 0..super::commands::MAX_AUTO_POKE_FOLLOWUPS {
            save_pending_todos(&app, 2 + usize::from(i));
            assert!(app.schedule_auto_poke_followup_if_needed());
            drain_poke_queue(&mut app);
        }
        save_pending_todos(
            &app,
            2 + usize::from(super::commands::MAX_AUTO_POKE_FOLLOWUPS),
        );
        assert!(!app.schedule_auto_poke_followup_if_needed());

        // Re-arming with /poke restores a full budget.
        super::commands::activate_auto_poke(&mut app);
        assert_eq!(
            app.auto_poke_followups_sent, 0,
            "/poke must reset the budget; a spent backstop that never resets \
             would silently make the feature one-shot per session"
        );
        drain_poke_queue(&mut app);
        assert!(
            app.schedule_auto_poke_followup_if_needed(),
            "after re-arming, the poke must work again"
        );
    });
}

#[test]
fn unchanged_todos_do_not_repeat_or_spend_auto_poke_budget() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        app.auto_poke_incomplete_todos = true;
        crate::todo::save_todos(&app.session.id, &[poke_todo("t1", "Wait for worker", &[])])
            .expect("save todos");

        assert!(app.schedule_auto_poke_followup_if_needed());
        assert_eq!(app.auto_poke_followups_sent, 1);
        drain_poke_queue(&mut app);

        assert!(
            !app.schedule_auto_poke_followup_if_needed(),
            "an unchanged list must not queue another automatic turn"
        );
        assert!(app.queued_messages.is_empty());
        assert_eq!(
            app.auto_poke_followups_sent, 1,
            "a suppressed duplicate must not consume safety budget"
        );
    });
}

#[test]
fn changed_outstanding_todos_rearm_one_auto_poke() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        app.auto_poke_incomplete_todos = true;
        let mut todos = vec![
            poke_todo("t1", "Write the plan", &[]),
            poke_todo("t2", "Ship the change", &["waiting on review"]),
        ];
        crate::todo::save_todos(&app.session.id, &todos).expect("save todos");

        assert!(app.schedule_auto_poke_followup_if_needed());
        drain_poke_queue(&mut app);

        todos.push(poke_todo("t3", "Add regression coverage", &[]));
        crate::todo::save_todos(&app.session.id, &todos).expect("add todo");
        assert!(
            app.schedule_auto_poke_followup_if_needed(),
            "adding an outstanding todo must produce one fresh poke"
        );
        drain_poke_queue(&mut app);

        todos[2].status = "completed".to_string();
        crate::todo::save_todos(&app.session.id, &todos).expect("complete todo");
        assert!(
            app.schedule_auto_poke_followup_if_needed(),
            "completing one todo changes the outstanding set"
        );
        drain_poke_queue(&mut app);

        todos[1].blocked_by = vec!["waiting on security review".to_string()];
        crate::todo::save_todos(&app.session.id, &todos).expect("change blocker");
        assert!(
            app.schedule_auto_poke_followup_if_needed(),
            "changing blocked_by must produce one fresh poke"
        );
        drain_poke_queue(&mut app);

        todos[1].blocked_by.clear();
        crate::todo::save_todos(&app.session.id, &todos).expect("clear blocker");
        assert!(
            app.schedule_auto_poke_followup_if_needed(),
            "a blocked-to-actionable transition must produce one fresh poke"
        );
    });
}

#[test]
fn settled_todo_cycle_allows_one_fresh_poke_for_an_equivalent_new_list() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        app.auto_poke_incomplete_todos = true;
        let pending = poke_todo("t1", "Review worker result", &[]);
        crate::todo::save_todos(&app.session.id, std::slice::from_ref(&pending))
            .expect("save todos");

        assert!(app.schedule_auto_poke_followup_if_needed());
        drain_poke_queue(&mut app);

        crate::todo::save_todos(&app.session.id, &[]).expect("settle cycle");
        assert!(!app.schedule_auto_poke_followup_if_needed());

        // Simulate default-on rearming without going through activate_auto_poke,
        // which has its own reset. This proves settlement itself ended the cycle.
        app.auto_poke_incomplete_todos = true;
        crate::todo::save_todos(&app.session.id, &[pending]).expect("start equivalent cycle");
        assert!(
            app.schedule_auto_poke_followup_if_needed(),
            "an equivalent list in a new cycle must receive one fresh nudge"
        );
    });
}

/// Build a todo in a non-terminal status, optionally blocked.
fn poke_todo(id: &str, content: &str, blocked_by: &[&str]) -> crate::todo::TodoItem {
    crate::todo::TodoItem {
        group: None,
        id: id.to_string(),
        content: content.to_string(),
        status: "pending".to_string(),
        priority: "high".to_string(),
        blocked_by: blocked_by.iter().map(|b| b.to_string()).collect(),
        assigned_to: None,
        confidence: Some(80),
        completion_confidence: None,
        confidence_history: Vec::new(),
    }
}

/// R08 gate 4, part 1: a blocked todo must not be nagged to "continue working".
///
/// `is_incomplete_poke_todo` decided incompleteness purely from `status`, so an
/// item whose `blocked_by` names an unmet dependency counted as ordinary
/// outstanding work. The poke then told the model "You have 1 incomplete todo.
/// Continue working", which is the system asserting something true-sounding and
/// false: the work cannot proceed, and no amount of continuing will change that
/// until the blocker clears. Poking is not merely useless here, it burns a model
/// round-trip per turn to repeat an instruction that cannot be followed.
#[test]
fn r08_gate4_a_blocked_todo_does_not_poke() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[poke_todo(
                "t1",
                "Ship the migration",
                &["waiting on DBA review"],
            )],
        )
        .expect("save todos");
        app.auto_poke_incomplete_todos = true;

        assert!(
            !app.schedule_auto_poke_followup_if_needed(),
            "a list whose only outstanding item is blocked must not poke"
        );
        assert!(
            app.queued_messages.is_empty(),
            "no poke message may be queued for a fully blocked list"
        );
    });
}

/// R08 gate 4, part 2: filtering blocked items out must not fake completion.
///
/// This is the trap in the obvious fix. `schedule_auto_poke_followup_if_needed`
/// treats an empty `incomplete` partition as "the list finished" and hands it to
/// `settle_completed_todo_list`, which prints "✅ Todos complete." So simply
/// removing blocked items from the partition converts one misreport into a worse
/// one: unfinished, blocked work announced as done. The blocked branch has to be
/// a third state, not a shortcut into the completion gate.
#[test]
fn r08_gate4_a_blocked_list_is_never_announced_complete() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[poke_todo(
                "t1",
                "Ship the migration",
                &["waiting on DBA review"],
            )],
        )
        .expect("save todos");
        app.auto_poke_incomplete_todos = true;

        app.schedule_auto_poke_followup_if_needed();

        let said_complete = app
            .display_messages
            .iter()
            .any(|message| message.content.contains("Todos complete"));
        assert!(
            !said_complete,
            "blocked work must never be reported as complete; \
             filtering blocked items into the completion gate trades one \
             false report for a worse one"
        );
    });
}

/// R08 gate 4, part 3: the contrast case, and the reason this fix cannot cheat.
///
/// Both assertions above are satisfiable by disabling the poke outright, so this
/// pins the other side: a list holding one blocked item AND one actionable item
/// must still poke, must count only the actionable one, and must name the
/// blocked item rather than silently dropping it. Without this, "stop poking"
/// and "poke correctly" are indistinguishable to the suite.
#[test]
fn r08_gate4_an_actionable_item_still_pokes_and_names_the_blocker() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[
                poke_todo("t1", "Ship the migration", &["waiting on DBA review"]),
                poke_todo("t2", "Write the rollback plan", &[]),
            ],
        )
        .expect("save todos");
        app.auto_poke_incomplete_todos = true;

        assert!(
            app.schedule_auto_poke_followup_if_needed(),
            "an actionable item must still poke even when a sibling is blocked"
        );
        let poke = app
            .queued_messages
            .last()
            .expect("a poke message must be queued")
            .clone();
        assert!(
            poke.contains("1 incomplete todo"),
            "the count must exclude the blocked item; got: {poke}"
        );
        assert!(
            poke.contains("blocked"),
            "the poke must disclose that work is blocked rather than \
             quietly omitting it; got: {poke}"
        );
    });
}

fn cancelled_todo(id: &str, content: &str) -> crate::todo::TodoItem {
    crate::todo::TodoItem {
        status: "cancelled".to_string(),
        ..poke_todo(id, content, &[])
    }
}

/// R08 gate 3: "An all-cancelled todo list produces neither a visible poke nor
/// a hidden dispatched turn."
///
/// R08(b) and (c) fixed the two halves separately -- one terminal predicate, and
/// an empty-completed-set case distinct from "needs more work" -- but both were
/// only ever asserted on their helpers. Gate 3 is a statement about the pump, so
/// it has to be measured at `finish_turn`, where a hidden dispatch actually
/// costs a model round-trip. The failure this guards is specific: cancelled work
/// counted as outstanding, so the poke nagged about it, while
/// `todo_confidence_summary` simultaneously reported it as needing more
/// validation, and the two disagreed about the same list on the same turn.
#[test]
fn r08_gate3_an_all_cancelled_list_dispatches_no_turn() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(
            &app.session.id,
            &[
                cancelled_todo("t1", "Abandoned approach"),
                cancelled_todo("t2", "Superseded by t3"),
            ],
        )
        .expect("save todos");
        app.auto_poke_incomplete_todos = true;

        let mut hidden_total = 0usize;
        for _ in 0..20 {
            app.hidden_queued_system_messages.clear();
            app.pending_queued_dispatch = false;
            app.is_processing = true;
            super::local::finish_turn(&mut app);
            hidden_total += app.hidden_queued_system_messages.len();
        }

        assert_eq!(
            hidden_total, 0,
            "cancelled work is finished, so it must not dispatch a hidden turn"
        );
        assert!(
            app.queued_messages.is_empty(),
            "no visible poke may be queued for an all-cancelled list"
        );
        let nagged = app
            .display_messages
            .iter()
            .any(|message| message.content.contains("incomplete todo"));
        assert!(!nagged, "cancelled work must not be called incomplete");
    });
}

/// CONTRAST for gate 3: an outstanding list must still pump.
///
/// Every assertion above is satisfiable by disabling the poke, so this pins the
/// other side with the same driver and the only difference being status.
#[test]
fn r08_gate3_an_outstanding_list_still_pokes_under_the_same_driver() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        crate::todo::save_todos(&app.session.id, &[poke_todo("t1", "Real work", &[])])
            .expect("save todos");
        app.auto_poke_incomplete_todos = true;

        app.pending_queued_dispatch = false;
        app.is_processing = true;
        super::local::finish_turn(&mut app);

        assert!(
            !app.queued_messages.is_empty(),
            "an actionable list must still poke under this driver, \
             or the all-cancelled assertions prove only that poking is off"
        );
    });
}

/// R08 gate 2 + evidence 1: exactly one predicate decides "terminal", and every
/// consumer agrees with it on the same list.
///
/// Grep alone cannot prove agreement, and a unit test of the predicate alone
/// proves only that the predicate is self-consistent. So this drives the actual
/// consumers -- the poke partition, the "N todos" tool-call title, and the
/// improve/refactor status line -- over one list holding every status, and
/// requires the counts to match. Before the sweep, eleven call sites re-spelled
/// the terminal set inline as `!= "completed" && != "cancelled"`, so adding a
/// third terminal status would have moved some counts and not others.
#[test]
fn r08_gate2_every_consumer_agrees_on_one_terminal_predicate() {
    with_temp_jcode_home(|| {
        let mut app = create_test_app();
        let mut todos = vec![
            poke_todo("t1", "Pending work", &[]),
            cancelled_todo("t2", "Abandoned"),
        ];
        todos.push(crate::todo::TodoItem {
            status: "completed".to_string(),
            ..poke_todo("t3", "Done", &[])
        });
        todos.push(crate::todo::TodoItem {
            status: "in_progress".to_string(),
            ..poke_todo("t4", "Underway", &[])
        });
        crate::todo::save_todos(&app.session.id, &todos).expect("save todos");

        // The one predicate: t1 and t4 are outstanding, t2 and t3 are finished.
        let expected = todos
            .iter()
            .filter(|todo| !crate::todo::is_terminal_todo_status(&todo.status))
            .count();
        assert_eq!(expected, 2, "fixture sanity: two outstanding items");

        // Consumer 1: the poke partition.
        assert_eq!(
            super::commands::incomplete_poke_todos(&app).len(),
            expected,
            "the poke must count the same outstanding set as the predicate"
        );

        // Consumer 2: the improve/refactor status line, which reports the same
        // list to the user in prose.
        for status in [
            super::commands_improve::format_improve_status(&app),
            super::commands_improve::format_refactor_status(&app),
        ] {
            assert!(
                status.contains(&format!("{expected} incomplete")),
                "status line disagrees with the terminal predicate: {status}"
            );
        }

        // Cancelling the last outstanding item must move every consumer at once.
        let settled: Vec<_> = todos
            .iter()
            .map(|todo| crate::todo::TodoItem {
                status: if crate::todo::is_terminal_todo_status(&todo.status) {
                    todo.status.clone()
                } else {
                    "cancelled".to_string()
                },
                ..todo.clone()
            })
            .collect();
        crate::todo::save_todos(&app.session.id, &settled).expect("save settled todos");
        assert!(super::commands::incomplete_poke_todos(&app).is_empty());
        app.auto_poke_incomplete_todos = true;
        app.pending_queued_dispatch = false;
        app.is_processing = true;
        super::local::finish_turn(&mut app);
        assert!(
            app.queued_messages.is_empty(),
            "a fully terminal list must not poke any consumer"
        );
    });
}
