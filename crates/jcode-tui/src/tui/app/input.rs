#![cfg_attr(test, allow(clippy::items_after_test_module))]

use super::{
    App, ContentBlock, DisplayMessage, Message, ProcessingStatus, Role, SendAction, SkillRegistry,
    commands, ctrl_bracket_fallback_to_esc, is_context_limit_error,
    is_request_payload_too_large_error, remote,
};
use crate::bus::{
    Bus, BusEvent, ClipboardPasteCompleted, ClipboardPasteContent, ClipboardPasteKind,
    InputShellCompleted,
};
use crate::util::truncate_str;
use anyhow::Result;
use base64::Engine;
use crossterm::event::{EventStream, KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

const INPUT_SHELL_MAX_OUTPUT_LEN: usize = 30_000;
mod clipboard;
mod copy_badge;
mod streaming;
mod submission;

use clipboard::{
    expand_paste_placeholders, handle_paste, is_clipboard_paste_shortcut, paste_from_clipboard,
    promote_dropped_images,
};

/// Remove reasoning-marked lines from committed transcript text. Reasoning lines
/// are wrapped in emphasis containing the invisible [`REASONING_SENTINEL`]
/// (see `jcode_tui_markdown::reasoning_line_markup`). Trailing blank lines left
/// behind are trimmed so the remaining answer renders cleanly.
pub(super) fn strip_reasoning_lines(content: &str) -> String {
    let sentinel = jcode_tui_markdown::REASONING_SENTINEL;
    let mut out_lines: Vec<&str> = Vec::new();
    for line in content.split('\n') {
        if line.contains(sentinel) {
            continue;
        }
        out_lines.push(line);
    }
    // Collapse runs of blank lines created by removed reasoning blocks, and trim
    // leading/trailing blank lines.
    let mut result = String::with_capacity(content.len());
    let mut prev_blank = true; // suppress leading blanks
    for line in out_lines {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue;
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
        prev_blank = is_blank;
    }
    result.trim_end().to_string()
}

fn mission_turn_reminder(session_id: &str) -> Option<String> {
    crate::mission::active_system_reminder(session_id)
        .map_err(|err| crate::logging::warn(&format!("failed to load active mission: {err}")))
        .ok()
        .flatten()
}

fn merge_turn_reminders(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => Some(format!("{}\n\n{}", a, b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

pub(super) fn extract_input_shell_command(input: &str) -> Option<&str> {
    input.trim().strip_prefix('!').map(str::trim)
}

fn build_input_shell_command(command: &str) -> std::process::Command {
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("cmd.exe");
        cmd.arg("/C").arg(command);
        cmd
    }

    #[cfg(not(windows))]
    {
        let mut cmd = std::process::Command::new("bash");
        cmd.arg("-c").arg(command);
        cmd
    }
}

fn combine_shell_output(stdout: &[u8], stderr: &[u8]) -> (String, bool) {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let mut output = String::new();

    if !stdout.is_empty() {
        output.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("[stderr]\n");
        output.push_str(&stderr);
    }

    let truncated = if output.len() > INPUT_SHELL_MAX_OUTPUT_LEN {
        output = truncate_str(&output, INPUT_SHELL_MAX_OUTPUT_LEN).to_string();
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("… output truncated");
        true
    } else {
        false
    };

    (output, truncated)
}

fn spawn_input_shell_command(session_id: String, command: String, cwd: Option<String>) {
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let mut cmd = build_input_shell_command(&command);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(dir) = cwd.as_ref() {
            cmd.current_dir(dir);
        }

        let event = match cmd.output() {
            Ok(output) => {
                let (combined_output, truncated) =
                    combine_shell_output(&output.stdout, &output.stderr);
                InputShellCompleted {
                    session_id,
                    result: crate::message::InputShellResult {
                        command,
                        cwd,
                        output: combined_output,
                        exit_code: output.status.code(),
                        duration_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
                        truncated,
                        failed_to_start: false,
                    },
                }
            }
            Err(error) => InputShellCompleted {
                session_id,
                result: crate::message::InputShellResult {
                    command,
                    cwd,
                    output: format!("Failed to run command: {}", error),
                    exit_code: None,
                    duration_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
                    truncated: false,
                    failed_to_start: true,
                },
            },
        };

        Bus::global().publish(BusEvent::InputShellCompleted(event));
    });
}

pub(super) struct PreparedInput {
    pub raw_input: String,
    pub expanded: String,
    pub images: Vec<(String, String)>,
}

// Roughly 500k English words at ~6 bytes/word including spaces. This is still
// low enough to avoid multi-megabyte submit-path hangs while allowing very large logs.
pub(super) const MAX_SUBMITTED_TEXT_BYTES: usize = 3 * 1024 * 1024;

fn oversized_message_notice(size: usize) -> String {
    format!(
        "Message is too large to send ({} bytes). Save it as a file or attach it instead. Your input was preserved.",
        crate::util::format_number(size)
    )
}

fn input_exceeds_submit_limit(input: &str) -> Option<String> {
    let size = input.len();
    (size > MAX_SUBMITTED_TEXT_BYTES).then(|| oversized_message_notice(size))
}

impl App {
    pub(in crate::tui::app) fn handle_clipboard_paste_completed(
        &mut self,
        result: ClipboardPasteCompleted,
    ) -> bool {
        if self.active_client_session_id() != Some(result.session_id.as_str()) {
            return false;
        }

        match result.content {
            ClipboardPasteContent::Image {
                media_type,
                base64_data,
            } => {
                attach_image(self, media_type, base64_data);
                true
            }
            ClipboardPasteContent::Text(text) => {
                handle_text_paste(self, text);
                true
            }
            ClipboardPasteContent::Empty => {
                match result.kind {
                    ClipboardPasteKind::Smart => {
                        self.set_status_notice("No text or image in clipboard");
                    }
                    ClipboardPasteKind::ImageOnly => {
                        self.set_status_notice("No image in clipboard")
                    }
                    ClipboardPasteKind::ImageUrl { fallback_text } => {
                        if let Some(text) = fallback_text {
                            handle_text_paste(self, text);
                        } else {
                            self.set_status_notice("Failed to download image");
                        }
                    }
                }
                true
            }
            ClipboardPasteContent::Error(message) => {
                if let ClipboardPasteKind::ImageUrl {
                    fallback_text: Some(text),
                } = result.kind
                {
                    self.set_status_notice(message);
                    handle_text_paste(self, text);
                } else {
                    self.set_status_notice(message);
                }
                true
            }
        }
    }
}

pub(super) fn insert_input_text(app: &mut App, text: &str) {
    if text.is_empty() {
        return;
    }

    let at_end = app.cursor_pos == app.input.len();

    // A habitual space typed after an auto-inserted picker separator would
    // only add noise. Swallow it so command + space + filter still produces
    // a single separator.
    if text == " " && at_end && matches!(app.input.trim_start(), "/login " | "/model " | "/models ")
    {
        return;
    }

    app.remember_input_undo_state();

    // After a picker command is fully typed (or completed without a trailing
    // space), the next printable character starts its filter. Insert the
    // separator instead of extending the command token and closing the picker.
    if at_end
        && matches!(app.input.trim_start(), "/login" | "/model" | "/models")
        && !text.starts_with(char::is_whitespace)
    {
        app.input.push(' ');
        app.cursor_pos = app.input.len();
    }

    app.input.insert_str(app.cursor_pos, text);
    app.cursor_pos += text.len();

    // Typing the final command character immediately arms picker filtering.
    // Without this, users can keep typing the command token or press Enter
    // without realizing the visible picker is ready to filter.
    if app.cursor_pos == app.input.len()
        && matches!(app.input.trim_start(), "/login" | "/model" | "/models")
    {
        app.input.push(' ');
        app.cursor_pos = app.input.len();
    }

    app.reset_tab_completion();
    app.sync_model_picker_preview_from_input();
}

pub(super) fn handle_text_input(app: &mut App, text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let onboarding_suggestions = matches!(
        app.onboarding_phase(),
        Some(crate::tui::app::onboarding_flow::OnboardingPhase::Suggestions)
    );
    if app.input.is_empty()
        && !app.is_processing
        && (app.display_messages.is_empty() || onboarding_suggestions)
    {
        let mut chars = text.chars();
        if let (Some(c), None) = (chars.next(), chars.next())
            && let Some(digit) = c.to_digit(10)
        {
            let suggestions = app.suggestion_prompts();
            let idx = digit as usize;
            if idx >= 1 && idx <= suggestions.len() {
                let (_label, prompt) = &suggestions[idx - 1];
                if !prompt.starts_with('/') {
                    app.remember_input_undo_state();
                    app.input = prompt.clone();
                    app.cursor_pos = app.input.len();
                    app.follow_chat_bottom_for_typing();
                    app.submit_input();
                    return true;
                }
            }
        }
    }

    insert_input_text(app, text);
    promote_dropped_images(app);
    true
}

fn visible_prompt_history(app: &App) -> Vec<String> {
    app.display_messages
        .iter()
        .filter(|message| message.role == "user")
        .map(|message| message.content.trim().to_string())
        .filter(|content| !content.is_empty())
        .collect()
}

fn byte_offset_for_line_column(
    input: &str,
    line_start: usize,
    line_end: usize,
    column: usize,
) -> usize {
    let mut offset = line_end;
    for (idx, (byte_offset, _)) in input[line_start..line_end].char_indices().enumerate() {
        if idx == column {
            offset = line_start + byte_offset;
            break;
        }
    }
    offset
}

pub(super) fn handle_multiline_input_navigation(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    if !modifiers.is_empty()
        || !matches!(code, KeyCode::Up | KeyCode::Down)
        || !app.input.contains('\n')
    {
        return false;
    }

    let input = app.input.as_str();
    let cursor = app.cursor_pos.min(input.len());
    let line_start = input[..cursor].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_end = input[cursor..]
        .find('\n')
        .map(|idx| cursor + idx)
        .unwrap_or(input.len());
    let column = input[line_start..cursor].chars().count();

    let target = match code {
        KeyCode::Up => {
            if line_start == 0 {
                return false;
            }
            let previous_line_end = line_start - 1;
            let previous_line_start = input[..previous_line_end]
                .rfind('\n')
                .map(|idx| idx + 1)
                .unwrap_or(0);
            byte_offset_for_line_column(input, previous_line_start, previous_line_end, column)
        }
        KeyCode::Down => {
            if line_end >= input.len() {
                return false;
            }
            let next_line_start = line_end + 1;
            let next_line_end = input[next_line_start..]
                .find('\n')
                .map(|idx| next_line_start + idx)
                .unwrap_or(input.len());
            byte_offset_for_line_column(input, next_line_start, next_line_end, column)
        }
        _ => return false,
    };

    app.cursor_pos = target;
    true
}

/// True when `modifiers` is exactly one of Ctrl, Alt(Option) or Cmd(Super),
/// the set of single modifiers we treat as "recall queued prompts / browse
/// history" when combined with Up/Down. Shift or any combination is excluded so
/// it doesn't shadow selection-extension or other chords.
pub(super) fn is_prompt_recall_modifier(modifiers: KeyModifiers) -> bool {
    matches!(
        modifiers,
        KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER
    )
}

pub(super) fn handle_prompt_history_navigation(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    let explicit_history = modifiers == KeyModifiers::CONTROL;
    if !(modifiers.is_empty() || explicit_history) || !matches!(code, KeyCode::Up | KeyCode::Down) {
        return false;
    }

    let history = visible_prompt_history(app);
    if history.is_empty() {
        return false;
    }

    let target = if app.input.is_empty() {
        match code {
            KeyCode::Up => Some(history.len() - 1),
            KeyCode::Down => None,
            _ => None,
        }
    } else {
        let Some(current_index) = history.iter().rposition(|prompt| prompt == &app.input) else {
            if explicit_history && matches!(code, KeyCode::Up) {
                return history
                    .last()
                    .map(|prompt| {
                        app.input = prompt.clone();
                        app.cursor_pos = app.input.len();
                        app.reset_tab_completion();
                        app.sync_model_picker_preview_from_input();
                    })
                    .is_some();
            }
            return false;
        };
        match code {
            KeyCode::Up => Some(current_index.saturating_sub(1)),
            KeyCode::Down if current_index + 1 < history.len() => Some(current_index + 1),
            KeyCode::Down => {
                app.input.clear();
                app.cursor_pos = 0;
                app.reset_tab_completion();
                app.sync_model_picker_preview_from_input();
                return true;
            }
            _ => None,
        }
    };

    let Some(target) = target else {
        return false;
    };
    let Some(prompt) = history.get(target) else {
        return false;
    };
    app.input = prompt.clone();
    app.cursor_pos = app.input.len();
    app.reset_tab_completion();
    app.sync_model_picker_preview_from_input();
    true
}

fn associated_text_for_key_event(_event: &KeyEvent) -> Option<String> {
    // Future hook: prefer terminal-provided associated text when crossterm exposes it.
    // Today crossterm does not surface this on KeyEvent even though the kitty protocol
    // defines a REPORT_ASSOCIATED_TEXT flag.
    None
}

pub(super) fn text_input_for_key_event(event: &KeyEvent) -> Option<String> {
    associated_text_for_key_event(event)
        .filter(|text| !text.is_empty())
        .or_else(|| text_input_for_key(event.code, event.modifiers))
}

pub(super) fn text_input_for_key(code: KeyCode, modifiers: KeyModifiers) -> Option<String> {
    let KeyCode::Char(c) = code else {
        return None;
    };

    if !modifiers_allow_printable_text(c, modifiers) {
        return None;
    }

    Some(shifted_printable_fallback(c, modifiers).to_string())
}

fn modifiers_allow_printable_text(c: char, modifiers: KeyModifiers) -> bool {
    if modifiers.intersects(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META) {
        return false;
    }

    let has_control = modifiers.contains(KeyModifiers::CONTROL);
    let has_alt = modifiers.contains(KeyModifiers::ALT);
    match (has_control, has_alt) {
        (false, false) => true,
        // Some terminals report AltGr/layout-generated symbols as Ctrl+Alt plus the final
        // printable character. Preserve that character when it cannot be confused with normal
        // Ctrl/Alt letter shortcuts. If the terminal only reports the physical base key, we still
        // refuse to synthesize a layout-specific character we cannot know.
        (_, true) => is_layout_modified_text_char(c),
        (true, false) => false,
    }
}

fn is_layout_modified_text_char(c: char) -> bool {
    !c.is_control() && c != ' ' && !c.is_ascii_alphanumeric()
}

fn shifted_printable_fallback(c: char, modifiers: KeyModifiers) -> char {
    if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_lowercase() {
        return c.to_ascii_uppercase();
    }

    c
}

pub(super) fn clear_input_for_escape(app: &mut App) {
    let had_input = !app.input.is_empty();
    if had_input {
        app.remember_input_undo_state();
    }
    app.input.clear();
    app.cursor_pos = 0;
    app.reset_tab_completion();
    app.sync_model_picker_preview_from_input();
    if had_input {
        app.set_status_notice("Input cleared - Ctrl+Z to restore");
    }
}

pub(super) fn expand_paste_placeholders(app: &mut App, input: &str) -> String {
    let mut result = input.to_string();
    for content in app.pasted_contents.iter().rev() {
        let placeholder = paste_placeholder(content);
        if let Some(pos) = result.rfind(&placeholder) {
            result.replace_range(pos..pos + placeholder.len(), content);
        }
    }
    result
}

pub(super) fn queue_message(app: &mut App) {
    let prepared = take_prepared_input(app);
    app.queued_messages.push(prepared.expanded);
}

pub(super) fn retrieve_pending_message_for_edit(app: &mut App) -> bool {
    if !app.input.is_empty() {
        return false;
    }

    let mut parts: Vec<String> = Vec::new();
    let mut had_pending = false;

    if !app.pending_soft_interrupts.is_empty() {
        parts.extend(std::mem::take(&mut app.pending_soft_interrupts));
        app.pending_soft_interrupt_requests.clear();
        had_pending = true;
    }
    if let Some(msg) = app.interleave_message.take()
        && !msg.is_empty()
    {
        parts.push(msg);
        had_pending = true;
    }
    if !app.queued_messages.is_empty() {
        parts.extend(std::mem::take(&mut app.queued_messages));
        if !app.has_queued_followups() {
            app.pending_queued_dispatch = false;
        }
        had_pending = true;
    }

    if !parts.is_empty() {
        app.input = parts.join("\n\n");
        app.cursor_pos = app.input.len();
        let count = parts.len();
        app.set_status_notice(format!(
            "Retrieved {} pending message{} for editing",
            count,
            if count == 1 { "" } else { "s" }
        ));
    }

    had_pending
}

pub(super) fn send_action(app: &App, alternate_shortcut: bool) -> SendAction {
    if !app.is_processing {
        return SendAction::Submit;
    }
    if app.input.trim().starts_with('/') || app.input.trim().starts_with('!') {
        return SendAction::Submit;
    }
    if alternate_shortcut {
        if app.queue_mode {
            SendAction::Interleave
        } else {
            SendAction::Queue
        }
    } else if app.queue_mode {
        SendAction::Queue
    } else {
        SendAction::Interleave
    }
}

pub(super) fn handle_shift_enter(app: &mut App) {
    insert_input_text(app, "\n");
}

impl App {
    pub(super) fn has_queued_followups(&self) -> bool {
        self.interleave_message.is_some()
            || !self.queued_messages.is_empty()
            || !self.hidden_queued_system_messages.is_empty()
    }

    /// True when a startup submission is staged and ready to auto-send.
    ///
    /// Headed spawns (and reloads with a resume prompt) stage their initial
    /// prompt into `self.input` and set `submit_input_on_startup`, rather than
    /// pushing onto `queued_messages`. The post-connect dispatcher must treat
    /// this as pending work so the prompt is actually submitted once the remote
    /// session history loads. See issues #267/#268/#76.
    pub(super) fn has_pending_startup_submission(&self) -> bool {
        self.submit_input_on_startup
            && (!self.input.trim().is_empty() || !self.pending_images.is_empty())
    }

    pub(super) fn schedule_auto_poke_followup_if_needed(&mut self) -> bool {
        super::commands_poke::schedule_auto_poke_followup_if_needed(self)
    }

    pub(super) fn schedule_queued_dispatch_after_interrupt(&mut self) {
        if self.has_queued_followups() {
            self.pending_queued_dispatch = true;
        }
    }

    pub(crate) fn toggle_next_prompt_new_session_routing(&mut self) {
        self.route_next_prompt_to_new_session = !self.route_next_prompt_to_new_session;
        if self.route_next_prompt_to_new_session {
            self.set_status_notice("Next prompt → new session");
        } else {
            self.set_status_notice("Next-prompt new session canceled");
        }
    }

    /// Whether the configured `keybindings.new_terminal` chord matches this key.
    pub(crate) fn new_terminal_key_matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        self.new_terminal_key
            .binding
            .as_ref()
            .map(|binding| binding.matches(code, modifiers))
            .unwrap_or(false)
    }

    /// Whether the configured `keybindings.open_resume` chord matches this key.
    pub(crate) fn open_resume_key_matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        self.open_resume_key
            .binding
            .as_ref()
            .map(|binding| binding.matches(code, modifiers))
            .unwrap_or(false)
    }

    /// Whether the configured `keybindings.fallback_switch` chord matches this key.
    pub(crate) fn fallback_switch_key_matches(
        &self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> bool {
        self.fallback_switch_key
            .binding
            .as_ref()
            .map(|binding| binding.matches(code, modifiers))
            .unwrap_or(false)
    }

    /// Spawn a brand-new jcode session in a new terminal window.
    pub(crate) fn handle_new_terminal_hotkey(&mut self) {
        let cwd = commands::active_working_dir(self)
            .filter(|path| path.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        match super::spawn_fresh_session_in_new_terminal(&cwd) {
            Ok(true) => self.set_status_notice("↗ New terminal opened"),
            Ok(false) => {
                self.push_display_message(DisplayMessage::system(format!(
                    "No supported terminal found.\n\n{}\n\nRun `jcode` manually to start a new session.",
                    crate::terminal_launch::terminal_spawn_failure_hint()
                )));
                self.set_status_notice("No supported terminal found")
            }
            Err(error) => self.set_status_notice(format!("New terminal failed: {}", error)),
        }
    }
}

pub(super) fn is_next_prompt_new_session_hotkey(code: KeyCode, modifiers: KeyModifiers) -> bool {
    if code != KeyCode::Char(' ') {
        return false;
    }
    // Accept either Command/Super+Space (macOS Cmd, often eaten by Spotlight) or
    // Option/Alt+Space (macOS Option) so the fork-to-new-session arming hotkey is
    // reachable across terminals. Reject Ctrl/Hyper combos so other chords still
    // route to their own handlers.
    let has_super = modifiers.contains(KeyModifiers::SUPER);
    let has_alt = modifiers.contains(KeyModifiers::ALT);
    (has_super || has_alt) && !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::HYPER)
}

