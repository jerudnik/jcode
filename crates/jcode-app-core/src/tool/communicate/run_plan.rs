use super::run_plan_errors::*;
use super::*;

fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

const RUN_PLAN_LIVENESS_INTERVAL_SECS: u64 = 5 * 60;

#[derive(Debug, Clone, Copy)]
struct RunPlanBudget {
    graph_started_at_unix_ms: u64,
    graph_wall_clock_limit_ms: u64,
}

fn compact_run_plan_duration(secs: u64) -> String {
    if secs < 60 {
        return format!("{secs}s");
    }
    if secs < 60 * 60 {
        return format!("{}m", secs / 60);
    }
    let hours = secs / (60 * 60);
    let minutes = (secs % (60 * 60)) / 60;
    if minutes == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {minutes}m")
    }
}

fn run_plan_progress_message(
    message: String,
    total_nodes: usize,
    driver_elapsed_secs: u64,
    now_unix_ms: u64,
    budget: Option<RunPlanBudget>,
) -> String {
    let interval = driver_elapsed_secs / RUN_PLAN_LIVENESS_INTERVAL_SECS;
    if interval == 0 {
        return message;
    }

    let mut message = format!(
        "{message} · liveness {}",
        compact_run_plan_duration(interval * RUN_PLAN_LIVENESS_INTERVAL_SECS)
    );
    if let Some(budget) = budget {
        let graph_elapsed_secs =
            now_unix_ms.saturating_sub(budget.graph_started_at_unix_ms) / 1_000;
        message.push_str(&format!(
            " · graph size: {total_nodes} nodes · budget: wall clock {}/{}",
            compact_run_plan_duration(graph_elapsed_secs),
            compact_run_plan_duration(budget.graph_wall_clock_limit_ms / 1_000),
        ));
    }
    message
}

/// Read the plan's wall-clock window: when its budget clock started and how
/// long it may run.
///
/// A plan with no persisted safety ledger predates budgets or was seeded
/// through a path that does not write one. Treat that as a plan starting its
/// clock now under current configuration rather than refusing to schedule it:
/// the budget exists to stop runaway growth, and declining to run a plan at all
/// is a strictly worse failure than running it under a freshly derived limit.
fn graph_wall_clock(summary: &PlanGraphStatus) -> Result<(u64, u64)> {
    // Fail closed. Deriving a fresh window here instead would restart the clock
    // on every call, so a graph missing its ledger could never age out at all.
    // A seed path that does not persist a ledger is the bug to fix; refusing is
    // the safe response to one that slips through.
    let raw = summary
        .phases_by_id
        .get(jcode_plan::dag::PLAN_SAFETY_STATUS_META_ID)
        .ok_or_else(|| {
            anyhow::anyhow!("run_plan refused to schedule a graph without a persisted safety ledger")
        })?;
    let (started, limit) = raw.split_once(':').ok_or_else(|| {
        anyhow::anyhow!("run_plan refused a graph with an invalid persisted safety ledger")
    })?;
    Ok((started.parse()?, limit.parse()?))
}

/// Hard wall-clock verdict for a running task graph. Pure so the budget
/// boundary is unit-testable without a live swarm: a mutation probe deleted
/// the previous inline comparison in the scheduler loop and zero tests went
/// red, so the boundary now lives here where a test pins it.
pub(super) fn wall_clock_exhausted(elapsed_ms: u64, limit_ms: u64) -> bool {
    elapsed_ms > limit_ms
}

/// Decide how many swarm workers `run_plan` keeps active at once.
///
/// Policy:
///   * an explicit `requested` limit always wins (clamped to >= 1);
///   * deep mode with no explicit limit fans out wide: use `deep_cap`, where
///     `0` means "no extra cap" (`usize::MAX`) so the whole ready set is
///     dispatched, bounded only by the swarm member cap;
///   * light mode with no explicit limit keeps the small, cheap fan-out default.
///
/// Pure and side-effect free so the concurrency contract is unit-testable
/// without a live swarm.
pub(super) fn resolve_run_plan_concurrency(
    requested: Option<usize>,
    is_deep: bool,
    deep_cap: usize,
) -> usize {
    match requested {
        Some(explicit) => explicit.max(1),
        None if is_deep => {
            if deep_cap == 0 {
                usize::MAX
            } else {
                deep_cap
            }
        }
        None => LIGHT_MODE_DEFAULT_CONCURRENCY,
    }
}

/// Return true when graph growth has crossed the next doubling checkpoint.
/// The caller advances `seed_count` to the next doubling after reporting so a
/// stable graph does not emit the same checkpoint again.
pub(super) fn growth_alarm(seed_count: usize, node_count: usize) -> bool {
    seed_count > 0 && node_count > seed_count.saturating_mul(2)
}

pub(super) fn advance_growth_alarm_baseline(mut seed_count: usize, node_count: usize) -> usize {
    while growth_alarm(seed_count, node_count) {
        let next = seed_count.saturating_mul(2);
        if next == seed_count {
            break;
        }
        seed_count = next;
    }
    seed_count
}

