use super::*;

pub(in crate::server) fn status_age_secs(last_status_change: Instant) -> u64 {
    last_status_change.elapsed().as_secs()
}

/// Maximum number of live members (agents) in a single swarm. Re-exported from
/// `jcode_swarm_core` so the server, tools, and prompts all agree on the one
/// runaway-prevention cap for the task-graph model. There is intentionally no
/// spawn-depth limit and no per-node fan-out limit: the spawn tree may nest and
/// fan out freely until the swarm reaches this many live members, at which point
/// further spawns are refused.
pub(in crate::server) use jcode_swarm_core::MAX_SWARM_MEMBERS;

/// Walk the `report_back_to_session_id` chain upward from `session_id`,
/// returning the list of ancestor session ids (parent first, root last).
///
/// The spawner/parent edge is encoded by `report_back_to_session_id`: a child
/// spawned by `P` reports back to `P`. Walking that chain reconstructs the spawn
/// tree without persisting a separate parent field. Cycles (which should never
/// happen) are guarded against with a visited set.
pub(in crate::server) fn swarm_ancestors(
    members: &HashMap<String, SwarmMember>,
    session_id: &str,
) -> Vec<String> {
    let mut ancestors = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(session_id.to_string());
    let mut current = session_id.to_string();
    while let Some(parent) = members
        .get(&current)
        .and_then(|member| member.report_back_to_session_id.clone())
    {
        if parent == current || !visited.insert(parent.clone()) {
            break;
        }
        ancestors.push(parent.clone());
        current = parent;
    }
    ancestors
}

/// Depth of `session_id` in the spawn tree: number of ancestors reachable via
/// the report-back chain. Root coordinators (no report-back owner) are depth 0.
///
/// Test-only: the spawn tree no longer enforces a depth cap, so production code
/// does not consult depth. Kept (behind `cfg(test)`) because the spawn-tree tests
/// assert ancestor-chain depth directly.
#[cfg(test)]
pub(in crate::server) fn swarm_spawn_depth(
    members: &HashMap<String, SwarmMember>,
    session_id: &str,
) -> u32 {
    swarm_ancestors(members, session_id).len() as u32
}

/// Outcome of resolving a user-supplied target (session ID or friendly name)
/// against a swarm's members. Shared by the DM path and the assignment path so
/// friendly names behave identically everywhere (F5 in the orchestration
/// hardening audit: the assign path used to accept session IDs only).
pub(in crate::server) enum SwarmTargetResolution {
    /// Unique resolution to a session ID.
    Session(String),
    /// The target matched no session ID and no friendly name.
    Unknown,
    /// The friendly name matched more than one member; contains
    /// `(session_id, friendly_name)` pairs for the error message.
    Ambiguous(Vec<(String, String)>),
}

/// Resolve `target` as an exact session ID first, then as a friendly name,
/// considering only members whose session ID is in `candidate_session_ids`.
/// Pure lookup: callers decide how to phrase Unknown/Ambiguous errors.
pub(in crate::server) fn resolve_swarm_target(
    target: &str,
    candidate_session_ids: &[String],
    members: &HashMap<String, SwarmMember>,
) -> SwarmTargetResolution {
    if candidate_session_ids
        .iter()
        .any(|session_id| session_id == target)
    {
        return SwarmTargetResolution::Session(target.to_string());
    }

    let mut matches: Vec<(String, String)> = candidate_session_ids
        .iter()
        .filter_map(|session_id| {
            let member = members.get(session_id)?;
            member
                .friendly_name
                .as_deref()
                .filter(|friendly_name| *friendly_name == target)
                .map(|friendly_name| (session_id.clone(), friendly_name.to_string()))
        })
        .collect();
    matches.sort_by(|(left_session, _), (right_session, _)| left_session.cmp(right_session));
    matches.dedup_by(|(left_session, _), (right_session, _)| left_session == right_session);
    match matches.len() {
        0 => SwarmTargetResolution::Unknown,
        1 => SwarmTargetResolution::Session(matches.remove(0).0),
        _ => SwarmTargetResolution::Ambiguous(matches),
    }
}

