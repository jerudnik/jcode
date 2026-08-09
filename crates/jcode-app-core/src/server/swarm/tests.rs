use super::{
    broadcast_swarm_plan, broadcast_swarm_plan_with_previous, broadcast_swarm_status,
    member_status_is_dead, now_unix_ms, parse_swarm_tasks, refresh_swarm_task_staleness,
    remove_session_from_swarm, salvage_assignments_of_dead_member, swarm_ancestors,
    swarm_is_self_or_ancestor, swarm_spawn_depth, terminal_status_for_turn_error,
    touch_swarm_task_progress, update_member_status, update_member_status_with_report,
};
use crate::plan::PlanItem;
use crate::protocol::{NotificationType, ServerEvent};
use crate::server::{SwarmMember, VersionedPlan};
use jcode_swarm_core::{
    append_swarm_completion_report_instructions, summarize_plan_items, truncate_detail,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};

fn plan_item(id: &str, content: &str) -> PlanItem {
    PlanItem {
        content: content.to_string(),
        status: "pending".to_string(),
        priority: "medium".to_string(),
        id: id.to_string(),
        subsystem: None,
        file_scope: Vec::new(),
        blocked_by: Vec::new(),
        assigned_to: None,
    }
}

#[test]
fn truncate_detail_collapses_whitespace_and_ellipsizes() {
    assert_eq!(truncate_detail("hello   there\nworld", 11), "hello th...");
}

#[test]
fn terminal_status_maps_typed_interruption_to_stopped_cancelled() {
    let interrupted = crate::agent::Agent::interrupted_turn_error_for_tests();
    assert_eq!(
        terminal_status_for_turn_error(&interrupted),
        ("stopped", "cancelled".to_string())
    );

    // Wrapped interruptions keep the deterministic label.
    let wrapped = interrupted.context("while awaiting detached turn");
    assert_eq!(
        terminal_status_for_turn_error(&wrapped),
        ("stopped", "cancelled".to_string())
    );
}

#[test]
fn terminal_status_keeps_failed_for_non_interruption_errors() {
    let plain = anyhow::anyhow!("provider exploded");
    assert_eq!(
        terminal_status_for_turn_error(&plain),
        ("failed", "provider exploded".to_string())
    );

    // A lookalike display string must not become stopped/cancelled.
    let lookalike = anyhow::anyhow!("turn interrupted");
    assert_eq!(terminal_status_for_turn_error(&lookalike).0, "failed");
}

/// W7b determinism: a cancelled detached turn ends `stopped/cancelled`
/// regardless of whether the cancel path or the turn-completion consumer
/// writes the terminal member status last. Both writers now derive the
/// label from the same typed mapping, so both orderings converge.
#[tokio::test]
async fn detached_cancel_final_member_status_is_stopped_for_both_write_orders() {
    for completion_writes_last in [false, true] {
        let swarm_members = Arc::new(RwLock::new(HashMap::new()));
        let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
            "swarm-1".to_string(),
            HashSet::from(["worker".to_string()]),
        )])));
        let (mut worker, _worker_rx) = swarm_member("worker", "agent", true);
        worker.status = "running".to_string();
        swarm_members
            .write()
            .await
            .insert("worker".to_string(), worker);

        // The cancel path writes the literal stopped/cancelled pair
        // (client_lifecycle::cancel_processing_message). The completion
        // consumer maps the interrupted error through the shared helper.
        let interrupted = crate::agent::Agent::interrupted_turn_error_for_tests();
        let (completion_status, completion_detail) = terminal_status_for_turn_error(&interrupted);

        let writes: [(&str, String); 2] = if completion_writes_last {
            [
                ("stopped", "cancelled".to_string()),
                (completion_status, completion_detail),
            ]
        } else {
            [
                (completion_status, completion_detail),
                ("stopped", "cancelled".to_string()),
            ]
        };
        for (status, detail) in writes {
            update_member_status(
                "worker",
                status,
                Some(detail),
                &swarm_members,
                &swarms_by_id,
                None,
                None,
                None,
            )
            .await;
        }

        let members = swarm_members.read().await;
        let worker = members.get("worker").expect("worker member");
        assert_eq!(
            worker.status, "stopped",
            "final label must be stopped when completion writes last = {completion_writes_last}"
        );
        assert_eq!(worker.detail.as_deref(), Some("cancelled"));
    }
}

#[test]
fn summarize_plan_items_limits_output() {
    let items = vec![
        plan_item("1", "inspect"),
        plan_item("2", "refactor"),
        plan_item("3", "test"),
    ];

    assert_eq!(
        summarize_plan_items(&items, 2),
        "inspect; refactor (+1 more)"
    );
}

#[test]
fn parse_swarm_tasks_accepts_wrapped_json() {
    let text = "Plan:\n[{\"description\":\"A\",\"prompt\":\"B\",\"subagent_type\":\"general\"}]";
    let tasks = parse_swarm_tasks(text);

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].description, "A");
    assert_eq!(tasks[0].prompt, "B");
    assert_eq!(tasks[0].subagent_type.as_deref(), Some("general"));
}

#[test]
fn append_swarm_completion_report_instructions_is_idempotent() {
    let prompt = "Implement the task.";
    let with_instructions = append_swarm_completion_report_instructions(prompt);

    assert!(with_instructions.starts_with(prompt));
    assert!(with_instructions.contains("SWARM COMPLETION REPORT REQUIRED"));
    assert!(with_instructions.contains("swarm tool with action=\"report\""));
    assert_eq!(
        append_swarm_completion_report_instructions(&with_instructions),
        with_instructions
    );
}

fn swarm_member(
    session_id: &str,
    role: &str,
    is_headless: bool,
) -> (SwarmMember, mpsc::UnboundedReceiver<ServerEvent>) {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    (
        SwarmMember {
            session_id: session_id.to_string(),
            event_tx,
            event_txs: HashMap::new(),
            working_dir: None,
            swarm_id: Some("swarm-1".to_string()),
            swarm_enabled: true,
            status: "ready".to_string(),
            detail: None,
            task_label: None,
            subagent_type: None,
            friendly_name: Some(session_id.to_string()),
            report_back_to_session_id: None,
            initial_prompt_delivered: None,
            latest_completion_report: None,
            role: role.to_string(),
            joined_at: Instant::now(),
            last_status_change: Instant::now(),
            is_headless,
            output_tail: None,
            todo_progress: None,
            todo_items: Vec::new(),
            runtime: crate::protocol::SwarmMemberRuntime::default(),
        },
        event_rx,
    )
}