/// Running tally of how well a `run_plan` drive used its concurrency budget.
///
/// Deep mode's promise is comprehensiveness through parallel fan-out, so a run
/// that finishes with peak parallelism ~1 despite a 32+ slot budget means the
/// graph was decomposed serially and the budget was wasted. Tracking this per
/// loop (max in-flight, plus how often open slots sat idle with no ready work)
/// turns "did we actually use the budget?" into a measured, reportable number
/// instead of a hope.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct RunPlanUtilization {
    /// Highest number of simultaneously in-flight tasks observed.
    pub(super) peak_in_flight: usize,
    /// Coordination loops observed.
    pub(super) loops: usize,
    /// Loops where open worker slots existed but the plan had nothing ready to
    /// dispatch into them (budget idle due to graph narrowness, not the cap).
    pub(super) starved_loops: usize,
}

#[derive(Debug, Default)]
pub(super) struct RunPlanChurnGuard {
    consecutive_assignment_waves_without_completion: usize,
    churned_nodes: std::collections::BTreeSet<String>,
    pub(super) lost_workers: std::collections::BTreeSet<String>,
}

impl RunPlanChurnGuard {
    pub(super) const MAX_WAVES_WITHOUT_COMPLETION: usize = 3;

    #[cfg(test)]
    pub(super) fn max_created_sessions_before_abort(concurrency_limit: usize) -> usize {
        concurrency_limit.saturating_mul(Self::MAX_WAVES_WITHOUT_COMPLETION)
    }

    pub(super) fn record_wave(
        &mut self,
        assignments: &[(String, String)],
        completed_before: usize,
        completed_after: usize,
    ) -> Option<String> {
        if completed_after > completed_before {
            self.consecutive_assignment_waves_without_completion = 0;
            self.churned_nodes.clear();
            self.lost_workers.clear();
            return None;
        }
        if assignments.is_empty() {
            // No progress, but also no new assignment: leave the counter as-is
            // so slow churn (assign, idle loop, assign, ...) still trips the
            // breaker instead of resetting it every quiet loop.
            return None;
        }

        self.consecutive_assignment_waves_without_completion += 1;
        for (node_id, session_id) in assignments {
            self.churned_nodes.insert(node_id.clone());
            self.lost_workers.insert(session_id.clone());
        }

        (self.consecutive_assignment_waves_without_completion >= Self::MAX_WAVES_WITHOUT_COMPLETION)
            .then(|| self.diagnostic())
    }

    pub(super) fn diagnostic(&self) -> String {
        let nodes = if self.churned_nodes.is_empty() {
            "unknown".to_string()
        } else {
            self.churned_nodes
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        };
        let workers = if self.lost_workers.is_empty() {
            "unknown".to_string()
        } else {
            self.lost_workers
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "run_plan aborted after {} consecutive assignment wave(s) produced no completed nodes; possible spawn churn. Churned node(s): {nodes}. Lost worker(s): {workers}. Residue policy: pre-prompt failed sessions are finished owned workers; run_plan error cleanup cleans them by default, while retain_agents=true retains them for inspection. Inspect swarm membership and worker subscribe logs before retrying.",
            self.consecutive_assignment_waves_without_completion
        )
    }
}

impl RunPlanUtilization {
    /// Record one coordination loop. `open_slots` is `None` when the budget is
    /// unbounded (`concurrency_limit == usize::MAX`): an infinite budget has no
    /// meaningful starvation denominator, so only peak parallelism is tracked.
    pub(super) fn record_loop(
        &mut self,
        in_flight: usize,
        open_slots: Option<usize>,
        dispatched: usize,
    ) {
        self.loops += 1;
        self.peak_in_flight = self.peak_in_flight.max(in_flight + dispatched);
        if let Some(open_slots) = open_slots
            && open_slots > 0
            && dispatched < open_slots
        {
            self.starved_loops += 1;
        }
    }

    /// Render the utilization line for the terminal report. In deep mode a
    /// starved run also gets an actionable hint, because the fix (wider
    /// decomposition) belongs to the model reading this output.
    pub(super) fn report(&self, concurrency_limit: usize, is_deep: bool) -> String {
        let limit_label = if concurrency_limit == usize::MAX {
            "unbounded".to_string()
        } else {
            concurrency_limit.to_string()
        };
        let mut line = format!(
            "Budget utilization: peak {} of {} concurrent worker slot(s); {} of {} loop(s) had idle capacity with nothing ready.",
            self.peak_in_flight, limit_label, self.starved_loops, self.loops
        );
        let mostly_starved = self.loops > 0 && self.starved_loops * 2 >= self.loops;
        let ran_narrow = self.loops >= 3 && self.peak_in_flight <= 2;
        if is_deep && (mostly_starved || ran_narrow) {
            line.push_str(
                "\nDeep-mode hint: the graph ran much narrower than the agent budget. If coverage \
                 matters, expand remaining or follow-up work into MANY independent sibling nodes \
                 (depends_on only for real data dependencies) so the ready set fills the budget.",
            );
        }
        line
    }
}

/// Extract the background task id from its output file path
/// (`<task_id>.output`), mirroring the bash tool's convention so progress
/// updates can be routed back to the background task manager.
pub(super) fn task_id_from_output_path(path: &std::path::Path) -> Option<&str> {
    path.file_name()?.to_str()?.strip_suffix(".output")
}

/// Progress/log sink for a `run_plan` execution.
///
/// In background mode this appends human-readable lines to the background
/// task's output file and pushes determinate progress (terminal/total plan
/// nodes) into the background task manager, so the UI renders a live swarm
/// progress card and `bg status` stays meaningful. In inline (blocking) mode
/// every method is a no-op.
pub(super) struct RunPlanReporter {
    pub(super) task_id: Option<String>,
    output_path: Option<std::path::PathBuf>,
    started_at: std::time::Instant,
    budget: std::sync::OnceLock<RunPlanBudget>,
}

