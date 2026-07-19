# R03 Independent Adversarial Review

**Reviewer model:** claude-opus-4-8 (independent adversarial reviewer, read-only except this file)
**Node:** R03 — explicit TUI terminal-mode ownership across exec handoff
**Repo:** `/Users/jrudnik/labs/jcode` @ HEAD `377e0ade3`
**Implementation commits reviewed:** `bd3c4a300`, `579d2f072`, `a0676f781` (all confirmed ancestors of HEAD)
**Files under review:** `src/cli/terminal.rs`, `src/cli/tui_launch.rs`

---

## Verdict summary

I independently reread the incident analysis, reviewed all three diffs against the pre-R03 baseline, traced every exec action and every fallible successor-init boundary in production code, and reran the acceptance suite myself. All four acceptance gates hold. The design closes the exact stranding window described in the incident: the outgoing guard now stays armed across `replace_process`, and an early successor guard owns inherited modes before any fallible init. I found no blocking defect. Four non-blocking observations are recorded below.

---

## Independent test + build verification (I reran these)

```
scripts/dev_cargo.sh test --profile selfdev -p jcode --lib cli::terminal::tests
=> 14 passed; 0 failed; 0 ignored   (compiled jcode 0.46.0, selfdev)

scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode
=> Finished `selfdev` profile in 15.83s (clean, no warnings surfaced)
```

The passing set includes the pre-existing guard tests `guard_drop_restores_terminal_when_not_finished` and `guard_finish_disarms_drop_restore`, plus the roundtrip/reject/handoff-metadata/exec-action tests. Gate 4 satisfied on my own run, not merely on the author's detached worktree.

---

## Gate-by-gate independent findings

### Gate 1 — Failed exec after handoff export emits all four disables + restore

`finish_for_run_result` (terminal.rs:151-167), exec branch:
1. `export_tui_exec_handoff` sets env + logs (no terminal writes).
2. `cleanup_tui_runtime(&state, false)` — `restore_terminal=false` skips the entire `if restore_terminal` block, so **no** protocol writes, **no** `ratatui::restore()`. Purely a log + image-state clear.
3. `action()?` returns `Err` and propagates via `?` **before** `self.armed = false`, so the guard stays armed.
4. `Drop` (terminal.rs:170-182) fires `cleanup_tui_runtime(&state, true)` → `write_terminal_protocol_cleanup` (all four disables) + `ratatui::restore()`.

Test `failed_exec_handoff_restores_all_inherited_modes` asserts the record sequence `[(false,t,t,t),(true,t,t,t)]` and `GUARD_DROP_RESTORES == 1`. I confirmed the assertion matches the code path. **PASS.**

### Gate 2 — Injected failure at each successor init boundary before guard construction

The `InheritedTerminalGuard` is constructed at terminal.rs:302-303, **before** theme resume, terminal init, env removal, and mode reassertion; ownership transfers only at terminal.rs:382-385 after `TuiRuntimeGuard::new`. I traced every real fallible `?` boundary in `init_tui_runtime` and confirmed the inherited guard is live across all of them:

- `init_tui_terminal(inherited_terminal)?` (line 312) — covers `enable_raw_mode`, `Terminal::new`, `terminal.clear` inside `init_tui_terminal_resume`.
- `EnableBracketedPaste` (335), `EnableFocusChange` (337), `EnableMouseCapture` (340).

Each `?` on error drops the still-armed inherited guard → `cleanup_tui_runtime(&state, true)`. Env removal at 320-321 only strips metadata; the guard already copied the mode bits at construction (line 19-28), so post-removal failures still restore. The `.expect("validated handoff modes")` at line 303 is sound: `inherited_terminal == is_resuming && inherited_modes.is_some()`, so it cannot panic.

Tests `inherited_guard_restores_at_every_successor_init_boundary` (10 boundaries + unwind, each asserting `[(true,t,t,t)]`) and `inherited_guard_transfers_without_early_cleanup` pass. **PASS** (see Observation 1 on test shape).