/// Format the standard "ambiguous friendly name" error detail from
/// [`SwarmTargetResolution::Ambiguous`] matches.
pub(in crate::server) fn format_ambiguous_target_matches(matches: &[(String, String)]) -> String {
    matches
        .iter()
        .map(|(session_id, friendly_name)| format!("{} [{}]", friendly_name, session_id))
        .collect::<Vec<_>>()
        .join(", ")
}

/// True when `ancestor` is `session_id` itself or any transitive spawner of it.
/// Used to decide whether a requester may manage (stop/control) a target: an
/// agent owns its entire spawned subtree.
pub(in crate::server) fn swarm_is_self_or_ancestor(
    members: &HashMap<String, SwarmMember>,
    ancestor: &str,
    session_id: &str,
) -> bool {
    ancestor == session_id
        || swarm_ancestors(members, session_id)
            .iter()
            .any(|candidate| candidate == ancestor)
}

const DEFAULT_SWARM_STATUS_DEBOUNCE_MEMBER_THRESHOLD: usize = 2;
const DEFAULT_SWARM_STATUS_DEBOUNCE_MS: u64 = 75;
const DEFAULT_SWARM_TASK_HEARTBEAT_SECS: u64 = 10;
const DEFAULT_SWARM_TASK_STALE_AFTER_SECS: u64 = 45;
const DEFAULT_SWARM_TASK_SWEEP_INTERVAL_SECS: u64 = 5;
const DEFAULT_SWARM_TASK_REAP_AFTER_SECS: u64 = 180;
const DEFAULT_SWARM_DEAD_PID_SWEEP_INTERVAL_SECS: u64 = 5;
const DEFAULT_SWARM_TERMINAL_MEMBER_RETENTION_SECS: u64 = 24 * 60 * 60;
const DEFAULT_SWARM_TERMINAL_MEMBER_GC_INTERVAL_SECS: u64 = 60;
#[derive(Default, Clone, Copy)]
pub(in crate::server) struct PendingSwarmStatusBroadcast {
    pub(in crate::server) scheduled: bool,
    pub(in crate::server) dirty: bool,
}

pub(in crate::server) fn pending_swarm_status_broadcasts()
-> &'static StdMutex<HashMap<String, PendingSwarmStatusBroadcast>> {
    static PENDING: OnceLock<StdMutex<HashMap<String, PendingSwarmStatusBroadcast>>> =
        OnceLock::new();
    PENDING.get_or_init(|| StdMutex::new(HashMap::new()))
}

pub(in crate::server) fn swarm_status_debounce_member_threshold() -> usize {
    static CACHED: OnceLock<AtomicUsize> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            let configured = std::env::var("JCODE_SWARM_STATUS_DEBOUNCE_MEMBER_THRESHOLD")
                .ok()
                .and_then(|value| value.trim().parse::<usize>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_SWARM_STATUS_DEBOUNCE_MEMBER_THRESHOLD);
            AtomicUsize::new(configured)
        })
        .load(Ordering::Relaxed)
}

pub(in crate::server) fn swarm_status_debounce_ms() -> u64 {
    static CACHED: OnceLock<AtomicU64> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            let configured = std::env::var("JCODE_SWARM_STATUS_DEBOUNCE_MS")
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_SWARM_STATUS_DEBOUNCE_MS);
            AtomicU64::new(configured)
        })
        .load(Ordering::Relaxed)
}

fn configured_positive_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

pub(in crate::server) fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(in crate::server) fn log_swarm_lifecycle(phase: &str, fields: Vec<(&str, String)>) {
    crate::logging::event_info(
        "SWARM_LIFECYCLE",
        Vec::from([("phase", phase.to_string())])
            .into_iter()
            .chain(fields)
            .collect::<Vec<_>>(),
    );
}

pub(in crate::server) fn swarm_task_heartbeat_interval() -> Duration {
    Duration::from_secs(configured_positive_u64(
        "JCODE_SWARM_TASK_HEARTBEAT_SECS",
        DEFAULT_SWARM_TASK_HEARTBEAT_SECS,
    ))
}

