use super::*;

impl App {
    pub(super) fn insert_thought_line(&mut self, line: String) {
        if self.thought_line_inserted || line.is_empty() {
            return;
        }
        self.thought_line_inserted = true;
        let mut prefix = line;
        if !prefix.ends_with('\n') {
            prefix.push('\n');
        }
        prefix.push('\n');
        if self.streaming.streaming_text.is_empty() {
            self.replace_streaming_text(prefix);
        } else {
            self.replace_streaming_text(format!("{}{}", prefix, self.streaming.streaming_text));
        }
    }

    /// Begin a reasoning region. Reasoning renders as dim, italic text (no
    /// blockquote gutter, no header, no footer). Idempotent while open.
    pub(super) fn open_reasoning_region(&mut self) {
        if self.reasoning_streaming {
            return;
        }
        // Separate the reasoning block from any prior content with a blank line.
        if !self.streaming.streaming_text.is_empty() {
            if self.streaming.streaming_text.ends_with("\n\n") {
                // already separated
            } else if self.streaming.streaming_text.ends_with('\n') {
                self.append_streaming_text("\n");
            } else {
                self.append_streaming_text("\n\n");
            }
        }
        self.reasoning_streaming = true;
        self.reasoning_pending_line.clear();
        self.reasoning_partial_len = 0;
        // Remember where this reasoning block starts in the stream so `current`
        // mode can later slice it back out in place (without disturbing any
        // preceding answer text) once the model starts answering.
        self.reasoning_block_start = Some(self.streaming.streaming_text.len());
    }

    /// Remove the live partial-reasoning tail (the rendered, not-yet-committed
    /// in-progress line) from the streaming buffer so it can be rebuilt. No-op
    /// when there is no live partial.
    fn strip_reasoning_partial_tail(&mut self) {
        if self.reasoning_partial_len > 0 {
            let new_len = self
                .streaming
                .streaming_text
                .len()
                .saturating_sub(self.reasoning_partial_len);
            self.streaming.streaming_text.truncate(new_len);
            self.reasoning_partial_len = 0;
        }
    }