### Gate 3 — Signal-path cleanup emits the full disable set

`restore_signal_terminal` (terminal.rs:461-475) calls `write_terminal_protocol_cleanup(..., inherited_all_modes_state())` (all modes true → superset), then `disable_raw_mode` via the injected closure, then `LeaveAlternateScreen` + `Show`. `handle_termination_signal` (line 580-592) routes through it. Tests assert emission of `\x1b[?2004l`, `\x1b[?1004l`, `\x1b[?1006l`, `\x1b[<1u` (PopKeyboardEnhancementFlags), and `\x1b[?1049l`, plus that raw-mode disable ran. This is a genuine improvement over the pre-R03 handler, which the incident report (section 4) flagged as emitting only `disable_raw_mode` + `LeaveAlternateScreen`. **PASS.**

### Gate 4 — Pre-existing guards pass, build succeeds

Confirmed above on my own run. **PASS.**

---

## Adversarial focus findings

### (a) Double-cleanup: is any terminal state toggled twice, visibly or harmfully?

No. In the failed-exec sequence the `restore_terminal=false` call is **inert for terminal state** (it only logs and clears image state), and the `restore_terminal=true` drop call performs the single real restore. Net terminal toggles: exactly one. The kitty `PopKeyboardEnhancementFlags` fires exactly once, correctly undoing the single ancestor-process push that survives across `exec` (kitty enhancement is terminal-side stack state, so one pop per inherited level is correct — this is not "popping flags never pushed").

I checked the only path that could double-arm: between `TuiRuntimeGuard::new` (line 377) and `transfer_to` (line 383) both the inherited guard and the runtime guard are momentarily armed. There is **no fallible operation** (no `?`, no realistically-panicking call) between those two statements — just a struct construction and a match/move — so an unwind cannot occur in that window and a double `cleanup(true)`/double-pop is unreachable in practice. `transfer_to` disarms the inherited guard (line 31), and its own `Drop` is then a no-op, verified by `inherited_guard_transfers_without_early_cleanup` (records empty after transfer). A signal racing the failed-exec window is also safe: the handler does a full best-effort disable and `process::exit`, so the drop path does not additionally run; even redundant DECSET resets are idempotent no-ops.

### (b) Success path: can Drop run during a successful exec, or via a pre-exec early return?

On success, `replace_process` → `execvp` never returns, so `self.armed = false` at line 165 is never reached and no `Drop` runs (the whole process image is replaced). Safe.

I audited every exec action reachable through `execute_requested_action` (hot_exec.rs:15-33) for a pre-exec `Ok(())` early return:
- `hot_reload`, `hot_restart`, `hot_update`, `exec_rebuilt_session` all terminate in either a never-returning `replace_process` or an `Err(...)`. None returns `Ok(())` without execing.
- The exec branch of `finish_for_run_result` is only taken when `run_result_will_exec` is true, which requires one of reload/rebuild/update/restart to be `Some`; `execute_requested_action` then calls the matching `hot_*`, which cannot return `Ok`.

So the invariant "exec branch ⇒ `action()` never returns `Ok`" holds today, and the guard is never disarmed without a restore on this path. See Observation 2 for the fragility of that coupling.

### (c) Guard ordering vs theme resume and `JCODE_TUI_INHERITED_MODES` removal

The inherited guard is constructed (line 302-303) strictly **before** `init_theme_mode_for_resume` (304-311), before `init_tui_terminal` (312), and before `remove_var(INHERITED_MODES_ENV)` / `remove_var(INHERITED_THEME_ENV)` (320-321). The env vars are read into locals at 287-289 and the mode bits are copied into the guard at construction, so consuming/removing the env after guard construction strands nothing: if construction of the runtime guard or any intervening step fails, the armed inherited guard owns and restores the modes. Ownership of the modes is unambiguous at every point. Sound.

### (d) Non-handoff (fresh start) path unaffected

