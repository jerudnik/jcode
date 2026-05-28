//! Explicit invalidation hooks for the runtime cache layers covered by
//! TASK-89.
//!
//! AC#2 (cache-isolation) ensured that a stale entry from session A,
//! workspace X, or schema-version N can never be _served_ to session B,
//! workspace Y, or schema-version N+1: keys carry enough discriminator
//! that a lookup with the wrong context misses. AC#3 (this module) adds
//! _explicit_ invalidation hooks so that on well-known cross-cutting
//! events the caches are eagerly cleared instead of waiting for natural
//! LRU eviction.
//!
//! Events that warrant explicit invalidation:
//! - **session resume** — long-lived server process attaches to a new
//!   session id; per-session render entries from the prior session are
//!   no longer useful and should release their memory.
//! - **model/provider change** — a different active model namespace
//!   means a different provider-catalog topology; in-memory disk-cache
//!   memos rooted at the prior provider's path are now cold and the
//!   semantic-embedding fingerprint may also shift.
//!
//! All hooks are infallible and cheap (single mutex acquire + drop of a
//! `HashMap`/`VecDeque`).

use crate::memory::clear_graph_cache;
use jcode_provider_openrouter::clear_disk_cache_memos as clear_openrouter_disk_memos;
use jcode_tui_messages::clear_message_cache;

/// Invalidate every per-session render or per-path graph cache layer
/// after a session-resume event.
///
/// Does **not** touch the semantic-embed cache: that one is owned by
/// `CompactionManager` and is already invalidated by
/// `CompactionManager::reset()` on the new session's compaction
/// initialization path.
pub(crate) fn on_session_resume() {
    clear_message_cache();
    clear_graph_cache();
}

/// Invalidate disk-cache memos and TUI render caches after the active
/// provider or model changes.
///
/// The TUI render cache is folded against
/// `IsolationKey::context_fingerprint()` which already excludes provider
/// for render purposes; we clear it anyway as defense in depth because
/// model-driven rendering paths (e.g. provider-specific tool result
/// formatting) may grow in the future.
pub(crate) fn on_provider_or_model_change() {
    clear_message_cache();
    clear_openrouter_disk_memos();
}