fn input_routes_to_new_session(app: &App) -> bool {
    if !app.route_next_prompt_to_new_session || app.input.is_empty() {
        return false;
    }
    let trimmed = app.input.trim_start();
    !trimmed.starts_with('/') && extract_input_shell_command(trimmed).is_none()
}

fn route_prompt_to_new_session_local(app: &mut App) -> bool {
    if !input_routes_to_new_session(app) {
        return false;
    }

    app.route_next_prompt_to_new_session = false;
    let prepared = take_prepared_input(app);
    let restored_raw = prepared.raw_input.clone();
    let restored_images = prepared.images.clone();
    match commands::launch_prompt_in_new_session_local(app, prepared.expanded, prepared.images) {
        Ok(_) => true,
        Err(error) => {
            app.input = restored_raw;
            app.cursor_pos = app.input.len();
            app.pending_images = restored_images;
            app.set_status_notice("Prompt launch failed");
            app.push_display_message(DisplayMessage::error(format!(
                "Failed to launch prompt in a new session: {}",
                error
            )));
            true
        }
    }
}

pub(super) fn handle_alternate_enter(app: &mut App) {
    if app.activate_picker_from_preview() {
        return;
    }

    if app.input.is_empty() {
        return;
    }

    if route_prompt_to_new_session_local(app) {
        return;
    }

    match send_action(app, true) {
        SendAction::Submit => app.submit_input(),
        SendAction::Queue => queue_message(app),
        SendAction::Interleave => {
            let prepared = take_prepared_input(app);
            stage_local_interleave(app, prepared.expanded);
        }
    }
}

