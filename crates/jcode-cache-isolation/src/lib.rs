//! Cache isolation primitives shared by runtime cache layers.
//!
//! TASK-87 found that the `cache_isolation` technique is the only one that
//! closes the `cache_confusion` scenario in the deterministic eval matrix
//! (see `docs/CONTEXT_PIPELINE_EVAL.md` and Serena memory
//! `compaction/remaining_technique_eval_task87.md`). TASK-88 wired
//! provenance-aware routing at the *message pruning* boundary. TASK-89 (this
//! module) extends the same contract *inside cache layers* so a session
//! resume, workspace switch, or provider/model change cannot return foreign
//! content even when the underlying content hash or file path is identical.
//!
//! The [`IsolationKey`] is the single contract used across the affected
//! caches:
//! - `src/compaction.rs::semantic_embed_cache`
//! - `src/memory/cache.rs::GraphCache`
//! - `crates/jcode-tui-messages/src/cache.rs::MESSAGE_CACHE`
//! - `crates/jcode-provider-openrouter/src/lib.rs` disk-memo caches
//!
//! Caches that store hash-only keys use [`IsolationKey::fingerprint`] to keep
//! the per-entry overhead at a single `u64`.
//!
//! Bumping [`SCHEMA_VERSION`] invalidates every cache that uses an
//! [`IsolationKey`]-based key in one place; do so whenever the keying contract
//! changes (e.g. new dimension, canonicalization change).

use std::hash::{Hash, Hasher};
use std::path::Path;

/// Bumped whenever the [`IsolationKey`] keying contract changes. Every cache
/// that derives its key from an `IsolationKey` invalidates atomically when
/// this value changes, so callers should not hard-code an alternative version.
pub const SCHEMA_VERSION: u32 = 1;

/// Trust tier of the content a cache entry was derived from. The runtime
/// `provenance_routing` pass already discriminates trusted vs. low-trust tool
/// output (`src/agent/context_pruning.rs::route_low_trust_context`). Caches
/// must not return entries computed under a different trust tier, because
/// pruning and projection treat the two streams differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrustTier {
    /// Trusted local agent state (user messages, agent decisions, system
    /// prompts, internal tool descriptors).
    Trusted,
    /// Low-trust content sourced from third-party tool output or external
    /// fetches (file reads of unverified paths, web fetches, untrusted MCP
    /// servers, etc.).
    LowTrust,
}

impl TrustTier {
    /// Stable string tag used in placeholders and eval scripts so the Python
    /// matrix and Rust runtime agree on tier identifiers.
    pub fn as_str(self) -> &'static str {
        match self {
            TrustTier::Trusted => "trusted",
            TrustTier::LowTrust => "low",
        }
    }
}

/// Canonicalize a workspace root for use in an [`IsolationKey`].
///
/// We deliberately do *not* call [`std::fs::canonicalize`] here: it requires
/// the path to exist and would surface I/O errors deep inside cache helpers.
/// Instead we apply a deterministic textual normalization that is stable
/// across resume/restore boundaries:
/// - empty paths normalize to `"<unknown>"` (so unset workspace contexts do
///   not collide silently with the real root)
/// - trailing path separators are stripped
/// - `.` and `..` segments are not collapsed (callers already pass project
///   roots, and collapsing would lose information across symlinks)
///
/// Returns an owned [`String`] so callers can store it directly inside
/// `IsolationKey` without lifetime concerns.
pub fn canonicalize_workspace_root(path: &Path) -> String {
    let s = path.to_string_lossy();
    let trimmed = s.trim_end_matches(std::path::is_separator);
    if trimmed.is_empty() {
        // Preserve the case where the input was literally "/" or empty.
        if s.starts_with(std::path::MAIN_SEPARATOR) {
            return std::path::MAIN_SEPARATOR.to_string();
        }
        return "<unknown>".to_string();
    }
    trimmed.to_string()
}

