use crate::DisplayMessage;
use jcode_config_types::{DiagramDisplayMode, DiffDisplayMode};
use ratatui::layout::Alignment;
use ratatui::text::{Line, Span};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MessageCacheKey {
    /// Fingerprint of the runtime isolation context (session + workspace +
    /// SCHEMA_VERSION). Folded into the key so two sessions or two workspaces
    /// served by the same long-lived TUI process never share a render-cache
    /// hit, even when their (message_hash, width, content_len, ...) tuples
    /// otherwise collide.
    ///
    /// MESSAGE_CACHE is render-only so trust_tier / provider / model are
    /// intentionally not folded in here (caller passes `0` for those via
    /// IsolationKey::context_fingerprint).
    isolation_fp: u64,
    width: u16,
    diff_mode: DiffDisplayMode,
    message_hash: u64,
    content_len: usize,
    diagram_mode: DiagramDisplayMode,
    centered: bool,
    mermaid_epoch: u64,
    mermaid_aspect_bucket: Option<u16>,
}

#[derive(Default)]
struct MessageCacheState {
    entries: HashMap<MessageCacheKey, Arc<Vec<Line<'static>>>>,
    order: VecDeque<MessageCacheKey>,
}

impl MessageCacheState {
    fn get(&self, key: &MessageCacheKey) -> Option<Vec<Line<'static>>> {
        self.entries.get(key).map(|arc| arc.as_ref().clone())
    }

    fn insert(&mut self, key: MessageCacheKey, lines: Vec<Line<'static>>) {
        let arc = Arc::new(lines);
        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            self.entries.entry(key.clone())
        {
            entry.insert(arc);
            return;
        }

        self.entries.insert(key.clone(), arc);
        self.order.push_back(key);

        while self.order.len() > MESSAGE_CACHE_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

static MESSAGE_CACHE: OnceLock<Mutex<MessageCacheState>> = OnceLock::new();

fn message_cache() -> &'static Mutex<MessageCacheState> {
    MESSAGE_CACHE.get_or_init(|| Mutex::new(MessageCacheState::default()))
}

const MESSAGE_CACHE_LIMIT: usize = 2048;

/// Runtime-sensitive inputs that affect message rendering but are not intrinsic to a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MessageCacheContext {
    pub diagram_mode: DiagramDisplayMode,
    pub centered: bool,
    pub mermaid_epoch: u64,
    pub mermaid_aspect_bucket: Option<u16>,
    /// Fingerprint of the (session, workspace) pair this render is for.
    /// Compute with `IsolationKey::for_session(...).context_fingerprint()`
    /// (or equivalent). The render cache folds this into every lookup so a
    /// long-lived TUI process cannot serve a Line-vec rendered for session
    /// A or workspace X to session B or workspace Y on a hash collision.
    pub isolation_fp: u64,
}

