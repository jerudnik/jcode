# Terminal-mode garbage during server reload

**Scope:** read-only investigation of `/Users/jrudnik/labs/jcode` at `a87c5f27128f9a1634d5a0613c05ddfc198a3f45` (2026-07-18 21:31 -0400). No repository files were modified, built, or reloaded. Source references below are 1-based and pinned to that HEAD. Runtime evidence is from `~/.jcode/logs/jcode-2026-07-18.log`.

## Executive conclusion

This was not the server writing keyboard/mouse reports to Monkey's terminal. The server fan-outs a `ServerEvent::Reloading` to **every live attached client**, including the non-trigger Monkey TUI. Because Monkey was a self-dev, idle remote client with a newer client binary, receiving that event requested a **client self-re-exec**. That behavior is intentional, but its terminal-handoff ownership is unsafe.

The old Monkey TUI (PID 35219) deliberately exported `JCODE_TUI_INHERITED_MODES=mouse=1,keyboard=1,focus=1`, deliberately skipped terminal cleanup, and disarmed its RAII guard *before* it attempted its exec. The next client process either failed to arrive at normal initialization or failed before the guard was constructed. The log has the handoff and no corresponding `phase=initialized` for PID 35219. Thus no owner emitted the mode-off sequences. When the process subsequently returned to a cooked, echoing terminal state, Ghostty correctly delivered still-enabled kitty keyboard, SGR mouse, and focus reports as input bytes. Cooked echo rendered those bytes as red literal text.

The directly demonstrated defect is: **the old guard is disarmed before an attempted `exec`, while the new process does not have an early, pre-initialization cleanup guard.** An exec failure and every fallible successor-startup step prior to `TuiRuntimeGuard::new` strands terminal modes. This is an error-return path, not a Rust unwind, so neither the panic hook nor `Drop` can repair it.

## 1. Server reload versus attached TUI behavior

### Server path

1. `handle_reload` gathers every swarm member with nonempty `event_txs`, then sends each a `ServerEvent::Reloading` (`crates/jcode-app-core/src/server/client_session.rs:760-785`). It does **not** limit this to `triggering_session`.
2. It sends the daemon reload signal (`:787-801`).
3. `await_reload_signal` drains work, then invokes `replace_process` on `jcode serve --socket ...` (`crates/jcode-app-core/src/server/reload.rs:152-202`). This is a server process exec only. It does not exec attached client TUI processes.
4. The server log proves this fanout: at `21:38:15.420`, `handle_reload` reports `triggering_session=session_dolphin...`, `reload_notified_sessions=3`, and `reload_notified_clients=3`. At `21:38:16.289` the server execs its shared-server binary and at `21:38:18.721` publishes `SocketReady`.

### Attached remote TUI path

1. A TUI receiving `ServerEvent::Reloading` marks `server_reload_in_progress`, records “server reload in progress,” forwards the event to the app, and otherwise continues (`crates/jcode-tui/src/tui/app/remote.rs:765-773`). A socket disconnect causes its outer loop to reconnect (`:746-764`; `run_shell.rs:581-595`). So the ordinary response is **wait/reconnect/re-attach**, not an automatic exec.
2. The server event handler also calls `maybe_self_reload_after_server_reload` (`crates/jcode-tui/src/tui/app/remote/server_events.rs:1444-1452`). That special path changes the answer for Monkey: it requests a client re-exec if the TUI is remote, self-dev/canary, idle, and has a newer binary (`tui_lifecycle_runtime.rs:210-232`). It sets `reload_requested` and `should_quit` (`:225-231`).
3. `run_remote` turns that into `RunResult { reload_session: ... }` (`crates/jcode-tui/src/tui/app/run_shell.rs:625-636`). `run_tui_client` then calls `finish_for_run_result`, followed by `execute_requested_action` (`src/cli/tui_launch.rs:144-158`). `execute_requested_action` calls `hot_reload` (`src/cli/hot_exec.rs:15-32`), which sets `JCODE_RESUMING=1` and calls `replace_process` on the preferred binary (`:54-124`).

**Answer:** a normal attached TUI re-attaches after the daemon restarts. An idle self-dev TUI with a newer client binary, such as Monkey, additionally exits its app loop and **execs itself**. The server itself never directly restarts/execs the client.

## 2. Terminal-state handoff and skipped teardown