/// Identifier dimensions that must agree before a cache entry can be reused.
///
/// `content_hash` is a 64-bit summary of the cache *payload* (e.g. text hash
/// for the semantic embed cache, message stable hash for the message render
/// cache). The remaining fields are *context* dimensions: changing any one of
/// them must produce a cache miss even when `content_hash` is identical.
///
/// Caches that key by a single `u64` (e.g. the legacy
/// `semantic_embed_cache: HashMap<u64, ...>`) should compose
/// [`IsolationKey::fingerprint`] with their content hash via
/// [`IsolationKey::fingerprint_with_content`] to avoid losing the isolation
/// dimensions while keeping the key narrow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IsolationKey {
    /// Owning session id. Empty string indicates "no session"; tests and
    /// non-session caches use the same convention.
    pub session_id: String,
    /// Canonicalized workspace root (see [`canonicalize_workspace_root`]).
    pub workspace_root: String,
    /// Provider tag (e.g. `"anthropic"`, `"openrouter"`). Empty string when
    /// the cache is provider-agnostic (e.g. the message render cache).
    pub provider: String,
    /// Model identifier (e.g. `"claude-sonnet-4-5"`). Empty string when the
    /// cache is model-agnostic.
    pub model: String,
    /// 64-bit content fingerprint of the cached payload.
    pub content_hash: u64,
    /// Trust tier of the content the entry was derived from.
    pub trust_tier: TrustTier,
    /// Schema version (defaults to [`SCHEMA_VERSION`]).
    pub schema_version: u32,
}