impl RunPlanReporter {
    pub(super) fn inline() -> Self {
        Self {
            task_id: None,
            output_path: None,
            started_at: std::time::Instant::now(),
            budget: std::sync::OnceLock::new(),
        }
    }

    pub(super) fn background(output_path: &std::path::Path) -> Self {
        Self {
            task_id: task_id_from_output_path(output_path).map(str::to_string),
            output_path: Some(output_path.to_path_buf()),
            started_at: std::time::Instant::now(),
            budget: std::sync::OnceLock::new(),
        }
    }

    /// Whether this reporter feeds a live background progress card (inline
    /// reporters are no-ops, so refresh polling would be wasted requests).
    pub(super) fn is_background(&self) -> bool {
        self.task_id.is_some()
    }

    fn set_budget(&self, graph_started_at_unix_ms: u64, graph_wall_clock_limit_ms: u64) {
        let _ = self.budget.set(RunPlanBudget {
            graph_started_at_unix_ms,
            graph_wall_clock_limit_ms,
        });
    }

    pub(super) async fn log(&self, line: &str) {
        let Some(path) = &self.output_path else {
            return;
        };
        use tokio::io::AsyncWriteExt;
        if let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
        {
            let _ = file.write_all(format!("{}\n", line).as_bytes()).await;
        }
    }

    pub(super) async fn progress(&self, terminal: usize, total: usize, message: String) {
        let Some(task_id) = &self.task_id else {
            return;
        };
        let message = run_plan_progress_message(
            message,
            total,
            self.started_at.elapsed().as_secs(),
            unix_now_ms(),
            self.budget.get().copied(),
        );
        let progress = crate::bus::BackgroundTaskProgress {
            kind: crate::bus::BackgroundTaskProgressKind::Determinate,
            percent: None,
            message: Some(message),
            current: Some(terminal as u64),
            total: Some(total as u64),
            unit: Some("nodes".to_string()),
            eta_seconds: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
            source: crate::bus::BackgroundTaskProgressSource::Reported,
        }
        .normalize();
        let _ = crate::background::global()
            .update_progress(task_id, progress)
            .await;
    }

    /// Record an explicit checkpoint (a JCODE_CHECKPOINT-style milestone) on
    /// the background task, so pause/alert moments surface as checkpoint events
    /// in the UI instead of only trailing the output log. No-op inline.
    async fn checkpoint(&self, message: &str) {
        self.log(message).await;
        let Some(task_id) = &self.task_id else {
            return;
        };
        let progress = crate::bus::BackgroundTaskProgress {
            kind: crate::bus::BackgroundTaskProgressKind::Indeterminate,
            percent: None,
            message: Some(message.to_string()),
            current: None,
            total: None,
            unit: None,
            eta_seconds: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
            source: crate::bus::BackgroundTaskProgressSource::Reported,
        }
        .normalize();
        let _ = crate::background::global()
            .update_checkpoint(task_id, progress)
            .await;
    }

    /// Rewrite the output file so `summary` leads and the progressive log
    /// trails it. Background completion previews take the first ~500 chars of
    /// the output file, so the terminal summary must come first for the
    /// agent's wake notification to be useful.
    pub(super) async fn finalize(&self, summary: &str) {
        let Some(path) = &self.output_path else {
            return;
        };
        let log = tokio::fs::read_to_string(path).await.unwrap_or_default();
        let content = if log.trim().is_empty() {
            format!("{}\n", summary)
        } else {
            format!("{}\n\n--- run log ---\n{}", summary, log)
        };
        let _ = tokio::fs::write(path, content).await;
    }
}

/// Per-process registry of sessions with a `run_plan` driver claimed or
/// running. The duplicate-driver guard does its check-and-insert under this
/// one lock, so two `run_plan` calls racing in the same batch cannot both
/// pass. Deliberately per-process: a stale `Running` status file left on disk
/// by a previous (reloaded/crashed) server process must never block
/// restarting the driver.
pub(super) fn run_plan_driver_claims()
-> &'static std::sync::Mutex<HashMap<String, RunPlanDriverClaim>> {
    static CLAIMS: std::sync::OnceLock<std::sync::Mutex<HashMap<String, RunPlanDriverClaim>>> =
        std::sync::OnceLock::new();
    CLAIMS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

pub(super) enum RunPlanDriverClaim {
    /// Claimed, background task not spawned yet.
    Starting,
    /// Driver spawned as this background task.
    Running(String),
}

pub(super) enum RunPlanDriverClaimResult {
    Claimed(RunPlanClaimGuard),
    /// A driver already holds the claim. Carries its task id when known
    /// (None while the winner is still between claim and spawn).
    AlreadyRunning(Option<String>),
}

/// RAII holder for a `Starting` claim. Dropping it without
/// [`RunPlanClaimGuard::record_task`] releases the claim, so a cancelled or
/// failed startup path cannot permanently block `run_plan` for the session.
pub(super) struct RunPlanClaimGuard {
    session_id: String,
    defused: bool,
}

impl RunPlanClaimGuard {
    /// Upgrade the claim to `Running(task_id)`. From here staleness is
    /// resolved via `BackgroundTaskManager::is_live_task`: once the driver
    /// task finishes (and is pruned from the live map), the next claim
    /// replaces this entry.
    pub(super) fn record_task(mut self, task_id: &str) {
        let mut claims = run_plan_driver_claims()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        claims.insert(
            self.session_id.clone(),
            RunPlanDriverClaim::Running(task_id.to_string()),
        );
        self.defused = true;
    }
}