    /// Append streamed reasoning text, rendering the in-progress line live so
    /// reasoning trickles in token-by-token (like normal output) rather than one
    /// whole line at a time. Complete lines (terminated by `\n`) are committed as
    /// dim+italic markdown; the trailing partial line is rendered as a live tail
    /// that is re-emitted in place on each delta. The whole-line emphasis run is
    /// preserved (each line is its own `*…*`) so styling never breaks mid-line.
    pub(super) fn append_reasoning_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if !self.reasoning_streaming {
            self.open_reasoning_region();
        }
        // Drop the previous live tail; we rebuild committed lines + a fresh tail.
        self.strip_reasoning_partial_tail();
        let mut committed = String::new();
        for ch in text.chars() {
            if ch == '\n' {
                let line = std::mem::take(&mut self.reasoning_pending_line);
                committed.push_str(&jcode_tui_markdown::reasoning_line_markup(&line));
            } else {
                self.reasoning_pending_line.push(ch);
            }
        }
        if !committed.is_empty() {
            self.streaming.streaming_text.push_str(&committed);
        }
        // Re-append the live tail for the in-progress (partial) line.
        let partial = jcode_tui_markdown::reasoning_partial_markup(&self.reasoning_pending_line);
        self.reasoning_partial_len = partial.len();
        self.streaming.streaming_text.push_str(&partial);
        self.refresh_split_view_if_needed();
    }

    /// Promote the live partial line to a committed line and end the region. The
    /// `_footer` argument is ignored (the "Thought for Xs" footer was removed);
    /// it is kept for call-site compatibility.
    pub(super) fn close_reasoning_region(&mut self, _footer: Option<String>) {
        if !self.reasoning_streaming {
            return;
        }
        // Replace the live tail with the committed (newline-terminated) line.
        self.strip_reasoning_partial_tail();
        let pending = std::mem::take(&mut self.reasoning_pending_line);
        if !pending.is_empty() {
            self.streaming
                .streaming_text
                .push_str(&jcode_tui_markdown::reasoning_line_markup(&pending));
        }
        self.reasoning_streaming = false;

        // In `current` mode, reasoning is ephemeral: it is never written to the
        // persistent transcript. The closed block is sliced out of the live
        // stream and anchored *in place* as a display-only reasoning message in
        // the transcript flow: it never moves again (no bottom-following, no
        // hoisting), stays readable for the rest of the turn, and is removed
        // when the next user prompt starts a new turn.
        if self.reasoning_current_mode() {
            self.anchor_current_reasoning_block();
            return;
        }

        // Terminate the reasoning block with a blank line so following output
        // renders as a normal paragraph.
        if !self.streaming.streaming_text.ends_with("\n\n") {
            if self.streaming.streaming_text.ends_with('\n') {
                self.streaming.streaming_text.push('\n');
            } else {
                self.streaming.streaming_text.push_str("\n\n");
            }
        }
        self.refresh_split_view_if_needed();
    }

    /// True when the active reasoning-display mode is `current` (live-only,
    /// ephemeral reasoning).
    pub(super) fn reasoning_current_mode(&self) -> bool {
        matches!(
            crate::config::config().display.reasoning_display(),
            crate::config::ReasoningDisplayMode::Current
        )
    }

    /// Slice the just-closed reasoning block out of `streaming_text` and anchor
    /// it as a display-only reasoning message in the transcript flow, exactly
    /// where it streamed. Used in `current` mode: the trace keeps its position
    /// (content below it can only be appended, never inserted above), so the
    /// thought stays readable and anchored until the next user prompt removes
    /// the turn's traces.
    pub(super) fn anchor_current_reasoning_block(&mut self) {
        let block_start = self
            .reasoning_block_start
            .take()
            .unwrap_or(0)
            .min(self.streaming.streaming_text.len());
        // Everything from the block start onward is the reasoning markup. Split it
        // off so the preceding answer text (if any) stays in the live stream.
        let block = self.streaming.streaming_text.split_off(block_start);
        // Drop the separator the open path added before the reasoning block so the
        // surrounding answer text rejoins cleanly.
        while self.streaming.streaming_text.ends_with('\n') {
            self.streaming.streaming_text.pop();
        }
        let block = block.trim_matches('\n').to_string();
        if block.is_empty() {
            self.refresh_split_view_if_needed();
            return;
        }
        // Answer text that streamed *before* the block must commit first so the
        // anchored trace lands after it in the transcript (chronological order).
        if !self.streaming.streaming_text.trim().is_empty() {
            let preceding = self.take_streaming_text();
            let preceding = self.collapse_reasoning_for_commit(preceding);
            if !preceding.trim().is_empty() {
                self.push_display_message(DisplayMessage::assistant(preceding));
            }
        }
        self.turn_reasoning_traces
            .push(crate::tui::app::TurnReasoningTrace {
                display_index: self.display_messages.len(),
                // Snapshot the transcript height when this trace anchors. The trace
                // begins life at the viewport tail; once the transcript grows a
                // full viewport beyond this point the trace is provably off-screen
                // (while tail-following) and can be GC'd without visible motion.
                wrapped_lines_at_anchor: crate::tui::ui::last_total_wrapped_lines(),
            });
        self.push_display_message(DisplayMessage::reasoning(block));
        self.refresh_split_view_if_needed();
    }

    /// Remove the current turn's anchored reasoning traces from the transcript.
    /// Called when the next user prompt is submitted so `current` mode stays
    /// ephemeral across turns: the trace never moves while on screen, it is
    /// simply gone the next time the user acts (a moment when the transcript
    /// reflows anyway).
    pub(super) fn clear_turn_reasoning_traces(&mut self) {
        if self.turn_reasoning_traces.is_empty() {
            return;
        }
        let traces = std::mem::take(&mut self.turn_reasoning_traces);
        let removed = self.remove_reasoning_trace_messages(traces.iter().map(|t| t.display_index));
        if removed > 0 {
            self.bump_display_messages_version();
            self.refresh_split_view_if_needed();
        }
    }

    /// Garbage-collect *stale* reasoning traces (every anchored trace except
    /// the most recent one) that are provably above the tail-following
    /// viewport, so their removal causes zero visible motion. Keeps `current`
    /// mode meaning "the current thought": old thoughts dissolve once they
    /// scroll out of view instead of accumulating across a long agentic turn.
    /// Skipped entirely while the user has scrolled up (their reading position
    /// must not shift).
    pub(super) fn gc_offscreen_reasoning_traces(&mut self) -> bool {
        // Only the traces *before* the most recent one are stale.
        if self.turn_reasoning_traces.len() < 2 {
            return false;
        }
        if self.auto_scroll_paused {
            // User is reading history; never remove anything they might see.
            return false;
        }
        let total = crate::tui::ui::last_total_wrapped_lines();
        let viewport = crate::tui::ui::last_layout_snapshot()
            .map(|l| l.messages_area.height as usize)
            .unwrap_or(0);
        if total == 0 || viewport == 0 {
            return false;
        }
        // A trace anchored when the transcript was `at_anchor` lines tall sits
        // entirely above wrapped line `at_anchor`. While tail-following, the
        // viewport shows the last `viewport` lines, so once the transcript has
        // grown a full viewport past the anchor point (with margin for the
        // separator blank line), the trace cannot be on screen.
        let last = self.turn_reasoning_traces.len() - 1;
        let stale: Vec<usize> = self.turn_reasoning_traces[..last]
            .iter()
            .filter(|t| total.saturating_sub(t.wrapped_lines_at_anchor) > viewport + 2)
            .map(|t| t.display_index)
            .collect();
        if stale.is_empty() {
            return false;
        }
        let removed = self.remove_reasoning_trace_messages(stale.iter().copied());
        if removed > 0 {
            // Re-track surviving traces with adjusted display indices.
            self.turn_reasoning_traces.retain_mut(|t| {
                if stale.contains(&t.display_index) {
                    return false;
                }
                let shift = stale.iter().filter(|&&s| s < t.display_index).count();
                t.display_index -= shift;
                true
            });
            self.bump_display_messages_version();
            self.refresh_split_view_if_needed();
            return true;
        }
        false
    }

    /// Remove reasoning display messages at the given (pre-removal) indices.
    /// Returns how many were removed.
    fn remove_reasoning_trace_messages(&mut self, indices: impl Iterator<Item = usize>) -> usize {
        let mut sorted: Vec<usize> = indices.collect();
        sorted.sort_unstable();
        let mut removed = 0usize;
        for idx in sorted {
            let idx = idx.saturating_sub(removed);
            if idx < self.display_messages.len() && self.display_messages[idx].role == "reasoning" {
                self.display_messages.remove(idx);
                removed += 1;
            }
        }
        removed
    }

    pub(super) fn append_streaming_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Invariant: answer text is never appended *into* an open reasoning
        // region. If a region is still open when real (non-whitespace) answer
        // text arrives, close it first so the next `open_reasoning_region` still
        // inserts its blank-line separator. Without this, a stale
        // `reasoning_streaming` flag makes `open_reasoning_region` early-return
        // and the answer tail gets glued directly onto the next reasoning run
        // (e.g. `...patch + build.Ah, I see...`). Whitespace-only appends (the
        // separators emitted by the reasoning helpers themselves) never trip
        // this. `open_reasoning_region` only appends its separator *before*
        // setting the flag, so this cannot recurse.
        if self.reasoning_streaming && !text.trim().is_empty() {
            self.close_reasoning_region(None);
        }
        self.streaming.streaming_text.push_str(text);
        self.refresh_split_view_if_needed();
    }

    /// Apply a batch of paced [`StreamOp`]s from the segment-aware
    /// [`StreamBuffer`](crate::tui::stream_buffer::StreamBuffer) to the live
    /// streaming view, preserving arrival order across answer text, reasoning
    /// text, and reasoning-region boundaries. Returns true when anything
    /// visible changed.
    pub(super) fn apply_stream_ops(
        &mut self,
        ops: Vec<crate::tui::stream_buffer::StreamOp>,
    ) -> bool {
        use crate::tui::stream_buffer::StreamOp;
        let mut changed = false;
        for op in ops {
            match op {
                StreamOp::Text(text) => {
                    if !text.is_empty() {
                        // `append_streaming_text` enforces the invariant that real
                        // answer text closes any still-open reasoning region first
                        // (so the region's blank-line separator is preserved). The
                        // buffer also queues an explicit CloseReasoning before
                        // non-whitespace text, so this is normally already closed.
                        self.append_streaming_text(&text);
                        changed = true;
                    }
                }
                StreamOp::Reasoning(text) => {
                    if !text.is_empty() {
                        self.append_reasoning_text(&text);
                        changed = true;
                    }
                }
                StreamOp::CloseReasoning => {
                    if self.reasoning_streaming {
                        self.close_reasoning_region(None);
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// In `current` reasoning display mode, reasoning is shown live but collapsed
    /// once the assistant commits a message or runs a tool. Strip any
    /// reasoning-marked lines (identified by [`REASONING_SENTINEL`]) from text
    /// about to be committed to the transcript. Other modes pass through.
    pub(super) fn collapse_reasoning_for_commit(&self, content: String) -> String {
        if !matches!(
            crate::config::config().display.reasoning_display(),
            crate::config::ReasoningDisplayMode::Current
        ) {
            return content;
        }
        strip_reasoning_lines(&content)
    }

    pub(super) fn replace_streaming_text(&mut self, text: String) {
        self.streaming.streaming_text = text;
        self.refresh_split_view_if_needed();
    }

    pub(super) fn clear_streaming_render_state(&mut self) {
        self.streaming.streaming_text.clear();
        self.stream_message_ended = false;
        self.reasoning_streaming = false;
        self.reasoning_pending_line.clear();
        self.reasoning_partial_len = 0;
        // The stream (and any block offset into it) is gone.
        self.reasoning_block_start = None;
        self.refresh_split_view_if_needed();
        self.streaming_md_renderer.borrow_mut().reset();
        crate::tui::mermaid::clear_streaming_preview_diagram();
    }

    /// Reset provider-reported usage that belongs to a transcript being fully
    /// discarded. Render-only resets intentionally preserve these counters,
    /// so full session clears must call this separately.
    pub(super) fn clear_live_usage_state(&mut self) {
        self.streaming.streaming_input_tokens = 0;
        self.streaming.streaming_output_tokens = 0;
        self.streaming.streaming_cache_read_tokens = None;
        self.streaming.streaming_cache_creation_tokens = None;
        self.streaming.streaming_context_stale = false;
        self.streaming.streaming_usage_call_reset_pending = false;
        self.kv_cache.current_api_usage_recorded = false;
    }

    /// Discard all client-side render state for the current streaming attempt:
    /// the live streaming buffer, in-progress tool calls, thinking-line state,
    /// and any assistant transcript messages that were already committed
    /// mid-attempt at tool-call boundaries.
    ///
    /// Used when the provider reports a `RetryRollback`: a transient transport
    /// fault interrupted the response mid-stream and the request is being
    /// replayed from the top, so everything from the aborted attempt must
    /// disappear or the replay would render duplicated output.
    pub(super) fn rollback_streaming_attempt(&mut self) {
        self.stream_buffer.clear();
        self.clear_streaming_render_state();
        self.streaming_tool_calls.clear();
        self.batch_progress = None;
        self.thought_line_inserted = false;
        self.thinking_prefix_emitted = false;
        self.thinking_buffer.clear();
        self.thinking_start = None;
        // Assistant text committed to the transcript during this attempt (a
        // ToolStart boundary commits the pending streamed text) must also go;
        // the retry re-streams the entire response. `push_display_message`
        // counts the trailing run of assistant messages and resets on any
        // user/tool/system fence, so this removes exactly the current
        // attempt's committed segments and never touches earlier turns.
        let to_remove = self.attempt_committed_assistant_messages;
        for _ in 0..to_remove {
            if self
                .display_messages
                .last()
                .is_some_and(|m| m.role == "assistant")
            {
                let idx = self.display_messages.len() - 1;
                self.remove_display_message(idx);
            } else {
                break;
            }
        }
        self.attempt_committed_assistant_messages = 0;
    }

    pub(super) fn take_streaming_text(&mut self) -> String {
        let content = std::mem::take(&mut self.streaming.streaming_text);
        self.stream_message_ended = false;
        self.reasoning_streaming = false;
        self.reasoning_pending_line.clear();
        self.reasoning_partial_len = 0;
        self.reasoning_block_start = None;
        self.refresh_split_view_if_needed();
        self.streaming_md_renderer.borrow_mut().reset();
        crate::tui::mermaid::clear_streaming_preview_diagram();
        content
    }

    pub(super) fn commit_pending_streaming_assistant_message(&mut self) -> bool {
        let ops = self.stream_buffer.flush();
        self.apply_stream_ops(ops);
        // A commit is a hard message boundary: end any still-open reasoning
        // region so `current` mode retains/discards the trace correctly.
        if self.reasoning_streaming {
            self.close_reasoning_region(None);
        }

        if self.streaming.streaming_text.is_empty() {
            self.stream_buffer.clear();
            // Tool-only boundary (no answer text): keep the retained trace on
            // screen so the thought stays readable while the tool runs. It
            // folds when superseded by the next trace or at end of turn.
            //
            // The ephemeral mermaid preview slot mirrors the (now empty) live
            // buffer, so any surviving entry here is stale by definition. The
            // buffer can only become empty without the slot being cleared via
            // `replace_streaming_text` (remote TextReplace, debug snapshot
            // restore); `take_streaming_text` and `clear_streaming_render_state`
            // both clear it themselves.
            crate::tui::mermaid::clear_streaming_preview_diagram();
            return false;
        }

        // `take_streaming_text` also clears the streaming mermaid preview
        // slot, so the whitespace-only early return below cannot leak it.
        let content = self.take_streaming_text();
        let content = self.collapse_reasoning_for_commit(content);
        if content.trim().is_empty() {
            // Nothing left after collapsing reasoning-only content; same
            // tool-only situation as above, keep the trace readable.
            self.stream_buffer.clear();
            return false;
        }
        self.push_display_message(DisplayMessage::assistant(content));
        self.stream_buffer.clear();
        true
    }

    pub(super) fn accumulate_streaming_output_tokens(
        &mut self,
        output_tokens: u64,
        call_output_tokens_seen: &mut u64,
    ) {
        let delta = if output_tokens >= *call_output_tokens_seen {
            output_tokens - *call_output_tokens_seen
        } else {
            // Usage snapshots should be monotonic within one API call. If they are not,
            // treat this as a reset and count the full value once.
            output_tokens
        };
        if self.streaming.streaming_tps_collect_output {
            self.streaming.streaming_total_output_tokens += delta;
            if delta > 0 {
                self.snapshot_streaming_tps();
            }
        }
        *call_output_tokens_seen = output_tokens;
    }
}