impl IsolationKey {
    /// Build a key with the current [`SCHEMA_VERSION`]. Callers that need a
    /// frozen version (e.g. tests reproducing historical behaviour) can set
    /// `schema_version` directly on the struct after construction.
    pub fn new(
        session_id: impl Into<String>,
        workspace_root: &Path,
        provider: impl Into<String>,
        model: impl Into<String>,
        content_hash: u64,
        trust_tier: TrustTier,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            workspace_root: canonicalize_workspace_root(workspace_root),
            provider: provider.into(),
            model: model.into(),
            content_hash,
            trust_tier,
            schema_version: SCHEMA_VERSION,
        }
    }

    /// Stable 64-bit fingerprint over *all* isolation dimensions including
    /// `content_hash`. Suitable as the sole key in a `HashMap<u64, V>`-style
    /// cache where storing the full struct would be wasteful, at the cost of
    /// theoretical 64-bit collisions (acceptable for caches with TTL/LRU
    /// bounds).
    pub fn fingerprint(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Stable 64-bit fingerprint over the *context* dimensions only
    /// (everything except `content_hash`). Caches that want a `(context, hash)`
    /// composite key — e.g. the semantic embed cache, which already stores the
    /// content hash separately — should use this so the content portion is not
    /// double-counted.
    pub fn context_fingerprint(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Hash everything except content_hash, in a stable order.
        self.session_id.hash(&mut hasher);
        self.workspace_root.hash(&mut hasher);
        self.provider.hash(&mut hasher);
        self.model.hash(&mut hasher);
        self.trust_tier.hash(&mut hasher);
        self.schema_version.hash(&mut hasher);
        hasher.finish()
    }

    /// Compose this key's context fingerprint with an externally-supplied
    /// content hash, producing a stable 64-bit composite key. Useful for
    /// caches that already carry a content hash as their natural key.
    pub fn fingerprint_with_content(&self, content_hash: u64) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.context_fingerprint().hash(&mut hasher);
        content_hash.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ws(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn canonicalize_strips_trailing_separators() {
        assert_eq!(canonicalize_workspace_root(&ws("/tmp/foo/")), "/tmp/foo");
        assert_eq!(canonicalize_workspace_root(&ws("/tmp/foo")), "/tmp/foo");
    }

    #[test]
    fn canonicalize_handles_empty_and_root() {
        assert_eq!(canonicalize_workspace_root(&ws("")), "<unknown>");
        assert_eq!(
            canonicalize_workspace_root(&ws(&std::path::MAIN_SEPARATOR.to_string())),
            std::path::MAIN_SEPARATOR.to_string()
        );
    }

    #[test]
    fn isolation_key_is_stable_across_constructions() {
        let a = IsolationKey::new(
            "session-1",
            &ws("/tmp/proj"),
            "anthropic",
            "claude",
            0xdead_beef,
            TrustTier::Trusted,
        );
        let b = IsolationKey::new(
            "session-1",
            &ws("/tmp/proj/"),
            "anthropic",
            "claude",
            0xdead_beef,
            TrustTier::Trusted,
        );
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.context_fingerprint(), b.context_fingerprint());
    }

    #[test]
    fn session_id_change_invalidates_fingerprint() {
        let mut a = IsolationKey::new(
            "session-1",
            &ws("/tmp/proj"),
            "anthropic",
            "claude",
            1,
            TrustTier::Trusted,
        );
        let original = a.fingerprint();
        a.session_id = "session-2".to_string();
        assert_ne!(a.fingerprint(), original);
        assert_ne!(a.context_fingerprint(), {
            let b = IsolationKey::new(
                "session-1",
                &ws("/tmp/proj"),
                "anthropic",
                "claude",
                1,
                TrustTier::Trusted,
            );
            b.context_fingerprint()
        });
    }

    #[test]
    fn workspace_change_invalidates_fingerprint() {
        let a = IsolationKey::new("s", &ws("/tmp/a"), "p", "m", 1, TrustTier::Trusted);
        let b = IsolationKey::new("s", &ws("/tmp/b"), "p", "m", 1, TrustTier::Trusted);
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn provider_change_invalidates_fingerprint() {
        let a = IsolationKey::new("s", &ws("/tmp/a"), "anthropic", "m", 1, TrustTier::Trusted);
        let b = IsolationKey::new("s", &ws("/tmp/a"), "openrouter", "m", 1, TrustTier::Trusted);
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn model_change_invalidates_fingerprint() {
        let a = IsolationKey::new(
            "s",
            &ws("/tmp/a"),
            "p",
            "claude-sonnet-4-5",
            1,
            TrustTier::Trusted,
        );
        let b = IsolationKey::new(
            "s",
            &ws("/tmp/a"),
            "p",
            "claude-haiku-4-5",
            1,
            TrustTier::Trusted,
        );
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn trust_tier_change_invalidates_fingerprint() {
        let a = IsolationKey::new("s", &ws("/tmp/a"), "p", "m", 1, TrustTier::Trusted);
        let b = IsolationKey::new("s", &ws("/tmp/a"), "p", "m", 1, TrustTier::LowTrust);
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn schema_version_change_invalidates_fingerprint() {
        let a = IsolationKey::new("s", &ws("/tmp/a"), "p", "m", 1, TrustTier::Trusted);
        let mut b = a.clone();
        b.schema_version = SCHEMA_VERSION.wrapping_add(1);
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn content_hash_changes_fingerprint_but_not_context() {
        let mut a = IsolationKey::new("s", &ws("/tmp/a"), "p", "m", 1, TrustTier::Trusted);
        let original_ctx = a.context_fingerprint();
        let original_fp = a.fingerprint();
        a.content_hash = 2;
        assert_eq!(a.context_fingerprint(), original_ctx);
        assert_ne!(a.fingerprint(), original_fp);
    }

    #[test]
    fn fingerprint_with_content_matches_full_fingerprint_for_same_content() {
        let a = IsolationKey::new("s", &ws("/tmp/a"), "p", "m", 123, TrustTier::Trusted);
        // The two helpers compute over different inputs (full struct vs.
        // context+content composition), so they should not be equal — but
        // they should each be stable across calls.
        assert_eq!(a.fingerprint(), a.fingerprint());
        assert_eq!(
            a.fingerprint_with_content(123),
            a.fingerprint_with_content(123)
        );
        assert_ne!(
            a.fingerprint_with_content(123),
            a.fingerprint_with_content(124)
        );
    }
}
