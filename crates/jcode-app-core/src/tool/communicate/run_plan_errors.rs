use super::*;

pub(super) enum AssignErrorAction {
    /// No more runnable work or no eligible workers: stop assigning this loop
    /// and continue with in-flight work.
    BreakGracefully,
    /// The swarm hit its total member cap so fresh spawns are refused: free
    /// finished owned workers and/or fall back to reusing ready workers instead
    /// of aborting the whole run.
    RecoverCapacity,
    /// Anything else is a real failure.
    Fail,
}

pub(super) fn classify_assign_error(message: &str) -> AssignErrorAction {
    if message.contains("No runnable unassigned tasks")
        || message.contains("No ready or completed swarm agents")
    {
        AssignErrorAction::BreakGracefully
    } else if message.contains("Swarm member limit reached") {
        AssignErrorAction::RecoverCapacity
    } else {
        AssignErrorAction::Fail
    }
}

/// Next step for a slot whose assignment was refused by the member cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CapRecoveryStep {
    /// Cleanup freed capacity: retry the slot keeping the fresh-spawn preference.
    RetryFresh,
    /// Nothing was freed: retry the slot in reuse-only mode (no spawning).
    RetryReuse,
    /// Recovery already ran and the cap still refuses this slot: stop assigning
    /// this loop and continue with in-flight work.
    GiveUp,
}

/// Pure recovery policy for a member-cap refusal, keyed on how many times this
/// slot already hit the cap (`cap_hits`) and how many workers the incremental
/// cleanup freed. Kept side-effect free so the fallback contract is unit
/// testable without a live swarm.
pub(super) fn cap_recovery_step(cap_hits: usize, freed: usize) -> CapRecoveryStep {
    if cap_hits > 1 {
        CapRecoveryStep::GiveUp
    } else if freed > 0 {
        CapRecoveryStep::RetryFresh
    } else {
        CapRecoveryStep::RetryReuse
    }
}

/// Count each plan node at most once as terminal: completed, failed, blocked,
/// and cycle sets overlap in places (and failed nodes appear in none of the
/// legacy three), so a plain sum both over- and under-counts.
pub(super) fn plan_terminal_node_count(summary: &PlanGraphStatus) -> usize {
    summary
        .completed_ids
        .iter()
        .chain(summary.failed_ids.iter())
        .chain(summary.blocked_ids.iter())
        .chain(summary.cycle_ids.iter())
        .collect::<std::collections::HashSet<_>>()
        .len()
}

/// Numbers for the `run_plan` background progress card. Pure for unit testing.
///
/// The percent-driving pair is `(completed, total)`: only *completed* nodes
/// count toward 100%, so a run where most nodes failed reads as mostly
/// unfinished instead of "98% complete" (failed/blocked counts are surfaced in
/// the message instead). `live_active` is the count of in-flight worker
/// sessions observed from member state; the card shows whichever of plan
/// execution state (`active_ids`) or live member state is larger, so nodes
/// assigned outside this driver (e.g. manual `assign_task`) still show as
/// active.
pub(super) fn run_plan_progress_snapshot(
    summary: &PlanGraphStatus,
    live_active: usize,
    assignment_count: usize,
) -> (usize, usize, String) {
    let completed = summary.completed_ids.len();
    let active = summary.active_ids.len().max(live_active);
    let message = format!(
        "completed {} · failed {} · blocked {} · active {} · assignments {}",
        completed,
        summary.failed_ids.len(),
        summary.blocked_ids.len(),
        active,
        assignment_count
    );
    (completed, summary.item_count, message)
}

/// Terminal-state summary line for `run_plan`, including failed nodes so a run
/// with failures never reads like a clean finish. Pure for unit testing.
pub(super) fn format_run_plan_terminal_summary(
    loop_count: usize,
    summary: &PlanGraphStatus,
    assignment_count: usize,
) -> String {
    let mut output = format!(
        "Swarm plan reached terminal/blocked state after {} loop(s). completed={} failed={} blocked={} cycles={} active={} assignments={}",
        loop_count,
        summary.completed_ids.len(),
        summary.failed_ids.len(),
        summary.blocked_ids.len(),
        summary.cycle_ids.len(),
        summary.active_ids.len(),
        assignment_count
    );
    if summary.mode.eq_ignore_ascii_case("deep") {
        output.push_str(&format!(
            "\nGrowth: {} seeded -> {} nodes ({} machinery-grown: expansions, gate-injected gaps, gates).",
            summary.seeded_count, summary.item_count, summary.grown_count
        ));
    }
    if !summary.failed_ids.is_empty() {
        output.push_str(&format!(
            "\nFailed nodes: {}. This run did NOT finish cleanly; inspect them with `swarm plan_status` and retry or salvage before trusting the result.",
            summary.failed_ids.join(", ")
        ));
        // Recorded failure reasons make the summary self-explanatory: a wave
        // of "task failed: ... 401 Unauthorized" lines names the root cause
        // without another plan_status round-trip.
        for id in &summary.failed_ids {
            if let Some(reason) = summary.failed_reasons.get(id) {
                output.push_str(&format!("\n  {}: {}", id, reason));
            }
        }
    }
    output
}

/// Minimum number of credential-failed workers that count as a wave rather
/// than an isolated bad worker.
const CREDENTIAL_FAILURE_WAVE_MIN_WORKERS: usize = 2;