pub(super) fn handle_control_key(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('u') => {
            delete_input_to_start(app);
            true
        }
        KeyCode::Char('k') => {
            delete_input_to_end(app);
            true
        }
        KeyCode::Char('z') => {
            app.undo_input_change();
            true
        }
        KeyCode::Char('x') => {
            cut_input_line_to_clipboard(app);
            true
        }
        KeyCode::Char('a') => {
            app.cursor_pos = 0;
            true
        }
        KeyCode::Char('e') => {
            app.cursor_pos = app.input.len();
            true
        }
        KeyCode::Char('b') => {
            if app.cursor_pos > 0 {
                app.cursor_pos = app.find_word_boundary_back();
            }
            true
        }
        KeyCode::Char('f') => {
            if app.cursor_pos < app.input.len() {
                app.cursor_pos = app.find_word_boundary_forward();
            }
            true
        }
        KeyCode::Char('w') | KeyCode::Char('\u{8}') | KeyCode::Backspace => {
            delete_input_word_back(app);
            true
        }
        KeyCode::Char('s') => {
            app.toggle_input_stash();
            true
        }
        KeyCode::Char('p') => {
            super::commands::toggle_auto_poke_hotkey_local(app);
            true
        }
        KeyCode::Char('v') => {
            paste_from_clipboard(app);
            true
        }
        KeyCode::Tab | KeyCode::Char('t') => {
            app.queue_mode = !app.queue_mode;
            let mode_str = if app.queue_mode {
                "Queue mode: messages wait until response completes"
            } else {
                "Immediate mode: messages send next (no interrupt)"
            };
            app.set_status_notice(mode_str);
            true
        }
        KeyCode::Left => {
            if app.cursor_pos > 0 {
                app.cursor_pos = app.find_word_boundary_back();
            }
            true
        }
        KeyCode::Right => {
            if app.cursor_pos < app.input.len() {
                app.cursor_pos = app.find_word_boundary_forward();
            }
            true
        }
        KeyCode::Up => {
            retrieve_pending_message_for_edit(app);
            true
        }
        _ => false,
    }
}