pub fn left_pad_lines_for_centered_mode(lines: &mut [Line<'static>], width: u16) {
    let max_line_width = lines.iter().map(Line::width).max().unwrap_or(0);
    let pad = (width as usize).saturating_sub(max_line_width) / 2;
    if pad == 0 {
        return;
    }

    let pad_str = " ".repeat(pad);
    for line in lines {
        line.spans.insert(0, Span::raw(pad_str.clone()));
        line.alignment = Some(Alignment::Left);
    }
}

pub fn centered_wrap_width(width: u16, centered: bool, centered_max_width: usize) -> usize {
    let width = width as usize;
    if centered {
        width.min(centered_max_width).max(1)
    } else {
        width.max(1)
    }
}

pub fn get_cached_message_lines<F>(
    msg: &DisplayMessage,
    width: u16,
    diff_mode: DiffDisplayMode,
    context: MessageCacheContext,
    render: F,
) -> Vec<Line<'static>>
where
    F: FnOnce(&DisplayMessage, u16, DiffDisplayMode) -> Vec<Line<'static>>,
{
    if cfg!(test) {
        return render(msg, width, diff_mode);
    }

    let key = MessageCacheKey {
        isolation_fp: context.isolation_fp,
        width,
        diff_mode,
        message_hash: msg.stable_cache_hash(),
        content_len: msg.content.len(),
        diagram_mode: context.diagram_mode,
        centered: context.centered,
        mermaid_epoch: context.mermaid_epoch,
        mermaid_aspect_bucket: context.mermaid_aspect_bucket,
    };

    let mut cache = match message_cache().lock() {
        Ok(c) => c,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(lines) = cache.get(&key) {
        return lines;
    }

    let lines = render(msg, width, diff_mode);
    cache.insert(key, lines.clone());
    lines
}

/// Drop every entry from the static `MESSAGE_CACHE`.
///
/// Used by the TUI backend as an explicit invalidation hook on
/// session-resume / workspace-switch / provider-or-model-change events
/// (TASK-89 AC#3). Entries from a previous session/workspace would
/// otherwise linger until they were naturally evicted by the LRU bound
/// (`MESSAGE_CACHE_LIMIT`). Stale entries are already _safe_ — they can
/// only be served back when the caller's `MessageCacheContext.isolation_fp`
/// happens to match, which by construction (TASK-89 AC#2) only occurs
/// inside the same (session, workspace) — but eager clearing keeps memory
/// pressure proportional to the active session and makes the invariant
/// easy to reason about.
///
/// Cheap: takes the cache mutex once and drops both the `HashMap` and the
/// `VecDeque` LRU spine.
pub fn clear_message_cache() {
    let mut cache = match message_cache().lock() {
        Ok(c) => c,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.entries.clear();
    cache.order.clear();
}

/// Drop every entry from the static `MESSAGE_CACHE` whose
/// `isolation_fp` matches `isolation_fp`.
///
/// Surgical sibling of `clear_message_cache` for the workspace-switch hook
/// (TASK-89 AC#3): when switching _away from_ a known prior workspace we
/// want to drop only its entries and leave entries for the now-active
/// workspace intact. Compute `isolation_fp` from the prior
/// `IsolationKey::context_fingerprint()`.
pub fn clear_message_cache_for_isolation(isolation_fp: u64) {
    let mut cache = match message_cache().lock() {
        Ok(c) => c,
        Err(poisoned) => poisoned.into_inner(),
    };
    cache.entries.retain(|k, _| k.isolation_fp != isolation_fp);
    cache.order.retain(|k| k.isolation_fp != isolation_fp);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_wrap_width_caps_centered_width() {
        assert_eq!(centered_wrap_width(120, true, 96), 96);
        assert_eq!(centered_wrap_width(80, true, 96), 80);
        assert_eq!(centered_wrap_width(120, false, 96), 120);
    }

    #[test]
    fn left_pad_lines_aligns_to_centered_block() {
        let mut lines = vec![Line::from("abc")];
        left_pad_lines_for_centered_mode(&mut lines, 9);
        assert_eq!(lines[0].to_string(), "   abc");
        assert_eq!(lines[0].alignment, Some(Alignment::Left));
    }

    /// TASK-89 AC#2/AC#4: a different `isolation_fp` must produce a different
    /// `MessageCacheKey`, so the static MESSAGE_CACHE never serves a render
    /// from session/workspace A back to session/workspace B even when every
    /// other key component (message_hash, width, diff_mode, content_len,
    /// diagram_mode, centered, mermaid_*) matches.
    #[test]
    fn message_cache_key_isolates_by_isolation_fp() {
        fn key(isolation_fp: u64) -> MessageCacheKey {
            MessageCacheKey {
                isolation_fp,
                width: 80,
                diff_mode: DiffDisplayMode::default(),
                message_hash: 0xDEAD_BEEF,
                content_len: 42,
                diagram_mode: DiagramDisplayMode::default(),
                centered: false,
                mermaid_epoch: 0,
                mermaid_aspect_bucket: None,
            }
        }
        let a = key(1);
        let b = key(2);
        let a2 = key(1);
        assert_ne!(a, b, "different isolation_fp must produce different keys");
        assert_eq!(a, a2, "same isolation_fp must produce equal keys");

        use std::collections::HashMap;
        let mut map: HashMap<MessageCacheKey, &'static str> = HashMap::new();
        map.insert(a.clone(), "session-A");
        map.insert(b.clone(), "session-B");
        assert_eq!(map.get(&a), Some(&"session-A"));
        assert_eq!(map.get(&b), Some(&"session-B"));
        // sanity: same-fp lookup hits the existing entry
        assert_eq!(map.get(&a2), Some(&"session-A"));
    }

    fn dummy_key(isolation_fp: u64) -> MessageCacheKey {
        MessageCacheKey {
            isolation_fp,
            width: 80,
            diff_mode: DiffDisplayMode::default(),
            message_hash: 0xABCD,
            content_len: 1,
            diagram_mode: DiagramDisplayMode::default(),
            centered: false,
            mermaid_epoch: 0,
            mermaid_aspect_bucket: None,
        }
    }

    /// Serialize tests that mutate the process-wide `MESSAGE_CACHE`
    /// static so cargo's default parallel test runner cannot interleave
    /// them and observe each other's leftover entries.
    fn message_cache_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// TASK-89 AC#3: `clear_message_cache` must drop every entry from
    /// the static MESSAGE_CACHE regardless of `isolation_fp` so a
    /// session-resume or process-wide invalidation event reliably
    /// reclaims memory.
    #[test]
    fn clear_message_cache_drops_all_entries() {
        let _guard = message_cache_test_lock();
        // Reset baseline; another test may have left entries behind.
        clear_message_cache();
        {
            let mut cache = message_cache().lock().unwrap();
            cache.insert(dummy_key(1), vec![Line::from("a")]);
            cache.insert(dummy_key(2), vec![Line::from("b")]);
            assert_eq!(cache.entries.len(), 2);
            assert_eq!(cache.order.len(), 2);
        }
        clear_message_cache();
        let cache = message_cache().lock().unwrap();
        assert!(cache.entries.is_empty(), "entries map must be empty");
        assert!(cache.order.is_empty(), "LRU spine must be empty");
    }

    /// TASK-89 AC#3: `clear_message_cache_for_isolation` must drop only
    /// entries whose `isolation_fp` matches the supplied value and leave
    /// every other entry intact — the surgical workspace-switch sibling
    /// of `clear_message_cache`.
    #[test]
    fn clear_message_cache_for_isolation_drops_only_matching_fp() {
        let _guard = message_cache_test_lock();
        clear_message_cache();
        {
            let mut cache = message_cache().lock().unwrap();
            cache.insert(dummy_key(11), vec![Line::from("keep")]);
            cache.insert(dummy_key(22), vec![Line::from("drop")]);
            cache.insert(dummy_key(33), vec![Line::from("keep")]);
            assert_eq!(cache.entries.len(), 3);
        }
        clear_message_cache_for_isolation(22);
        let cache = message_cache().lock().unwrap();
        assert_eq!(cache.entries.len(), 2, "only one entry should be dropped");
        assert!(
            cache.entries.contains_key(&dummy_key(11)),
            "isolation_fp=11 entry must survive"
        );
        assert!(
            cache.entries.contains_key(&dummy_key(33)),
            "isolation_fp=33 entry must survive"
        );
        assert!(
            !cache.entries.contains_key(&dummy_key(22)),
            "isolation_fp=22 entry must be evicted"
        );
        assert_eq!(
            cache.order.len(),
            2,
            "LRU spine must mirror entries map after surgical clear"
        );
        assert!(
            cache.order.iter().all(|k| k.isolation_fp != 22),
            "LRU spine must not retain dropped isolation_fp"
        );
    }

    /// TASK-89 AC#4 (integration-style): simulate a session resume across
    /// two workspaces and confirm no foreign content reaches projection.
    ///
    /// Scenario:
    /// 1. Workspace X (session A, isolation_fp = FP_AX) caches a rendered
    ///    `message_hash = H` -> "lines-from-A".
    /// 2. Workspace Y (session B, isolation_fp = FP_BY) attempts a lookup
    ///    of the same `H` (intentional content-hash collision across
    ///    workspaces / sessions, e.g. both sessions touched the same
    ///    file). The lookup MUST miss — even on collision, the cache
    ///    refuses to serve A's render into B's projection.
    /// 3. Session A is "resumed" via the same hook the server calls
    ///    (`clear_message_cache()` — the implementation behind
    ///    `cache_invalidation::on_session_resume`). After the hook,
    ///    A's prior render is gone, so a re-render under FP_AX would
    ///    naturally call the render closure rather than returning a
    ///    stale Line-vec from a pre-resume frame.
    ///
    /// This proves the runtime cache layer enforces the AC#2 key axis
    /// (isolation_fp) AND the AC#3 invalidation hook end-to-end without
    /// any cross-bleed even when content_hash collides.
    #[test]
    fn session_resume_across_workspaces_blocks_foreign_render_bleed() {
        let _guard = message_cache_test_lock();
        clear_message_cache();

        // Fingerprints crafted to be unequal but otherwise arbitrary;
        // in production these come from
        // `IsolationKey::for_session(...).context_fingerprint()`.
        const FP_AX: u64 = 0xA1A1_0000_0000_0001; // session A, workspace X
        const FP_BY: u64 = 0xB2B2_0000_0000_0002; // session B, workspace Y
        const COLLIDING_MESSAGE_HASH: u64 = 0xDEAD_BEEF_CAFE_BABE;

        let key_a = MessageCacheKey {
            isolation_fp: FP_AX,
            width: 80,
            diff_mode: DiffDisplayMode::default(),
            message_hash: COLLIDING_MESSAGE_HASH,
            content_len: 42,
            diagram_mode: DiagramDisplayMode::default(),
            centered: false,
            mermaid_epoch: 0,
            mermaid_aspect_bucket: None,
        };
        let key_b = MessageCacheKey {
            isolation_fp: FP_BY,
            // Same content axes as key_a — only isolation_fp differs.
            ..key_a.clone()
        };

        // Step 1: session A in workspace X populates the cache.
        {
            let mut cache = message_cache().lock().unwrap();
            cache.insert(key_a.clone(), vec![Line::from("lines-from-session-A")]);
            assert_eq!(cache.entries.len(), 1);
        }

        // Step 2: session B in workspace Y looks up the same content
        // hash. The key differs only in isolation_fp, so the cache MUST
        // miss — no foreign content reaches B's projection.
        {
            let cache = message_cache().lock().unwrap();
            assert!(
                cache.get(&key_b).is_none(),
                "cross-isolation lookup with colliding message_hash must miss"
            );
            // Sanity: A can still hit its own entry.
            assert!(
                cache.get(&key_a).is_some(),
                "same-isolation lookup must hit"
            );
        }

        // Step 3: session A is resumed. The server-side hook
        // (`cache_invalidation::on_session_resume`) calls
        // `clear_message_cache()` so any pre-resume render frames are
        // dropped. Verify A's entry is gone afterwards.
        clear_message_cache();
        let cache = message_cache().lock().unwrap();
        assert!(
            cache.get(&key_a).is_none(),
            "post-resume lookup must miss — pre-resume frame must not bleed across the resume boundary"
        );
        assert!(
            cache.entries.is_empty(),
            "on_session_resume hook must reclaim all entries"
        );
    }
}