fn member_with_parent(session_id: &str, parent: Option<&str>) -> SwarmMember {
    let (mut member, _rx) = swarm_member(session_id, "agent", false);
    member.report_back_to_session_id = parent.map(str::to_string);
    member
}

include!("swarm_tests/dead_pid.rs");

#[test]
fn swarm_depth_and_ancestry_follow_report_back_chain() {
    let mut members: HashMap<String, SwarmMember> = HashMap::new();
    for (id, parent) in [
        ("root", None),
        ("a", Some("root")),
        ("b", Some("a")),
        ("c", Some("b")),
    ] {
        members.insert(id.to_string(), member_with_parent(id, parent));
    }

    assert_eq!(swarm_spawn_depth(&members, "root"), 0);
    assert_eq!(swarm_spawn_depth(&members, "a"), 1);
    assert_eq!(swarm_spawn_depth(&members, "c"), 3);
    assert_eq!(swarm_ancestors(&members, "c"), vec!["b", "a", "root"]);

    // Ownership: an ancestor (or self) owns the subtree.
    assert!(swarm_is_self_or_ancestor(&members, "a", "c"));
    assert!(swarm_is_self_or_ancestor(&members, "root", "c"));
    assert!(swarm_is_self_or_ancestor(&members, "c", "c"));
    // A sibling/descendant is not an ancestor.
    assert!(!swarm_is_self_or_ancestor(&members, "c", "a"));
    assert!(!swarm_is_self_or_ancestor(&members, "b", "a"));
}

#[test]
fn swarm_ancestry_guards_against_cycles() {
    let mut members: HashMap<String, SwarmMember> = HashMap::new();
    // x -> y -> x is a (pathological) cycle; depth must terminate.
    members.insert("x".to_string(), member_with_parent("x", Some("y")));
    members.insert("y".to_string(), member_with_parent("y", Some("x")));
    assert_eq!(swarm_spawn_depth(&members, "x"), 1);
    assert_eq!(swarm_ancestors(&members, "x"), vec!["y"]);
}

#[tokio::test]
async fn broadcast_swarm_plan_with_previous_includes_newly_ready_ids() {
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        VersionedPlan {
            items: vec![
                PlanItem {
                    content: "setup".to_string(),
                    status: "completed".to_string(),
                    priority: "high".to_string(),
                    id: "setup".to_string(),
                    subsystem: None,
                    file_scope: Vec::new(),
                    blocked_by: Vec::new(),
                    assigned_to: None,
                },
                PlanItem {
                    content: "follow-up".to_string(),
                    status: "queued".to_string(),
                    priority: "high".to_string(),
                    id: "follow-up".to_string(),
                    subsystem: None,
                    file_scope: Vec::new(),
                    blocked_by: vec!["setup".to_string()],
                    assigned_to: None,
                },
            ],
            version: 2,
            participants: HashSet::from(["worker".to_string()]),
            task_progress: HashMap::new(),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    let (worker, mut worker_rx) = swarm_member("worker", "agent", false);
    let swarm_members = Arc::new(RwLock::new(HashMap::from([("worker".to_string(), worker)])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["worker".to_string()]),
    )])));
    let previous_items = vec![
        PlanItem {
            content: "setup".to_string(),
            status: "running".to_string(),
            priority: "high".to_string(),
            id: "setup".to_string(),
            subsystem: None,
            file_scope: Vec::new(),
            blocked_by: Vec::new(),
            assigned_to: Some("worker".to_string()),
        },
        PlanItem {
            content: "follow-up".to_string(),
            status: "queued".to_string(),
            priority: "high".to_string(),
            id: "follow-up".to_string(),
            subsystem: None,
            file_scope: Vec::new(),
            blocked_by: vec!["setup".to_string()],
            assigned_to: None,
        },
    ];

    broadcast_swarm_plan_with_previous(
        "swarm-1",
        Some("task_completed".to_string()),
        Some(&previous_items),
        &swarm_plans,
        &swarm_members,
        &swarms_by_id,
    )
    .await;

    match worker_rx.recv().await.expect("swarm plan event") {
        ServerEvent::SwarmPlan {
            reason,
            summary: Some(summary),
            ..
        } => {
            assert_eq!(reason.as_deref(), Some("task_completed"));
            assert_eq!(summary.newly_ready_ids, vec!["follow-up".to_string()]);
            assert_eq!(summary.next_ready_ids, vec!["follow-up".to_string()]);
        }
        other => panic!("expected SwarmPlan event, got {other:?}"),
    }
}