### Setup and handoff

* Normal init enables kitty keyboard enhancement through `PushKeyboardEnhancementFlags(DISAMBIGUATE_ESCAPE_CODES | REPORT_EVENT_TYPES)` (`crates/jcode-tui/src/tui/mod.rs:81-116`), bracketed paste, focus change, and mouse capture (`src/cli/terminal.rs:292-309`).
* `finish_for_run_result` detects an action that will exec, exports modes, invokes cleanup with `restore_terminal=false`, then sets `armed=false` (`src/cli/terminal.rs:111-120`). `run_result_will_exec` returns true for reload/rebuild/update/restart (`:370-376`).
* `export_tui_exec_handoff` puts the three mode bits in `JCODE_TUI_INHERITED_MODES` and logs the exact handoff (`src/cli/terminal.rs:378-394`).
* The successor accepts the handoff only when `JCODE_RESUMING` and valid inherited modes are both present (`:67-72`, `:239-251`). It enables raw mode and builds/clears a terminal through `init_tui_terminal_resume` (`:413-427`), reasserts bracketed paste/focus/mouse without pushing kitty keyboard again (`:276-290`), and finally constructs the guard at `:325-332`.

### Exact teardown which was skipped

The skipped source-level teardown is `cleanup_tui_runtime(..., true)` at `src/cli/terminal.rs:335-357`:

```rust
DisableBracketedPaste;
DisableFocusChange;       // when state.focus_change
DisableMouseCapture;      // when state.mouse_capture
tui::disable_keyboard_enhancement(); // PopKeyboardEnhancementFlags
ratatui::restore();
```

`DisableMouseCapture` is Crossterm's mouse-protocol shutdown, including disabling SGR extended mouse reporting (`DECSET ?1006` is reset with `CSI ?1006 l`) and Crossterm's other capture modes. `PopKeyboardEnhancementFlags` exits the kitty keyboard enhancement stack. The remaining `ratatui::restore()` restores terminal/raw/alternate-screen state. The observed literal forms match these exact categories:

* `CSI ... u` fragments, such as `1:1A;1:3A`, are kitty keyboard reports.
* `CSI < ... M` fragments, such as `14;1M35;14;2M35`, are SGR mouse reports.
* Focus reporting was also enabled, though no particular focus fragment was required to establish the failure.

At `21:38:15.453` the log records exactly the opposite of teardown for PID 35219:

```
phase=exec_handoff ... raw_mode=true modes=mouse=1,keyboard=1,focus=1
phase=cleanup ... restore_terminal=false raw_mode=true mouse_capture=true keyboard_enhanced=true focus_change=true
```

No `phase=cleanup ... restore_terminal=true` follows for that PID in the incident window.

## 3. Monkey-specific causal chain

1. PID 35219 is the Monkey client. It initialized at `04:14:51.217`; Monkey's session was created and subscribed immediately afterward at `04:14:52.540` (`session_monkey_1784362492491_7e9347f17f5a28ce`). Its environment snapshot says `is_selfdev=true`.
2. The distinct trigger was Dolphin, not Monkey. The log names `session_dolphin_...` as `triggering_session` at `21:38:15.420`.
3. Monkey nevertheless received the reload fanout because all three live clients were notified. Its session was then closed/stopped in the server's reload-side session processing at `21:38:15.479-15.481`.
4. As the self-dev client, Monkey followed the `maybe_self_reload_after_server_reload` route. At `21:38:15.453`, PID 35219 performed the terminal exec handoff and explicitly did not restore terminal modes. The next log is `Reloading with binary built 53 seconds ago`.
5. The daemon did successfully exec and reach SocketReady. There is a later `jcode starting` at `21:38:17.486`, consistent with the successor launch, but there is **no** `TUI_TERMINAL_MODES phase=initialized pid=35219 resuming=true handoff=true` and no normal full cleanup. This means the terminal mode handoff did not reach a healthy, guarded successor lifecycle.
6. After that owner disappeared, the terminal eventually operated in cooked/echoing mode but still had kitty keyboard and mouse reporting enabled. User actions then caused the terminal emulator to send protocol bytes, which shell/cooked echo displayed literally. This explains why garbage appeared in the non-trigger Monkey window.

