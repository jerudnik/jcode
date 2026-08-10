#![allow(
    unknown_lints,
    clippy::collapsible_match,
    clippy::manual_checked_ops,
    clippy::unnecessary_sort_by,
    clippy::useless_conversion
)]

//! Presentation layer for jcode (terminal UI + offline replay export).
//!
//! This crate holds the `tui` and `video_export` modules that were extracted
//! out of the monolithic root `jcode` crate so they compile as a separate
//! rustc unit. The application core it builds on (server, agent, provider,
//! auth, session, tool, config, ...) lives in `jcode-app-core` and is
//! re-exported here as an explicit module surface, so the TUI can keep using
//! declared `crate::<module>` paths without a wildcard export. The root
//! `jcode` crate (cli + bin) re-exports this crate through an explicit
//! compatibility list.

// Application core: re-export the declared `jcode-app-core` modules (which
// itself re-exports `jcode-base`) so `crate::<module>` paths resolve here
// exactly as they did before the split.
pub use jcode_app_core::{
    agent, ambient, ambient_runner, ambient_scheduler, auth, background, browser, build, bus,
    cache_invalidation, cache_tracker, catchup, channel, client_input, compaction, config,
    copilot_usage, dictation, embedding, embedding_backend, env, external_auth, gateway,
    generated_image, get_current_session, gmail, goal, herdr, hooks, id, import, live_tests,
    logging, login_qr, mcp, memory, memory_agent, memory_graph, memory_judge_metrics, memory_log,
    memory_rerank, memory_types, message, mission, mobile_server, model_pricing, network_retry,
    notifications, overnight, perf, plan, platform, power_inhibit, process_memory, process_title,
    prompt, protocol, provider, provider_activity, provider_catalog, registry, replay,
    restart_snapshot, runtime_memory_log, safety, secret_input, server, server_spawn, session,
    session_effort, session_launch, session_list_cache, session_metrics, session_rebuild,
    set_current_session, setup_hints, side_panel, sidecar, skill, soft_interrupt_store, sponsors,
    ssh_remote, startup_profile, stdin_detect, storage, subscription_api, subscription_catalog,
    surface_workspace, swarm_verbs, telegram, telemetry, terminal_launch, todo, tool, transport,
    update, usage, util,
};

// Presentation layer (kept in this crate).
pub mod tui;
pub mod video_export;
mod video_export_box_drawing;