/// Deterministic demonstration of the mutate->broadcast version-inversion
/// race (wiring-audit.plan-broadcast-ordering).
///
/// `broadcast_swarm_plan_with_previous` snapshots `(version, items)` under
/// `swarm_plans.read()`, releases the lock, and only later (after further
/// await points on `swarms_by_id.read()` / `swarm_members.read()`) sends
/// on `member.event_tx`. A second mutator can bump the version AND
/// complete its own broadcast inside that window, so a single ordered
/// mpsc channel can deliver v6 before v5.
///
/// This test parks broadcast A (snapshot v5, empty participants, so it
/// must await `swarms_by_id.read()`) behind a held `swarms_by_id.write()`
/// guard, lets mutator B bump to v6 and broadcast it, then releases A.
/// The worker receives [6, 5]: inverted versions on one channel.
///
/// If this test starts failing with versions == [6, 6] or [5, 6], the
/// race has been fixed (e.g. by holding the plan lock through send or by
/// stamping a send-order sequence); update the wiring audit and consider
/// whether the TUI-side monotonicity guard (server_events.rs SwarmPlan
/// handler currently overwrites `swarm_plan_version` unconditionally) is
/// still needed.
#[tokio::test]
async fn swarm_plan_broadcast_versions_can_invert_on_one_member_channel() {
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        VersionedPlan {
            items: vec![plan_item("t1", "task one")],
            version: 5,
            // Empty participants: broadcast A takes the swarms_by_id
            // fallback path, which is where we deterministically park it.
            participants: HashSet::new(),
            task_progress: HashMap::new(),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    let (worker, mut worker_rx) = swarm_member("worker", "agent", false);
    let swarm_members = Arc::new(RwLock::new(HashMap::from([("worker".to_string(), worker)])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["worker".to_string()]),
    )])));

    // Hold a write guard on swarms_by_id so broadcast A parks after it
    // has already snapshotted version 5 from swarm_plans.
    let gate = swarms_by_id.write().await;

    let a = tokio::spawn({
        let swarm_plans = Arc::clone(&swarm_plans);
        let swarm_members = Arc::clone(&swarm_members);
        let swarms_by_id = Arc::clone(&swarms_by_id);
        async move {
            broadcast_swarm_plan(
                "swarm-1",
                Some("mutator_1".to_string()),
                &swarm_plans,
                &swarm_members,
                &swarms_by_id,
            )
            .await;
        }
    });
    // Current-thread test runtime: yielding runs A until it parks on the
    // contended swarms_by_id.read().await, past its v5 snapshot.
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }

    // Mutator B: bump to v6 and register an explicit participant so B's
    // broadcast skips the swarms_by_id fallback and is not blocked by
    // the gate. This mirrors real mutators (write, release, broadcast).
    {
        let mut plans = swarm_plans.write().await;
        let vp = plans.get_mut("swarm-1").expect("plan");
        vp.version = 6;
        vp.participants.insert("worker".to_string());
    }
    broadcast_swarm_plan(
        "swarm-1",
        Some("mutator_2".to_string()),
        &swarm_plans,
        &swarm_members,
        &swarms_by_id,
    )
    .await;

    // Release A: it resumes with its stale v5 snapshot and sends it
    // after v6 on the same ordered channel.
    drop(gate);
    a.await.expect("broadcast task");

    let mut versions = Vec::new();
    while let Ok(event) = worker_rx.try_recv() {
        if let ServerEvent::SwarmPlan { version, .. } = event {
            versions.push(version);
        }
    }
    assert_eq!(
        versions,
        vec![6, 5],
        "expected version inversion on one member channel; if this fails \
         the mutate->broadcast race may have been fixed (update the \
         wiring audit)"
    );
}

/// Deterministic demonstration of the SwarmStatus immediate-path
/// snapshot-vs-send inversion (wiring-audit.status-proposal-ordering).
///
/// `broadcast_swarm_status_now` snapshots member statuses under
/// `swarm_members.read()`, drops the guard, then awaits
/// `fanout_session_event` (a `swarm_members.write()` acquisition) before
/// sending. Swarms below `JCODE_SWARM_STATUS_DEBOUNCE_MEMBER_THRESHOLD`
/// (default 2) take this immediate, non-debounced path on every status
/// change, so two concurrent broadcasts can deliver an old snapshot after
/// a newer one on the same ordered mpsc channel. A last-write-wins
/// consumer (the TUI SwarmStatus handler) is then left showing the stale
/// status until the next unrelated broadcast.
///
/// Unlike the SwarmPlan inversion test above, the status path snapshots
/// from the same `swarm_members` lock it later writes, so holding any
/// guard also blocks the mutator. This test uses tokio's cooperative
/// budget (128 units per task poll on a current-thread runtime; every
/// RwLock acquisition consumes exactly one). Draining 126 units leaves
/// broadcast A exactly enough for `swarms_by_id.read()` and the
/// `swarm_members.read()` snapshot, forcing a yield at the (uncontended)
/// `swarm_members.write()` inside `fanout_session_event`, i.e. precisely
/// inside what used to be the race window between snapshot and send.
///
/// The race is now CLOSED: W1's dual-write funnel adds a
/// `control_log_sync` read-lock acquisition inside
/// `broadcast_swarm_status` (before delegating to
/// `broadcast_swarm_status_now`), consuming a coop-budget unit ahead of
/// the snapshot. Broadcast A therefore snapshots the post-mutation
/// "running" state rather than a stale "ready", so this test asserts NO
/// inversion. If it regresses to `["running", "ready"]`, the dual-write
/// funnel stopped ordering the snapshot after concurrent mutations and
/// the race has reopened. If it parks somewhere else, the tokio coop
/// budget constants changed: re-derive the `128 - 2` drain count.
#[tokio::test]
async fn swarm_status_immediate_broadcasts_do_not_invert_on_one_member_channel() {
    let (worker, mut worker_rx) = swarm_member("worker", "agent", false);
    let swarm_members = Arc::new(RwLock::new(HashMap::from([("worker".to_string(), worker)])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["worker".to_string()]),
    )])));

    // Broadcast A: snapshots status "ready", then is forced to yield at
    // the fanout write acquisition, before sending.
    let a = tokio::spawn({
        let swarm_members = Arc::clone(&swarm_members);
        let swarms_by_id = Arc::clone(&swarms_by_id);
        async move {
            // Initial task budget is 128. Leave exactly 2 units so the two
            // read acquisitions (session-id list + status snapshot)
            // succeed and the fanout write acquisition forces a yield.
            for _ in 0..126 {
                tokio::task::coop::consume_budget().await;
            }
            broadcast_swarm_status("swarm-1", &swarm_members, &swarms_by_id).await;
        }
    });
    // Single yield on the current-thread runtime: A runs its entire first
    // poll (budget drain + both reads) and parks after snapshotting
    // "ready". Its coop yield happens *before* joining the lock queue, so
    // every acquisition below is uncontended and the mutator finishes
    // within one poll, before A is re-polled.
    tokio::task::yield_now().await;

    // Concurrent mutator: flips the status and completes its own
    // immediate broadcast while A is parked between snapshot and send.
    {
        let mut members = swarm_members.write().await;
        members.get_mut("worker").expect("worker member").status = "running".to_string();
    }
    broadcast_swarm_status("swarm-1", &swarm_members, &swarms_by_id).await;

    // Release A: it resumes with a fresh budget and sends its stale
    // "ready" snapshot after "running" on the same ordered channel.
    a.await.expect("broadcast task");

    let mut statuses = Vec::new();
    while let Ok(event) = worker_rx.try_recv() {
        if let ServerEvent::SwarmStatus { members } = event {
            assert_eq!(members.len(), 1);
            assert_eq!(members[0].session_id, "worker");
            statuses.push(members[0].status.clone());
        }
    }
    assert_eq!(
        statuses,
        vec!["running".to_string(), "running".to_string()],
        "expected NO status inversion: W1's dual-write funnel adds a \
         control_log_sync read-lock acquisition inside \
         broadcast_swarm_status before the snapshot in \
         broadcast_swarm_status_now, so broadcast A now snapshots the \
         post-mutation 'running' state instead of a stale 'ready'. If \
         this regresses to [\"running\", \"ready\"] the snapshot-vs-send \
         race has reopened (the dual-write funnel stopped ordering the \
         snapshot after concurrent mutations); re-audit \
         broadcast_swarm_status. If the coop budget constants changed, \
         re-derive the `128 - 2` drain count."
    );
}

