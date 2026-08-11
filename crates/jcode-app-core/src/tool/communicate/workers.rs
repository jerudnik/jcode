use super::run_plan_errors::run_plan_progress_snapshot;
use super::*;

pub(super) async fn cleanup_swarm_workers(
    ctx: &ToolContext,
    params: &CommunicateInput,
) -> Result<String> {
    let members = fetch_swarm_members(&ctx.session_id).await?;
    let target_status = params
        .target_status
        .clone()
        .unwrap_or_else(default_cleanup_target_statuses);
    let session_ids = params.session_ids.clone().unwrap_or_default();
    let force = params.force.unwrap_or(false);
    let candidates = cleanup_candidate_session_ids(
        &ctx.session_id,
        &members,
        &target_status,
        &session_ids,
        force,
    );

    if candidates.is_empty() {
        return Ok(format!(
            "No cleanup candidates found. Default cleanup only stops sessions spawned by this coordinator with status in [{}].",
            target_status.join(", ")
        ));
    }

    Ok(stop_swarm_sessions(ctx, candidates, force).await.describe())
}

/// Result of stopping a batch of swarm sessions: which stops succeeded and
/// which failed (with reasons). Split from the human-readable formatting so
/// callers like the mid-run capacity recovery can count freed slots.
pub(super) struct WorkerCleanupOutcome {
    stopped: Vec<String>,
    failed: Vec<String>,
}

impl WorkerCleanupOutcome {
    pub(super) fn describe(&self) -> String {
        let mut output = String::new();
        if self.stopped.is_empty() {
            output.push_str("Stopped no swarm workers.");
        } else {
            output.push_str(&format!(
                "Stopped {} swarm worker(s): {}",
                self.stopped.len(),
                self.stopped.join(", ")
            ));
        }
        if !self.failed.is_empty() {
            output.push_str(&format!(
                "\nFailed to stop {} worker(s): {}",
                self.failed.len(),
                self.failed.join(", ")
            ));
        }
        output
    }
}

pub(super) async fn stop_swarm_sessions(
    ctx: &ToolContext,
    candidates: Vec<String>,
    force: bool,
) -> WorkerCleanupOutcome {
    let mut stopped = Vec::new();
    let mut failed = Vec::new();
    for target in candidates {
        let request = Request::CommStop {
            id: REQUEST_ID,
            session_id: ctx.session_id.clone(),
            target_session: target.clone(),
            force: Some(force),
            cross_swarm: false,
        };
        match send_request(request).await {
            Ok(response) => match ensure_success(&response) {
                Ok(()) => stopped.push(target),
                Err(error) => failed.push(format!("{} ({})", target, error)),
            },
            Err(error) => failed.push(format!("{} ({})", target, error)),
        }
    }
    WorkerCleanupOutcome { stopped, failed }
}

/// Free swarm member capacity mid-run by stopping finished workers owned by
/// this coordinator. `run_plan` spawns a fresh worker per node by default and
/// normally cleans up only at the end of the run, so on large plans membership
/// grows monotonically toward the swarm member cap and fresh spawns start
/// getting refused. `exclude` protects workers assigned earlier in this loop
/// whose queued status may not have propagated yet. Returns how many workers
/// were stopped.
///
/// Tradeoff: a stopped `ready` worker may have been a composite planner whose
/// synthesis node would otherwise be routed back to it (planner affinity).
/// Assignment falls back to a fresh or other eligible worker in that case,
/// which is an acceptable degradation when the alternative is aborting the run
/// at the member cap.
pub(super) async fn cleanup_finished_workers_for_capacity(
    ctx: &ToolContext,
    exclude: &[String],
    reporter: &RunPlanReporter,
) -> usize {
    let Ok(members) = fetch_swarm_members(&ctx.session_id).await else {
        return 0;
    };
    let candidates: Vec<String> = cleanup_candidate_session_ids(
        &ctx.session_id,
        &members,
        &default_cleanup_target_statuses(),
        &[],
        false,
    )
    .into_iter()
    .filter(|session_id| !exclude.iter().any(|assigned| assigned == session_id))
    .collect();
    if candidates.is_empty() {
        return 0;
    }
    let outcome = stop_swarm_sessions(ctx, candidates, false).await;
    reporter
        .log(&format!("member-cap recovery: {}", outcome.describe()))
        .await;
    outcome.stopped.len()
}