pub(super) fn delete_input_to_start(app: &mut App) {
    if app.cursor_pos > 0 {
        app.remember_input_undo_state();
    }
    app.input.drain(..app.cursor_pos);
    app.cursor_pos = 0;
    app.sync_model_picker_preview_from_input();
}

pub(super) fn delete_input_to_end(app: &mut App) {
    if app.cursor_pos < app.input.len() {
        app.remember_input_undo_state();
    }
    app.input.truncate(app.cursor_pos);
    app.sync_model_picker_preview_from_input();
}

pub(super) fn handle_super_key(app: &mut App, code: KeyCode) -> bool {
    match code {
        // Cmd+5 toggles the onboarding simulator (a dev aid for walking through
        // every first-run onboarding screen without touching real auth state).
        KeyCode::Char('5') => {
            app.toggle_onboarding_simulator();
            true
        }
        // macOS terminals that forward Command may report Command+Delete as Super+Backspace,
        // Super+Delete, or Super+DEL. Treat all of them as delete-the-previous-word, matching
        // the requested Cmd+Backspace = delete-by-word behavior.
        KeyCode::Backspace | KeyCode::Delete | KeyCode::Char('\u{7f}') => {
            delete_input_word_back(app);
            true
        }
        KeyCode::Left | KeyCode::Home | KeyCode::Char('a') => {
            app.cursor_pos = 0;
            true
        }
        KeyCode::Right | KeyCode::End | KeyCode::Char('e') => {
            app.cursor_pos = app.input.len();
            true
        }
        KeyCode::Char('z') => {
            app.undo_input_change();
            true
        }
        KeyCode::Char('x') => {
            cut_input_line_to_clipboard(app);
            true
        }
        KeyCode::Char('v') => {
            paste_from_clipboard(app);
            true
        }
        _ => false,
    }
}

pub(super) fn delete_input_word_back(app: &mut App) {
    let start = app.find_word_boundary_back();
    if start < app.cursor_pos {
        app.remember_input_undo_state();
    }
    app.input.drain(start..app.cursor_pos);
    app.cursor_pos = start;
    app.sync_model_picker_preview_from_input();
}

pub(super) fn handle_alt_key(app: &mut App, code: KeyCode) -> bool {
    match code {
        // Alt/Option+Left/Right move by word, matching Alt+B / Alt+F.
        KeyCode::Left | KeyCode::Char('b') => {
            app.cursor_pos = app.find_word_boundary_back();
            true
        }
        KeyCode::Right | KeyCode::Char('f') => {
            app.cursor_pos = app.find_word_boundary_forward();
            true
        }
        KeyCode::Char('d') => {
            let end = app.find_word_boundary_forward();
            if app.cursor_pos < end {
                app.remember_input_undo_state();
            }
            app.input.drain(app.cursor_pos..end);
            app.sync_model_picker_preview_from_input();
            true
        }
        // macOS terminals vary between Backspace, Delete, and DEL for Option+Delete.
        // Keep all aliases on word-delete-back so the documented Alt/Option+Backspace works.
        KeyCode::Backspace | KeyCode::Delete | KeyCode::Char('\u{7f}') => {
            delete_input_word_back(app);
            true
        }
        KeyCode::Char('v') => {
            paste_from_clipboard(app);
            true
        }
        KeyCode::Char('a') if app.input.is_empty() => {
            app.copy_chat_viewport_context_to_clipboard();
            true
        }
        _ => false,
    }
}

pub(super) fn handle_navigation_shortcuts(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    if let Some(amount) = app.scroll_keys.scroll_amount(code, modifiers) {
        if amount < 0 {
            app.scroll_up((-amount) as usize);
        } else {
            app.scroll_down(amount as usize);
        }
        return true;
    }

    if let Some(dir) = app.scroll_keys.prompt_jump(code, modifiers) {
        if dir < 0 {
            app.scroll_to_prev_prompt();
        } else {
            app.scroll_to_next_prompt();
        }
        return true;
    }

    if let Some(ratio) = App::ctrl_side_panel_ratio_preset(&code, modifiers) {
        app.set_side_panel_ratio_preset(ratio);
        return true;
    }

    if let Some(rank) = App::ctrl_prompt_rank(&code, modifiers) {
        app.scroll_to_recent_prompt_rank(rank);
        return true;
    }

    if app.scroll_keys.is_bookmark(code, modifiers) {
        app.toggle_scroll_bookmark();
        return true;
    }

    if app.toggle_keys.diff_mode_cycle.matches(code, modifiers) {
        app.diff_mode = app.diff_mode.cycle();
        if !app.diff_pane_visible() {
            app.diff_pane_focus = false;
        }
        let status = format!("Diffs: {}", app.diff_mode.label());
        app.set_status_notice(&status);
        return true;
    }

    false
}