/// Restored (persisted) plan participants with dead channels starve live
/// swarm members of plan broadcasts: the fallback to swarms_by_id only
/// triggers when `participants` is EMPTY, so a participant set that only
/// contains stale sessions (e.g. restored after a server restart, where
/// `from_persisted_member` gives every member a closed event_tx) means
/// nobody receives the snapshot, not even live members of the swarm.
#[tokio::test]
async fn stale_participants_starve_live_members_of_plan_broadcasts() {
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        VersionedPlan {
            items: vec![plan_item("t1", "task one")],
            version: 7,
            // "ghost" is a participant restored from disk whose session
            // no longer exists in this server process.
            participants: HashSet::from(["ghost".to_string()]),
            task_progress: HashMap::new(),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    // Ghost member as produced by swarm_persistence restore: present in
    // the member map but with a closed event channel.
    let (ghost, ghost_rx) = swarm_member("ghost", "agent", true);
    drop(ghost_rx);
    let (live, mut live_rx) = swarm_member("live", "agent", false);
    let swarm_members = Arc::new(RwLock::new(HashMap::from([
        ("ghost".to_string(), ghost),
        ("live".to_string(), live),
    ])));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["ghost".to_string(), "live".to_string()]),
    )])));

    broadcast_swarm_plan(
        "swarm-1",
        Some("test".to_string()),
        &swarm_plans,
        &swarm_members,
        &swarms_by_id,
    )
    .await;

    assert!(
        live_rx.try_recv().is_err(),
        "live member unexpectedly received the plan broadcast; stale \
         participant starvation may have been fixed (update the wiring \
         audit)"
    );
}

#[tokio::test]
async fn remove_session_from_swarm_reassigns_to_non_headless_member() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from([
            "coord".to_string(),
            "headless".to_string(),
            "worker".to_string(),
        ]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "coord".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        VersionedPlan {
            items: vec![PlanItem {
                content: "task".to_string(),
                status: "pending".to_string(),
                priority: "medium".to_string(),
                id: "1".to_string(),
                subsystem: None,
                file_scope: Vec::new(),
                blocked_by: Vec::new(),
                assigned_to: Some("coord".to_string()),
            }],
            version: 1,
            participants: HashSet::from(["coord".to_string()]),
            task_progress: HashMap::new(),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));

    let (coord, _coord_rx) = swarm_member("coord", "coordinator", false);
    let (headless, mut headless_rx) = swarm_member("headless", "agent", true);
    let (worker, mut worker_rx) = swarm_member("worker", "agent", false);
    {
        let mut members = swarm_members.write().await;
        members.insert("coord".to_string(), coord);
        members.insert("headless".to_string(), headless);
        members.insert("worker".to_string(), worker);
        members.remove("coord");
    }

    remove_session_from_swarm(
        "coord",
        "swarm-1",
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
    )
    .await;

    assert_eq!(
        swarm_coordinators
            .read()
            .await
            .get("swarm-1")
            .map(String::as_str),
        Some("worker")
    );
    assert!(
        swarm_plans
            .read()
            .await
            .get("swarm-1")
            .is_some_and(|plan| plan.participants.contains("worker"))
    );
    assert_eq!(
        swarm_members
            .read()
            .await
            .get("worker")
            .map(|member| member.role.as_str()),
        Some("coordinator")
    );
    assert_eq!(
        swarm_members
            .read()
            .await
            .get("headless")
            .map(|member| member.role.as_str()),
        Some("agent")
    );

    let headless_events: Vec<_> = std::iter::from_fn(|| headless_rx.try_recv().ok()).collect();
    assert!(headless_events.iter().all(|event| {
        !matches!(
            event,
            ServerEvent::Notification {
                notification_type: NotificationType::Message { .. },
                message,
                ..
            } if message == "You are now the coordinator for this swarm."
        )
    }));

    let worker_events: Vec<_> = std::iter::from_fn(|| worker_rx.try_recv().ok()).collect();
    assert!(worker_events.iter().any(|event| {
        matches!(
            event,
            ServerEvent::Notification {
                notification_type: NotificationType::Message { .. },
                message,
                ..
            } if message == "You are now the coordinator for this swarm."
        )
    }));
}

#[tokio::test]
async fn remove_session_reparents_children_to_live_grandparent() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["root".to_string(), "mid".to_string(), "leaf".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "root".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::new()));

    let (root, _root_rx) = swarm_member("root", "coordinator", false);
    let (mut mid, _mid_rx) = swarm_member("mid", "agent", true);
    mid.report_back_to_session_id = Some("root".to_string());
    let (mut leaf, _leaf_rx) = swarm_member("leaf", "agent", true);
    leaf.report_back_to_session_id = Some("mid".to_string());
    {
        let mut members = swarm_members.write().await;
        members.insert("root".to_string(), root);
        members.insert("mid".to_string(), mid);
        members.insert("leaf".to_string(), leaf);
    }

    remove_session_from_swarm(
        "mid",
        "swarm-1",
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
    )
    .await;

    // Leaf follows the report-back chain up to its grandparent instead of
    // dangling on the removed session.
    let members = swarm_members.read().await;
    assert_eq!(
        members
            .get("leaf")
            .and_then(|member| member.report_back_to_session_id.as_deref()),
        Some("root")
    );
    assert!(swarm_is_self_or_ancestor(&members, "root", "leaf"));
}