pub(in crate::server) fn swarm_task_stale_after() -> Duration {
    Duration::from_secs(configured_positive_u64(
        "JCODE_SWARM_TASK_STALE_AFTER_SECS",
        DEFAULT_SWARM_TASK_STALE_AFTER_SECS,
    ))
}

pub(in crate::server) fn swarm_task_sweep_interval() -> Duration {
    Duration::from_secs(configured_positive_u64(
        "JCODE_SWARM_TASK_SWEEP_INTERVAL_SECS",
        DEFAULT_SWARM_TASK_SWEEP_INTERVAL_SECS,
    ))
}

pub(in crate::server) fn swarm_dead_pid_sweep_interval() -> Duration {
    Duration::from_secs(configured_positive_u64(
        "JCODE_SWARM_DEAD_PID_SWEEP_INTERVAL_SECS",
        DEFAULT_SWARM_DEAD_PID_SWEEP_INTERVAL_SECS,
    ))
}

fn last_dead_pid_sweep_ms() -> &'static AtomicU64 {
    static LAST_SWEEP_MS: OnceLock<AtomicU64> = OnceLock::new();
    LAST_SWEEP_MS.get_or_init(|| AtomicU64::new(0))
}

pub(in crate::server) fn claim_dead_pid_sweep(now_ms: u64, interval: Duration) -> bool {
    let interval_ms = interval.as_millis() as u64;
    let last = last_dead_pid_sweep_ms().load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) < interval_ms {
        return false;
    }
    last_dead_pid_sweep_ms()
        .compare_exchange(last, now_ms, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
}

/// Reconcile persisted Active sessions with dead owner PIDs and mirror lost
/// sessions into swarm member state. This is intentionally cheap and opportunistic:
/// it runs at most once per interval from daemon-side swarm status traffic, so
/// dead visible workers stop looking alive even when nobody opens the picker.
pub(in crate::server) async fn sweep_dead_pid_swarm_members(
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    _swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
) -> Vec<String> {
    // The sweep resolves JCODE_HOME and mutates its persisted session/marker
    // state. In tests, retain a transferable environment lease across every
    // await so opportunistic status broadcasts cannot enter another test's
    // temporary home while that test is constructing its fixture. A foreign
    // writer-child may itself be calling this best-effort sweep, so defer
    // rather than blocking against its exclusion and deadlocking the task.
    #[cfg(test)]
    let Some(_test_env_lease) = crate::storage::try_lock_test_env_fixture() else {
        return Vec::new();
    };
    let _ = crate::session::reconcile_active_sessions();
    // Only members not already in a terminal state can newly transition to
    // lost, so skip the rest BEFORE touching disk. This keeps the per-sweep
    // `Session::load` count proportional to live members instead of O(all members)
    // — dead members otherwise accumulate and get re-loaded from disk every tick.
    let session_ids: Vec<String> = {
        let members = swarm_members.read().await;
        members
            .iter()
            .filter(|(_, member)| !member.lifecycle().is_terminal())
            .map(|(session_id, _)| session_id.clone())
            .collect()
    };

    let lost_sessions: HashSet<String> = session_ids
        .into_iter()
        .filter(|session_id| {
            crate::session::Session::load(session_id).is_ok_and(|session| {
                matches!(
                    session.status,
                    crate::session::SessionStatus::Crashed { .. }
                )
            })
        })
        .collect();
    if lost_sessions.is_empty() {
        return Vec::new();
    }

    let mut changed_swarms = HashSet::new();
    let mut persisted = Vec::new();
    {
        let mut members = swarm_members.write().await;
        for session_id in &lost_sessions {
            let Some(member) = members.get_mut(session_id) else {
                continue;
            };
            if member.lifecycle().is_terminal() {
                continue;
            }
            let detail = "client process exited".to_string();
            if !member.apply_lifecycle_event(
                jcode_swarm_core::MemberLifecycleEvent::ProcessLost {
                    reason: Some(detail.clone()),
                },
                now_unix_ms(),
            ) {
                continue;
            }
            member.detail = Some(detail);
            member.last_status_change = Instant::now();
            persisted.push((session_id.clone(), member.lifecycle()));
            if let Some(swarm_id) = member.swarm_id.clone() {
                changed_swarms.insert(swarm_id);
            }
        }
    }

    for (session_id, lifecycle) in persisted {
        let _ = crate::session::Session::persist_swarm_lifecycle(
            &session_id,
            crate::session::StoredSwarmLifecycleStatus {
                state: lifecycle.state.as_str().to_string(),
                assignment_epoch: lifecycle.assignment_epoch,
                revision: lifecycle.revision,
                reason: lifecycle.reason,
                updated_at_unix_ms: lifecycle.updated_at_unix_ms,
            },
        );
    }

    changed_swarms.into_iter().collect()
}

