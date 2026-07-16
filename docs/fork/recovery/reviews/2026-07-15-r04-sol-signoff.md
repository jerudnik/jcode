# Independent Sol sign-off: R04 authoritative ledger

Verdict: **FAIL**

Reviewed SHA: `b4d39860abc5337c1937260af13bae45d2405d06`

Scope constraints honored: source/repo read-only; no live daemon, network, credentials, destructive actions, or test execution; no Fable sign-off artifact read.

## IMPORTANT findings

1. **IMPORTANT: the authoritative ledger overclaims a complete terminal-writer census.**
   - Ledger checkpoint 3 says terminal writers are enumerated and ordered, citing only `session.rs`, `session/crash.rs`, `server/swarm.rs`, and `background.rs`: `docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md:50`.
   - The ledger's terminal-writer table likewise lists Session, Swarm member, Detached background, Non-detached background, Reload, and Process markers, but omits disconnect cleanup: `docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md:57-66`.
   - `client_disconnect_cleanup.rs` is an R04 lifecycle terminal writer: it chooses `Closed`/`Crashed`/`Reloading` disposition at `crates/jcode-app-core/src/server/client_disconnect_cleanup.rs:26-35,74-79`, persists terminal session state through `agent.mark_closed()` / `agent.mark_crashed()` at `:106-129`, maps swarm status to `stopped` or `crashed` at `:181-200`, removes the member at `:203-235`, and removes shutdown/background/interrupt state before aborting tasks at `:240-251`.
   - The persistence path is real: `Agent::mark_closed` and `Agent::mark_crashed` call `session.mark_closed()` / `session.mark_crashed()` and persist session state at `crates/jcode-app-core/src/agent.rs:970-1012`; `Session::mark_closed` / `mark_crashed` set terminal status and unregister markers at `crates/jcode-base/src/session.rs:1035-1043`.
   - This was not hidden source evidence: the preserved Grok review explicitly called out this path and its semantic risk at `docs/fork/recovery/seams/R04-session-process-background-lifecycle/grok-review.md:50-66`. Terra did not carry it into the authoritative census or explain why it was excluded.
   - Impact: the ledger can still be substantively correct on retain-fork and strict-pilot scope, but the requested sign-off criterion for a **complete terminal-writer census** is not met. A terminal writer omitted from the authoritative census also weakens the “no overclaim” requirement.

## CRITICAL findings

None found.

## Minor notes

- Review preservation verified: repository copies of Opus and Grok match the ledger SHA-256 values and are byte-identical to `/tmp/jcode-r04-opus-review.md` and `/tmp/jcode-r04-grok-review.md` by `cmp -s`.
- Fixed refs verified: fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, and merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` resolve; `git merge-base fork upstream` returns the recorded base; `git diff --stat fork b4d39860a -- crates src` is empty.
- Two-population authority split verified: `reload.rs`, `reload_recovery.rs`, `background.rs`, and `background/model.rs` are fork/upstream identical by `git diff --quiet`; the fork-specific defense numstat matches the ledger for `headless.rs`, `lifecycle.rs`, `session/crash.rs`, and `active_pids.rs`.
- Terra's strict-pilot versus lifecycle-widening resolution is source-supported by `docs/fork/recovery/RESPONSIBILITIES.md:72-88` and ledger lines `91-110`.
- Claimed exact test symbols exist in source. I did not rerun tests because the task constrained source/repo read-only and no live-daemon side effects. The ledger preserves pass claims but not raw command output beyond the ledger text.
- Widening-contract item 2 uses source-file-style `server::client_lifecycle_tests::...` at ledger line `102`, while the compiled exact identifier is consistent with checkpoint 8's `server::client_lifecycle::tests::...` because `client_lifecycle.rs:3133-3134` includes `client_lifecycle_tests.rs` as `mod tests`.

## Commands run

```bash
pwd
git rev-parse HEAD
git status --short
git log --oneline -1 --decorate
find . -type f \( -iname 'ledger.md' -o -iname '*review*' -o -iname '*r04*' -o -iname '*terra*' -o -iname '*sol*' \) | grep -vi fable
shasum -a 256 docs/fork/recovery/seams/R04-session-process-background-lifecycle/{opus-review.md,grok-review.md}
cmp -s /tmp/jcode-r04-opus-review.md docs/fork/recovery/seams/R04-session-process-background-lifecycle/opus-review.md
cmp -s /tmp/jcode-r04-grok-review.md docs/fork/recovery/seams/R04-session-process-background-lifecycle/grok-review.md
git rev-parse 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d b4d39860abc5337c1937260af13bae45d2405d06
git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b
git diff --stat 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 b4d39860abc5337c1937260af13bae45d2405d06 -- crates src
git diff --quiet 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b -- crates/jcode-app-core/src/server/reload.rs crates/jcode-app-core/src/server/reload_recovery.rs crates/jcode-base/src/background.rs crates/jcode-base/src/background/model.rs
git diff --numstat 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 -- crates/jcode-app-core/src/server/lifecycle.rs crates/jcode-base/src/session/crash.rs crates/jcode-storage/src/active_pids.rs crates/jcode-app-core/src/server/headless.rs
git grep -n "fn reconcile_marks_orphan_from_reloaded_process_failed\|fn cancel_aborts_detached_streaming_turn_with_stale_stop_signal\|fn graceful_shutdown_sessions_signals_all_running_sessions_including_initiator\|fn graceful_shutdown_sessions_times_out_on_partial_checkpoint\|fn reload_interrupted_tool_result" -- crates
nl -ba docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md
nl -ba docs/fork/recovery/seams/R04-session-process-background-lifecycle/{opus-review.md,grok-review.md}
nl -ba crates/jcode-base/src/session.rs
nl -ba crates/jcode-base/src/session/crash.rs
nl -ba crates/jcode-storage/src/active_pids.rs
nl -ba crates/jcode-app-core/src/server/{swarm.rs,reload.rs,reload_recovery.rs,client_disconnect_cleanup.rs,client_lifecycle.rs,reload_tests.rs,client_lifecycle_tests.rs}
nl -ba crates/jcode-base/src/background.rs
nl -ba crates/jcode-base/src/background/tests.rs
nl -ba crates/jcode-app-core/src/agent.rs
nl -ba crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs
git grep -n "R03B\|R05B\|R05A" docs/fork/recovery docs/fork/recovery/seams
git grep -n "provider_session_id\s*=\s*None\|reset_provider_session\|provider_session_id = None" crates
wc -l crates/jcode-app-core/src/server/swarm.rs crates/jcode-app-core/src/server/client_session.rs crates/jcode-app-core/src/overnight.rs
grep -nE '\b(panic!|unwrap\(|expect\()' selected R04 files
```

## Confidence and gaps

Confidence: **high** on the FAIL finding and fixed-ref/review-integrity checks; **medium** on the claimed executed tests because I verified symbols and ledger text but did not rerun tests or find raw preserved logs.

Gaps: no live daemon/reload/UI testing by instruction; no network/credentials; no Fable sign-off artifact read; did not inspect every historical session artifact outside the repo and the two non-Fable `/tmp` review files.