pub(super) fn is_scroll_only_key(app: &App, code: KeyCode, modifiers: KeyModifiers) -> bool {
    let mut code = code;
    let mut modifiers = modifiers;
    ctrl_bracket_fallback_to_esc(&mut code, &mut modifiers);

    if app.scroll_keys.scroll_amount(code, modifiers).is_some()
        || app.scroll_keys.prompt_jump(code, modifiers).is_some()
        || App::ctrl_side_panel_ratio_preset(&code, modifiers).is_some()
        || App::ctrl_prompt_rank(&code, modifiers).is_some()
        || app.scroll_keys.is_bookmark(code, modifiers)
        || (modifiers.contains(KeyModifiers::ALT)
            && matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'g')))
    {
        return true;
    }

    if app.diff_pane_focus && !modifiers.contains(KeyModifiers::CONTROL) {
        match code {
            KeyCode::Char('j')
            | KeyCode::Down
            | KeyCode::Char('k')
            | KeyCode::Up
            | KeyCode::Char('d')
            | KeyCode::PageDown
            | KeyCode::Char('u')
            | KeyCode::PageUp
            | KeyCode::Char('g')
            | KeyCode::Home
            | KeyCode::Char('G')
            | KeyCode::End
            | KeyCode::Char('+')
            | KeyCode::Char('=')
            | KeyCode::Char('-')
            | KeyCode::Char('0')
            | KeyCode::Esc => return true,
            _ => {}
        }
    }

    let diagram_available = app.diagram_available();
    if diagram_available && app.diagram_focus && !modifiers.contains(KeyModifiers::CONTROL) {
        match code {
            KeyCode::Char('h')
            | KeyCode::Left
            | KeyCode::Char('l')
            | KeyCode::Right
            | KeyCode::Char('k')
            | KeyCode::Up
            | KeyCode::Char('j')
            | KeyCode::Down
            | KeyCode::Char('+')
            | KeyCode::Char('=')
            | KeyCode::Char('-')
            | KeyCode::Char('_')
            | KeyCode::Char(']')
            | KeyCode::Char('[')
            | KeyCode::Char('o')
            | KeyCode::Esc => return true,
            _ => {}
        }
    }

    if modifiers.contains(KeyModifiers::CONTROL) {
        if diagram_available {
            match code {
                KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
                    return true;
                }
                _ => {}
            }
        }
        if app.diff_pane_visible() {
            match code {
                KeyCode::Char('h') | KeyCode::Char('l') => return true,
                _ => {}
            }
        }
    }

    false
}

pub(super) fn handle_pre_control_shortcuts(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    // Plain Ctrl+K kills to end of line (emacs habit). Ctrl+Shift+K must fall
    // through to the scroll handler: with the Kitty keyboard protocol enabled,
    // terminals report Ctrl+Shift+K as Char('k') + CONTROL|SHIFT, so without the
    // Shift guard this would swallow the scroll-up chord and wipe the draft.
    if modifiers.contains(KeyModifiers::CONTROL)
        && !modifiers.contains(KeyModifiers::SHIFT)
        && matches!(code, KeyCode::Char('k'))
        && !app.input.is_empty()
    {
        delete_input_to_end(app);
        return true;
    }

    if is_clipboard_paste_shortcut(code, modifiers) {
        paste_from_clipboard(app);
        return true;
    }

    let macos_option_shortcut =
        crate::tui::keybind::shortcut_char_for_macos_option_key(code, modifiers);
    if app.toggle_keys.copy_selection.matches(code, modifiers) {
        app.toggle_copy_selection_mode();
        return true;
    }

    if handle_visible_copy_shortcut(app, code, modifiers) {
        return true;
    }

    if app.toggle_keys.side_panel.matches(code, modifiers) {
        app.toggle_side_panel();
        return true;
    }
    if app.toggle_keys.diagram_pane.matches(code, modifiers) {
        app.toggle_diagram_pane_position();
        return true;
    }
    if app.toggle_keys.typing_scroll_lock.matches(code, modifiers) {
        app.toggle_typing_scroll_lock();
        return true;
    }
    if app.toggle_keys.info_widget.matches(code, modifiers) {
        crate::tui::info_widget::toggle_enabled();
        let status = if crate::tui::info_widget::is_enabled() {
            "Info widget: ON"
        } else {
            "Info widget: OFF"
        };
        app.set_status_notice(status);
        return true;
    }
    if app.toggle_keys.todo_card.matches(code, modifiers) {
        app.toggle_todo_card();
        return true;
    }
    if app.dictation_key_matches(code, modifiers) {
        app.handle_dictation_trigger();
        return true;
    }

    // Inline swarm panel: Alt+N focuses/unfocuses the managed-agents panel.
    // While focused, Alt+↑/↓ select, Alt+O pops the selection out to a terminal,
    // Alt+Shift+P opens the swarm prompt, and Esc exits. Plain typing is NOT
    // captured (it keeps flowing to the chat input).
    if app.toggle_keys.swarm_panel_focus.matches(code, modifiers) {
        if app.toggle_swarm_panel_focus() {
            app.set_status_notice("Swarm: alt+↑/↓ select · alt+o open · alt+shift+p prompt · esc");
        }
        return true;
    }
    {
        use crate::tui::TuiState as _;
        if app.swarm_panel_focused() && app.handle_swarm_panel_key(code, modifiers) {
            return true;
        }
    }
    if app.new_terminal_key_matches(code, modifiers) {
        app.handle_new_terminal_hotkey();
        return true;
    }
    if app.open_resume_key_matches(code, modifiers) {
        app.record_keybinding_fast(super::shortcut_hints::LearnableAction::Resume);
        app.open_session_picker();
        return true;
    }
    if let Some(direction) = app.model_switch_keys.direction_for(code, modifiers) {
        app.record_keybinding_fast(super::shortcut_hints::LearnableAction::ModelSwitch);
        app.cycle_model(direction);
        return true;
    }
    if let Some(direction) = app.effort_switch_keys.direction_for(code, modifiers) {
        app.record_keybinding_fast(super::shortcut_hints::LearnableAction::EffortCycle);
        app.cycle_effort(direction);
        return true;
    }
    if cfg!(target_os = "macos")
        && !matches!(app.status, ProcessingStatus::RunningTool(_))
        && let Some(direction) = app
            .effort_switch_keys
            .macos_option_arrow_escape_direction_for(code, modifiers)
    {
        app.cycle_effort(direction);
        return true;
    }
    if app.centered_toggle_keys.matches(code, modifiers) {
        app.record_keybinding_fast(super::shortcut_hints::LearnableAction::Alignment);
        app.toggle_centered_mode();
        return true;
    }

    app.normalize_diagram_state();
    let diagram_available = app.diagram_available();
    if app.handle_diagram_focus_key(code, modifiers, diagram_available) {
        return true;
    }
    if app.handle_diff_pane_focus_key(code, modifiers) {
        return true;
    }
    if modifiers.contains(KeyModifiers::ALT) && handle_alt_key(app, code) {
        return true;
    }
    if let Some(shortcut) = macos_option_shortcut
        && handle_alt_key(app, KeyCode::Char(shortcut))
    {
        return true;
    }
    if modifiers.contains(KeyModifiers::SUPER) && handle_super_key(app, code) {
        return true;
    }

    handle_navigation_shortcuts(app, code, modifiers)
}