/// How long terminal members remain visible in the active swarm listing. This
/// keeps completion reports available for inspection without allowing durable
/// history to grow forever.
pub(in crate::server) fn swarm_terminal_member_retention() -> Duration {
    Duration::from_secs(configured_positive_u64(
        "JCODE_SWARM_TERMINAL_MEMBER_RETENTION_SECS",
        DEFAULT_SWARM_TERMINAL_MEMBER_RETENTION_SECS,
    ))
}

/// How often the live server removes terminal members whose retention window
/// has elapsed. Startup loading performs the same pruning synchronously.
pub(in crate::server) fn swarm_terminal_member_gc_interval() -> Duration {
    Duration::from_secs(configured_positive_u64(
        "JCODE_SWARM_TERMINAL_MEMBER_GC_INTERVAL_SECS",
        DEFAULT_SWARM_TERMINAL_MEMBER_GC_INTERVAL_SECS,
    ))
}

/// Terminal members are historical records, not live agents. They remain
/// visible temporarily for reports and diagnostics but must not consume the
/// runaway-prevention spawn budget.
#[deprecated(note = "use SwarmLifecycleStatus::is_terminal_state")]
#[allow(
    dead_code,
    reason = "temporary compatibility bridge during W23 lifecycle migration"
)]
pub(in crate::server) fn member_status_is_terminal(status: &str) -> bool {
    jcode_swarm_core::MemberLifecycleState::from_compatibility_status(status).is_terminal()
}

pub(in crate::server) fn member_consumes_swarm_capacity(member: &SwarmMember) -> bool {
    !member.lifecycle().is_terminal()
}

pub(in crate::server) fn expired_terminal_member_ids(
    members: &HashMap<String, SwarmMember>,
    retention: Duration,
) -> Vec<String> {
    members
        .values()
        .filter(|member| member.lifecycle().is_terminal())
        .filter(|member| member.last_status_change.elapsed() >= retention)
        .map(|member| member.session_id.clone())
        .collect()
}

/// Lifecycle statuses that mean a member can no longer drive an assignment:
/// the session's agent loop is gone, so no heartbeat or turn end will ever
/// arrive for tasks it holds.
#[deprecated(note = "use SwarmLifecycleStatus::is_dead_state")]
#[allow(dead_code, reason = "parity bridge kept for the W23 compatibility tests")]
pub(in crate::server) fn member_status_is_dead(status: &str) -> bool {
    matches!(
        jcode_swarm_core::MemberLifecycleState::from_compatibility_status(status),
        jcode_swarm_core::MemberLifecycleState::Failed
            | jcode_swarm_core::MemberLifecycleState::Stopped
            | jcode_swarm_core::MemberLifecycleState::Lost
    )
}

/// Outcome of salvaging one dead member's plan assignments.
#[derive(Debug, Default, PartialEq, Eq)]
pub(in crate::server) struct DeadMemberSalvage {
    /// Tasks released back to `queued` for automatic re-dispatch.
    pub requeued_task_ids: Vec<String>,
    /// Tasks marked `failed` because the automatic reclaim cap was reached.
    pub failed_task_ids: Vec<String>,
}

impl DeadMemberSalvage {
    pub(in crate::server) fn is_empty(&self) -> bool {
        self.requeued_task_ids.is_empty() && self.failed_task_ids.is_empty()
    }