#[tokio::test]
async fn remove_session_reparents_children_to_coordinator_when_no_grandparent() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from([
            "coord".to_string(),
            "peer_root".to_string(),
            "child".to_string(),
        ]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "coord".to_string(),
    )])));
    let swarm_plans = Arc::new(RwLock::new(HashMap::new()));

    // peer_root is itself a root (no parent), so its children have no
    // grandparent to inherit; they should fall back to the coordinator.
    let (coord, _coord_rx) = swarm_member("coord", "coordinator", false);
    let (peer_root, _peer_rx) = swarm_member("peer_root", "agent", false);
    let (mut child, _child_rx) = swarm_member("child", "agent", true);
    child.report_back_to_session_id = Some("peer_root".to_string());
    {
        let mut members = swarm_members.write().await;
        members.insert("coord".to_string(), coord);
        members.insert("peer_root".to_string(), peer_root);
        members.insert("child".to_string(), child);
    }

    remove_session_from_swarm(
        "peer_root",
        "swarm-1",
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
    )
    .await;

    let members = swarm_members.read().await;
    assert_eq!(
        members
            .get("child")
            .and_then(|member| member.report_back_to_session_id.as_deref()),
        Some("coord")
    );
}

#[tokio::test]
async fn update_member_status_notifies_coordinator_when_headless_worker_returns_ready() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["coord".to_string(), "worker".to_string()]),
    )])));

    let (coord, mut coord_rx) = swarm_member("coord", "coordinator", false);
    let (mut worker, _worker_rx) = swarm_member("worker", "agent", true);
    worker.status = "running".to_string();
    worker.detail = Some("doing task".to_string());
    worker.report_back_to_session_id = Some("coord".to_string());
    {
        let mut members = swarm_members.write().await;
        members.insert("coord".to_string(), coord);
        members.insert("worker".to_string(), worker);
    }

    update_member_status(
        "worker",
        "ready",
        None,
        &swarm_members,
        &swarms_by_id,
        None,
        None,
        None,
    )
    .await;

    let events: Vec<_> = std::iter::from_fn(|| coord_rx.try_recv().ok()).collect();
    assert!(events.iter().any(|event| {
        matches!(
            event,
            ServerEvent::Notification {
                notification_type: NotificationType::Message { .. },
                message,
                ..
            } if message.contains("finished their work and is ready for more")
        )
    }));
}

#[tokio::test]
async fn member_elapsed_time_runs_only_while_active_and_freezes_afterward() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["worker".to_string()]),
    )])));
    let (mut worker, _worker_rx) = swarm_member("worker", "agent", true);
    worker.runtime.elapsed_secs = Some(12);
    swarm_members
        .write()
        .await
        .insert("worker".to_string(), worker);

    update_member_status(
        "worker",
        "running",
        None,
        &swarm_members,
        &swarms_by_id,
        None,
        None,
        None,
    )
    .await;
    assert_eq!(
        swarm_members
            .read()
            .await
            .get("worker")
            .and_then(|member| member.runtime.elapsed_secs),
        None,
        "active members should derive elapsed time from joined_at"
    );

    {
        let mut members = swarm_members.write().await;
        members.get_mut("worker").unwrap().joined_at = Instant::now() - Duration::from_secs(37);
    }
    update_member_status(
        "worker",
        "completed",
        None,
        &swarm_members,
        &swarms_by_id,
        None,
        None,
        None,
    )
    .await;

    let frozen = swarm_members
        .read()
        .await
        .get("worker")
        .and_then(|member| member.runtime.elapsed_secs)
        .expect("terminal member should retain frozen elapsed time");
    assert!((37..=38).contains(&frozen), "frozen={frozen}");
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        swarm_members
            .read()
            .await
            .get("worker")
            .and_then(|member| member.runtime.elapsed_secs),
        Some(frozen)
    );
}

#[tokio::test]
async fn update_member_status_prefers_explicit_report_back_owner_over_coordinator() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from([
            "coord".to_string(),
            "owner".to_string(),
            "worker".to_string(),
        ]),
    )])));

    let (coord, mut coord_rx) = swarm_member("coord", "coordinator", false);
    let (owner, mut owner_rx) = swarm_member("owner", "agent", false);
    let (mut worker, _worker_rx) = swarm_member("worker", "agent", true);
    worker.status = "running".to_string();
    worker.detail = Some("doing task".to_string());
    worker.report_back_to_session_id = Some("owner".to_string());
    {
        let mut members = swarm_members.write().await;
        members.insert("coord".to_string(), coord);
        members.insert("owner".to_string(), owner);
        members.insert("worker".to_string(), worker);
    }

    update_member_status(
        "worker",
        "ready",
        None,
        &swarm_members,
        &swarms_by_id,
        None,
        None,
        None,
    )
    .await;

    let owner_events: Vec<_> = std::iter::from_fn(|| owner_rx.try_recv().ok()).collect();
    assert!(owner_events.iter().any(|event| {
        matches!(
            event,
            ServerEvent::Notification {
                notification_type: NotificationType::Message { .. },
                message,
                ..
            } if message.contains("finished their work and is ready for more")
        )
    }));
    let coord_events: Vec<_> = std::iter::from_fn(|| coord_rx.try_recv().ok()).collect();
    assert!(coord_events.iter().all(|event| {
        !matches!(
            event,
            ServerEvent::Notification {
                notification_type: NotificationType::Message { .. },
                message,
                ..
            } if message.contains("finished their work and is ready for more")
        )
    }));
}

#[tokio::test]
async fn update_member_status_includes_completion_report_in_owner_notification() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["coord".to_string(), "worker".to_string()]),
    )])));

    let (coord, mut coord_rx) = swarm_member("coord", "coordinator", false);
    let (mut worker, _worker_rx) = swarm_member("worker", "agent", true);
    worker.status = "running".to_string();
    worker.report_back_to_session_id = Some("coord".to_string());
    {
        let mut members = swarm_members.write().await;
        members.insert("coord".to_string(), coord);
        members.insert("worker".to_string(), worker);
    }

    update_member_status_with_report(
        "worker",
        "ready",
        None,
        Some("Validated the parser and all tests passed.".to_string()),
        &swarm_members,
        &swarms_by_id,
        None,
        None,
        None,
    )
    .await;

    let events: Vec<_> = std::iter::from_fn(|| coord_rx.try_recv().ok()).collect();
    assert!(events.iter().any(|event| {
        matches!(
            event,
            ServerEvent::Notification {
                notification_type: NotificationType::Message { .. },
                message,
                ..
            } if message.contains("Report:\nValidated the parser")
                && !message.contains("No final textual report")
        )
    }));
}