/// How often the background progress card is refreshed from live plan state
/// while the driver is blocked awaiting workers.
const RUN_PLAN_PROGRESS_REFRESH_SECS: u64 = 15;

/// Whether the driver should abandon the current member-await and start a new
/// coordination loop because the plan's ready frontier grew while it was
/// blocked. Pure for unit testing.
///
/// `ready_baseline` is the set of ready item ids observed at the top of the
/// loop that started this await. Any *new* ready id means work the driver has
/// never had a chance to dispatch: a failed node re-queued via `swarm retry`,
/// a node unblocked by an externally-driven completion, or a gate-injected
/// gap. Comparing against the baseline (instead of `!ready.is_empty()`) is
/// what prevents wake storms: items that were already ready when the await
/// began (e.g. just-assigned tasks still momentarily `queued`, or ready nodes
/// that could not be assigned to any drivable worker) do not re-trigger, so a
/// permanently-stuck ready node wakes the driver at most once per await.
pub(super) fn await_should_wake_for_new_ready(
    ready_baseline: &std::collections::HashSet<String>,
    summary: &PlanGraphStatus,
) -> bool {
    summary
        .ready_ids
        .iter()
        .any(|id| !ready_baseline.contains(id))
}

pub(super) async fn await_swarm_progress(
    ctx: &ToolContext,
    session_ids: Vec<String>,
    timeout_minutes: u64,
    reporter: &RunPlanReporter,
    assignment_count: usize,
    ready_baseline: &std::collections::HashSet<String>,
) -> Result<()> {
    let request = Request::CommAwaitMembers {
        id: REQUEST_ID,
        session_id: ctx.session_id.clone(),
        target_status: default_run_await_statuses(),
        session_ids,
        mode: Some("any".to_string()),
        timeout_secs: Some(timeout_minutes.max(1) * 60),
        // run_plan needs the result inline to drive its coordination loop, so it
        // explicitly opts out of the background-by-default behavior.
        background: false,
        notify: false,
        wake: false,
    };
    let socket_timeout = std::time::Duration::from_secs(timeout_minutes.max(1) * 60 + 30);
    let await_members = send_request_with_timeout(request, Some(socket_timeout));
    tokio::pin!(await_members);

    // While blocked on the await (potentially many minutes), periodically
    // re-read live plan + member state and push it to the progress card.
    // Without this, worker completions and externally-assigned work (manual
    // `assign_task`) only surface at the driver's own wave boundaries, so the
    // card goes stale for the whole await. Refresh failures are ignored: the
    // card is best-effort and the await result is what drives the loop.
    let refresh_period = std::time::Duration::from_secs(RUN_PLAN_PROGRESS_REFRESH_SECS);
    let mut refresh =
        tokio::time::interval_at(tokio::time::Instant::now() + refresh_period, refresh_period);
    refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let response = loop {
        tokio::select! {
            result = &mut await_members => break result,
            _ = refresh.tick() => {
                let summary = match fetch_plan_status(&ctx.session_id).await {
                    Ok(summary) => summary,
                    Err(_) => continue,
                };
                if reporter.is_background() {
                    let live_active = fetch_in_flight_swarm_sessions(&ctx.session_id)
                        .await
                        .map(|sessions| sessions.len())
                        .unwrap_or(0);
                    let (completed, total, message) =
                        run_plan_progress_snapshot(&summary, live_active, assignment_count);
                    reporter.progress(completed, total, message).await;
                }
                // Ready frontier grew while blocked (a `swarm retry` re-queued
                // failed nodes, an external completion unblocked work, a gate
                // injected gaps): return to the coordination loop so the new
                // work is dispatched under the normal budget instead of
                // waiting out the current wave. The abandoned await is a
                // plain request future; dropping it cancels only our wait,
                // not the workers.
                if await_should_wake_for_new_ready(ready_baseline, &summary) {
                    reporter
                        .log("ready frontier grew during await (retry/requeue or external unblock); re-entering dispatch loop")
                        .await;
                    return Ok(());
                }
            }
        }
    };

    match response {
        Ok(ServerEvent::CommAwaitMembersResponse {
            completed, summary, ..
        }) => {
            if completed {
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "Timed out waiting for swarm progress: {}",
                    summary
                ))
            }
        }
        Ok(response) => ensure_success(&response),
        Err(e) => Err(anyhow::anyhow!(
            "Failed while awaiting swarm progress: {}",
            e
        )),
    }
}
