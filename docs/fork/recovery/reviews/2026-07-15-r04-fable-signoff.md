# R04 Fable independent sign-off

Exact commit: `b4d39860abc5337c1937260af13bae45d2405d06` (`docs: Adjudicate R04 lifecycle seam.`)

Verdict: **FAIL**

I did not read any Sol sign-off. I used source/repo read-only inspection only. I did not use a live daemon, network, credentials, stash replay, ref/worktree mutation, destructive action, or publication. The only write was this file.

## CRITICAL findings

### C1. The ledger's current marker and terminal-writer invariant is false for the `Session::reconcile_dead_owner` path.

The ledger claims the current R04 invariant that "a dead owner becomes durably terminal before its marker can be consumed" and "a stale marker cannot delete a replaced live marker" (`docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md:31`). It then cites `session.rs:39-54`, `session/crash.rs:331-390`, and the marker compare-and-delete guard as terminal-writer evidence (`ledger.md:50-51`, `ledger.md:61`, `ledger.md:66`).

The source supports that invariant only for the `session/crash.rs` marker-scan path, not for `Session::reconcile_dead_owner`:

- `Session::reconcile_dead_owner` calls `detect_crash()` and only then saves: `crates/jcode-base/src/session.rs:1097-1101`.
- `detect_crash()` calls `self.mark_crashed(...)`: `crates/jcode-base/src/session.rs:1075-1080` and `:1083-1090`.
- `Session::mark_crashed()` sets the in-memory status and immediately calls `unregister_active_pid(&self.id)`: `crates/jcode-base/src/session.rs:1040-1044`.
- `unregister_active_pid()` unconditionally removes `active_pids/<session_id>` and `streaming_pids/<session_id>` under the marker lock: `crates/jcode-storage/src/active_pids.rs:67-78`. It does not compare observed bytes and does not re-check liveness.
- The guarded helper exists separately: `remove_active_pid_marker_if_stale_and_matches()` acquires the common lock at `crates/jcode-storage/src/active_pids.rs:167-178`, and the byte/liveness guard is at `:251-263`. `session/crash.rs` uses that helper after a successful save at `crates/jcode-base/src/session/crash.rs:364-370`.

Consequences:

1. If `Session::reconcile_dead_owner()` detects a crash and `session.save()` fails, the marker has already been consumed by `mark_crashed()`. This contradicts the ledger's save-before-consume invariant.
2. If a stale reader loads an active session, then a live successor re-registers the same session marker before `mark_crashed()`, the unguarded `unregister_active_pid()` can delete the replacement marker by session id. This contradicts the stale-marker replacement invariant. The test `conditional_cleanup_preserves_a_replaced_live_marker` covers only the guarded helper (`crates/jcode-storage/src/active_pids.rs:483-506`), not `mark_crashed()` or `reconcile_dead_owner()`.

This is not merely a missing future fixture. It is a current source path that the ledger explicitly lists in its terminal writer census and current invariant. R04 cannot be signed off as written until the ledger is narrowed or the code and tests are fixed.

## IMPORTANT findings

### I1. The terminal-writer census omits `client_disconnect_cleanup`, which is an R04 terminal writer and also uses the unguarded crash-marking path.

The ledger's terminal writer table lists session reconciliation, swarm member sweep, background tasks, reload, and process markers (`ledger.md:57-66`), but it does not list the disconnect cleanup path. That path is material to R04's owned create/attach/cancel/shutdown/terminal-state surface (`RESPONSIBILITIES.md:26`).

Source evidence:

- `cleanup_client_connection()` classifies disconnects as `Closed`, `Crashed`, or `Reloading`: `crates/jcode-app-core/src/server/client_disconnect_cleanup.rs:19-35` and `:73-80`.
- It marks the agent session closed or crashed: `client_disconnect_cleanup.rs:113-129`.
- It maps the swarm member to `stopped` or `crashed`: `client_disconnect_cleanup.rs:181-200`.
- If the agent lock is stuck, it logs and skips graceful session marking after a two-second timeout: `client_disconnect_cleanup.rs:110-176`.
- `Agent::mark_crashed()` calls `self.session.mark_crashed(message)` and only afterwards persists best-effort when messages are non-empty: `crates/jcode-app-core/src/agent.rs:1002-1012`. That inherits C1's unguarded marker consumption before durable terminal persistence.

This omission matters because the ledger says terminal writers are enumerated and ordered (`ledger.md:50`, `ledger.md:57-66`). The final adjudication preserved Grok's concern about this path in the copied review, but the authoritative ledger did not carry it into the terminal-writer census or current invariant analysis.

## Minor notes and supported checks