#[tokio::test]
async fn update_member_status_skips_noop_broadcasts() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["worker".to_string()]),
    )])));

    let (worker, mut worker_rx) = swarm_member("worker", "agent", false);
    swarm_members
        .write()
        .await
        .insert("worker".to_string(), worker);

    update_member_status(
        "worker",
        "ready",
        None,
        &swarm_members,
        &swarms_by_id,
        None,
        None,
        None,
    )
    .await;

    assert!(worker_rx.try_recv().is_err());

    update_member_status(
        "worker",
        "busy",
        Some("working".to_string()),
        &swarm_members,
        &swarms_by_id,
        None,
        None,
        None,
    )
    .await;

    assert!(matches!(
        worker_rx.try_recv(),
        Ok(ServerEvent::SwarmStatus { members }) if members.len() == 1
            && members[0].session_id == "worker"
            && members[0].status == "busy"
            && members[0].detail.as_deref() == Some("working")
    ));
}

#[tokio::test]
async fn refresh_swarm_task_staleness_marks_running_tasks_stale_and_heartbeat_revives() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["worker".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::new()));
    let now_ms = now_unix_ms();
    let stale_age_ms = super::swarm_task_stale_after().as_millis() as u64 + 5_000;
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        VersionedPlan {
            items: vec![PlanItem {
                content: "task".to_string(),
                status: "running".to_string(),
                priority: "medium".to_string(),
                id: "task-1".to_string(),
                subsystem: None,
                file_scope: Vec::new(),
                blocked_by: Vec::new(),
                assigned_to: Some("worker".to_string()),
            }],
            version: 1,
            participants: HashSet::from(["worker".to_string()]),
            task_progress: HashMap::from([(
                "task-1".to_string(),
                crate::server::SwarmTaskProgress {
                    assigned_session_id: Some("worker".to_string()),
                    started_at_unix_ms: Some(now_ms.saturating_sub(stale_age_ms)),
                    last_heartbeat_unix_ms: Some(now_ms.saturating_sub(stale_age_ms)),
                    ..Default::default()
                },
            )]),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));
    let (worker, _worker_rx) = swarm_member("worker", "agent", true);
    swarm_members
        .write()
        .await
        .insert("worker".to_string(), worker);

    refresh_swarm_task_staleness(
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;

    {
        let plans = swarm_plans.read().await;
        let plan = plans.get("swarm-1").expect("plan");
        assert_eq!(plan.items[0].status, "running_stale");
        assert!(
            plan.task_progress
                .get("task-1")
                .and_then(|progress| progress.stale_since_unix_ms)
                .is_some()
        );
    }

    let revived = touch_swarm_task_progress(
        "swarm-1",
        "task-1",
        Some("worker"),
        Some("still working".to_string()),
        Some("checkpoint saved".to_string()),
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;
    assert!(revived);

    let plans = swarm_plans.read().await;
    let plan = plans.get("swarm-1").expect("plan");
    assert_eq!(plan.items[0].status, "running");
    let progress = plan.task_progress.get("task-1").expect("progress");
    assert_eq!(
        progress.checkpoint_summary.as_deref(),
        Some("checkpoint saved")
    );
    assert!(progress.stale_since_unix_ms.is_none());
}

#[test]
fn member_status_is_dead_matches_terminal_non_success_states() {
    for status in ["failed", "stopped", "crashed"] {
        assert!(member_status_is_dead(status), "{status} should be dead");
        assert!(
            crate::swarm_verbs::member_status_is_dead(status),
            "verb/report path should agree that {status} is dead"
        );
    }
    for status in ["ready", "running", "running_stale", "queued", "completed"] {
        assert!(!member_status_is_dead(status), "{status} should be alive");
        assert!(
            !crate::swarm_verbs::member_status_is_dead(status),
            "verb/report path should agree that {status} is alive"
        );
    }
}

fn running_plan_assigned_to(
    assignee: &str,
    reclaims: Option<u32>,
) -> Arc<RwLock<HashMap<String, VersionedPlan>>> {
    Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        VersionedPlan {
            items: vec![PlanItem {
                content: "task".to_string(),
                status: "running".to_string(),
                priority: "medium".to_string(),
                id: "task-1".to_string(),
                subsystem: None,
                file_scope: Vec::new(),
                blocked_by: Vec::new(),
                assigned_to: Some(assignee.to_string()),
            }],
            version: 1,
            participants: HashSet::from([assignee.to_string()]),
            task_progress: HashMap::from([(
                "task-1".to_string(),
                crate::server::SwarmTaskProgress {
                    assigned_session_id: Some(assignee.to_string()),
                    dead_assignee_reclaims: reclaims,
                    ..Default::default()
                },
            )]),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])))
}