    /// Human-readable notification body for the coordinator/owner.
    fn describe(&self, worker_label: &str) -> String {
        let mut parts = vec![format!(
            "⚠ Worker {} died while holding swarm task assignment(s).",
            worker_label
        )];
        if !self.requeued_task_ids.is_empty() {
            parts.push(format!(
                "Requeued for automatic re-dispatch: {}.",
                self.requeued_task_ids.join(", ")
            ));
        }
        if !self.failed_task_ids.is_empty() {
            parts.push(format!(
                "Marked failed (automatic reclaim cap reached): {}. Use retry or assign_task to redispatch explicitly.",
                self.failed_task_ids.join(", ")
            ));
        }
        parts.push(
            "Queued tasks will be picked up by assign_next/run_plan; check plan_status for details."
                .to_string(),
        );
        parts.join(" ")
    }
}

/// Requeue (or, past [`crate::plan::MAX_DEAD_ASSIGNEE_RECLAIMS`], fail) every
/// non-terminal plan item assigned to `session_id`.
///
/// This is the eager counterpart to the assign-time stranded-task reclaim: a
/// worker that crashes, stops, or leaves the swarm mid-task leaves its items
/// `running`/`queued` and assigned to a corpse, where the scheduler cannot see
/// them and a driving `run_plan` stalls into its transient-stall error.
/// Salvaging at the moment the member dies converts that silent strand into
/// normal queued work. Uses the same per-node reclaim counter and cap as the
/// assign-time path so repeatedly lethal nodes fail loudly instead of cycling
/// workers forever.
fn salvage_plan_assignments_of(plan: &mut VersionedPlan, session_id: &str) -> DeadMemberSalvage {
    let now_ms = now_unix_ms();
    let mut outcome = DeadMemberSalvage::default();
    let assigned_ids: Vec<String> = plan
        .items
        .iter()
        .filter(|item| {
            item.assigned_to.as_deref() == Some(session_id)
                && !crate::plan::is_terminal_status(&item.status)
        })
        .map(|item| item.id.clone())
        .collect();
    for task_id in assigned_ids {
        let reclaims = plan
            .task_progress
            .get(&task_id)
            .and_then(|progress| progress.dead_assignee_reclaims)
            .unwrap_or(0);
        if reclaims >= crate::plan::MAX_DEAD_ASSIGNEE_RECLAIMS {
            if let Some(item) = plan.items.iter_mut().find(|item| item.id == task_id) {
                item.status = "failed".to_string();
                item.assigned_to = None;
            }
            let progress = plan.task_progress.entry(task_id.clone()).or_default();
            progress.assigned_session_id = None;
            progress.completed_at_unix_ms = Some(now_ms);
            progress.stale_since_unix_ms = None;
            jcode_plan::append_progress_provenance(
                progress,
                format!(
                    "failed: assigned worker {} died and the automatic reclaim cap was reached",
                    session_id
                ),
            );
            plan.version += 1;
            outcome.failed_task_ids.push(task_id);
        } else if crate::plan::reclaim_stranded_assignment(plan, &task_id) {
            if let Some(item) = plan.items.iter_mut().find(|item| item.id == task_id) {
                item.status = "queued".to_string();
            }
            let progress = plan.task_progress.entry(task_id.clone()).or_default();
            progress.stale_since_unix_ms = None;
            outcome.requeued_task_ids.push(task_id);
        }
    }
    outcome
}

/// Salvage `session_id`'s plan assignments in `swarm_id`, then persist,
/// broadcast the plan change, and notify the swarm coordinator so the death is
/// visible instead of silent. No-ops (and skips all I/O) when the member held
/// no non-terminal assignments.
pub(in crate::server) async fn salvage_assignments_of_dead_member(
    session_id: &str,
    swarm_id: &str,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
) -> DeadMemberSalvage {
    let outcome = {
        let mut plans = swarm_plans.write().await;
        match plans.get_mut(swarm_id) {
            Some(plan) => salvage_plan_assignments_of(plan, session_id),
            None => DeadMemberSalvage::default(),
        }
    };
    if outcome.is_empty() {
        return outcome;
    }

    log_swarm_lifecycle(
        "dead_member_tasks_salvaged",
        vec![
            ("session_id", session_id.to_string()),
            ("swarm_id", swarm_id.to_string()),
            ("requeued_task_ids", outcome.requeued_task_ids.join(",")),
            ("failed_task_ids", outcome.failed_task_ids.join(",")),
        ],
    );

    let swarm_state = SwarmState {
        members: Arc::clone(swarm_members),
        swarms_by_id: Arc::clone(swarms_by_id),
        plans: Arc::clone(swarm_plans),
        coordinators: Arc::clone(swarm_coordinators),
    };
    persist_swarm_state_for(swarm_id, &swarm_state).await;
    broadcast_swarm_plan(
        swarm_id,
        Some("task_salvaged_dead_worker".to_string()),
        swarm_plans,
        swarm_members,
        swarms_by_id,
    )
    .await;
    notify_coordinator_of_salvage(
        session_id,
        swarm_id,
        &outcome,
        swarm_members,
        swarm_coordinators,
    )
    .await;
    outcome
}