- Review preservation: **supported**. `/tmp/jcode-r04-opus-review.md` and the repository copy both hash to `56d7cd1e16d72cf19e9885e7820252a5a863345bab51a846a93456bae7d149fa`; `/tmp/jcode-r04-grok-review.md` and the repository copy both hash to `9113aab3a404196c6f5b998e9573ca451a63cac6799a53fe3af8933681b6906d`; both `cmp -s` checks passed.
- Strict no-tool pilot exclusion: **supported from R04's scope**. `RESPONSIBILITIES.md:74-88` defines one fixture-backed no-tool turn and says R04 is not a prerequisite unless reload, resume, cancellation, or detached/background task is added. R04 ledger `:93-96` correctly does not override R01/R03A/R12/R09 blockers.
- Conditional lifecycle widening blockers: **supported and should remain blockers**. The wait-like reload interruption path is explicitly non-error for `bg wait`, `swarm await_members`, and `swarm run_plan`: `turn_streaming_mpsc.rs:45-67`, `:1530-1577`, and tests at `:1681-1702`. The ledger correctly requires a downstream evidence/rendering contract before widening (`ledger.md:105`, `:117-120`).
- Reload handoff and orphan handling: **mostly supported by static read**. Background detached completion only succeeds on exit code `0` and owner-tagged orphan records become `Failed` (`background.rs:139-210`, `:231-302`). Reload persists intents before shutdown and delivers only on exact accepted continuation (`reload.rs:95-137`, `:210-310`; `reload_recovery.rs:234-241`, `:286-364`). C1 remains the blocker for the marker/terminal invariant.
- Exact test names: **source identifiers exist**. The four cited lifecycle tests are present at `background/tests.rs:345`, `client_lifecycle_tests.rs:393`, `reload_tests.rs:155`, and `reload_tests.rs:588`; `client_lifecycle.rs:3132-3134` wires the first app-core test file as `server::client_lifecycle::tests`, and `reload.rs:514` wires `server::reload::reload_tests`. I did not rerun Cargo tests because this sign-off was constrained to source/repo read-only inspection.
- R09/no-update: **supported for this commit**. `git show --name-only b4d39860a` lists only the three R04 docs files, and no gate script, ratchet, baseline, source, or Cargo path was changed.
- R13 reset census challenge: **no important mismatch found**. The R04 reset sites named by R04/R13 are present, including `overnight.rs:188`, `server/client_actions.rs:698`, `session/crash.rs:139`, and `conversation_state.rs:835-836`. The broader `provider_session_id` grep shows the expected R02/R04/R12/R13 classes.
- R05B/R03B challenge gap: there is no R05B or R03B authoritative seam ledger present at this commit. R04's cross-seam statements align with `RESPONSIBILITIES.md:25-28` and `:67`, but I could only challenge them against the responsibility map and preserved reviews, not against a finalized R05B/R03B ledger.

## Commands run

Read-only commands, excluding ordinary `git show | nl` source inspection variants:

```bash
pwd && git rev-parse HEAD && git status --short && git show -s --format='%H %s' b4d39860a
find . -maxdepth 4 -type f \( -iname '*r04*' -o -iname '*ledger*' -o -iname '*seam*' \) | sed 's#^./##' | sort | grep -vi 'sol' | head -200
git ls-tree -r --name-only b4d39860a | grep -Ei '(^|/)(R04|r04|ledger|seam|signoff|sign-off|lifecycle|session|pilot|reload|orphan|reset|marker|writer)' | grep -vi 'sol'
git grep -n -i --heading -e 'R04' -e 'authoritative' -e 'terminal writer' -e 'pilot' -e 'orphan' -e 'R09' b4d39860a -- ':!**/*[Ss]ol*' ':!**/sol/**'
shasum -a 256 /tmp/jcode-r04-opus-review.md /tmp/jcode-r04-grok-review.md
git show b4d39860a:docs/fork/recovery/seams/R04-session-process-background-lifecycle/{opus-review.md,grok-review.md} | shasum -a 256
cmp -s checks for the two external review files against the repository copies
git grep -n -E 'mark_(crashed|closed|stopped|completed|failed)|set_status\(|SessionStatus::(Crashed|Completed|Closed|Failed|Cancelled)' b4d39860a -- crates/jcode-app-core/src crates/jcode-base/src
git grep -n -E 'fn (reconcile_marks_orphan_from_reloaded_process_failed|cancel_aborts_detached_streaming_turn_with_stale_stop_signal|graceful_shutdown_sessions_signals_all_running_sessions_including_initiator|graceful_shutdown_sessions_times_out_on_partial_checkpoint)' b4d39860a -- crates
git grep -n 'unregister_active_pid' b4d39860a -- crates/jcode-base/src crates/jcode-storage/src
git grep -n -E 'reconcile_dead_owner|save-before|remove_active_pid_marker_if_stale|mark_crashed\(|replaced.*marker' b4d39860a -- crates/jcode-base/src crates/jcode-storage/src
git show --stat --oneline --name-only b4d39860a
git show --name-only --format='' b4d39860a | grep -E '(^scripts/|ratchet|baseline|quality|Cargo|crates/|src/)' || true
git grep -n 'provider_session_id' b4d39860a -- '*.rs' | grep -v '_tests' | grep -v '/tests/'
```

## Confidence and gaps

Confidence: **high** for C1 and I1 because they are direct source-control-flow contradictions of the ledger's current terminal/marker claims. **Medium-high** for the supported reload/orphan/reset/pilot boundary checks because I inspected source but did not execute tests. **Medium** for exceptional multi-daemon interleavings beyond the specific C1 race, because this was static read-only review.

Gaps: I did not run Cargo tests, launch a daemon, inspect live runtime state, use network/credentials, or read any Sol sign-off. I did not validate external test-pass logs beyond confirming the cited test identifiers exist and are wired into the expected modules.
