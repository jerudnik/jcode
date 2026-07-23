//! F03 in-process state-machine fixtures for the shutdown coordinator.
//!
//! These drive REAL `ShutdownCoordinator` instances (private authority per
//! test, leaked for `&'static`) through the races the pure model tests
//! cannot reach: begin/upgrade concurrency, idle-claim vs acquisition,
//! drain-until-release, reload refusal and mid-drain upgrade, and terminal
//! publication. Runtime process-level fixtures (real daemon, real signals,
//! residue) live in `docs/fork/ideal-base/evidence/F03/lease_class_fixtures.sh`.

use super::shutdown::{
    AcceptLoopExitDisposition, BeginOutcome, ExitReason, RefusalReason, ReloadRefused,
    ShutdownCoordinator, TerminalOutcome,
};
use jcode_core::activity::{ActivityClass, ActivityLeaseAuthority};
use std::time::Duration;

fn acquire(
    authority: &std::sync::Arc<super::shutdown::ServerActivityLeaseAuthority>,
    class: ActivityClass,
) -> jcode_core::activity::ActivityLeaseToken {
    authority
        .acquire(class, "fixture")
        .expect("acquisition should succeed while Running")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn begin_and_wait_reaches_cleaned_with_correct_code() {
    let (coordinator, _authority) = ShutdownCoordinator::leaked_for_test();
    let outcome = coordinator
        .begin_and_wait(ExitReason::SigTerm)
        .await
        .expect("sigterm begin is never refused");
    assert_eq!(
        outcome,
        TerminalOutcome::Cleaned {
            reason: ExitReason::SigTerm,
            code: 0
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn idle_begin_refused_while_any_lease_held_then_succeeds_after_release() {
    let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();

    let token = acquire(&authority, ActivityClass::McpCall);
    assert_eq!(
        coordinator.begin(ExitReason::PersistentIdle),
        BeginOutcome::Refused(RefusalReason::NotQuiescent),
        "idle begin must lose the atomic claim while a lease is held"
    );
    // The refused claim must NOT have closed acquisition.
    let token2 = authority
        .acquire(ActivityClass::ProviderTurn, "post-refusal")
        .expect("acquisition must still work after a refused idle claim");
    authority.release(token2);
    authority.release(token);

    assert_eq!(
        coordinator.begin(ExitReason::PersistentIdle),
        BeginOutcome::Accepted
    );
    let outcome = coordinator.wait_terminal().await;
    assert_eq!(
        outcome,
        TerminalOutcome::Cleaned {
            reason: ExitReason::PersistentIdle,
            code: super::EXIT_IDLE_TIMEOUT
        }
    );
    // After a successful claim, acquisition fails with the typed refusal.
    assert!(authority.acquire(ActivityClass::DebugJob, "late").is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_idle_claims_and_acquisitions_never_coexist() {
    // Race N acquirer tasks against M idle-claim attempts on the same
    // authority: an accepted idle begin implies the table was empty at claim
    // time and no later acquisition may succeed.
    for _ in 0..50 {
        let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();
        let mut acquirers = Vec::new();
        for i in 0..4 {
            let authority = std::sync::Arc::clone(&authority);
            acquirers.push(tokio::spawn(async move {
                let mut held = Vec::new();
                for _ in 0..5 {
                    if let Ok(token) =
                        authority.acquire(ActivityClass::ProviderTurn, &format!("racer-{i}"))
                    {
                        tokio::task::yield_now().await;
                        held.push(token);
                    } else {
                        return (held, true); // observed refusal
                    }
                }
                (held, false)
            }));
        }
        let claimer = tokio::spawn(async move {
            for _ in 0..20 {
                match coordinator.begin(ExitReason::PersistentIdle) {
                    BeginOutcome::Accepted => return true,
                    BeginOutcome::Refused(RefusalReason::NotQuiescent) => {
                        tokio::task::yield_now().await;
                    }
                    other => panic!("unexpected outcome: {other:?}"),
                }
            }
            false
        });

        let mut all_tokens = Vec::new();
        for task in acquirers {
            let (tokens, _refused) = task.await.unwrap();
            all_tokens.extend(tokens);
        }
        let claimed = claimer.await.unwrap();
        if claimed {
            // Invariant: an accepted claim means the table was empty, so
            // every token acquired before the claim was released... but the
            // racers never release. Therefore acceptance can only have
            // happened when zero acquisitions had succeeded yet, OR after
            // all racers were refused. Check the strong form: at acceptance
            // no live lease existed, i.e. every successful acquisition
            // happened before... simplest checkable invariant: post-claim
            // acquisition MUST fail.
            assert!(
                authority
                    .acquire(ActivityClass::McpCall, "post-claim")
                    .is_err(),
                "acquisition after an accepted idle claim must be refused"
            );
            // And the claim can only have been accepted with an empty table:
            // any still-held token disproves emptiness at claim time.
            assert!(
                all_tokens.is_empty(),
                "idle claim was accepted while {} lease(s) were held",
                all_tokens.len()
            );
        }
        for token in all_tokens {
            authority.release(token);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn drain_waits_for_release_then_cleans() {
    let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();
    let token = acquire(&authority, ActivityClass::BackgroundTask);

    assert_eq!(
        coordinator.begin(ExitReason::SigTerm),
        BeginOutcome::Accepted
    );
    // Coordinator is draining: acquisition refused (I6).
    assert!(authority.acquire(ActivityClass::McpCall, "late").is_err());

    // Hold briefly, then release; the executor should complete promptly
    // (well before the 2s SigTerm drain deadline).
    tokio::time::sleep(Duration::from_millis(200)).await;
    authority.release(token);

    let outcome = tokio::time::timeout(Duration::from_secs(3), coordinator.wait_terminal())
        .await
        .expect("terminal outcome within drain budget");
    assert_eq!(
        outcome,
        TerminalOutcome::Cleaned {
            reason: ExitReason::SigTerm,
            code: 0
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn drain_deadline_abandons_stuck_lease() {
    let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();
    let _stuck = acquire(&authority, ActivityClass::SwarmWaiter); // never released

    assert_eq!(
        coordinator.begin(ExitReason::SigTerm),
        BeginOutcome::Accepted
    );
    // SigTerm drain budget is 2s; cleanup follows. Allow margin.
    let outcome = tokio::time::timeout(Duration::from_secs(5), coordinator.wait_terminal())
        .await
        .expect("bounded: stuck lease must be abandoned at the deadline");
    assert_eq!(
        outcome,
        TerminalOutcome::Cleaned {
            reason: ExitReason::SigTerm,
            code: 0
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn weaker_begin_is_superseded_stronger_upgrades_and_shortens() {
    let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();
    let _hold = acquire(&authority, ActivityClass::DebugJob); // keep it draining

    // Reload drain budget (5s) is the driving deadline.
    assert_eq!(
        coordinator.begin(ExitReason::TemporaryOwnerExit),
        BeginOutcome::Accepted
    );
    let deadline_before = coordinator
        .drain_deadline_for_test()
        .expect("draining has a deadline");

    // Weaker reason: superseded, deadline unchanged.
    assert_eq!(
        coordinator.begin(ExitReason::PersistentIdle),
        BeginOutcome::SupersededBy(ExitReason::TemporaryOwnerExit)
    );
    assert_eq!(coordinator.drain_deadline_for_test(), Some(deadline_before));

    // Stronger reason: accepted, reason replaced, deadline never extends.
    assert_eq!(
        coordinator.begin(ExitReason::SigTerm),
        BeginOutcome::Accepted
    );
    assert_eq!(
        coordinator.driving_reason_for_test(),
        Some(ExitReason::SigTerm)
    );
    let deadline_after = coordinator
        .drain_deadline_for_test()
        .expect("still draining");
    assert!(
        deadline_after <= deadline_before,
        "an upgrade must shorten or preserve the drain deadline"
    );

    let outcome = tokio::time::timeout(Duration::from_secs(6), coordinator.wait_terminal())
        .await
        .expect("bounded");
    assert_eq!(
        outcome,
        TerminalOutcome::Cleaned {
            reason: ExitReason::SigTerm,
            code: 0
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn pairwise_begin_races_produce_exactly_one_terminal_outcome() {
    // Every ordered pair of termination reasons raced concurrently: exactly
    // one terminal outcome, driven by one of the two reasons, cleanup once.
    use ExitReason::*;
    let reasons = [
        SigTerm,
        ReloadExecFailed,
        AcceptLoopFailure,
        TemporaryOwnerExit,
    ];
    for a in reasons {
        for b in reasons {
            let (coordinator, _authority) = ShutdownCoordinator::leaked_for_test();
            let first = tokio::spawn(async move { coordinator.begin(a) });
            let second = tokio::spawn(async move { coordinator.begin(b) });
            let (r1, r2) = (first.await.unwrap(), second.await.unwrap());
            // At least one begin was accepted; none was refused.
            assert!(
                matches!(r1, BeginOutcome::Accepted) || matches!(r2, BeginOutcome::Accepted),
                "one of the racers must drive ({a:?} vs {b:?}): {r1:?} / {r2:?}"
            );
            let outcome = tokio::time::timeout(Duration::from_secs(4), coordinator.wait_terminal())
                .await
                .unwrap_or_else(|_| {
                    panic!("terminal outcome for pair ({a:?}, {b:?}): {r1:?}/{r2:?}")
                });
            let TerminalOutcome::Cleaned { reason, .. } = outcome;
            assert!(
                reason == a || reason == b,
                "terminal reason {reason:?} must be one of the racers ({a:?}, {b:?})"
            );
            // Stronger racer wins when both were accepted as upgrades.
            if a != b {
                let stronger = if a.priority() >= b.priority() { a } else { b };
                if matches!(r1, BeginOutcome::Accepted) && matches!(r2, BeginOutcome::Accepted) {
                    assert_eq!(
                        reason, stronger,
                        "upgrade must let the stronger reason drive"
                    );
                }
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reload_refused_on_temporary_and_during_termination() {
    // Temporary server: typed refusal.
    let (coordinator, _authority) = ShutdownCoordinator::leaked_for_test();
    coordinator.configure(super::shutdown::ShutdownConfig {
        server_name: "test-temp".into(),
        socket_path: std::env::temp_dir().join("f03-none.sock"),
        debug_socket_path: std::env::temp_dir().join("f03-none-debug.sock"),
        temporary: true,
        intake_cancel: None,
        mcp_pool: None,
    });
    assert_eq!(
        coordinator.begin(ExitReason::Reload),
        BeginOutcome::Refused(RefusalReason::TemporaryServerNoReload)
    );
    assert!(matches!(
        coordinator.begin_reload_drain().await,
        Err(ReloadRefused::TemporaryServer)
    ));

    // Termination in progress: reload loses.
    let (coordinator, _authority) = ShutdownCoordinator::leaked_for_test();
    assert_eq!(
        coordinator.begin(ExitReason::SigTerm),
        BeginOutcome::Accepted
    );
    assert!(matches!(
        coordinator.begin_reload_drain().await,
        Err(ReloadRefused::ShutdownInProgress(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reload_drain_upgraded_by_sigterm_hands_completion_to_termination() {
    let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();
    let token = acquire(&authority, ActivityClass::ProviderTurn);

    // Reload drain parks on the held lease (reload budget 5s).
    let drain = tokio::spawn(async move { coordinator.begin_reload_drain().await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    // Reload must have closed acquisition already (F02-R2-I1 ordering).
    assert!(
        authority
            .acquire(ActivityClass::McpCall, "during-reload")
            .is_err()
    );

    // SIGTERM upgrades mid-drain: reload aborts, termination completes.
    assert_eq!(
        coordinator.begin(ExitReason::SigTerm),
        BeginOutcome::Accepted
    );
    let drain_result = tokio::time::timeout(Duration::from_secs(3), drain)
        .await
        .expect("reload drain returns after upgrade")
        .unwrap();
    assert!(matches!(
        drain_result,
        Err(ReloadRefused::ShutdownInProgress(ExitReason::SigTerm))
    ));

    authority.release(token);
    let outcome = tokio::time::timeout(Duration::from_secs(4), coordinator.wait_terminal())
        .await
        .expect("termination completes after the upgrade");
    assert_eq!(
        outcome,
        TerminalOutcome::Cleaned {
            reason: ExitReason::SigTerm,
            code: 0
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reload_drain_reaches_handoff_when_leases_release() {
    let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();
    let token = acquire(&authority, ActivityClass::McpCall);

    let drain = tokio::spawn(async move { coordinator.begin_reload_drain().await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    authority.release(token);

    let result = tokio::time::timeout(Duration::from_secs(3), drain)
        .await
        .expect("drain completes after release")
        .unwrap();
    assert!(
        result.is_ok(),
        "reload drain must reach Handoff: {result:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reload_exec_failure_reenters_termination_with_code_42() {
    let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();
    // Empty table: reload drain completes immediately into Handoff.
    let drain = coordinator.begin_reload_drain().await;
    assert!(drain.is_ok());
    drop(authority);

    let outcome = tokio::time::timeout(Duration::from_secs(4), coordinator.reload_exec_failed())
        .await
        .expect("exec-failure termination is bounded");
    assert_eq!(
        outcome,
        TerminalOutcome::Cleaned {
            reason: ExitReason::ReloadExecFailed,
            code: 42
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn many_waiters_all_observe_the_single_terminal_outcome() {
    let (coordinator, _authority) = ShutdownCoordinator::leaked_for_test();
    let mut waiters = Vec::new();
    for _ in 0..16 {
        waiters.push(tokio::spawn(
            async move { coordinator.wait_terminal().await },
        ));
    }
    assert_eq!(
        coordinator.begin(ExitReason::AcceptLoopFailure),
        BeginOutcome::Accepted
    );
    for waiter in waiters {
        let outcome = tokio::time::timeout(Duration::from_secs(4), waiter)
            .await
            .expect("waiter resolves")
            .unwrap();
        assert_eq!(
            outcome,
            TerminalOutcome::Cleaned {
                reason: ExitReason::AcceptLoopFailure,
                code: super::shutdown::EXIT_ACCEPT_LOOP_FAILURE
            }
        );
    }
}

// ---------------------------------------------------------------------------
// R04: reload drain vs accept-loop-exit race
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn accept_loop_exit_during_reload_drain_does_not_upgrade_reason() {
    // R04 incident replay: a drain-blocking lease (the streaming turn) parks
    // the reload drain; the drain's own intake cancellation stops the accept
    // loops. `run()` observes the accept-loop exit and must classify it as
    // drain-induced (AwaitTerminal), leaving the driving reason Reload so
    // the handoff proceeds when the lease releases.
    let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();
    let token = acquire(&authority, ActivityClass::ProviderTurn);

    let drain = tokio::spawn(async move { coordinator.begin_reload_drain().await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        coordinator.driving_reason_for_test(),
        Some(ExitReason::Reload),
        "reload drain must be in progress"
    );

    // The accept-loop exit event `run()` would observe: classify it. It must
    // NOT upgrade the reason (the incident called begin(AcceptLoopFailure)
    // unconditionally here).
    assert_eq!(
        coordinator.classify_accept_loop_exit(),
        AcceptLoopExitDisposition::AwaitTerminal,
        "drain-cancelled accept-loop exit must await terminal, not upgrade"
    );
    assert_eq!(
        coordinator.driving_reason_for_test(),
        Some(ExitReason::Reload),
        "driving reason must remain Reload after the accept-loop exit"
    );

    // Release the streaming turn: the reload drain reaches Handoff.
    authority.release(token);
    let result = tokio::time::timeout(Duration::from_secs(3), drain)
        .await
        .expect("drain completes after release")
        .unwrap();
    assert!(
        result.is_ok(),
        "reload drain must reach Handoff, not be refused: {result:?}"
    );
    assert_eq!(
        coordinator.driving_reason_for_test(),
        Some(ExitReason::Reload),
        "handoff must still be driven by Reload"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn accept_loop_exit_during_termination_drain_awaits_terminal() {
    // The same misclassification could upgrade e.g. TemporaryOwnerExit
    // (priority 2) to AcceptLoopFailure (4) during its own drain. Any begun
    // drain must classify as AwaitTerminal.
    let (coordinator, authority) = ShutdownCoordinator::leaked_for_test();
    let token = acquire(&authority, ActivityClass::McpCall);

    assert_eq!(
        coordinator.begin(ExitReason::TemporaryOwnerExit),
        BeginOutcome::Accepted
    );
    assert_eq!(
        coordinator.classify_accept_loop_exit(),
        AcceptLoopExitDisposition::AwaitTerminal
    );
    assert_eq!(
        coordinator.driving_reason_for_test(),
        Some(ExitReason::TemporaryOwnerExit)
    );

    authority.release(token);
    let outcome = tokio::time::timeout(Duration::from_secs(4), coordinator.wait_terminal())
        .await
        .expect("termination completes");
    assert_eq!(
        outcome,
        TerminalOutcome::Cleaned {
            reason: ExitReason::TemporaryOwnerExit,
            code: super::EXIT_IDLE_TIMEOUT
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn accept_loop_exit_while_running_still_fails_with_code_45() {
    // Genuine failure: the loop exits while the coordinator is Running.
    // Classification must say Failure, and the failure path must still
    // produce AcceptLoopFailure / exit 45.
    let (coordinator, _authority) = ShutdownCoordinator::leaked_for_test();
    assert_eq!(
        coordinator.classify_accept_loop_exit(),
        AcceptLoopExitDisposition::Failure,
        "an accept-loop exit while Running is a genuine failure"
    );
    let outcome = tokio::time::timeout(
        Duration::from_secs(4),
        coordinator.begin_and_wait(ExitReason::AcceptLoopFailure),
    )
    .await
    .expect("bounded")
    .expect("accept-loop-failure begin is never refused");
    assert_eq!(
        outcome,
        TerminalOutcome::Cleaned {
            reason: ExitReason::AcceptLoopFailure,
            code: super::shutdown::EXIT_ACCEPT_LOOP_FAILURE
        }
    );
}