impl Drop for RunPlanClaimGuard {
    fn drop(&mut self) {
        if self.defused {
            return;
        }
        let mut claims = run_plan_driver_claims()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Only release a claim this guard still owns.
        if matches!(
            claims.get(&self.session_id),
            Some(RunPlanDriverClaim::Starting)
        ) {
            claims.remove(&self.session_id);
        }
    }
}

/// Atomically claim the `run_plan` driver slot for `session_id`.
///
/// Check-and-insert happens under one lock. An existing `Running` claim only
/// blocks while its background task is still live in this process; a claim
/// left by a finished (pruned) or pre-reload driver is replaced.
pub(super) fn try_claim_run_plan_driver(
    manager: &crate::background::BackgroundTaskManager,
    session_id: &str,
) -> RunPlanDriverClaimResult {
    let mut claims = run_plan_driver_claims()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match claims.get(session_id) {
        Some(RunPlanDriverClaim::Starting) => {
            return RunPlanDriverClaimResult::AlreadyRunning(None);
        }
        Some(RunPlanDriverClaim::Running(task_id)) => {
            if manager.is_live_task(task_id) {
                return RunPlanDriverClaimResult::AlreadyRunning(Some(task_id.clone()));
            }
            // Stale claim: the driver task already finished or belonged to a
            // previous process image. Fall through and take over.
        }
        None => {}
    }
    claims.insert(session_id.to_string(), RunPlanDriverClaim::Starting);
    RunPlanDriverClaimResult::Claimed(RunPlanClaimGuard {
        session_id: session_id.to_string(),
        defused: false,
    })
}

/// Drive `run_plan` as a managed background task and return immediately.
///
/// The coordinating agent stays responsive: the plan loop runs inside the
/// shared `BackgroundTaskManager` (task id, progress card, `bg` tool
/// integration), and completion is delivered through the standard notify/wake
/// path like any other background task.
pub(super) async fn run_swarm_plan_in_background(
    ctx: &ToolContext,
    params: CommunicateInput,
) -> Result<ToolOutput> {
    // Validate the plan inline so an empty/broken plan errors immediately
    // instead of as a delayed background failure.
    let initial_summary = fetch_plan_status(&ctx.session_id).await?;
    if initial_summary.item_count == 0 {
        return Ok(ToolOutput::new("No swarm plan items to run."));
    }

    // Refuse to start a second driver for the same session: two concurrent
    // run_plan loops would race on assignments and double-spawn workers. The
    // claim is check-and-insert under one lock, so two run_plan calls in the
    // same batch cannot both pass. Only drivers live in this process count; a
    // stale "running" status file left by a server reload must not block
    // restarting the driver (the claim map is per-process and dead task ids
    // fail the is_live_task check).
    let manager = crate::background::global();
    let claim = match try_claim_run_plan_driver(manager, &ctx.session_id) {
        RunPlanDriverClaimResult::Claimed(claim) => claim,
        RunPlanDriverClaimResult::AlreadyRunning(existing) => {
            return Ok(ToolOutput::new(match existing {
                Some(task_id) => format!(
                    "A swarm run_plan driver is already running for this session (task {}). \
                     Check it with `bg action=\"status\" task_id=\"{}\"` or `swarm plan_status` instead of starting another.",
                    task_id, task_id
                ),
                None => "A swarm run_plan driver is already starting for this session. \
                         Check it with `swarm plan_status` instead of starting another."
                    .to_string(),
            }));
        }
    };

    let notify = params.notify.unwrap_or(true);
    let wake = params.wake.unwrap_or(true);
    // Keep the display name free of the "·" separator used by the background
    // notification markdown header, or downstream parsing mis-splits the label.
    let display_name = format!(
        "run_plan ({} nodes, {} mode)",
        initial_summary.item_count, initial_summary.mode
    );

    let bg_ctx = ctx.clone();
    let info = crate::background::global()
        .spawn_with_notify(
            "swarm",
            Some(display_name.clone()),
            &ctx.session_id,
            notify,
            wake,
            move |output_path| async move {
                let reporter = RunPlanReporter::background(&output_path);
                match run_swarm_plan_to_terminal(&bg_ctx, &params, &reporter).await {
                    Ok(output) => {
                        reporter.finalize(&output.output).await;
                        Ok(TaskResult::completed(Some(0)))
                    }
                    Err(error) => {
                        let message = format!("run_plan failed: {}", error);
                        reporter.finalize(&message).await;
                        Ok(TaskResult::failed(None, message))
                    }
                }
            },
        )
        .await;
    // Cap/drain refusal (F12): the plan task never ran; do not record the
    // dead task id or report a running plan.
    if let Some(reason) = &info.refused {
        return Err(anyhow::anyhow!("Swarm plan refused: {reason}"));
    }
    claim.record_task(&info.task_id);

    let delivery_note = if wake {
        "You'll be woken with the result when the plan reaches a terminal state."
    } else if notify {
        "A notification will appear when the plan reaches a terminal state."
    } else {
        "Notifications disabled. Use the `bg` tool to check status."
    };
    let output = format!(
        "🐝 Swarm plan running in background.\n\n\
         Task ID: {}\n\
         Plan: {} node(s), {} mode\n\
         Output file: {}\n\n\
         {}\n\
         Check progress: use the `bg` tool with action=\"status\" and task_id=\"{}\", or `swarm plan_status`.\n\
         Note: a server reload stops this driver (workers keep running); rerun `swarm run_plan` to resume driving the same plan.",
        info.task_id,
        initial_summary.item_count,
        initial_summary.mode,
        info.output_file.display(),
        delivery_note,
        info.task_id,
    );

    Ok(ToolOutput::new(output)
        .with_title(format!("Swarm run_plan in background: {}", info.task_id))
        .with_metadata(json!({
            "background": true,
            "swarm": true,
            "task_id": info.task_id,
            "display_name": display_name,
            "output_file": info.output_file.to_string_lossy(),
            "status_file": info.status_file.to_string_lossy(),
        })))
}