/// How recent a worker's credential failure must be (via `status_age_secs`) to
/// count toward a wave. Old failed workers from a previous, already-diagnosed
/// wave must not re-trip the breaker after the user fixes auth and retries.
pub(super) const CREDENTIAL_FAILURE_WAVE_WINDOW_SECS: u64 = 60;

/// A wave of worker failures that share one credential-shaped root cause.
///
/// When dispatched workers die within seconds of assignment with 401 /
/// `invalid_grant` / `authentication_error`-style errors, the credential is
/// broken for every worker on that route: assigning more nodes only fails more
/// of the plan. Detecting the wave lets `run_plan` pause dispatching and
/// surface the one real fix instead of silently burning the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CredentialFailureWave {
    /// Failed worker sessions in the wave.
    session_ids: Vec<String>,
    /// Representative failure detail (first observed).
    sample_detail: String,
    /// Provider named by the failing workers, when known (e.g. "anthropic").
    provider: Option<String>,
}

/// Detect a credential-failure wave in a swarm member snapshot.
///
/// A wave exists when, with **zero completed plan nodes**, at least
/// [`CREDENTIAL_FAILURE_WAVE_MIN_WORKERS`] drivable workers sit in `failed`
/// status whose detail classifies as a credential failure (via the shared
/// [`crate::provider::error_looks_like_credential_failure`] classifier) and
/// whose failure is recent (`status_age_secs <= window_secs`). Pure over its
/// inputs so the breaker contract is unit-testable without a live swarm.
pub(super) fn detect_credential_failure_wave(
    members: &[AgentInfo],
    coordinator_session_id: &str,
    completed_node_count: usize,
    window_secs: u64,
) -> Option<CredentialFailureWave> {
    if completed_node_count > 0 {
        return None;
    }
    let mut session_ids = Vec::new();
    let mut sample_detail: Option<String> = None;
    let mut provider: Option<String> = None;
    for member in members {
        if member.session_id == coordinator_session_id {
            continue;
        }
        if !swarm_member_is_drivable_worker(member, coordinator_session_id) {
            continue;
        }
        if member.status.as_deref() != Some("failed") {
            continue;
        }
        let Some(detail) = member.detail.as_deref() else {
            continue;
        };
        if !crate::provider::error_looks_like_credential_failure(detail) {
            continue;
        }
        // Require a known, recent failure age: stale failed workers (or ones
        // whose age did not propagate) must not re-trip the breaker.
        if !matches!(member.status_age_secs, Some(age) if age <= window_secs) {
            continue;
        }
        session_ids.push(member.session_id.clone());
        if sample_detail.is_none() {
            sample_detail = Some(detail.to_string());
        }
        if provider.is_none() {
            provider = member.provider_name.clone();
        }
    }
    if session_ids.len() < CREDENTIAL_FAILURE_WAVE_MIN_WORKERS {
        return None;
    }
    Some(CredentialFailureWave {
        session_ids,
        sample_detail: sample_detail.unwrap_or_default(),
        provider,
    })
}

/// The `jcode login` invocation most likely to fix a credential wave for
/// `provider`, mapping provider names to their login provider keys.
pub(super) fn credential_login_fix_hint(provider: Option<&str>) -> String {
    let lowered = provider.map(str::to_ascii_lowercase);
    let target = match lowered.as_deref() {
        Some("anthropic" | "claude") => "claude",
        Some("openai" | "codex") => "openai",
        Some("google" | "gemini") => "gemini",
        Some(other) if !other.trim().is_empty() => other,
        _ => "<provider>",
    };
    format!("`jcode login --provider {target}`")
}

/// Actionable pause message for a credential-failure wave: names the failed
/// workers, the credential-shaped root cause, and the fix. Pure for unit
/// testing; used both as the run error and as the swarm broadcast body.
pub(super) fn format_credential_failure_wave_error(
    wave: &CredentialFailureWave,
    window_secs: u64,
) -> String {
    format!(
        "run_plan paused dispatching: {count} worker(s) failed within {window_secs}s with \
         credential/auth failures and no plan node has completed (e.g. {first}: \"{sample}\"). \
         A broken credential (expired OAuth session, revoked refresh token, or invalid API key) \
         fails every worker on that route, so assigning more nodes would only fail more of the \
         plan. Fix auth first: run {login_hint} (or pin a working API-key route), then requeue \
         the failed nodes (`swarm retry`) and run `swarm run_plan` again.",
        count = wave.session_ids.len(),
        first = wave
            .session_ids
            .first()
            .map(String::as_str)
            .unwrap_or("worker"),
        sample = wave.sample_detail,
        login_hint = credential_login_fix_hint(wave.provider.as_deref()),
    )
}

/// Best-effort broadcast of a plan-level alert to the whole swarm, so live
/// members and attached UIs see why dispatch stopped.
pub(super) async fn broadcast_plan_alert(ctx: &ToolContext, message: &str) -> Result<()> {
    let request = Request::CommMessage {
        id: REQUEST_ID,
        from_session: ctx.session_id.clone(),
        message: message.to_string(),
        to_session: None,
        wake: None,
        delivery: None,
        tldr: Some("run_plan paused: credential failure wave; fix auth then retry".to_string()),
    };
    match send_request(request).await {
        Ok(response) => ensure_success(&response),
        Err(e) => Err(anyhow::anyhow!("Failed to broadcast plan alert: {}", e)),
    }
}
