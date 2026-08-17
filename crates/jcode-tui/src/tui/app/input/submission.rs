use super::clipboard::parse_dropped_paths;
use super::*;

impl App {
    pub(in crate::tui::app) fn submit_input(&mut self) {
        promote_dropped_images(self);
        if self.activate_picker_from_preview() {
            return;
        }

        let raw_input = std::mem::take(&mut self.input);
        let mut input = self.expand_paste_placeholders(&raw_input);
        if let Some(notice) = input_exceeds_submit_limit(&input) {
            self.input = raw_input;
            self.cursor_pos = self.input.len();
            self.set_status_notice(notice.clone());
            self.push_display_message(DisplayMessage::system(notice));
            return;
        }
        self.pasted_contents.clear();
        self.cursor_pos = 0;
        self.clear_input_undo_history();
        self.follow_chat_bottom(); // Reset to bottom and resume auto-scroll on new input

        // If the previous assistant turn still has visible streamed text that has not yet been
        // committed into chat history, finalize it before inserting the next user turn.
        // Otherwise the new prompt can appear directly under the last tool call, and the final
        // assistant paragraph shows up later out of order.
        self.commit_pending_streaming_assistant_message();

        if let Some(pending) = self.pending_login.take() {
            self.handle_login_input(pending, input);
            return;
        }

        if let Some(pending) = self.pending_account_input.take() {
            self.handle_pending_account_input(pending, input);
            return;
        }

        if let Some(name) = self.pending_ssh_remote_name.take() {
            commands::handle_pending_ssh_remote_target(self, name, input);
            return;
        }

        let trimmed = input.trim();
        let handled = commands::handle_help_command(self, trimmed)
            || commands::handle_keys_command(self, trimmed)
            || commands::handle_ssh_command(self, trimmed)
            || commands::handle_session_command(self, trimmed)
            || commands::handle_dictation_command(self, trimmed)
            || commands::handle_config_command(self, trimmed)
            || commands::handle_log_command(self, trimmed)
            || commands::handle_diff_command(self, trimmed)
            || commands::handle_model_status_command(self, trimmed)
            || crate::tui::app::debug::handle_debug_command(self, trimmed)
            || crate::tui::app::model_context::handle_model_command(self, trimmed)
            || crate::tui::app::commands::handle_usage_command(self, trimmed)
            || crate::tui::app::productivity::handle_productivity_command(self, trimmed)
            || crate::tui::app::commands::handle_feedback_command(self, trimmed)
            || crate::tui::app::support::handle_support_command(self, trimmed)
            || crate::tui::app::state_ui::handle_info_command(self, trimmed)
            || crate::tui::app::auth::handle_auth_command(self, trimmed)
            || crate::tui::app::tui_lifecycle_runtime::handle_dev_command(self, trimmed);
        if handled {
            if trimmed.starts_with('/') {
                crate::telemetry::record_command_family(trimmed);
            }
            return;
        }

        if let Some(command) = extract_input_shell_command(&input) {
            self.push_display_message(DisplayMessage::user(raw_input));

            if command.is_empty() {
                self.push_display_message(DisplayMessage::system(
                    "Shell command cannot be empty after !.",
                ));
                self.set_status_notice("Shell command is empty");
                return;
            }

            if self.is_remote {
                self.push_display_message(DisplayMessage::system(
                    "Input-line ! shell commands are only available in a local jcode TUI session.",
                ));
                self.set_status_notice("Local shell unavailable in remote mode");
                return;
            }

            self.set_status_notice(format!(
                "Running local shell: {}",
                crate::util::truncate_str(command, 48)
            ));
            spawn_input_shell_command(
                self.session.id.clone(),
                command.to_string(),
                self.session.working_dir.clone(),
            );
            return;
        }

        // A terminal file drop is user input even when its absolute path starts
        // with `/`. Check the filesystem-aware drop parser before slash routing
        // so a real file can never collide with a skill name.
        let skill_invocation = parse_dropped_paths(&input)
            .is_none()
            .then(|| SkillRegistry::parse_invocation(&input))
            .flatten();

        // Check for skill invocation.
        if let Some(invocation) = skill_invocation {
            let skill_name = invocation.name.to_string();
            let trailing_prompt = invocation.prompt.map(str::to_string);
            let mut skill = self.current_skills_snapshot().get(&skill_name).cloned();

            // Remote/minimal TUI clients may start with an empty skill snapshot, and
            // daemon-side `skill_manage reload_all` can update a different process.
            // On a slash miss, synchronously refresh from the active session working
            // directory before reporting Unknown skill so project-local skills such
            // as .jcode/skills/optimization work immediately after reload/build.
            if skill.is_none() {
                self.refresh_skills_snapshot();
                skill = self.current_skills_snapshot().get(&skill_name).cloned();
            }

            if let Some(skill) = skill {
                self.active_skill = Some(skill_name.clone());
                self.push_display_message(DisplayMessage::system(format!(
                    "Activated skill: {} - {}",
                    skill.name, skill.description
                )));
                if let Some(prompt) = trailing_prompt {
                    input = prompt;
                } else {
                    return;
                }
            } else {
                // Distinguish an endorsed-but-not-installed skill from a
                // typo: the skill list advertises endorsed skills, so a bare
                // "Unknown skill" for them reads like a bug (issue #445).
                let endorsed_hint = crate::skill::endorsed_skills()
                    .iter()
                    .find(|endorsed| endorsed.name == skill_name)
                    .map(|endorsed| match endorsed.install {
                        Some(install) => format!(
                            "Skill /{} is endorsed but not installed. Install it with `{}`, then run /skills or skill_manage reload_all.",
                            skill_name, install
                        ),
                        None => format!(
                            "Skill /{} is endorsed but not installed (source: {}). Install it into ~/.jcode/skills/{}/SKILL.md.",
                            skill_name, endorsed.source, skill_name
                        ),
                    });
                self.push_display_message(DisplayMessage::error(
                    endorsed_hint.unwrap_or_else(|| format!("Unknown skill: /{}", skill_name)),
                ));
                return;
            }
        }

        // Leaving the preview should happen as soon as the user acts on it.
        self.onboarding_preview_mode = false;

        // Never dispatch a blank *typed* turn: whitespace-only input survives
        // the `is_empty()` entry guards and would burn a model call.
        //
        // This does not cover programmatic empty sends. Hidden continuations
        // call `begin_remote_send` with an empty body and carry their payload
        // in a `system_reminder`, bypassing this path entirely; that class of
        // blank turn is fixed in the agent, not here. See
        // docs/issues/blank-continuation-turn.md.
        if input.trim().is_empty() && self.pending_images.is_empty() {
            crate::logging::info("Ignoring blank submit");
            return;
        }
        // Add user message to display (show placeholder to user, not full paste)
        // Remember the typed prompt so we can restore it to the input box if this
        // turn fails (e.g. "token refresh needed"), instead of dropping it.
        self.last_submitted_input = Some(raw_input.clone());
        self.push_display_message(DisplayMessage {
            role: "user".to_string(),
            content: raw_input, // Show placeholder to user (condensed view)
            tool_calls: vec![],
            duration_secs: None,
            title: None,
            tool_data: None,
        });
        // Send expanded content (with actual pasted text) to model
        let images = std::mem::take(&mut self.pending_images);
        if !images.is_empty() {
            crate::logging::info(&format!(
                "Submitting with {} image(s): {}",
                images.len(),
                images
                    .iter()
                    .map(|(t, d)| format!("{} ({}KB)", t, d.len() / 1024))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if images.is_empty() {
            self.current_turn_system_reminder = mission_turn_reminder(&self.session.id);
            self.add_provider_message(Message::user(&input));
            self.session.add_message(
                Role::User,
                vec![ContentBlock::Text {
                    text: input.clone(),
                    cache_control: None,
                }],
            );
        } else {
            self.current_turn_system_reminder = mission_turn_reminder(&self.session.id);
            self.add_provider_message(Message::user_with_images(&input, images.clone()));
            let mut blocks: Vec<ContentBlock> = images
                .into_iter()
                .map(|(media_type, data)| ContentBlock::Image { media_type, data })
                .collect();
            blocks.push(ContentBlock::Text {
                text: input.clone(),
                cache_control: None,
            });
            self.session.add_message(Role::User, blocks);
        }
        crate::telemetry::record_turn();
        self.session_save_pending = true;

        // A fresh user turn supersedes any post-error fallback offer from the
        // previous turn; drop it so a stale keypress can't switch+resend.
        self.clear_pending_fallback_offer();
        // Likewise drop any armed "merge the diverged update" offer.
        self.clear_update_merge_offer();

        // Set up processing state - actual processing happens after UI redraws
        self.is_processing = true;
        self.status = ProcessingStatus::Sending;
        self.clear_streaming_render_state();
        // A new prompt starts a new turn: the previous turn's anchored
        // reasoning traces leave the transcript (ephemeral `current` mode).
        self.clear_turn_reasoning_traces();
        self.stream_buffer.clear();
        self.thought_line_inserted = false;
        self.thinking_prefix_emitted = false;
        self.thinking_buffer.clear();
        self.streaming_tool_calls.clear();
        self.streaming.streaming_input_tokens = 0;
        self.streaming.streaming_output_tokens = 0;
        self.streaming.streaming_cache_read_tokens = None;
        self.streaming.streaming_cache_creation_tokens = None;
        self.kv_cache.current_api_usage_recorded = false;
        self.upstream_provider = None;
        self.status_detail = None;
        self.streaming.streaming_tps_start = None;
        self.streaming.streaming_tps_elapsed = Duration::ZERO;
        self.streaming.streaming_tps_collect_output = false;
        self.streaming.streaming_total_output_tokens = 0;
        self.streaming.streaming_tps_observed_output_tokens = 0;
        self.streaming.streaming_tps_observed_elapsed = Duration::ZERO;
        self.processing_started = Some(Instant::now());
        self.visible_turn_started = Some(Instant::now());
        self.pending_turn = true;
    }

    /// Drain `queued_messages` for dispatch, re-deriving any poke's todo count
    /// from the list as it stands right now. See
    /// `commands::refresh_poke_message_for_dispatch` for why, and for the
    /// dropped-when-resolved case.
    ///
    /// Split out of `process_queued_messages` so this is reachable from tests:
    /// that function needs a live terminal, so a refresh inlined there could be
    /// deleted without any test noticing.
    pub(in crate::tui::app) fn take_queued_messages_for_dispatch(&mut self) -> Vec<String> {
        std::mem::take(&mut self.queued_messages)
            .into_iter()
            .filter_map(|message| {
                crate::tui::app::commands::refresh_poke_message_for_dispatch(self, &message)
            })
            .collect()
    }

    /// Process all queued messages (combined into a single request)
    /// Loops until queue is empty (in case more messages are queued during processing)
    pub(in crate::tui::app) async fn process_queued_messages(
        &mut self,
        terminal: &mut DefaultTerminal,
        event_stream: &mut EventStream,
    ) {
        while !self.queued_messages.is_empty() || !self.hidden_queued_system_messages.is_empty() {
            // Combine all currently queued messages into one, treating [SYSTEM: ...]
            // startup continuations as system reminders rather than user turns.
            let queued_messages = self.take_queued_messages_for_dispatch();
            let hidden_reminders = std::mem::take(&mut self.hidden_queued_system_messages);
            let (messages, reminder, display_system_messages) =
                crate::tui::app::helpers::partition_queued_messages(
                    queued_messages,
                    hidden_reminders,
                );
            let combined = messages.join("\n\n");
            let has_combined = !combined.is_empty();
            let preserve_visible_turn =
                crate::tui::app::commands::queued_messages_are_only_pokes(&messages);

            self.commit_pending_streaming_assistant_message();

            for msg in display_system_messages {
                self.push_display_message(DisplayMessage::system(msg));
            }

            for msg in &messages {
                if !crate::tui::app::commands::is_poke_message(msg) {
                    self.push_display_message(DisplayMessage::user(msg.clone()));
                }
            }

            self.current_turn_system_reminder =
                merge_turn_reminders(reminder, mission_turn_reminder(&self.session.id));

            if has_combined {
                self.add_provider_message(Message::user(&combined));
                self.session.add_message(
                    Role::User,
                    vec![ContentBlock::Text {
                        text: combined.clone(),
                        cache_control: None,
                    }],
                );
            }
            self.session_save_pending = true;
            self.clear_streaming_render_state();
            self.stream_buffer.clear();
            self.thought_line_inserted = false;
            self.thinking_prefix_emitted = false;
            self.thinking_buffer.clear();
            self.streaming_tool_calls.clear();
            self.streaming.streaming_input_tokens = 0;
            self.streaming.streaming_output_tokens = 0;
            self.streaming.streaming_cache_read_tokens = None;
            self.streaming.streaming_cache_creation_tokens = None;
            self.kv_cache.current_api_usage_recorded = false;
            self.upstream_provider = None;
            self.status_detail = None;
            self.streaming.streaming_tps_start = None;
            self.streaming.streaming_tps_elapsed = Duration::ZERO;
            self.streaming.streaming_tps_collect_output = false;
            self.streaming.streaming_total_output_tokens = 0;
            self.streaming.streaming_tps_observed_output_tokens = 0;
            self.streaming.streaming_tps_observed_elapsed = Duration::ZERO;
            self.processing_started = Some(Instant::now());
            if has_combined {
                if preserve_visible_turn {
                    self.visible_turn_started.get_or_insert_with(Instant::now);
                } else {
                    self.visible_turn_started = Some(Instant::now());
                }
            }
            self.is_processing = true;
            self.status = ProcessingStatus::Sending;

            match self
                .run_turn_interactive(terminal, event_stream, None)
                .await
            {
                Ok(()) => {
                    self.last_stream_error = None;
                    self.last_submitted_input = None;
                }
                Err(e) => {
                    let err_str = crate::util::format_error_chain(&e);
                    if is_request_payload_too_large_error(&err_str) {
                        if !self
                            .try_recover_payload_too_large_and_retry(terminal, event_stream)
                            .await
                        {
                            self.handle_turn_error(err_str);
                        }
                    } else if is_context_limit_error(&err_str) {
                        if self
                            .try_auto_compact_and_retry(terminal, event_stream)
                            .await
                        {
                            // Successfully recovered
                        } else {
                            self.handle_turn_error(err_str);
                        }
                    } else {
                        self.handle_turn_error(err_str);
                    }
                }
            }
            self.current_turn_system_reminder = None;
            // Loop will check if more messages were queued during this turn
        }
    }

    pub(in crate::tui::app) fn flush_pending_session_save(&mut self) {
        if !self.session_save_pending {
            return;
        }

        match self.session.save() {
            Ok(()) => {
                self.session_save_pending = false;
            }
            Err(error) => {
                crate::logging::warn(&format!(
                    "Failed to persist pending session save for {}: {}",
                    self.session.id, error
                ));
            }
        }
    }
}