pub(super) fn handle_visible_copy_shortcut(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    let Some(c) = visible_copy_shortcut_key(code, modifiers) else {
        return false;
    };

    // Many terminals encode Alt+Shift+<letter> as just Alt + uppercase letter
    // instead of reporting an explicit Shift modifier. Accept either form so the
    // on-screen [Alt] [⇧] copy badges behave consistently.
    let explicit_shift = modifiers.contains(KeyModifiers::SHIFT);
    let implicit_shift = c.is_ascii_uppercase();
    let macos_option_shift =
        crate::tui::keybind::shortcut_char_for_macos_option_shift_key(code, modifiers).is_some();
    if !explicit_shift && !implicit_shift && !macos_option_shift {
        // Some terminals report Alt+Shift+E as Alt+lowercase `e` with no
        // explicit SHIFT modifier. Keep the relaxed fallback scoped to the
        // expand badge so plain Alt+letter copy shortcuts do not become active.
        if c.eq_ignore_ascii_case(&'e') && handle_expand_edit_badge_shortcut(app, c) {
            return true;
        }
        return false;
    }

    if handle_expand_edit_badge_shortcut(app, c) {
        return true;
    }

    if handle_inline_image_toggle_shortcut(app, c) {
        return true;
    }

    if let Some(target) = crate::tui::ui::recent_flicker_copy_target_for_key(c)
        .or_else(|| crate::tui::ui::visible_copy_target_for_key(c))
    {
        let success = super::copy_to_clipboard(&target.content);
        app.record_copy_badge_key_press(c);
        app.record_copy_badge_feedback(c, success);
        if success {
            app.set_status_notice(target.copied_notice);
        } else {
            app.set_status_notice(format!("Failed to copy {}", target.kind_label));
        }
        return true;
    }

    false
}

fn visible_copy_shortcut_key(code: KeyCode, modifiers: KeyModifiers) -> Option<char> {
    if let Some(key) =
        crate::tui::keybind::shortcut_char_for_macos_option_shift_key(code, modifiers)
    {
        return Some(key);
    }

    let KeyCode::Char(c) = code else {
        return None;
    };

    modifiers.contains(KeyModifiers::ALT).then_some(c)
}

/// Alt+Shift+I toggles inline transcript images between expanded and
/// collapsed label stubs. Only active when the transcript actually has
/// inline images, so the chord stays inert otherwise.
fn handle_inline_image_toggle_shortcut(app: &mut App, key: char) -> bool {
    if !key.eq_ignore_ascii_case(&'i') {
        return false;
    }
    use crate::tui::TuiState as _;
    if app.side_pane_images_signature().0 == 0 {
        return false;
    }
    app.record_copy_badge_key_press('i');
    app.toggle_inline_images();
    true
}

fn handle_expand_edit_badge_shortcut(app: &mut App, key: char) -> bool {
    if !key.eq_ignore_ascii_case(&'e') {
        return false;
    }

    let visible_expand_badge = crate::tui::ui::visible_expand_edit_badge();
    let has_edit_tool_message = app.display_edit_tool_message_count > 0
        || app.display_messages.iter().any(|message| {
            message
                .tool_data
                .as_ref()
                .map(|tool| crate::tui::ui::tools_ui::is_edit_tool_name(&tool.name))
                .unwrap_or(false)
        });

    // The inline edit badge is rendered from the inline diff mode itself, while
    // opening it from other diff modes requires at least one edit tool message.
    // Keep this predicate in one place so the [Alt] [⇧] [E] badge uses the same
    // shortcut path as visible copy badges without falling through to copy key E.
    if !visible_expand_badge && !app.diff_mode.is_inline() && !has_edit_tool_message {
        return false;
    }

    if app.diff_mode.is_full_inline() {
        return false;
    }

    app.diff_mode = crate::config::DiffDisplayMode::FullInline;
    app.record_copy_badge_key_press('e');
    app.copy_badge_ui.expand_feedback_until =
        Some(std::time::Instant::now() + std::time::Duration::from_millis(1100));
    app.copy_badge_ui.expand_feedback_line = crate::tui::ui::visible_expand_edit_badge_line();
    app.set_status_notice(format!(
        "Expanded edit diffs · Diffs: {}",
        app.diff_mode.label()
    ));
    true
}