/// Hint appended to every `run_plan` driver failure. Finished workers are
/// collected on failure exits too (W3a), but RUNNING workers are deliberately
/// kept alive so the plan can be resumed; the caller must know how to stop or
/// resume them.
const RUN_PLAN_WORKER_RETENTION_HINT: &str = "\nStill-running workers were kept alive; run `swarm cleanup` to stop them, rerun `swarm run_plan` to resume driving the same plan, or `swarm plan_status` to inspect.";

/// Append the worker-retention hint to a driver failure message, idempotently
/// so wrappers that re-report an already-hinted error do not duplicate it.
pub(super) fn with_worker_retention_hint(message: String) -> String {
    if message.contains("swarm cleanup") {
        message
    } else {
        format!("{message}{RUN_PLAN_WORKER_RETENTION_HINT}")
    }
}

pub(super) async fn run_swarm_plan_to_terminal(
    ctx: &ToolContext,
    params: &CommunicateInput,
    reporter: &RunPlanReporter,
) -> Result<ToolOutput> {
    match run_swarm_plan_loop(ctx, params, reporter).await {
        Ok(output) => Ok(output),
        Err(error) => {
            // W3a: driver-failure exits (assignment failure, await timeout,
            // stall, max-loops) used to skip the end-of-plan cleanup entirely,
            // leaking every spawned worker as a permanent "ready" member.
            // Collect FINISHED owned workers here too; running workers are
            // deliberately left alive so the plan can be resumed with a rerun
            // of run_plan. retain_agents=true keeps everything, as on success.
            let retain_agents = params.retain_agents.unwrap_or(false);
            let cleanup_note = if retain_agents {
                "Retained spawned workers because retain_agents=true.".to_string()
            } else {
                let cleanup_params = CommunicateInput {
                    force: None,
                    session_ids: None,
                    target_status: None,
                    ..params.clone()
                };
                match cleanup_swarm_workers(ctx, &cleanup_params).await {
                    Ok(cleanup) => format!("Finished workers collected: {cleanup}"),
                    Err(cleanup_error) => format!(
                        "Finished-worker cleanup also failed ({cleanup_error}); \
                         run `swarm cleanup` manually."
                    ),
                }
            };
            Err(anyhow::anyhow!(with_worker_retention_hint(format!(
                "{error}\n{cleanup_note}"
            ))))
        }
    }
}

