# F17 TUI test-rail burndown — worker assignment

Authoritative failing set: 38 (full-suite run against
`target/debug/deps/jcode_tui-4e58b743eb75f292`, 1823 pass / 38 fail / 14
ignored). Hard fork: **prefer FIX over ignore.** These tests assert *our*
deliberately-shipped behavior; repairing them restores real coverage.
`#[ignore]` is acceptable ONLY for genuinely env-sensitive cases that need
test-infra work (a keychain-probe hook, cfg-aware label constants). Every
ignore must carry `#[ignore = "<reason>; <causing-commit-or-env>"]`.

Verify each fix by running the single test against the prebuilt binary with
a scrubbed env:
```
B=target/debug/deps/jcode_tui-4e58b743eb75f292
T=$(mktemp -d); HOME=$T JCODE_HOME=$T $B --exact <full::test::path> --nocapture
```
(Rebuild the binary with `cargo test -p jcode-tui --no-run` only if you
change non-test product code; most fixes are test-only assertion updates.)

Do NOT touch product behavior to satisfy a stale test. If a test encodes a
real regression (product changed in a way we did NOT intend), STOP and flag
it in your report instead of "fixing" it.

---

## TERRA-MAX lane (gpt-5.6-terra, effort max) — UI render / pickers / cache / cosmetic + env-ignores

Known-stale (fix the assertion to match shipped behavior):
- test_model_picker_ctrl_b_bedrock_selection_saves_bedrock_default
  (set-default moved Ctrl+B -> Ctrl+O in 65e1bc30f; send Ctrl+O, rename test)
- test_pinned_content_uses_left_splitter_instead_of_rounded_box
  (pane title now renders "side Pinned", not lowercase "pinned")
- test_usage_report_updates_display_only_card_without_system_message
  (App::new seeds a synthetic Session-Context <system-reminder>; adjust the
  emptiness assertion to the new baseline)
- test_full_prep_cache_state_keeps_two_oversized_width_entries_hot
- test_full_prep_cache_state_retains_oversized_hot_entry
  (FULL_PREP_CACHE_MAX_BYTES raised 8->24MB in f6bc28e64; 12MB fixtures now
  land in the regular cache — update fixture sizes or expected bucket)
- test_prefix_reuse_mid_edit_reoffsets_region_below_boundary
  (equivalence holds; only the stale "taller upsert changes height"
  assumption is wrong)
- test_prompt_jump_ctrl_digit_is_recency_rank_in_app
- test_remote_prompt_jump_ctrl_digit_is_recency_rank
  (prompt-jump lands on positions.last() which is now line 0 for seeded
  content; fix expected offset, key routing is fine)

Env-sensitive (ignore short-term WITH reason):
- test_copy_badge_reserves_right_margin_for_info_widgets
- test_copy_badge_truncates_full_width_line_before_appending_shortcut
  (hardcode Linux "Alt" width; macOS renders "⌥" — ignore on macOS OR make
  the label-width constant cfg-aware; prefer cfg-aware if quick)
- test_prompt_entry_shimmer_color_moves_across_positions
  (TERM=dumb forces 256-color quantization collapsing adjacent blends to
  Indexed(231))

Order-dependent flakes (pass singly, fail in-suite via process globals):
- startup_check_ignores_synthetic_scaffolding_messages
- test_updates_header_repeated_renders_stay_stable_near_scrollbar_threshold
  (OnceLock/auth-cache/thread-local pollution. Prefer a real fix: reset the
  offending global in the test, or serialize with the repo's existing test
  lock. Ignore only if serialization is non-trivial.)

Triage-then-fix (classify first; most are picker-render assertion drift):
- test_local_model_picker_render_shows_antigravity_models_exactly_as_user_sees_them
- test_login_smoke_model_picker_renders_unstacked_provider_rows
- test_model_picker_preview_stays_open_and_updates_filter
- test_model_picker_includes_copilot_models_in_remote_mode
- test_agent_model_picker_inherit_row_uses_provider_default_when_inherited_model_is_unknown
- test_agents_picker_uses_provider_default_when_inherited_model_is_unknown
- test_agents_review_picker_saves_config_override
- test_mouse_click_in_wrapped_input_moves_cursor_to_second_visual_line
- test_chat_drag_into_composer_clamps_to_chat_pane
- test_queued_file_activity_repaint_does_not_leave_trailing_digit_artifact
- test_prompt_preview_reserves_rows_without_overwriting_visible_history
- test_pending_split_launch_shows_processing_status_in_ui
- test_debug_command_side_panel_latency_bench_reports_immediate_redraw
- test_tui_cerebras_paste_key_lifecycle_has_no_degraded_success_messages

---

## SOL-HIGH lane (gpt-5.6-sol-high) — remote dispatch-state / interrupt / judge-launch / auth-route semantics

These encode subtle spawn/reconnect state-machine behavior. Read the
surrounding remote reconnect code before touching assertions; a wrong
"fix" here masks a real durability regression.
- test_new_for_remote_does_not_requeue_acked_pending_soft_interrupts
- test_new_for_remote_fresh_spawn_restores_local_transcript
- test_new_for_remote_restored_interleave_triggers_dispatch_state
- test_new_for_remote_restored_soft_interrupt_resend_triggers_dispatch_state
- test_new_for_remote_restores_spawn_startup_hints_and_dispatch_state
- test_remote_error_with_retryable_pending_schedules_retry
- test_remote_startup_done_event_does_not_cancel_pending_judge_launch
- test_remote_startup_judge_hidden_prompt_dispatches_once_history_is_loaded
- test_remote_current_fpt_live_model_uses_fpt_route_not_copilot_without_cache
- test_remote_cached_oauth_only_claude_route_gains_api_key_route_in_picker
- test_azure_login_completion_switches_local_model_without_completion

---

## Report format (both workers)
Per test: FIXED (file:line + one-line what changed) | IGNORED (reason +
commit/env) | FLAGGED-REAL-REGRESSION (evidence). Do NOT commit; leave
changes in the worktree for the coordinator to review, gate, and commit.