pub(super) fn handle_modal_key(
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<bool> {
    if app.changelog_scroll.is_some() {
        app.handle_changelog_key(code)?;
        return Ok(true);
    }

    if app.help_scroll.is_some() {
        app.handle_help_key(code)?;
        return Ok(true);
    }

    if app.model_status_scroll.is_some() {
        app.handle_model_status_key(code)?;
        return Ok(true);
    }

    if app.session_picker_overlay.is_some() {
        app.handle_session_picker_key(code, modifiers)?;
        return Ok(true);
    }

    if app.login_picker_overlay.is_some() {
        app.handle_login_picker_key(code, modifiers)?;
        return Ok(true);
    }

    if app.account_picker_overlay.is_some() {
        if let Some(command) = app.next_account_picker_action(code, modifiers)? {
            app.handle_account_picker_command(command);
        }
        return Ok(true);
    }

    if app.copy_selection_mode {
        if modifiers.contains(KeyModifiers::CONTROL)
            && matches!(code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            return Ok(false);
        }

        let _ = app.handle_copy_selection_key(code, modifiers)
            || handle_navigation_shortcuts(app, code, modifiers);
        return Ok(true);
    }

    if let Some(ref picker) = app.inline_interactive_state
        && !picker.preview
    {
        app.handle_inline_interactive_key(code, modifiers)?;
        return Ok(true);
    }

    if app.handle_inline_interactive_preview_key(&code, modifiers)? {
        return Ok(true);
    }

    Ok(false)
}

pub(super) fn handle_global_control_shortcuts(
    app: &mut App,
    code: KeyCode,
    diagram_available: bool,
) -> bool {
    if app.handle_diagram_ctrl_key(code, diagram_available) {
        return true;
    }

    match code {
        KeyCode::Char('c') | KeyCode::Char('d') => {
            if app.is_processing {
                app.cancel_requested = true;
                app.interleave_message = None;
                app.pending_soft_interrupts.clear();
                app.pending_soft_interrupt_requests.clear();
                if app.cancel_overnight_for_interrupt() {
                    app.set_status_notice("Interrupting... Overnight cancelled");
                } else {
                    app.set_status_notice("Interrupting...");
                }
            } else {
                app.handle_quit_request();
            }
            true
        }
        KeyCode::Char('r') => {
            app.recover_session_without_tools();
            true
        }
        KeyCode::Char('a') if app.input.is_empty() => {
            app.copy_chat_viewport_context_to_clipboard();
            true
        }
        KeyCode::Char('l') => true,
        _ => handle_control_key(app, code),
    }
}

pub(super) fn handle_enter(app: &mut App) -> bool {
    promote_dropped_images(app);
    if app.activate_picker_from_preview() {
        return true;
    }
    if !app.input.is_empty() {
        if route_prompt_to_new_session_local(app) {
            return true;
        }
        match send_action(app, false) {
            SendAction::Submit => app.submit_input(),
            SendAction::Queue => queue_message(app),
            SendAction::Interleave => {
                let prepared = take_prepared_input(app);
                stage_local_interleave(app, prepared.expanded);
            }
        }
    }
    true
}

pub(super) fn handle_basic_key(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char(c) => handle_text_input(app, &c.to_string()),
        KeyCode::Backspace => {
            if app.cursor_pos > 0 {
                let prev = crate::tui::core::prev_char_boundary(&app.input, app.cursor_pos);
                app.remember_input_undo_state();
                app.input.drain(prev..app.cursor_pos);
                app.cursor_pos = prev;
                app.reset_tab_completion();
                app.sync_model_picker_preview_from_input();
            }
            true
        }
        KeyCode::Delete => {
            if app.cursor_pos < app.input.len() {
                let next = crate::tui::core::next_char_boundary(&app.input, app.cursor_pos);
                app.remember_input_undo_state();
                app.input.drain(app.cursor_pos..next);
                app.reset_tab_completion();
                app.sync_model_picker_preview_from_input();
            }
            true
        }
        KeyCode::Left => {
            if app.cursor_pos > 0 {
                app.cursor_pos = crate::tui::core::prev_char_boundary(&app.input, app.cursor_pos);
            } else {
                // Opt-in: Left on an empty input opens the active sessions
                // manager (no-op unless display.active_sessions_manager).
                app.maybe_open_active_sessions_on_left();
            }
            true
        }
        KeyCode::Right => {
            if app.cursor_pos < app.input.len() {
                app.cursor_pos = crate::tui::core::next_char_boundary(&app.input, app.cursor_pos);
            }
            true
        }
        KeyCode::Home => {
            app.cursor_pos = 0;
            true
        }
        KeyCode::End => {
            app.cursor_pos = app.input.len();
            true
        }
        KeyCode::Tab => {
            app.autocomplete();
            true
        }
        KeyCode::Up | KeyCode::PageUp => {
            let inc = if code == KeyCode::PageUp { 10 } else { 1 };
            app.scroll_up(inc);
            true
        }
        KeyCode::Down | KeyCode::PageDown => {
            let dec = if code == KeyCode::PageDown { 10 } else { 1 };
            app.scroll_down(dec);
            true
        }
        KeyCode::Esc => {
            if app
                .inline_interactive_state
                .as_ref()
                .map(|p| p.preview)
                .unwrap_or(false)
            {
                app.inline_interactive_state = None;
                app.inline_view_state = None;
                clear_input_for_escape(app);
            } else if app.inline_view_state.is_some() {
                app.inline_view_state = None;
                clear_input_for_escape(app);
            } else if app.is_processing {
                let disabled_auto_poke = app.auto_poke_incomplete_todos
                    || app
                        .queued_messages
                        .iter()
                        .any(|message| super::commands::is_poke_message(message));
                app.cancel_requested = true;
                app.interleave_message = None;
                app.pending_soft_interrupts.clear();
                app.pending_soft_interrupt_requests.clear();
                let cancelled_overnight = app.cancel_overnight_for_interrupt();
                if disabled_auto_poke {
                    super::commands::disable_auto_poke(app);
                    if cancelled_overnight {
                        app.set_status_notice("Interrupting... Auto-poke OFF, overnight cancelled");
                    } else {
                        app.set_status_notice("Interrupting... Auto-poke OFF");
                    }
                } else if cancelled_overnight {
                    app.set_status_notice("Interrupting... Overnight cancelled");
                } else {
                    app.set_status_notice("Interrupting...");
                }
            } else {
                app.follow_chat_bottom();
                clear_input_for_escape(app);
            }
            true
        }
        _ => false,
    }
}

pub(super) fn take_prepared_input(app: &mut App) -> PreparedInput {
    let raw_input = std::mem::take(&mut app.input);
    let expanded = expand_paste_placeholders(app, &raw_input);
    app.pasted_contents.clear();
    let images = std::mem::take(&mut app.pending_images);
    app.cursor_pos = 0;
    app.clear_input_undo_history();
    PreparedInput {
        raw_input,
        expanded,
        images,
    }
}

pub(super) fn stage_local_interleave(app: &mut App, content: String) {
    app.interleave_message = Some(content);
    app.set_status_notice("⏭ Sending now (interleave)");
}

fn attach_image(app: &mut App, media_type: String, base64_data: String) {
    let size_kb = base64_data.len() / 1024;
    app.pending_images.push((media_type.clone(), base64_data));
    let placeholder = format!("[image {}]", app.pending_images.len());
    app.remember_input_undo_state();
    app.input.insert_str(app.cursor_pos, &placeholder);
    app.cursor_pos += placeholder.len();
    app.sync_model_picker_preview_from_input();
    app.set_status_notice(format!("Pasted {} ({} KB)", media_type, size_kb));
}

fn paste_placeholder(content: &str) -> String {
    let line_count = content.lines().count().max(1);
    format!(
        "[pasted {} line{}]",
        line_count,
        if line_count == 1 { "" } else { "s" }
    )
}

