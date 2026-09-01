//! Ambient action tiers rank the risk of unattended tool actions.
//!
//! These tiers are not assignment grants. Assignment grants define a plan
//! worker's authority and are enforced separately in `crate::tool::grant`.
//! This module only decides what an ambient session may do without a human.
//!
//! A session is "ambient" for as long as it is registered here. Registration is
//! the gate's ONLY key, which is why `AmbientSessionGuard` exists: unattended
//! runs are fallible, and a leaked ID would gate a later, unrelated session
//! that happened to reuse it.

use crate::safety::ActionTier;
use anyhow::Result;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use super::{ToolContext, get_safety_system};

/// Session IDs currently allowed to use ambient-only permission workflows.
static AMBIENT_SESSION_IDS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn ambient_session_ids() -> &'static Mutex<HashSet<String>> {
    AMBIENT_SESSION_IDS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Mark a session ID as ambient-enabled for ambient-only tooling.
pub fn register_ambient_session(session_id: impl Into<String>) {
    if let Ok(mut ids) = ambient_session_ids().lock() {
        ids.insert(session_id.into());
    }
}

/// Remove a session ID from the ambient-enabled set.
pub fn unregister_ambient_session(session_id: &str) {
    if let Ok(mut ids) = ambient_session_ids().lock() {
        ids.remove(session_id);
    }
}

/// Registers a session as ambient for the guard's lifetime and unregisters on
/// drop.
///
/// Unattended agent runs are fallible (`Session::load`, `run_once_capture` and
/// `mark_closed` all return `Result`), so a plain register/unregister pair
/// around them leaks the registration on every `?`. A leaked entry is not
/// inert: session IDs are the gate's only key, so a stale ID would gate a
/// later, unrelated session that happened to reuse it. The guard makes the
/// unregister unconditional.
pub struct AmbientSessionGuard {
    session_id: String,
}

impl AmbientSessionGuard {
    pub fn new(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        register_ambient_session(session_id.clone());
        Self { session_id }
    }

    /// Registers `child_session_id` only if `parent_session_id` is itself
    /// ambient, returning `None` when the parent is interactive.
    ///
    /// A spawned worker runs with exactly as much authority as whoever spawned
    /// it, so the gate must be INHERITED rather than applied unconditionally.
    /// Registering every child would gate an interactive user's worker, which
    /// would break ordinary use; registering none leaves an unattended agent
    /// able to do through a worker what it may not do itself, since the worker
    /// runs on a fresh session ID that nothing registers.
    pub fn inherit(parent_session_id: &str, child_session_id: impl Into<String>) -> Option<Self> {
        if is_ambient_session_registered(parent_session_id) {
            Some(Self::new(child_session_id))
        } else {
            None
        }
    }
}

impl Drop for AmbientSessionGuard {
    fn drop(&mut self) {
        unregister_ambient_session(&self.session_id);
    }
}

pub(super) fn is_ambient_session_registered(session_id: &str) -> bool {
    ambient_session_ids()
        .lock()
        .map(|ids| ids.contains(session_id))
        .unwrap_or(false)
}

/// Control-plane tools an ambient cycle needs in order to run and stop at all.
///
/// These are exempt from the tier gate for structural reasons, not because they
/// are harmless:
///
/// - `end_ambient_cycle` terminates the cycle (`ambient/runner.rs` reads the
///   result via `take_cycle_result`). Gating it would leave a cycle unable to
///   finish.
/// - `request_permission` is the tool used to ASK for permission. Gating it
///   would deadlock: the only escape from the gate would itself be gated.
/// - `schedule_ambient` only enqueues work; it performs no tier-2 action
///   itself. This is only sound because the agent that later runs a scheduled
///   item inherits the gate: `spawn_session_for_scheduled_item` and
///   `resume_dead_session_with_reminder` in `ambient/runner.rs` both register
///   their session. Without that, scheduling would be a privilege escalation,
///   since a gated cycle could schedule the tier-2 action it may not perform
///   and have an ungated agent carry it out.
/// - `send_message` is bounded by the channels the user configured in
///   `SafetyConfig`.
/// - `batch` is a dispatch wrapper: it re-enters `Registry::execute` for each
///   inner call (`tool/batch.rs`), so every inner tool is classified on its own.
///   Gating the wrapper would block tier-1 reads that happen to be batched.
const TIER_GATE_EXEMPT: &[&str] = &[
    "end_ambient_cycle",
    "request_permission",
    "schedule_ambient",
    "send_message",
    "batch",
];

/// Refusal text returned to an unattended agent whose action needs a human.
///
/// This is a REFUSAL, not a suspension. Nothing in the safety system resumes a
/// tool call after `record_decision(approved = true)`: the decision is appended
/// to history and the request is dropped from the queue (`safety.rs`), and no
/// caller re-runs the original action. So the gate tells the agent to ask, and
/// the agent re-attempts the work itself once a human answers.
fn tier_refusal(tool: &str) -> String {
    format!(
        "Tool '{tool}' requires user permission in an unattended session. \
         Call request_permission with action='{tool}' and wait for the user's \
         decision before retrying."
    )
}

/// Decide whether an unattended agent may run `tool` without asking first.
///
/// Returns `Err` with the refusal text when the action is tier 2. Interactive
/// sessions are never gated: only sessions registered via
/// `register_ambient_session` are unattended.
pub(crate) fn check_ambient_action_tier(session_id: &str, tool: &str) -> Result<()> {
    if !is_ambient_session_registered(session_id) {
        return Ok(());
    }
    if TIER_GATE_EXEMPT.contains(&tool) {
        return Ok(());
    }
    match get_safety_system().classify(tool) {
        ActionTier::AutoAllowed => Ok(()),
        ActionTier::RequiresPermission => Err(anyhow::anyhow!(tier_refusal(tool))),
    }
}

pub(super) fn ensure_ambient_session(ctx: &ToolContext) -> Result<()> {
    if is_ambient_session_registered(&ctx.session_id) {
        Ok(())
    } else {
        anyhow::bail!(
            "request_permission is only available to ambient sessions (session '{}')",
            ctx.session_id
        )
    }
}