/// Deliver a salvage notification to the swarm's current coordinator (when it
/// is not the dead session itself).
async fn notify_coordinator_of_salvage(
    session_id: &str,
    swarm_id: &str,
    outcome: &DeadMemberSalvage,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
) {
    let coordinator_id = {
        let coordinators = swarm_coordinators.read().await;
        coordinators.get(swarm_id).cloned()
    };
    let Some(coordinator_id) = coordinator_id.filter(|id| id != session_id) else {
        return;
    };
    let label = {
        let members = swarm_members.read().await;
        members
            .get(session_id)
            .and_then(|member| member.friendly_name.clone())
    }
    .unwrap_or_else(|| session_id[..8.min(session_id.len())].to_string());
    let _ = fanout_session_event(
        swarm_members,
        &coordinator_id,
        ServerEvent::Notification {
            from_session: session_id.to_string(),
            from_name: Some(label.clone()),
            notification_type: NotificationType::Message {
                scope: Some("swarm".to_string()),
                tldr: None,
            },
            message: outcome.describe(&label),
        },
    )
    .await;
}

/// How long a task may sit `running_stale` before the sweeper reaps it
/// (fails it) when its assignee is no longer a live swarm member. Generous
/// relative to `swarm_task_stale_after` so a slow-but-alive worker whose
/// member record briefly disappears (reload) is not raced.
pub(in crate::server) fn swarm_task_reap_after() -> Duration {
    Duration::from_secs(configured_positive_u64(
        "JCODE_SWARM_TASK_REAP_AFTER_SECS",
        DEFAULT_SWARM_TASK_REAP_AFTER_SECS,
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "task progress touch updates durable progress plus swarm persistence and coordinator-facing state in one helper"
)]
pub(in crate::server) async fn touch_swarm_task_progress(
    swarm_id: &str,
    task_id: &str,
    assigned_session_id: Option<&str>,
    detail: Option<String>,
    checkpoint_summary: Option<String>,
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
) -> bool {
    let now_ms = now_unix_ms();
    let revived = {
        let mut plans = swarm_plans.write().await;
        let Some(plan) = plans.get_mut(swarm_id) else {
            return false;
        };
        let Some(item) = plan.items.iter_mut().find(|item| item.id == task_id) else {
            return false;
        };
        let progress = plan.task_progress.entry(task_id.to_string()).or_default();
        if let Some(session_id) = assigned_session_id {
            progress.assigned_session_id = Some(session_id.to_string());
        }
        // Heartbeats/checkpoints are proof of life for the assigned session:
        // fold them into the member activity clock so swarm status reflects
        // busy workers whose lifecycle status has not changed in a while.
        if let Some(session_id) = progress.assigned_session_id.as_deref() {
            crate::session_metrics::record_activity(session_id);
        }
        progress.last_heartbeat_unix_ms = Some(now_ms);
        progress.heartbeat_count = Some(progress.heartbeat_count.unwrap_or(0) + 1);
        if let Some(detail) = detail {
            progress.last_detail = Some(truncate_detail(&detail, 120));
        }
        if let Some(summary) = checkpoint_summary {
            progress.last_checkpoint_unix_ms = Some(now_ms);
            progress.checkpoint_summary = Some(truncate_detail(&summary, 120));
            progress.checkpoint_count = Some(progress.checkpoint_count.unwrap_or(0) + 1);
        }
        if item.status == "running_stale" {
            item.status = "running".to_string();
            progress.stale_since_unix_ms = None;
            plan.version += 1;
            true
        } else {
            false
        }
    };
    let swarm_state = SwarmState {
        members: Arc::clone(swarm_members),
        swarms_by_id: Arc::clone(swarms_by_id),
        plans: Arc::clone(swarm_plans),
        coordinators: Arc::clone(swarm_coordinators),
    };
    persist_swarm_state_for(swarm_id, &swarm_state).await;
    revived
}

pub(in crate::server) async fn refresh_swarm_task_staleness(
    swarm_members: &Arc<RwLock<HashMap<String, SwarmMember>>>,
    swarms_by_id: &Arc<RwLock<HashMap<String, HashSet<String>>>>,
    swarm_plans: &Arc<RwLock<HashMap<String, VersionedPlan>>>,
    swarm_coordinators: &Arc<RwLock<HashMap<String, String>>>,
) {
    let now_ms = now_unix_ms();
    let stale_after_ms = swarm_task_stale_after().as_millis() as u64;
    let reap_after_ms = swarm_task_reap_after().as_millis() as u64;
    let changed_swarm_ids = {
        // Snapshot live members first (separate lock) so the reap check below
        // does not read members while holding the plans write lock.
        let live_sessions: HashSet<String> = {
            let members = swarm_members.read().await;
            members.keys().cloned().collect()
        };
        let mut plans = swarm_plans.write().await;
        let mut changed = Vec::new();
        for (swarm_id, plan) in plans.iter_mut() {
            let mut swarm_changed = false;
            for item in &mut plan.items {
                if !matches!(item.status.as_str(), "running" | "running_stale") {
                    continue;
                }
                let progress = plan.task_progress.entry(item.id.clone()).or_default();
                let last_heartbeat = progress
                    .last_heartbeat_unix_ms
                    .or(progress.started_at_unix_ms)
                    .or(progress.assigned_at_unix_ms);
                let is_stale = last_heartbeat
                    .map(|ts| now_ms.saturating_sub(ts) >= stale_after_ms)
                    .unwrap_or(true);
                match (item.status.as_str(), is_stale) {
                    ("running", true) => {
                        item.status = "running_stale".to_string();
                        progress.stale_since_unix_ms.get_or_insert(now_ms);
                        plan.version += 1;
                        swarm_changed = true;
                    }
                    ("running_stale", false) => {
                        item.status = "running".to_string();
                        progress.stale_since_unix_ms = None;
                        plan.version += 1;
                        swarm_changed = true;
                    }
                    ("running_stale", true) => {
                        // W3 reaper: staleness must not be a dead end. When the
                        // assignee is no longer a live member AND the task has
                        // been stale past the reap deadline, fail it so
                        // retry/salvage (requeue_failed, task_control retry)
                        // and blocked awaits can proceed. Live members are
                        // never raced: their slow tasks stay running_stale and
                        // may still be revived by a heartbeat.
                        let assignee_departed = item
                            .assigned_to
                            .as_ref()
                            .is_some_and(|assignee| !live_sessions.contains(assignee));
                        let past_reap_deadline = progress
                            .stale_since_unix_ms
                            .map(|since| now_ms.saturating_sub(since) >= reap_after_ms)
                            .unwrap_or(false);
                        if assignee_departed && past_reap_deadline {
                            crate::logging::event_warn(
                                "SWARM_LIFECYCLE",
                                vec![
                                    ("phase", "reap_orphaned_task".to_string()),
                                    ("swarm_id", swarm_id.clone()),
                                    ("task_id", item.id.clone()),
                                    (
                                        "departed_assignee",
                                        item.assigned_to.clone().unwrap_or_default(),
                                    ),
                                ],
                            );
                            item.status = "failed".to_string();
                            progress.completed_at_unix_ms = Some(now_ms);
                            plan.version += 1;
                            swarm_changed = true;
                        }
                    }
                    _ => {}
                }
            }
            if swarm_changed {
                changed.push(swarm_id.clone());
            }
        }
        changed
    };

    for swarm_id in changed_swarm_ids {
        let swarm_state = SwarmState {
            members: Arc::clone(swarm_members),
            swarms_by_id: Arc::clone(swarms_by_id),
            plans: Arc::clone(swarm_plans),
            coordinators: Arc::clone(swarm_coordinators),
        };
        persist_swarm_state_for(&swarm_id, &swarm_state).await;
        broadcast_swarm_plan(
            &swarm_id,
            Some("task_staleness_changed".to_string()),
            swarm_plans,
            swarm_members,
            swarms_by_id,
        )
        .await;
    }

    // Second phase: salvage in-flight items whose assignee is dead. Staleness
    // marking above only reflects missing heartbeats; when the assigned member
    // is gone from the swarm or sits in a terminal lifecycle status, no
    // heartbeat or turn-end will ever arrive, so the item must be requeued
    // (or failed at the reclaim cap) instead of pulsing running_stale forever.
    // A terminal-status member gets a grace period before salvage: reload
    // recovery briefly marks resumable members `crashed` before restoring
    // them, and salvaging inside that window would double-assign their work.
    let salvage_grace = swarm_task_stale_after();
    let salvage_candidates: Vec<(String, String)> = {
        let plans = swarm_plans.read().await;
        let members = swarm_members.read().await;
        let mut pairs = std::collections::BTreeSet::new();
        for (swarm_id, plan) in plans.iter() {
            for item in &plan.items {
                if !matches!(item.status.as_str(), "running" | "running_stale" | "queued") {
                    continue;
                }
                let assignee = item.assigned_to.as_deref().or_else(|| {
                    plan.task_progress
                        .get(&item.id)
                        .and_then(|progress| progress.assigned_session_id.as_deref())
                });
                let Some(assignee) = assignee else {
                    continue;
                };
                let assignee_is_dead = match members.get(assignee) {
                    None => true,
                    Some(member) => {
                        member.lifecycle.is_dead_state()
                            && member.last_status_change.elapsed() >= salvage_grace
                    }
                };
                if assignee_is_dead {
                    pairs.insert((swarm_id.clone(), assignee.to_string()));
                }
            }
        }
        pairs.into_iter().collect()
    };
    for (swarm_id, session_id) in salvage_candidates {
        salvage_assignments_of_dead_member(
            &session_id,
            &swarm_id,
            swarm_members,
            swarms_by_id,
            swarm_plans,
            swarm_coordinators,
        )
        .await;
    }
}

#[cfg(test)]
mod lifecycle_consistency_tests {
    use jcode_swarm_core::MemberLifecycleState;

    /// Every surface that accepts legacy status strings funnels them through
    /// `from_compatibility_status` (await targets, persistence recovery, the
    /// member registry), so this mapping is the single vocabulary authority.
    #[test]
    fn compatibility_inputs_collapse_to_canonical_states() {
        let cases = [
            ("ready", MemberLifecycleState::Ready),
            ("completed", MemberLifecycleState::Succeeded),
            ("done", MemberLifecycleState::Succeeded),
            ("succeeded", MemberLifecycleState::Succeeded),
            ("error", MemberLifecycleState::Failed),
            ("failed", MemberLifecycleState::Failed),
            ("cancelled", MemberLifecycleState::Stopped),
            ("stopped", MemberLifecycleState::Stopped),
            ("crashed", MemberLifecycleState::Lost),
            ("lost", MemberLifecycleState::Lost),
        ];

        for (input, expected) in cases {
            assert_eq!(
                MemberLifecycleState::from_compatibility_status(input),
                expected,
                "compatibility input {input} must parse to {expected:?}"
            );
        }
    }

    /// "ready" is a live state (a worker awaiting more work), never a
    /// completion, while every terminal alias classifies as terminal.
    #[test]
    fn terminal_classification_follows_canonical_states() {
        assert!(!MemberLifecycleState::from_compatibility_status("ready").is_terminal());
        for input in ["completed", "done", "failed", "stopped", "crashed"] {
            assert!(
                MemberLifecycleState::from_compatibility_status(input).is_terminal(),
                "compatibility input {input} must classify as terminal"
            );
        }
    }
}