fn handle_key_early_routes(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Result<bool> {
    if handle_modal_key(app, code, modifiers)? {
        return Ok(true);
    }

    // The onboarding simulator owns all key handling while active so the
    // real onboarding handlers never fire (no real logins/imports).
    if app.handle_onboarding_sim_key(code, modifiers) {
        return Ok(true);
    }

    if app.handle_onboarding_continue_prompt_key(code) {
        return Ok(true);
    }

    // Inline hotkey feedback: when a known-but-rarely-used chord is pressed,
    // show "you just pressed X → does Y". Placed after the modal/overlay
    // handlers so overlay-local keys stay silent. Unknown chords are
    // reported at the fall-through points below.
    app.observe_known_hotkey(code, modifiers, false);

    if code == KeyCode::BackTab {
        app.cycle_model_favorite_hotkey();
        return Ok(true);
    }

    // While the model picker preview is visible, route its favorite/default
    // hotkeys (Ctrl+O set default, Ctrl+N toggle favorite) to the focused
    // picker handler before the global control shortcuts can claim them. This
    // makes the hotkeys work directly in the preview list the user always
    // sees, without colliding with the readline/tmux keys (Ctrl+B/Ctrl+F).
    if app.model_picker_preview_hotkey(code, modifiers)? {
        return Ok(true);
    }

    if app.pending_provider_failover.is_some() && !app.is_processing {
        if code == KeyCode::Esc {
            app.cancel_pending_provider_failover("Provider auto-switch canceled");
            return Ok(true);
        }
        if !is_scroll_only_key(app, code, modifiers) {
            app.cancel_pending_provider_failover("Provider auto-switch canceled");
        }
    }

    // Accept an armed post-error fallback offer: switch to the next best
    // model/auth-method and resend the failed turn.
    if app.pending_fallback_offer.is_some()
        && !app.is_processing
        && app.fallback_switch_key_matches(code, modifiers)
    {
        app.apply_pending_fallback_offer();
        return Ok(true);
    }

    // Accept an armed "merge the diverged update" offer: spawn a jcode agent
    // to reconcile the branches. Shares the fallback-switch accept key.
    if app.merge_offer_key_matches(code, modifiers) {
        app.accept_update_merge_offer();
        return Ok(true);
    }

    if is_next_prompt_new_session_hotkey(code, modifiers) {
        app.toggle_next_prompt_new_session_routing();
        return Ok(true);
    }

    if app.handle_command_suggestion_key(code, modifiers) {
        return Ok(true);
    }

    if handle_pre_control_shortcuts(app, code, modifiers) {
        return Ok(true);
    }

    Ok(false)
}

impl App {
    pub(super) fn handle_key_event(&mut self, event: crossterm::event::KeyEvent) {
        // Record the event if recording is active
        use crate::tui::test_harness::{TestEvent, record_event};
        let modifiers: Vec<String> = {
            let mut mods = vec![];
            if event.modifiers.contains(KeyModifiers::CONTROL) {
                mods.push("ctrl".to_string());
            }
            if event.modifiers.contains(KeyModifiers::ALT) {
                mods.push("alt".to_string());
            }
            if event.modifiers.contains(KeyModifiers::SHIFT) {
                mods.push("shift".to_string());
            }
            mods
        };
        let code_str = format!("{:?}", event.code);
        record_event(TestEvent::Key {
            code: code_str,
            modifiers,
        });

        self.update_copy_badge_key_event(event);
        if matches!(
            event.kind,
            crossterm::event::KeyEventKind::Press | crossterm::event::KeyEventKind::Repeat
        ) {
            let _ = self.handle_key_press_event(event);
        }
    }

    pub(super) fn handle_key_press_event(&mut self, event: KeyEvent) -> Result<()> {
        self.handle_key_core(
            event.code,
            event.modifiers,
            text_input_for_key_event(&event),
        )
    }

    #[cfg(test)]
    pub(super) fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        self.handle_key_core(code, modifiers, None)
    }

    fn handle_key_core(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        text_input: Option<String>,
    ) -> Result<()> {
        let mut code = code;
        let mut modifiers = modifiers;
        ctrl_bracket_fallback_to_esc(&mut code, &mut modifiers);

        if handle_key_early_routes(self, code, modifiers)? {
            return Ok(());
        }

        self.normalize_diagram_state();
        let diagram_available = self.diagram_available();

        // Ctrl / Alt(Option) / Cmd(Super) + Up all recall queued/pending messages
        // for editing and then walk prompt history. We accept any of the three
        // single modifiers so the gesture works regardless of which one a given
        // terminal forwards (some send Option as Alt, some forward Command as
        // Super), without the user having to rebind anything.
        if code == KeyCode::Up && is_prompt_recall_modifier(modifiers) {
            if retrieve_pending_message_for_edit(self) {
                return Ok(());
            }
            // Normalize to CONTROL so handle_prompt_history_navigation takes its
            // explicit-history path (jump straight into history even mid-draft).
            handle_prompt_history_navigation(self, KeyCode::Up, KeyModifiers::CONTROL);
            return Ok(());
        }

        if code == KeyCode::Down && is_prompt_recall_modifier(modifiers) {
            handle_prompt_history_navigation(self, KeyCode::Down, KeyModifiers::CONTROL);
            return Ok(());
        }

        // Handle ctrl combos regardless of processing state
        if modifiers.contains(KeyModifiers::CONTROL)
            && handle_global_control_shortcuts(self, code, diagram_available)
        {
            return Ok(());
        }

        // Ctrl+Enter / Cmd+Enter: does opposite of queue_mode during processing
        if code == KeyCode::Enter
            && modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
        {
            handle_alternate_enter(self);
            return Ok(());
        }

        // Shift+Enter and Alt/Option+Enter insert a newline in the input box.
        if code == KeyCode::Enter && modifiers.intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) {
            handle_shift_enter(self);
            return Ok(());
        }

        // When the model picker preview is visible, arrow keys navigate the picker list
        if self
            .inline_interactive_state
            .as_ref()
            .map(|p| p.preview)
            .unwrap_or(false)
        {
            match code {
                KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                    return self.handle_inline_interactive_key(code, modifiers);
                }
                _ => {}
            }
        }

        if handle_multiline_input_navigation(self, code, modifiers)
            || handle_prompt_history_navigation(self, code, modifiers)
        {
            return Ok(());
        }

        if let Some(text) = text_input.or_else(|| text_input_for_key(code, modifiers)) {
            handle_text_input(self, &text);
            return Ok(());
        }

        // Never fall through and insert literal text for unhandled Ctrl+key chords. This stays
        // after text_input so Ctrl+Alt/AltGr symbols delivered as final printable text still work.
        if modifiers.contains(KeyModifiers::CONTROL) {
            self.note_unrecognized_hotkey(code, modifiers, false);
            return Ok(());
        }

        if code == KeyCode::Enter {
            // During the onboarding model-selection phase, Enter on an empty
            // prompt opens the model picker instead of submitting nothing.
            if self.input.trim().is_empty()
                && matches!(
                    self.onboarding_phase(),
                    Some(crate::tui::app::onboarding_flow::OnboardingPhase::ModelSelect)
                )
            {
                self.open_model_picker();
                return Ok(());
            }
            handle_enter(self);
            return Ok(());
        }

        // A modified chord (or function key) that reached this point is not
        // bound to anything; tell the user instead of silently swallowing it or
        // inserting a surprise character.
        self.note_unrecognized_hotkey(code, modifiers, false);

        if handle_basic_key(self, code) {
            return Ok(());
        }

        Ok(())
    }

    pub(super) fn request_full_redraw(&mut self) {
        self.force_full_redraw = true;
    }

    /// Arm a full re-emit of every cell on the next frame without an
    /// intermediate ED2 clear escape. Prefer this over `request_full_redraw`
    /// when the real screen has not diverged from ratatui's model (e.g. chat
    /// scrolling), so image placeholder cells do not flash blank (issue #404).
    pub(super) fn request_full_repaint(&mut self) {
        self.force_full_repaint = true;
    }

    pub(super) fn should_redraw_after_resize(&mut self) -> bool {
        const RESIZE_REDRAW_MIN_INTERVAL: std::time::Duration =
            std::time::Duration::from_millis(33);

        let now = std::time::Instant::now();
        match self.last_resize_redraw {
            Some(last) if now.duration_since(last) < RESIZE_REDRAW_MIN_INTERVAL => false,
            _ => {
                self.last_resize_redraw = Some(now);
                self.handle_diagram_geometry_change();
                true
            }
        }
    }

    /// Try to paste whatever is in the clipboard.
    /// Prefers text when available, otherwise falls back to image data.
    /// Used by Ctrl+V handlers in both local and remote mode.
    pub(super) fn paste_from_clipboard(&mut self) {
        paste_from_clipboard(self);
    }

    /// Queue a message to be sent later
    /// Handle bracketed paste: store text content (image URLs are still detected inline)
    pub(super) fn handle_paste(&mut self, text: String) {
        handle_paste(self, text);
    }

    /// Expand paste placeholders in input with actual content
    pub(super) fn expand_paste_placeholders(&mut self, input: &str) -> String {
        expand_paste_placeholders(self, input)
    }

    pub(super) fn queue_message(&mut self) {
        queue_message(self);
    }

    /// Send an interleave message immediately to the server as a soft interrupt.
    /// Skips the intermediate buffer stage - goes directly to pending_soft_interrupts.
    pub(super) async fn send_interleave_now(
        &mut self,
        content: String,
        remote: &mut crate::tui::backend::RemoteConnection,
    ) {
        remote::send_interleave_now(self, content, remote).await;
    }

    /// Retrieve all pending unsent messages into the input for editing.
    /// Priority: pending soft interrupts first, then interleave, then queued.
    /// Returns true if pending soft interrupts were retrieved (caller should cancel on server).
    pub(super) fn retrieve_pending_message_for_edit(&mut self) -> bool {
        retrieve_pending_message_for_edit(self)
    }

    pub(super) fn send_action(&self, shift: bool) -> SendAction {
        send_action(self, shift)
    }
}
