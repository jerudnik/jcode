use crate::message::{ContentBlock, Message};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

const RECENT_MESSAGES_TO_PROTECT: usize = 12;
const ERROR_INPUT_PRUNE_AFTER_MESSAGES: usize = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct PruneStats {
    pub duplicate_tool_results: usize,
    pub stale_error_inputs: usize,
    pub chars_saved: usize,
}

pub(super) fn prune_provider_messages(messages: &mut [Message]) -> PruneStats {
    let mut stats = PruneStats::default();
    if messages.len() <= RECENT_MESSAGES_TO_PROTECT {
        return stats;
    }

    stats.chars_saved += prune_duplicate_tool_results(messages, &mut stats);
    stats.chars_saved += prune_stale_error_inputs(messages, &mut stats);
    stats
}

fn prune_duplicate_tool_results(messages: &mut [Message], stats: &mut PruneStats) -> usize {
    let protected_start = messages.len().saturating_sub(RECENT_MESSAGES_TO_PROTECT);
    let mut tool_signature_by_id: HashMap<String, u64> = HashMap::new();
    let mut latest_result_for_signature: HashMap<u64, (usize, usize)> = HashMap::new();
    let mut duplicate_results: Vec<(usize, usize)> = Vec::new();

    for (message_idx, message) in messages.iter().enumerate() {
        for (block_idx, block) in message.content.iter().enumerate() {
            match block {
                ContentBlock::ToolUse { id, name, input } => {
                    tool_signature_by_id.insert(id.clone(), stable_tool_signature(name, input));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    is_error,
                    ..
                } if is_error != &Some(true) => {
                    if let Some(signature) = tool_signature_by_id.get(tool_use_id).copied() {
                        if let Some(previous) =
                            latest_result_for_signature.insert(signature, (message_idx, block_idx))
                            && previous.0 < protected_start
                        {
                            duplicate_results.push(previous);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let latest_indices: HashSet<(usize, usize)> =
        latest_result_for_signature.values().copied().collect();
    let mut saved = 0usize;
    for (message_idx, block_idx) in duplicate_results {
        if latest_indices.contains(&(message_idx, block_idx)) {
            continue;
        }
        if let Some(ContentBlock::ToolResult { content, .. }) = messages
            .get_mut(message_idx)
            .and_then(|message| message.content.get_mut(block_idx))
        {
            let old_len = content.len();
            if old_len > 0 {
                *content = "[jcode dynamic context pruning: duplicate tool output omitted; a newer identical tool call/result is kept later in the conversation.]".to_string();
                saved = saved.saturating_add(old_len.saturating_sub(content.len()));
                stats.duplicate_tool_results += 1;
            }
        }
    }
    saved
}

fn prune_stale_error_inputs(messages: &mut [Message], stats: &mut PruneStats) -> usize {
    let protected_start = messages.len().saturating_sub(RECENT_MESSAGES_TO_PROTECT);
    let mut errored_tool_ids = HashSet::new();
    for message in messages.iter() {
        for block in &message.content {
            if let ContentBlock::ToolResult {
                tool_use_id,
                is_error: Some(true),
                ..
            } = block
            {
                errored_tool_ids.insert(tool_use_id.clone());
            }
        }
    }

    let mut saved = 0usize;
    let total_messages = messages.len();
    for (message_idx, message) in messages.iter_mut().enumerate() {
        if message_idx >= protected_start
            || message_idx + ERROR_INPUT_PRUNE_AFTER_MESSAGES >= total_messages
        {
            continue;
        }
        for block in &mut message.content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                if !errored_tool_ids.contains(id) || input == &json!({ "pruned": true }) {
                    continue;
                }
                let old_len = input.to_string().len();
                *input = json!({
                    "pruned": true,
                    "reason": "stale errored tool call input omitted by jcode dynamic context pruning",
                    "tool": name,
                });
                saved = saved.saturating_add(old_len.saturating_sub(input.to_string().len()));
                stats.stale_error_inputs += 1;
            }
        }
    }
    saved
}

fn stable_tool_signature(name: &str, input: &serde_json::Value) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    serde_json::to_string(input)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Message, Role};

    fn assistant_tool(id: &str, name: &str, input: serde_json::Value) -> Message {
        Message {
            role: Role::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: id.to_string(),
                name: name.to_string(),
                input,
            }],
            timestamp: None,
            tool_duration_ms: None,
        }
    }

    fn tool_result(id: &str, content: &str, is_error: bool) -> Message {
        Message::tool_result(id, content, is_error)
    }

    #[test]
    fn dedupes_old_repeated_tool_results_but_keeps_newest() {
        let mut messages = vec![Message::user("start")];
        messages.push(assistant_tool("a", "read", json!({ "file": "x" })));
        messages.push(tool_result("a", &"old output".repeat(100), false));
        messages.extend((0..13).map(|idx| Message::assistant_text(&format!("filler {idx}"))));
        messages.push(assistant_tool("b", "read", json!({ "file": "x" })));
        messages.push(tool_result("b", "new output", false));

        let stats = prune_provider_messages(&mut messages);
        assert_eq!(stats.duplicate_tool_results, 1);
        assert!(stats.chars_saved > 0);
        match &messages[2].content[0] {
            ContentBlock::ToolResult { content, .. } => {
                assert!(content.contains("duplicate tool output omitted"))
            }
            other => panic!("unexpected block: {other:?}"),
        }
        match messages.last().unwrap().content.first().unwrap() {
            ContentBlock::ToolResult { content, .. } => assert_eq!(content, "new output"),
            other => panic!("unexpected block: {other:?}"),
        }
    }

    #[test]
    fn prunes_stale_errored_tool_inputs_but_keeps_error_text() {
        let huge_input = "bad".repeat(500);
        let mut messages = vec![
            assistant_tool("err", "bash", json!({ "command": huge_input })),
            tool_result("err", "command failed with useful diagnostic", true),
        ];
        messages.extend((0..13).map(|idx| Message::assistant_text(&format!("filler {idx}"))));

        let stats = prune_provider_messages(&mut messages);
        assert_eq!(stats.stale_error_inputs, 1);
        match &messages[0].content[0] {
            ContentBlock::ToolUse { input, .. } => assert_eq!(input["pruned"], true),
            other => panic!("unexpected block: {other:?}"),
        }
        match &messages[1].content[0] {
            ContentBlock::ToolResult {
                content, is_error, ..
            } => {
                assert_eq!(*is_error, Some(true));
                assert_eq!(content, "command failed with useful diagnostic");
            }
            other => panic!("unexpected block: {other:?}"),
        }
    }
}