#[tokio::test]
async fn salvage_requeues_dead_members_tasks_and_notifies_coordinator() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["coord".to_string(), "worker".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "coord".to_string(),
    )])));
    let swarm_plans = running_plan_assigned_to("worker", None);
    {
        let mut plans = swarm_plans.write().await;
        let plan = plans.get_mut("swarm-1").expect("plan");
        let progress = plan.task_progress.get_mut("task-1").expect("progress");
        progress.last_heartbeat_unix_ms = Some(42);
        progress.last_detail = Some("old detail".to_string());
        progress.checkpoint_summary = Some("old checkpoint".to_string());
        progress.checkpoint_count = Some(3);
    }
    let (coord, mut coord_rx) = swarm_member("coord", "coordinator", false);
    let (worker, _worker_rx) = swarm_member("worker", "agent", true);
    {
        let mut members = swarm_members.write().await;
        members.insert("coord".to_string(), coord);
        members.insert("worker".to_string(), worker);
    }

    let outcome = salvage_assignments_of_dead_member(
        "worker",
        "swarm-1",
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;

    assert_eq!(outcome.requeued_task_ids, vec!["task-1".to_string()]);
    assert!(outcome.failed_task_ids.is_empty());
    {
        let plans = swarm_plans.read().await;
        let plan = plans.get("swarm-1").expect("plan");
        assert_eq!(plan.items[0].status, "queued");
        assert_eq!(plan.items[0].assigned_to, None);
        let progress = plan.task_progress.get("task-1").expect("progress");
        assert_eq!(progress.assigned_session_id, None);
        assert_eq!(progress.dead_assignee_reclaims, Some(1));
        assert_eq!(progress.last_heartbeat_unix_ms, Some(42));
        assert_eq!(progress.last_detail.as_deref(), Some("old detail"));
        assert_eq!(progress.checkpoint_count, Some(3));
        let checkpoint_summary = progress.checkpoint_summary.as_deref().unwrap_or_default();
        assert!(checkpoint_summary.contains("old checkpoint"));
        assert!(checkpoint_summary.contains("assignment reclaimed"));
    }

    let coord_events: Vec<_> = std::iter::from_fn(|| coord_rx.try_recv().ok()).collect();
    assert!(
        coord_events.iter().any(|event| matches!(
            event,
            ServerEvent::Notification { message, .. }
                if message.contains("died") && message.contains("task-1")
        )),
        "coordinator should be told about the salvage, got {coord_events:?}"
    );
}

#[tokio::test]
async fn salvage_fails_task_once_reclaim_cap_is_reached() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["worker".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::new()));
    let swarm_plans =
        running_plan_assigned_to("worker", Some(crate::plan::MAX_DEAD_ASSIGNEE_RECLAIMS));
    {
        let mut plans = swarm_plans.write().await;
        let plan = plans.get_mut("swarm-1").expect("plan");
        let progress = plan.task_progress.get_mut("task-1").expect("progress");
        progress.last_heartbeat_unix_ms = Some(42);
        progress.last_detail = Some("old detail".to_string());
        progress.checkpoint_summary = Some("old checkpoint".to_string());
        progress.checkpoint_count = Some(3);
    }
    let (worker, _worker_rx) = swarm_member("worker", "agent", true);
    swarm_members
        .write()
        .await
        .insert("worker".to_string(), worker);

    let outcome = salvage_assignments_of_dead_member(
        "worker",
        "swarm-1",
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;

    assert!(outcome.requeued_task_ids.is_empty());
    assert_eq!(outcome.failed_task_ids, vec!["task-1".to_string()]);
    let plans = swarm_plans.read().await;
    let plan = plans.get("swarm-1").expect("plan");
    assert_eq!(plan.items[0].status, "failed");
    assert_eq!(plan.items[0].assigned_to, None);
    let progress = plan.task_progress.get("task-1").expect("progress");
    assert_eq!(progress.last_heartbeat_unix_ms, Some(42));
    assert_eq!(progress.last_detail.as_deref(), Some("old detail"));
    assert_eq!(progress.checkpoint_count, Some(3));
    let checkpoint_summary = progress.checkpoint_summary.as_deref().unwrap_or_default();
    assert!(checkpoint_summary.contains("old checkpoint"));
    assert!(checkpoint_summary.contains("automatic reclaim cap was reached"));
}

#[tokio::test]
async fn remove_session_from_swarm_salvages_running_assignments() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["coord".to_string(), "worker".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "coord".to_string(),
    )])));
    let swarm_plans = running_plan_assigned_to("worker", None);
    let (coord, _coord_rx) = swarm_member("coord", "coordinator", false);
    let (worker, _worker_rx) = swarm_member("worker", "agent", true);
    {
        let mut members = swarm_members.write().await;
        members.insert("coord".to_string(), coord);
        members.insert("worker".to_string(), worker);
    }

    remove_session_from_swarm(
        "worker",
        "swarm-1",
        &swarm_members,
        &swarms_by_id,
        &swarm_coordinators,
        &swarm_plans,
    )
    .await;

    let plans = swarm_plans.read().await;
    let plan = plans.get("swarm-1").expect("plan");
    assert_eq!(plan.items[0].status, "queued");
    assert_eq!(plan.items[0].assigned_to, None);
}

#[tokio::test]
async fn staleness_sweep_salvages_tasks_of_vanished_assignee() {
    // The assignee is not a swarm member at all (zombie left over from a
    // previous process): no grace period applies and the sweep must
    // requeue its running task.
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["coord".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        "coord".to_string(),
    )])));
    let swarm_plans = running_plan_assigned_to("ghost", None);
    // Give the task a fresh heartbeat so the first sweep phase does not
    // interfere; the salvage phase must still fire on the dead assignee.
    {
        let mut plans = swarm_plans.write().await;
        let plan = plans.get_mut("swarm-1").expect("plan");
        let progress = plan.task_progress.get_mut("task-1").expect("progress");
        progress.last_heartbeat_unix_ms = Some(now_unix_ms());
    }
    let (coord, _coord_rx) = swarm_member("coord", "coordinator", false);
    swarm_members
        .write()
        .await
        .insert("coord".to_string(), coord);

    refresh_swarm_task_staleness(
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;

    let plans = swarm_plans.read().await;
    let plan = plans.get("swarm-1").expect("plan");
    assert_eq!(plan.items[0].status, "queued");
    assert_eq!(plan.items[0].assigned_to, None);
}

#[tokio::test]
async fn staleness_sweep_grants_grace_to_recently_crashed_member() {
    // A member marked crashed moments ago may be mid reload-recovery; the
    // sweep must not reclaim its work inside the grace window.
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["worker".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::new()));
    let swarm_plans = running_plan_assigned_to("worker", None);
    {
        let mut plans = swarm_plans.write().await;
        let plan = plans.get_mut("swarm-1").expect("plan");
        let progress = plan.task_progress.get_mut("task-1").expect("progress");
        progress.last_heartbeat_unix_ms = Some(now_unix_ms());
    }
    let (mut worker, _worker_rx) = swarm_member("worker", "agent", true);
    worker.status = "crashed".to_string();
    worker.last_status_change = Instant::now();
    swarm_members
        .write()
        .await
        .insert("worker".to_string(), worker);

    refresh_swarm_task_staleness(
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;

    let plans = swarm_plans.read().await;
    let plan = plans.get("swarm-1").expect("plan");
    assert_eq!(plan.items[0].status, "running");
    assert_eq!(plan.items[0].assigned_to.as_deref(), Some("worker"));
}