pub(super) async fn run_swarm_plan_loop(
    ctx: &ToolContext,
    params: &CommunicateInput,
    reporter: &RunPlanReporter,
) -> Result<ToolOutput> {
    let initial_summary = fetch_plan_status(&ctx.session_id).await?;
    let (graph_started_at_unix_ms, graph_wall_clock_limit_ms) = graph_wall_clock(&initial_summary)?;
    reporter.set_budget(graph_started_at_unix_ms, graph_wall_clock_limit_ms);
    let is_deep = initial_summary.mode.eq_ignore_ascii_case("deep");
    let initial_seed_count = if initial_summary.seeded_count > 0 {
        initial_summary.seeded_count
    } else {
        initial_summary.item_count.max(1)
    };
    let mut growth_alarm_baseline = initial_seed_count;

    let configured_deep_cap = crate::config::config().agents.swarm_max_concurrent_agents;
    let concurrency_limit =
        resolve_run_plan_concurrency(params.concurrency_limit, is_deep, configured_deep_cap);
    let timeout_minutes = params.timeout_minutes.unwrap_or(60).max(1);
    let retain_agents = params.retain_agents.unwrap_or(false);
    let spawn_if_needed = params.spawn_if_needed.or(Some(true));
    // Default to a fresh worker per task-graph node. Reusing a worker that already
    // completed a *different* node carries that node's conversation into the next
    // assignment, and the model often just re-reports its prior result instead of
    // doing the new work (observed leaving gap/synthesis nodes stuck). The task-DAG
    // model assumes clean, isolated workers, so unless the caller explicitly opts
    // into reuse (`prefer_spawn=false`), prefer spawning a fresh worker per node.
    let prefer_spawn = params.prefer_spawn.or(Some(true));
    let mut assignment_count = 0usize;
    let mut loop_count = 0usize;
    let max_loops = 200usize;
    let mut utilization = RunPlanUtilization::default();
    let mut churn_guard = RunPlanChurnGuard::default();
    // Consecutive loops where an active task exists but no drivable worker is
    // awaitable. This is normally a brief transition (a composite re-waking to
    // synthesize, or a just-finished task whose member status has not propagated),
    // so we back off and re-check a few times before declaring a real stall.
    let mut transient_stall_loops = 0usize;
    let max_transient_stall_loops = 5usize;

    loop {
        loop_count += 1;
        if loop_count > max_loops {
            return Err(anyhow::anyhow!(
                "run_plan exceeded {} coordination loops; leaving workers untouched for inspection",
                max_loops
            ));
        }

        let elapsed_ms = unix_now_ms().saturating_sub(graph_started_at_unix_ms);
        if wall_clock_exhausted(elapsed_ms, graph_wall_clock_limit_ms) {
            let message = format!(
                "Task graph paused: hard wall-clock budget exceeded (limit {}s, observed {}s). The scheduler rejected further coordination and froze graph growth. Inspect the plan and start a smaller replacement graph; ordinary unfreeze cannot bypass the exhausted budget.",
                graph_wall_clock_limit_ms / 1_000,
                elapsed_ms / 1_000
            );
            let response = send_request(Request::CommTaskControl {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                action: "freeze".to_string(),
                task_id: String::new(),
                target_session: None,
                message: Some(message.clone()),
            })
            .await
            .map_err(|error| {
                anyhow::anyhow!("Failed to pause wall-clock-exhausted plan: {error}")
            })?;
            ensure_success(&response)?;
            reporter.checkpoint(&message).await;
            if let Err(error) = broadcast_plan_alert(ctx, &message).await {
                reporter
                    .log(&format!(
                        "failed to broadcast wall-clock budget pause to the swarm: {error}"
                    ))
                    .await;
            }
            return Err(anyhow::anyhow!(message));
        }

        let summary = fetch_plan_status(&ctx.session_id).await?;
        if growth_alarm(growth_alarm_baseline, summary.item_count) {
            let message = format!(
                "Task graph growth checkpoint: seeded {} node(s), now {}. This crossed the next 2x growth threshold. Review `swarm plan_status`; if the new work is not intentional, call `swarm` with `action:\"freeze\"`. Otherwise keep the graph focused and continue.",
                initial_seed_count, summary.item_count
            );
            reporter.checkpoint(&message).await;
            if let Err(error) = broadcast_plan_alert(ctx, &message).await {
                reporter
                    .log(&format!(
                        "failed to broadcast task-graph growth checkpoint to the swarm: {error}"
                    ))
                    .await;
            }
            growth_alarm_baseline =
                advance_growth_alarm_baseline(growth_alarm_baseline, summary.item_count);
        }
        let completed_before_wave = summary.completed_ids.len();
        if summary.item_count == 0 {
            return Ok(ToolOutput::new("No swarm plan items to run."));
        }

        let members = fetch_swarm_members(&ctx.session_id).await?;
        let in_flight_sessions = in_flight_swarm_session_ids(&members, &ctx.session_id);

        // Credential-failure circuit breaker: when a wave of workers dies with
        // 401/invalid_grant-style auth errors and nothing has completed, the
        // credential is broken for every worker on that route. Pausing here
        // (before the terminal check) means even a fully-burned first wave
        // surfaces the root cause and the fix instead of a bare
        // "failed=N" terminal summary with no explanation.
        if let Some(wave) = detect_credential_failure_wave(
            &members,
            &ctx.session_id,
            summary.completed_ids.len(),
            CREDENTIAL_FAILURE_WAVE_WINDOW_SECS,
        ) {
            let message =
                format_credential_failure_wave_error(&wave, CREDENTIAL_FAILURE_WAVE_WINDOW_SECS);
            reporter.checkpoint(&message).await;
            if let Err(error) = broadcast_plan_alert(ctx, &message).await {
                reporter
                    .log(&format!(
                        "failed to broadcast credential-failure alert to the swarm: {error}"
                    ))
                    .await;
            }
            return Err(anyhow::anyhow!(message));
        }

        let terminal_count = plan_terminal_node_count(&summary);
        let (progress_completed, progress_total, progress_message) =
            run_plan_progress_snapshot(&summary, in_flight_sessions.len(), assignment_count);
        reporter
            .progress(progress_completed, progress_total, progress_message)
            .await;
        let no_more_runnable = summary.active_ids.is_empty()
            && summary.next_ready_ids.is_empty()
            && in_flight_sessions.is_empty();
        if no_more_runnable || terminal_count >= summary.item_count {
            let mut output =
                format_run_plan_terminal_summary(loop_count, &summary, assignment_count);
            output.push_str(&format!(
                "\n{}",
                utilization.report(concurrency_limit, is_deep)
            ));
            if !summary.low_confidence_ids.is_empty() {
                output.push_str(&format!(
                    "\nConfidence coverage: {} completed node(s) self-reported LOW confidence: {}. \
                     Consider seeding follow-up nodes to shore these up before trusting the result.",
                    summary.low_confidence_ids.len(),
                    summary.low_confidence_ids.join(", ")
                ));
            }
            if retain_agents {
                output.push_str("\nRetained spawned workers because retain_agents=true.");
            } else {
                // Run the automatic end-of-plan cleanup with a sanitized input:
                // `force`, `session_ids`, and `target_status` on the run_plan
                // call are meant for explicit stop/cleanup/await actions, and
                // leaking `force=true` here would force-stop every terminal
                // swarm member (including user-created idle sessions) instead
                // of only the workers this coordinator owns.
                let cleanup_params = CommunicateInput {
                    force: None,
                    session_ids: None,
                    target_status: None,
                    ..params.clone()
                };
                let cleanup = cleanup_swarm_workers(ctx, &cleanup_params).await?;
                output.push_str(&format!("\n{}", cleanup));
            }
            return Ok(ToolOutput::new(output));
        }

        let active_count = summary.active_ids.len().max(in_flight_sessions.len());
        let available_slots = concurrency_limit.saturating_sub(active_count);
        let mut assigned_sessions = Vec::new();
        let mut assigned_tasks = Vec::new();
        // Member-cap fallback state, reset each coordination loop. When the swarm
        // hits its total member cap, fresh spawns are refused; instead of aborting
        // the whole run we first free finished owned workers (incremental cleanup)
        // and retry, then fall back to reuse-only assignment (no spawning), and
        // only after that stop assigning and continue with in-flight work.
        let mut cap_hits = 0usize;
        let mut reuse_only = false;
        let mut slots_remaining = available_slots;
        while slots_remaining > 0 {
            let request = Request::CommAssignNext {
                id: REQUEST_ID,
                session_id: ctx.session_id.clone(),
                target_session: params.target_session.clone(),
                working_dir: params.working_dir.clone(),
                prefer_spawn: if reuse_only {
                    Some(false)
                } else {
                    prefer_spawn
                },
                spawn_if_needed: if reuse_only {
                    Some(false)
                } else {
                    spawn_if_needed
                },
                message: params.message.clone(),
                model: params.model.clone(),
                effort: params.effort.clone(),
            };
            match send_request(request).await {
                Ok(ServerEvent::CommAssignTaskResponse {
                    task_id,
                    target_session,
                    ..
                }) => {
                    assignment_count += 1;
                    slots_remaining -= 1;
                    reporter
                        .log(&format!("assigned {} -> {}", task_id, target_session))
                        .await;
                    assigned_tasks.push((task_id, target_session.clone()));
                    assigned_sessions.push(target_session);
                }
                Ok(ServerEvent::Error { message, .. }) => {
                    match classify_assign_error(&message) {
                        AssignErrorAction::BreakGracefully => break,
                        AssignErrorAction::RecoverCapacity => {
                            cap_hits += 1;
                            let freed = if cap_hits == 1 {
                                cleanup_finished_workers_for_capacity(
                                    ctx,
                                    &assigned_sessions,
                                    reporter,
                                )
                                .await
                            } else {
                                0
                            };
                            match cap_recovery_step(cap_hits, freed) {
                                CapRecoveryStep::RetryFresh => {
                                    // Cleanup freed member slots; retry this slot
                                    // with the fresh-spawn preference intact.
                                }
                                CapRecoveryStep::RetryReuse => {
                                    reuse_only = true;
                                    reporter
                                        .log(
                                            "member cap reached and no finished workers to free; \
                                             falling back to reusing ready workers (prefer_spawn=false)",
                                        )
                                        .await;
                                }
                                CapRecoveryStep::GiveUp => {
                                    reporter
                                        .log(
                                            "member cap still reached after recovery; \
                                             continuing with in-flight work",
                                        )
                                        .await;
                                    break;
                                }
                            }
                        }
                        AssignErrorAction::Fail => {
                            return Err(anyhow::anyhow!(message));
                        }
                    }
                }
                Ok(response) => ensure_success(&response)?,
                Err(e) => return Err(anyhow::anyhow!("Failed to assign next swarm task: {}", e)),
            }
        }
        utilization.record_loop(
            active_count,
            (concurrency_limit != usize::MAX).then_some(available_slots),
            assigned_sessions.len(),
        );

        let await_sessions = if assigned_sessions.is_empty() {
            in_flight_sessions
        } else {
            assigned_sessions
        };

        if await_sessions.is_empty() {
            if active_count > 0 {
                // An active task exists but nothing drivable is awaitable. This is
                // usually transient: a composite is re-waking to synthesize, or a
                // worker just finished and its member status has not propagated yet.
                // Re-check a few times with a short backoff before giving up, and
                // bail early if the plan reaches a terminal state in the meantime.
                transient_stall_loops += 1;
                if transient_stall_loops <= max_transient_stall_loops {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
                return Err(anyhow::anyhow!(
                    "run_plan found {} active task(s) but no running swarm members to await after {} re-checks; inspect plan_status and member list before retrying",
                    active_count,
                    max_transient_stall_loops
                ));
            }
            // Nothing was assigned this loop, nothing is in flight, yet the plan is
            // not terminal. This means some non-terminal task cannot be driven, e.g.
            // it is already assigned to a session run_plan cannot drive (a foreign or
            // stale member). Spinning here would busy-loop to the max-loop cap, so
            // surface the stuck state with the offending tasks instead.
            let stuck: Vec<String> = summary
                .next_ready_ids
                .iter()
                .chain(summary.ready_ids.iter())
                .cloned()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            let detail = if stuck.is_empty() {
                "no ready tasks and no in-flight workers".to_string()
            } else {
                format!(
                    "runnable task(s) {} could not be assigned to any drivable worker",
                    stuck.join(", ")
                )
            };
            return Err(anyhow::anyhow!(
                "run_plan stalled after {} loop(s): {}. This usually means a task is assigned to a session run_plan cannot drive (foreign or stale member). Reassign with an explicit target_session, or clear the stale assignment, then retry.",
                loop_count,
                detail
            ));
        }
        // Baseline for requeue pickup: everything ready at the top of this
        // loop either gets assigned below or is known-undispatchable this
        // wave. Anything ready *beyond* this set while we block (a retried
        // failure, an external unblock) should cut the await short.
        let ready_baseline: std::collections::HashSet<String> =
            summary.ready_ids.iter().cloned().collect();
        await_swarm_progress(
            ctx,
            await_sessions,
            timeout_minutes,
            reporter,
            assignment_count,
            &ready_baseline,
        )
        .await?;
        let post_await_summary = fetch_plan_status(&ctx.session_id).await?;
        if let Some(message) = churn_guard.record_wave(
            &assigned_tasks,
            completed_before_wave,
            post_await_summary.completed_ids.len(),
        ) {
            reporter.checkpoint(&message).await;
            if let Err(error) = broadcast_plan_alert(ctx, &message).await {
                reporter
                    .log(&format!(
                        "failed to broadcast run_plan churn alert to the swarm: {error}"
                    ))
                    .await;
            }
            return Err(anyhow::anyhow!(message));
        }
        // Real progress (an await completed); clear the transient-stall backoff so
        // a later genuine stall starts counting fresh.
        transient_stall_loops = 0;
    }
}

pub(super) async fn spawn_assignment_session(
    ctx: &ToolContext,
    params: &CommunicateInput,
) -> Result<String> {
    let spawn_request = Request::CommSpawn {
        id: REQUEST_ID,
        session_id: ctx.session_id.clone(),
        working_dir: params.working_dir.clone(),
        initial_message: None,
        request_nonce: Some(fresh_spawn_request_nonce(ctx)),
        spawn_mode: params.spawn_mode.clone(),
        model: params.model.clone(),
        effort: params.effort.clone(),
        label: None,
        subagent_type: params.subagent_type.clone(),
    };

    match send_request(spawn_request).await {
        Ok(ServerEvent::CommSpawnResponse { new_session_id, .. }) if !new_session_id.is_empty() => {
            Ok(new_session_id)
        }
        Ok(spawn_response) => {
            ensure_success(&spawn_response)?;
            Err(anyhow::anyhow!(
                "Spawn succeeded but new session ID was not returned."
            ))
        }
        Err(e) => Err(anyhow::anyhow!(
            "Failed to spawn agent for task assignment: {}",
            e
        )),
    }
}

pub(super) async fn assign_task_to_session(
    ctx: &ToolContext,
    params: &CommunicateInput,
    target_session: String,
    spawned_suffix: &str,
) -> Result<ToolOutput> {
    let retry_request = Request::CommAssignTask {
        id: REQUEST_ID,
        session_id: ctx.session_id.clone(),
        target_session: Some(target_session.clone()),
        task_id: params.task_id.clone(),
        message: params.message.clone(),
    };

    match send_request(retry_request).await {
        Ok(ServerEvent::CommAssignTaskResponse { task_id, .. }) => Ok(ToolOutput::new(format!(
            "Task '{}' assigned to {}{}",
            task_id, target_session, spawned_suffix
        ))),
        Ok(retry_response) => {
            ensure_success(&retry_response)?;
            Ok(ToolOutput::new(format!(
                "Assigned next runnable task to {}{}",
                target_session, spawned_suffix
            )))
        }
        Err(e) => Err(anyhow::anyhow!(
            "Failed to assign task after selecting {}: {}",
            target_session,
            e
        )),
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;

    #[test]
    fn graph_start_comes_from_persisted_status_and_missing_data_fails_closed() {
        let mut summary = PlanGraphStatus::empty_for_swarm("durable-clock");
        assert!(graph_wall_clock(&summary).is_err());
        summary.phases_by_id.insert(
            jcode_plan::dag::PLAN_SAFETY_STATUS_META_ID.to_string(),
            "1234:5678".to_string(),
        );
        assert_eq!(graph_wall_clock(&summary).unwrap(), (1234, 5678));
    }

    #[test]
    fn stable_plan_progress_changes_once_per_liveness_interval_with_graph_budget() {
        let budget = RunPlanBudget {
            graph_started_at_unix_ms: 1_000,
            graph_wall_clock_limit_ms: 2 * 60 * 60 * 1_000,
        };
        let before = run_plan_progress_message(
            "completed 2 of 8".to_string(),
            8,
            RUN_PLAN_LIVENESS_INTERVAL_SECS - 1,
            11 * 60 * 1_000,
            Some(budget),
        );
        let first = run_plan_progress_message(
            "completed 2 of 8".to_string(),
            8,
            RUN_PLAN_LIVENESS_INTERVAL_SECS,
            11 * 60 * 1_000,
            Some(budget),
        );
        let duplicate = run_plan_progress_message(
            "completed 2 of 8".to_string(),
            8,
            RUN_PLAN_LIVENESS_INTERVAL_SECS + 30,
            11 * 60 * 1_000,
            Some(budget),
        );
        let second = run_plan_progress_message(
            "completed 2 of 8".to_string(),
            8,
            RUN_PLAN_LIVENESS_INTERVAL_SECS * 2,
            11 * 60 * 1_000,
            Some(budget),
        );

        assert_eq!(before, "completed 2 of 8");
        assert_eq!(first, duplicate);
        assert_ne!(first, second);
        assert!(first.contains("liveness 5m"), "{first}");
        assert!(first.contains("graph size: 8 nodes"), "{first}");
        assert!(first.contains("budget: wall clock 10m/2h"), "{first}");
        assert!(second.contains("liveness 10m"), "{second}");
    }

    #[test]
    fn wall_clock_budget_holds_at_limit_and_exhausts_past_it() {
        assert!(!wall_clock_exhausted(0, 1_000));
        assert!(!wall_clock_exhausted(1_000, 1_000));
        assert!(wall_clock_exhausted(1_001, 1_000));
        assert!(wall_clock_exhausted(u64::MAX, 1_000));
    }
}
