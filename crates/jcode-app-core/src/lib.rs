#![allow(
    unknown_lints,
    clippy::collapsible_match,
    clippy::manual_checked_ops,
    clippy::unnecessary_sort_by,
    clippy::useless_conversion
)]
// The `swarm` tool's `json!` parameter schema is large; the default macro
// recursion limit (128) is exceeded once more properties are added.
#![recursion_limit = "256"]

//! Application core for jcode (upper layer).
//!
//! This crate holds the server/tool/agent layer and its presentation-adjacent
//! leaves. The foundational layer it builds on lives in the `jcode-base` crate
//! and is imported here explicitly, module by module, so the upper layer owns a
//! small, readable compatibility surface instead of a wildcard passthrough.

// Foundational layer exports. Keep the list explicit so the public API stays
// narrow while `crate::<module>` paths inside app-core continue to resolve.
pub use jcode_base::{
    auth, background, browser, bus, cache_invalidation, cache_tracker, client_input, compaction,
    config, copilot_usage, dictation, embedding_backend, env, gateway, generated_image, gmail,
    goal, hooks, id, import, inbox, live_tests, logging, login_qr, mcp, memory, memory_agent,
    memory_graph, memory_judge_metrics, memory_log, memory_rerank, memory_types, message,
    mobile_server, model_pricing, plan, platform, power_inhibit, process_memory, process_title,
    prompt, protocol, provider, provider_activity, provider_catalog, registry, runtime_memory_log,
    safety, secret_input, session, session_list_cache, session_metrics, side_panel, sidecar, skill,
    soft_interrupt_store, sponsors, stdin_detect, storage, subscription_api, subscription_catalog,
    surface_workspace, telegram, telemetry, terminal_launch, todo, transport, usage, util,
};

#[cfg(feature = "embeddings")]
pub use jcode_base::embedding;
#[cfg(not(feature = "embeddings"))]
pub use jcode_base::embedding_stub as embedding;

// Upper layer (server / tool / agent and supporting leaves).
pub mod agent;
pub mod ambient;
pub mod ambient_runner;
pub mod ambient_scheduler;
pub mod build;
pub mod catchup;
pub mod channel;
pub mod external_auth;
pub mod herdr;
pub mod mission;
pub mod network_retry;
pub mod notifications;
pub mod overnight;
pub mod perf;
pub mod replay;
pub mod restart_snapshot;
pub mod server;
pub mod server_spawn;
pub mod session_effort;
pub mod session_launch;
pub mod session_rebuild;
pub mod setup_hints;
pub mod ssh_remote;
pub mod startup_profile;
pub mod swarm_verbs;
pub mod tool;
pub mod turn_cancel_registry;
pub mod update;

#[cfg(test)]
#[path = "recovery_pilot_tests.rs"]
mod agent_tests;

use std::sync::Mutex;

static CURRENT_SESSION_ID: Mutex<Option<String>> = Mutex::new(None);

pub fn set_current_session(session_id: &str) {
    if let Ok(mut guard) = CURRENT_SESSION_ID.lock() {
        *guard = Some(session_id.to_string());
    }
}

pub fn get_current_session() -> Option<String> {
    CURRENT_SESSION_ID.lock().ok()?.clone()
}
