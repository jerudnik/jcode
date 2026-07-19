# R03 Implementation Evidence

## Outcome

R03 makes terminal-mode ownership explicit across client exec handoff while preserving the seamless success path.

- The outgoing `TuiRuntimeGuard` remains armed while the requested exec action runs. The handoff still exports inherited modes and skips terminal restoration before `exec`, so the successful replacement path does not flicker.
- If process replacement returns an error, the still-armed guard drops during error propagation and runs `cleanup_tui_runtime(..., true)`. The cleanup record proves the sequence is handoff cleanup with `restore_terminal=false`, followed by emergency cleanup with `restore_terminal=true` and all inherited mode bits set.
- A new `InheritedTerminalGuard` is constructed immediately after a valid resuming handoff is recognized, before theme resume and terminal initialization. It owns inherited mouse, kitty keyboard, and focus mode bits through all fallible successor initialization. Ownership transfers only after `TuiRuntimeGuard::new` succeeds.
- Protocol cleanup is centralized in a writer-injectable, best-effort helper. It attempts bracketed-paste disable, focus disable, Crossterm mouse disable including SGR mouse, and kitty keyboard flag pop even if an earlier write fails.
- Unix termination handling now attempts all four protocol disables, disables raw mode, leaves the alternate screen, and shows the cursor.

## Files changed

- `src/cli/terminal.rs`
- `src/cli/tui_launch.rs`

No files outside the R03 ownership set were modified or committed.

## Acceptance gates

### 1. Failed exec replacement restores all modes

Test: `cli::terminal::tests::failed_exec_handoff_restores_all_inherited_modes`

The test injects a mocked requested action returning `Err("mock replace_process failure")` after handoff export. It asserts cleanup records are exactly:

```text
(false, mouse=true, keyboard=true, focus=true)
(true,  mouse=true, keyboard=true, focus=true)
```

It also asserts the runtime guard performed exactly one emergency restore.

### 2. Successor initialization failures are owned

Tests:

- `cli::terminal::tests::inherited_guard_restores_at_every_successor_init_boundary`
- `cli::terminal::tests::inherited_guard_transfers_without_early_cleanup`

Injected error boundaries cover theme resume, raw-mode enable, terminal construction, terminal clear, hook installation, perf policy lookup, bracketed paste, focus, mouse, and runtime-guard construction. An unwind case is also covered. Every failure records one full cleanup with all inherited mode bits. The transfer test proves no early cleanup occurs once the normal runtime guard exists.

### 3. Signal cleanup emits the full disable set

Tests:

- `cli::terminal::tests::protocol_cleanup_emits_all_four_disables`
- `cli::terminal::tests::signal_cleanup_emits_full_disable_set_and_leaves_alt_screen`

Injected writers assert these byte sequences are emitted:

- `CSI ? 2004 l` for bracketed paste off
- `CSI ? 1004 l` for focus reporting off
- `CSI ? 1006 l` within Crossterm mouse capture shutdown for SGR mouse off
- `CSI < 1 u` for kitty keyboard enhancement pop
- `CSI ? 1049 l` for leaving the alternate screen on the signal path

### 4. Existing guards, focused tests, and build

Final commit validation was run in a clean detached worktree at `579d2f072`, sharing only the Cargo target cache with the active worktree.

Passed:

```bash
scripts/dev_cargo.sh test --profile selfdev -p jcode --lib cli::terminal::tests
# 14 passed, 0 failed

scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode
# succeeded
```

The 14 passing terminal tests include the pre-existing guard tests:

- `guard_drop_restores_terminal_when_not_finished`
- `guard_finish_disarms_drop_restore`
- inherited-mode encode/decode and exec-action coverage

Additional broader test observations, unrelated to R03:

- A clean full `jcode --lib` run reached 218 passed and 1 unrelated provider-auth environment failure: `auto_provider_noninteractive_skips_untrusted_external_auth_instead_of_blocking` selected an available OpenRouter credential.
- A clean full `jcode-tui --lib` run reached 1822 passed, 39 failed, and 14 ignored. The failures span existing auth/model-picker, rendering, scroll, Tokio-runtime, and image-protocol tests. R03 does not modify `crates/jcode-tui`.
- Whole-tree validation in the active worktree was temporarily blocked by concurrent uncommitted R01 changes in explicitly prohibited files. The clean detached-worktree build above validates the exact final R03 commits without stashing or modifying that worker's files.

## Commits

- `bd3c4a300 fix(tui): retain terminal ownership across exec`
- `579d2f072 chore(tui): scope signal capture helper to tests`
