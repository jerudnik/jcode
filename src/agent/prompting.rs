use super::Agent;
use crate::logging;
use crate::message::{Message, ToolDefinition};

impl Agent {
    pub(super) fn log_prompt_prefix_accounting(
        &self,
        split: &crate::prompt::SplitSystemPrompt,
        tools: &[ToolDefinition],
    ) {
        let system_tokens = split.estimated_tokens();
        let tool_tokens = ToolDefinition::aggregate_prompt_token_estimate(tools);
        let prefix_tokens = system_tokens + tool_tokens;
        logging::info(&format!(
            "Prompt prefix estimate: total={} tokens (system={} tools={})",
            prefix_tokens, system_tokens, tool_tokens
        ));
    }

    pub(super) fn build_memory_prompt_nonblocking_shared(
        &self,
        messages: std::sync::Arc<[Message]>,
        _memory_event_tx: Option<crate::memory::MemoryEventSink>,
    ) -> Option<crate::memory::PendingMemory> {
        if !self.memory_enabled {
            return None;
        }

        let session_id = &self.session.id;

        let pending = if crate::message::ends_with_fresh_user_turn(&messages) {
            crate::memory::take_pending_memory(session_id)
        } else {
            None
        };

        // Use the persistent memory-agent pipeline as the single source of truth.
        // Running both this and the legacy MemoryManager background retrieval path
        // can prepare overlapping pending prompts for the same turn, which makes
        // memory injection feel overly aggressive.
        crate::memory_agent::update_context_sync_with_dir(
            session_id,
            messages,
            self.session.working_dir.clone(),
        );

        pending
    }

    fn append_current_turn_system_reminder(&self, split: &mut crate::prompt::SplitSystemPrompt) {
        let Some(reminder) = self
            .current_turn_system_reminder
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            return;
        };

        if !split.dynamic_part.is_empty() {
            split.dynamic_part.push_str("\n\n");
        }
        split.dynamic_part.push_str("# System Reminder\n\n");
        split.dynamic_part.push_str(reminder);
    }

    /// Build split system prompt for better caching
    /// Returns static (cacheable) and dynamic (not cached) parts separately
    pub(super) fn build_system_prompt_split(
        &self,
        memory_prompt: Option<&str>,
    ) -> crate::prompt::SplitSystemPrompt {
        if let Some(ref override_prompt) = self.system_prompt_override {
            return crate::prompt::SplitSystemPrompt {
                static_part: override_prompt.clone(),
                dynamic_part: String::new(),
            };
        }

        let skills = self.current_skills_snapshot();
        let skill_prompt = self
            .active_skill
            .as_ref()
            .and_then(|name| skills.get(name).map(|skill| skill.get_prompt().to_string()));

        let available_skills: Vec<crate::prompt::SkillInfo> = self
            .current_skills_snapshot()
            .list()
            .iter()
            .map(|skill| crate::prompt::SkillInfo {
                name: skill.name.clone(),
                description: skill.description.clone(),
            })
            .collect();

        let working_dir = self
            .session
            .working_dir
            .as_ref()
            .map(std::path::PathBuf::from);

        let (mut split, _context_info) = crate::prompt::build_system_prompt_split(
            skill_prompt.as_deref(),
            &available_skills,
            self.session.is_canary,
            memory_prompt,
            working_dir.as_deref(),
        );

        if self.session.kind.is_meta() {
            let meta_prompt = r#"# Meta Co-Manager Mode

You are the user's persistent Jcode workspace co-manager. Treat this session as a conversational control plane for the shared Jcode server and the surrounding terminal workspace, not just as a normal coding session.

Responsibilities:
- Maintain a thread-like conversational UX: the user may ask architectural questions, feasibility questions, status questions, or give direct operational commands.
- Help supervise regular Jcode client sessions without replacing their autonomy. Observe first, summarize clearly, and only intervene or take over when asked or when a safety policy clearly warrants it.
- Use server/workspace tools proactively: `swarm` for session coordination and messaging, `debug_socket` for runtime/server inspection, `bg` for background task panes and watchers, `session_search` for prior sessions, and ordinary coding tools when the user asks you to change files.
- Prefer lightweight status/observation before disruptive actions. Ask only when an action could surprise the user, such as interrupting or taking over another active session.
- Keep regular agent sessions independent: they retain their own swarms, subagents, tools, and working context.

## Cross-swarm observation playbook

You belong to the `jcode-meta` swarm. Every regular client session lives in its own swarm. To see what other sessions are doing without joining their swarms:

1. Start broad: `swarm action=list_swarms` enumerates every swarm on the server with member counts, coordinator, plan version, and aggregate status counts.
2. Drill into a specific swarm:
   - `swarm action=list swarm_id=<swarm>` lists that swarm's members and their statuses.
   - `swarm action=list_channels swarm_id=<swarm>` lists its channels.
   - `swarm action=plan_status swarm_id=<swarm>` shows its plan graph.
3. Inspect a specific session within any swarm (cross-swarm reads are permitted for meta):
   - `swarm action=summary target_session=<id>` for a compact recent-activity view.
   - `swarm action=read_context target_session=<id>` for the full shared context window.
4. Only after you understand the state should you message, interrupt, or take over. Messaging and lifecycle actions (dm, broadcast, stop, reassign, etc.) still target sessions in your own swarm; if you need to coordinate inside another swarm, narrate the situation to the user and let them direct the next step rather than silently mutating someone else's swarm.

When the user asks to observe or supervise a client, identify the target session/client, gather state, summarize what it is doing, and propose or perform the next low-risk action."#;

            split.static_part.push_str("\n\n");
            split.static_part.push_str(meta_prompt);
        }

        self.append_current_turn_system_reminder(&mut split);

        split
    }

    /// Non-blocking memory prompt - takes pending result and spawns check for next turn
    pub(super) fn build_memory_prompt_nonblocking(
        &self,
        messages: &[Message],
        _memory_event_tx: Option<crate::memory::MemoryEventSink>,
    ) -> Option<crate::memory::PendingMemory> {
        self.build_memory_prompt_nonblocking_shared(messages.to_vec().into(), _memory_event_tx)
    }
}
