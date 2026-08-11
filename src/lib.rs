#![allow(
    unknown_lints,
    clippy::collapsible_match,
    clippy::manual_checked_ops,
    clippy::unnecessary_sort_by,
    clippy::useless_conversion
)]

//! Root `jcode` crate: the entrypoint + cli layer on top of the `jcode-tui`
//! presentation crate (which in turn re-exports `jcode-app-core` and
//! `jcode-base`).
//!
//! The presentation modules (`tui`, `video_export`) live in `jcode-tui` and the
//! non-presentation modules live in `jcode-app-core`. The root crate keeps an
//! explicit compatibility surface for the modules that remain part of the
//! supported public API, instead of wildcard-exporting the whole presentation
//! crate.

// Explicit compatibility surface for the modules the root crate still exposes.
pub use jcode_tui::{
    agent, ambient, ambient_runner, ambient_scheduler, auth, background, browser, build, bus,
    cache_invalidation, cache_tracker, catchup, channel, client_input, compaction, config,
    copilot_usage, dictation, embedding, embedding_backend, env, external_auth, gateway,
    generated_image, gmail, goal, herdr, hooks, id, import, live_tests, logging, login_qr, mcp,
    memory, memory_agent, memory_graph, memory_judge_metrics, memory_log, memory_rerank,
    memory_types, message, mission, mobile_server, model_pricing, network_retry, notifications,
    overnight, perf, plan, platform, power_inhibit, process_memory, process_title, prompt,
    protocol, provider, provider_activity, provider_catalog, registry, replay, restart_snapshot,
    runtime_memory_log, safety, secret_input, server, server_spawn, session, session_effort,
    session_launch, session_list_cache, session_metrics, session_rebuild, setup_hints, side_panel,
    sidecar, skill, soft_interrupt_store, sponsors, ssh_remote, startup_profile, stdin_detect,
    storage, subscription_api, subscription_catalog, surface_workspace, swarm_verbs, telegram,
    telemetry, terminal_launch, todo, tool, transport, tui, update, usage, util, video_export,
};

pub use jcode_tui::{get_current_session, set_current_session};

// Cli + entrypoint layer (kept in the root crate).
pub mod cli;

use anyhow::Result;

pub async fn run() -> Result<()> {
    cli::startup::run().await
}