#[tokio::test]
async fn update_member_status_notifies_owner_when_worker_crashes_mid_task() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-1".to_string(),
        HashSet::from(["owner".to_string(), "worker".to_string()]),
    )])));
    let (owner, mut owner_rx) = swarm_member("owner", "coordinator", false);
    let (mut worker, _worker_rx) = swarm_member("worker", "agent", true);
    worker.status = "running".to_string();
    worker.report_back_to_session_id = Some("owner".to_string());
    {
        let mut members = swarm_members.write().await;
        members.insert("owner".to_string(), owner);
        members.insert("worker".to_string(), worker);
    }

    update_member_status(
        "worker",
        "crashed",
        Some("client disconnected while processing".to_string()),
        &swarm_members,
        &swarms_by_id,
        None,
        None,
        None,
    )
    .await;

    let owner_events: Vec<_> = std::iter::from_fn(|| owner_rx.try_recv().ok()).collect();
    assert!(
        owner_events.iter().any(|event| matches!(
            event,
            ServerEvent::Notification { message, .. }
                if message.contains("crashed while working")
        )),
        "owner should be notified of the crash, got {owner_events:?}"
    );
}

/// W3 reaper (orchestration-hardening): a task that has been
/// `running_stale` beyond the reap deadline, whose assignee is no longer a
/// live swarm member, must be failed so retry/salvage paths (and blocked
/// awaits) can proceed. Today staleness marking is a dead end: the item
/// stays `running_stale` forever, holding awaits to their full timeout and
/// leaving children permanently blocked.
#[tokio::test]
async fn refresh_swarm_task_staleness_reaps_orphaned_tasks_past_deadline() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-reap".to_string(),
        HashSet::from(["coord".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::new()));
    let now_ms = now_unix_ms();
    let reaped_age_ms = super::swarm_task_reap_after().as_millis() as u64 + 5_000;
    // "ghost" owns the task but is NOT in swarm_members (crashed/evicted).
    // "coord" is a live member so the swarm itself is alive.
    let (coord, _coord_rx) = swarm_member("coord", "coordinator", false);
    swarm_members
        .write()
        .await
        .insert("coord".to_string(), coord);
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        "swarm-reap".to_string(),
        VersionedPlan {
            items: vec![PlanItem {
                content: "orphaned task".to_string(),
                status: "running_stale".to_string(),
                priority: "medium".to_string(),
                id: "task-orphan".to_string(),
                subsystem: None,
                file_scope: Vec::new(),
                blocked_by: Vec::new(),
                assigned_to: Some("ghost".to_string()),
            }],
            version: 1,
            participants: HashSet::from(["coord".to_string()]),
            task_progress: HashMap::from([(
                "task-orphan".to_string(),
                crate::server::SwarmTaskProgress {
                    assigned_session_id: Some("ghost".to_string()),
                    started_at_unix_ms: Some(now_ms.saturating_sub(reaped_age_ms)),
                    last_heartbeat_unix_ms: Some(now_ms.saturating_sub(reaped_age_ms)),
                    stale_since_unix_ms: Some(now_ms.saturating_sub(reaped_age_ms)),
                    ..Default::default()
                },
            )]),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));

    refresh_swarm_task_staleness(
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;

    let plans = swarm_plans.read().await;
    let plan = plans.get("swarm-reap").expect("plan");
    assert_eq!(
        plan.items[0].status, "failed",
        "orphaned running_stale task past the reap deadline must be failed \
         so retry/salvage can proceed (was left '{}')",
        plan.items[0].status
    );
}

/// Counterpart guard: a stale task whose assignee is still a live member is
/// NOT reaped (heartbeats may revive it; killing a live worker's task is
/// the salvage path's decision, not the sweeper's).
#[tokio::test]
async fn refresh_swarm_task_staleness_leaves_stale_tasks_of_live_members() {
    let swarm_members = Arc::new(RwLock::new(HashMap::new()));
    let swarms_by_id = Arc::new(RwLock::new(HashMap::from([(
        "swarm-noreap".to_string(),
        HashSet::from(["worker".to_string()]),
    )])));
    let swarm_coordinators = Arc::new(RwLock::new(HashMap::new()));
    let now_ms = now_unix_ms();
    let reaped_age_ms = super::swarm_task_reap_after().as_millis() as u64 + 5_000;
    let (worker, _worker_rx) = swarm_member("worker", "agent", true);
    swarm_members
        .write()
        .await
        .insert("worker".to_string(), worker);
    let swarm_plans = Arc::new(RwLock::new(HashMap::from([(
        "swarm-noreap".to_string(),
        VersionedPlan {
            items: vec![PlanItem {
                content: "slow task".to_string(),
                status: "running_stale".to_string(),
                priority: "medium".to_string(),
                id: "task-slow".to_string(),
                subsystem: None,
                file_scope: Vec::new(),
                blocked_by: Vec::new(),
                assigned_to: Some("worker".to_string()),
            }],
            version: 1,
            participants: HashSet::from(["worker".to_string()]),
            task_progress: HashMap::from([(
                "task-slow".to_string(),
                crate::server::SwarmTaskProgress {
                    assigned_session_id: Some("worker".to_string()),
                    started_at_unix_ms: Some(now_ms.saturating_sub(reaped_age_ms)),
                    last_heartbeat_unix_ms: Some(now_ms.saturating_sub(reaped_age_ms)),
                    stale_since_unix_ms: Some(now_ms.saturating_sub(reaped_age_ms)),
                    ..Default::default()
                },
            )]),
            mode: "light".to_string(),
            node_meta: HashMap::new(),
        },
    )])));

    refresh_swarm_task_staleness(
        &swarm_members,
        &swarms_by_id,
        &swarm_plans,
        &swarm_coordinators,
    )
    .await;

    let plans = swarm_plans.read().await;
    let plan = plans.get("swarm-noreap").expect("plan");
    assert_eq!(
        plan.items[0].status, "running_stale",
        "stale task of a LIVE member must not be reaped by the sweeper"
    );
}