For a fresh start `inherited_terminal` is false, so `inherited_guard` is `None` (the `.then(...)` at line 302 yields `None`), `init_theme_mode()` runs the normal OSC query, `init_tui_terminal(false)` calls `ratatui::init()`, modes are freshly pushed, and the final `match inherited_guard { None => runtime_guard }` returns the runtime guard unchanged. This is behaviorally identical to the pre-R03 `Ok((terminal, TuiRuntimeGuard::new(...)))`. The tui_launch.rs restructure also preserves the fresh normal-exit path: `run_result_will_exec` false → `cleanup_tui_runtime(&state, true)` full restore, then a no-op `execute_requested_action`, then resume-hint printing. Additionally, the new exit-code branch now does `finish(true)` (full restore) before `process::exit`, which is strictly safer than the prior ordering. No regression.

### (e) Windows / non-unix compile and behavior

The cfg partition is clean:
- `restore_signal_terminal`, `inherited_all_modes_state`, `handle_termination_signal`, `signal_crash_reason`, and the real `spawn_session_signal_watchers` are all `#[cfg(unix)]`; a `#[cfg(not(unix))]` no-op `spawn_session_signal_watchers` exists (line 624-625).
- `inherited_all_modes_state` is referenced only inside `restore_signal_terminal` (both unix), so no unused-symbol/dead-code warning on Windows.
- `write_terminal_protocol_cleanup` is **not** cfg-gated and is invoked by `cleanup_tui_runtime` on all platforms, so Windows still gets the full four-disable protocol cleanup on normal/guard-drop teardown.
- Test scaffolding (`CLEANUP_RECORDS`, `SUPPRESS_REAL_CLEANUP`) is `#[cfg(test)]`; production always runs real cleanup (`#[cfg(not(test))] let suppress_real_cleanup = false`).

I could not cross-compile to a Windows target on this macOS aarch64 host, but the cfg structure is well-formed and the only platform-specific surface is the signal path, which Windows never reaches. No behavioral change for non-unix.

---

## Non-blocking observations

1. **Gate-2 test is a structural proxy, not true fault injection.** `inherited_guard_restores_at_every_successor_init_boundary` constructs an `InheritedTerminalGuard` and immediately returns `Err`/panics; it does not inject a failure into `init_tui_runtime` at each real boundary. It also enumerates non-fallible "boundaries" (`theme_resume`, `perf_policy`, `hook_install`, `runtime_guard_construction`) that are not `?` points in production. The test therefore proves "an armed guard restores on early return," and I separately confirmed by inspection that the guard is live across every *real* fallible boundary. Correct in aggregate, but a future refactor that moves guard construction later, or adds a `?` before it, would not be caught by this test. Consider a real injection seam in `init_tui_runtime`.

2. **`finish_for_run_result` exec branch depends on an unenforced invariant.** Line 165 (`self.armed = false`) executes if `action()` returns `Ok(())` in the exec branch. Today every exec action ends in Err-or-never-return, so this is unreachable; but if a future exec action returned `Ok(())` without execing, the guard would disarm without restoring, re-creating the incident. A debug assertion or a comment binding `run_result_will_exec` to "action never returns Ok" would harden this coupling.

3. **Theoretical double-armed window** between `TuiRuntimeGuard::new` and `transfer_to` is unreachable via unwind (no fallible ops between them), so no double-pop occurs. Noted only for completeness; not actionable.

4. **Signal cleanup writes DECSET resets to stderr** while normal enable/cleanup uses stdout. Both fds target the same TTY so the resets reach the terminal; this matches pre-R03 behavior and is acceptable best-effort at signal time. Not a regression.

---

## Conclusion

The implementation faithfully realizes the incident report's minimal fix: (1) retain guard ownership until exec has replaced the process, (2) install an early successor handoff guard before any fallible init, (3) unify full mode restoration on the signal path. All four acceptance gates independently verified (14/14 tests pass, build clean). Adversarial probes on double-cleanup, success-path Drop, guard/env ordering, fresh-start neutrality, and non-unix compilation surfaced no blocking defect. The four observations above are quality/hardening notes, not blockers.

VERDICT: PASS