There is a second matching handoff for PID 5434 at `21:38:18.874`, likewise with `restore_terminal=false`; it reinforces that reload fanout can cause multiple attached client re-execs. The report does not need to attribute the Monkey display symptom to PID 5434 because PID 35219 is directly linked to Monkey's original session and is the first stranded handoff.

### If handoff aborts, who cleans up?

Today, effectively **nobody** reliably does:

* Before `replace_process`, the old process has disarmed `TuiRuntimeGuard`.
* If `replace_process` returns an error, `hot_reload` returns `Err` (`src/cli/hot_exec.rs:114-129`); `run_tui_client` returns that error after its guard is already disarmed (`src/cli/tui_launch.rs:152-158`).
* If exec succeeds but the successor fails in `init_tui_terminal_resume`, `Terminal::new`, `terminal.clear`, hook installation, policy lookup, or any `?` before `TuiRuntimeGuard::new` at `src/cli/terminal.rs:325`, no successor guard exists. The handoff env vars are removed at `:266-269`, but removing metadata does not disable terminal modes.

The latter pre-guard startup failure is particularly consistent with the log pattern: a successor starts but never produces the post-init terminal-mode event.

## 4. Panic hook and Drop guard

There is a `TuiRuntimeGuard` whose `Drop` calls full cleanup when still armed (`src/cli/terminal.rs:74-87`, `:123-134`). `run_tui_client` also deliberately relies on it when `app.run_remote` returns an error (`src/cli/tui_launch.rs:144-150`). Tests cover dropping an armed guard (`src/cli/terminal.rs:593-604`).

There is a panic hook (`src/cli/terminal.rs:145-168`), but it records crash/session information only. It does not restore terminal modes. On an ordinary Rust panic while an **armed** guard is in scope, unwinding runs `Drop`, which does restore. That is not this path:

* The old process used the intentional successful-result pathway, calls `finish_for_run_result`, and disarms before exec.
* An exec failure is an `Err`, not a panic.
* A successor failure before guard construction cannot invoke that guard's `Drop`.
* `SIGKILL`/abrupt process loss also cannot run either hook or Drop. The custom termination-signal handler only disables raw mode and leaves alternate screen/cursor (`src/cli/terminal.rs:463-509`); it does not emit DisableMouseCapture, DisableFocusChange, PopKeyboardEnhancementFlags, or DisableBracketedPaste, so it is not a complete mode cleanup fallback either.

## Minimal fix proposal (do not implement in this investigation)

1. **Retain ownership until exec is known to have replaced the process.** Change the API boundary so the terminal guard remains armed while `replace_process` is attempted. If it returns, invoke full `cleanup_tui_runtime(..., true)` before returning the error. Do not call a method that irrevocably disarms the guard before an operation that can return.
2. **Install an early successor handoff guard before any fallible initialization.** Immediately after parsing a valid `JCODE_TUI_INHERITED_MODES`, create a small handoff-cleanup guard that owns the inherited mode bits. It must full-cleanup on every early `Err`/unwind between `init_tui_terminal_resume` and construction of the regular runtime guard. Transfer/disarm it only once the normal guard exists.
3. **Use one full restoration primitive for all exit/signal paths.** Signal handling should use best-effort full mode cleanup (including mouse, focus, kitty keyboard, bracketed paste, raw mode/alternate screen), not merely `disable_raw_mode` plus `LeaveAlternateScreen`.
4. **Add deterministic tests.** Mock the exec replacement to return an error after `finish_for_run_result` would formerly have disarmed the guard. Assert a cleanup log/write includes the four protocol disables and `ratatui::restore`. Add an injected failure at each successor init boundary, especially `terminal.clear`, and assert inherited modes are disabled. Tests should also assert a non-trigger self-dev TUI receiving `Reloading` takes the client-reload path only when all four predicate conditions hold.

A conservative alternative is to restore modes before every client exec and let the successor initialize normally. That removes the handoff hazard but may visibly flicker. The two-guard transfer preserves the intended seamless handoff while making an aborted handoff safe.

## Confidence / limits

High confidence in the fanout, Monkey PID correlation, explicit skipped cleanup, and the unowned abort windows: all are directly sourced or logged. Medium confidence that the immediate successor failure was specifically before `TuiRuntimeGuard::new`; logging proves it did not reach the normal initialized event, but the log does not preserve the exact error/cause. No panic or signal record was found in the incident window, so neither is required for the causal chain.
