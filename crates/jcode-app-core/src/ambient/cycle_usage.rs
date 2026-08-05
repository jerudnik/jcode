//! What one ambient cycle spent, carried from the agent that spent it to the
//! scheduler's usage log.
//!
//! These two live at different depths of the runner: the agent is local to
//! `run_cycle`, which has five exits, while the log belongs to the scheduler in
//! `run_loop`. Rather than widen `AmbientCycleResult`, which is serialized to
//! disk and constructed in a dozen places, the runner parks the numbers here and
//! the loop drains them once per cycle.

use crate::agent::Agent;
use crate::ambient_scheduler::{UsageRecord, UsageSource};
use crate::provider::Provider;
use chrono::Utc;
use std::sync::Arc;

/// Tokens one ambient cycle spent, as reported by the provider.
pub(super) struct CycleUsage {
    input_tokens: u64,
    output_tokens: u64,
    provider: String,
}

impl CycleUsage {
    /// Read what the cycle spent, or `None` when the provider reported no usage
    /// at all, which is the case for a cycle that never reached the model.
    ///
    /// The totals come from the session rather than `Agent::last_usage` because
    /// one cycle is many provider calls and the log wants the whole cycle.
    pub(super) fn from_agent(agent: &Agent, provider: &Arc<dyn Provider>) -> Option<Self> {
        let totals = agent.token_usage_totals();
        if totals.messages_with_token_usage == 0 {
            return None;
        }
        Some(CycleUsage {
            input_tokens: totals.input_tokens,
            output_tokens: totals.output_tokens,
            provider: provider.name().to_string(),
        })
    }

    /// Shape this as the log's own record type.
    pub(super) fn into_record(self) -> UsageRecord {
        UsageRecord {
            timestamp: Utc::now(),
            source: UsageSource::Ambient,
            // The log's fields are u32; saturate rather than wrap, so a huge
            // cycle reads as huge instead of as nearly free.
            tokens_input: self.input_tokens.try_into().unwrap_or(u32::MAX),
            tokens_output: self.output_tokens.try_into().unwrap_or(u32::MAX),
            provider: self.provider,
        }
    }
}
