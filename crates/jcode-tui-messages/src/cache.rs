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
}
