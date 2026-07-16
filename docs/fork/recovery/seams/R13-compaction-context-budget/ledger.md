# R13 Compaction and context-budget policy: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `f5a8999d81311d237d1c106a9d980fd86fa34b6e`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light` (pilot conditional per `RESPONSIBILITIES.md`; this ledger discharges pilot prerequisite 6) |
| Research budget | 10 decisive checkpoints for the R06A/R07C/R13 batch; shared batch consumed 10 |
| Recommended disposition | `retain-fork` |
| Confidence | high for the pilot avoidance proof and writer census; medium for full 413-recovery behavior (see gaps) |

R13 owns token/image estimation, thresholds and modes, emergency truncation, 413 recovery, recent-turn preservation, and post-compaction invalidation of provider session/cache/tool state. It excludes provider route selection (R02), evidence storage (R06A), and memory recall (R06B). The R00/R09/R11 overlays bind this ledger. Cross-seam invariant #3 requires this ledger to enumerate and classify every `provider_session_id` writer and reset site across R02, R04, R12, and R13.

## Findings

| Finding | Evidence | Consequence |
|---|---|---|
| Compaction policy is fork-dominant | `git diff --stat 631935dd..HEAD -- crates/jcode-compaction-core crates/jcode-base/src/compaction.rs`: fork adds `Summary::advance` (`jcode-compaction-core/src/lib.rs:96-115`) and ~150 lines in `compaction.rs`; upstream shows zero changes to these files since the merge base | Nothing upstream to adopt at the fixed refs; `retain-fork` |
| Thresholds are explicit constants | `crates/jcode-compaction-core/src/lib.rs:6-19`: `DEFAULT_TOKEN_BUDGET = 200_000`, `COMPACTION_THRESHOLD = 0.80`, `CRITICAL_THRESHOLD = 0.95`, `MANUAL_COMPACT_MIN_THRESHOLD = 0.10`, `RECENT_TURNS_TO_KEEP = 10` | The pilot avoidance proof below is arithmetic, not probabilistic |
| The short pilot cannot trigger compaction | `should_compact_with` (`crates/jcode-base/src/compaction.rs:857-873`) requires `active.len() > RECENT_TURNS_TO_KEEP` (10 turns) before any threshold math applies; the pilot is one no-tool turn, and automatic thresholds additionally need ~160k estimated tokens (0.80 x 200k); manual compaction requires an explicit command the pilot never issues; 413 recovery requires a provider payload-too-large error, impossible for a small fixture prompt | Prerequisite 6's first branch is satisfied: the pilot provably avoids compaction, so the joint invalidation check is defense-in-depth rather than a pilot gate |
| Estimation is deterministic and tested | `bash scripts/dev_cargo.sh test -p jcode-compaction-core --lib` passed 18/18 on 2026-07-15, including `image_token_cost_is_bounded_not_base64_length` (flat per-image cost per `lib.rs:53-57`) | Token/image estimation cannot nondeterministically trip the threshold during the pilot |
| Post-compaction invalidation clears both session-id copies | All five R13 reset sites clear `self.provider_session_id` and `self.session.provider_session_id` together (census below); `messages_for_provider_applies_manual_compaction_in_native_auto_mode` (`crates/jcode-app-core/src/agent_tests.rs:426-476`) asserts both are `None` after compaction, reproduced 2026-07-15 in a clean `JCODE_HOME` | Invariant #3's agent/persisted divergence is guarded on the compaction path by a passing test |

## provider_session_id writer/reset census (invariant #3)

Non-test sites at fork `f5a8999d8`, classified by owning responsibility. Method: workspace-wide symbol search for `provider_session_id`, excluding `*_tests*` and `tests/` paths.

**Writers (set to a provider-supplied or restored value):**

| Owner | Site | Semantics |
|---|---|---|
| R12 | `crates/jcode-app-core/src/agent/turn_loops.rs:503-504`; `agent/turn_streaming_mpsc.rs:790-791` | Provider returns a session id mid-turn; both agent and session copies set together |
| R12 (TUI local turn) | `crates/jcode-tui/src/tui/app/turn.rs:724` | Sets only the agent copy; the session copy is synced later (e.g. `conversation_state.rs:627` on quit). This is the one identified divergence window; see escalation trigger |
| R12 | `crates/jcode-app-core/src/agent/turn_execution.rs:250-251` (`undo_rewind`); `crates/jcode-tui/src/tui/app/commands.rs:1998-1999` | Restore both copies from a rewind-undo snapshot |
| R04/R06A restore | `agent/turn_execution.rs:599` (`restore_session_with_working_dir`); `crates/jcode-base/src/session.rs:351,387,711` (startup stub, remote snapshot, journal meta); `crates/jcode-tui/src/tui/app/tui_lifecycle_runtime.rs:629,648` (dev-command agent-to-session copy) | Persistence/restore round trips; value provenance is the stored session |
| R06A import | `crates/jcode-base/src/import.rs:593,712,820-823,937,1057` | Foreign-session importers seed the id; out of pilot scope |

**Reset sites (set to `None`):**

| Owner | Site | Trigger |
|---|---|---|
| R13 | `crates/jcode-app-core/src/agent.rs:687-688` (`apply_openai_native_compaction`); `agent/compaction.rs:7-8,163-164,224-225,268-269` (applied, auto after context limit, 413 recovery, oversized-native recovery) | Compaction completion or recovery; both copies cleared together |
| R13 (TUI mirror) | `crates/jcode-tui/src/tui/app/conversation_state.rs:317-318,394-395`; `app/local.rs:210-211`; `app/model_context.rs:960-961` (`reset_state_for_compaction_retry`) | Compaction events in the TUI state machine; both copies cleared together |
| R02 | `app/model_context.rs:41-42` (`finalize_model_switch`), `494-495` (fallback offer), `1327-1329` (`run_fix_command`); `app/inline_interactive.rs:2979-2980` (picker-driven switch) | Model/route change invalidates the provider session |
| R12 | `agent/turn_execution.rs:189` (`clear`, agent copy only within full-state clear), `197-198` (`reset_provider_session`), `233-234` (`rewind_to_message`) | Turn-state reset and rewind |
| R04 | `crates/jcode-app-core/src/overnight.rs:188` (coordinator child); `server/client_actions.rs:698` (transfer child); `crates/jcode-base/src/session/crash.rs:139` (crash recovery); `tui_lifecycle_runtime.rs:324,331` (restore declines to reuse a stale id); `app/commands.rs:406,2105-2106`; `app/commands_review.rs:267,306`; `app/conversation_state.rs:835` (`recover_session_without_tools`, clears only the agent copy at line 835 but line 836 immediately replaces `self.session` with a freshly built `new_session`, so no stale persisted copy survives) | New/child/recovered session must not inherit a provider session |

Census result: far more than the "two writers" the original invariant text assumed, consistent with the Phase 1 re-review (`reviews/2026-07-15-responsibility-adjudication-rereview-opus.md`). Every R13 reset site clears both copies. The single-copy sites are `turn.rs:724` (writer, R12), `turn_execution.rs:189` (`clear`, R12), and `conversation_state.rs:835` (`recover_session_without_tools`, R04); the latter two are benign reset-then-session-replacement sites where the whole session object is replaced immediately after the agent-copy reset.

## Joint invalidation check (defense-in-depth, since the pilot avoids compaction)

If any pilot variant could reach compaction, the deterministic check is: after any compaction-completion or recovery event, assert `agent.provider_session_id.is_none() && agent.session.provider_session_id.is_none()` before the next provider request. This is already encoded in `messages_for_provider_applies_manual_compaction_in_native_auto_mode` (jcode-app-core) and `test_handle_server_event_compaction_shows_completion_message_in_remote_mode` (jcode-tui); running the first is the cheapest form.

## Reproduction

```bash
git diff --stat 631935dd1d3b..HEAD -- crates/jcode-compaction-core crates/jcode-base/src/compaction.rs
git diff --stat 631935dd1d3b..802f6909825809e882d9c2d575b7e478dce57d3b -- crates/jcode-compaction-core crates/jcode-base/src/compaction.rs  # empty
bash scripts/dev_cargo.sh test -p jcode-compaction-core --lib   # 18 passed
JCODE_HOME=$(mktemp -d) bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- \
  messages_for_provider_applies_manual_compaction_in_native_auto_mode  # 1 passed
grep -rn "provider_session_id" --include='*.rs' crates src | grep -v test  # census input
```

## Negative findings and explicit gaps

- No compaction path was found that clears only one of the two id copies: every R13 compaction reset clears the pair. The three single-copy sites in the census (`turn.rs:724`, `turn_execution.rs:189`, `conversation_state.rs:835`) are non-compaction R12/R04 sites, and the latter two are benign because the session object is replaced immediately after the reset.
- 413 recovery (`try_recover_after_payload_too_large`) and emergency truncation were census'd but not behavior-tested here; they are unreachable in the pilot.
- Semantic/embedding compaction mode (`should_compact_semantic`, `compaction.rs:560`) and the `Summary::advance` watermark math were not adjudicated; they do not affect pilot avoidance.
- The `b3ed82a6b` squash was not searched for absorbed upstream compaction behavior; upstream has no post-base compaction changes, making absorption moot at these refs.

## Disposition and conditions

- Recommended disposition: `retain-fork`. Compaction policy is fork-only at the fixed refs and the pilot proof depends on its explicit constants.
- Pilot entry check: pilot plan confirms one no-tool turn, no manual `/compact`, prompt far below 160k estimated tokens. Pilot exit check: no compaction event appears in the session journal or evidence stream for the fixture run.
- Acceptance or retirement condition: accepted for the pilot when the coordinator approves and both checks pass; retires into full review if compaction enters any integration slice's blast radius.
- Escalate to full review if: a compaction event fires during the pilot; any new reset site clears only one id copy; the `turn.rs:724` single-copy write is shown to persist a stale session id across a save (joint R12/R13 escalation); thresholds or `RECENT_TURNS_TO_KEEP` change; or R02/R04 dispute the census classification above.
- Coordinator approval: `pass`; the one-turn avoidance arithmetic, 18-test compaction baseline, and complete non-test writer/reset census were reproduced before integration.
- Independent Opus review: `pass` after the omitted `conversation_state.rs:835` reset was added and bounded re-review found no remaining IMPORTANT or CRITICAL findings; see [`2026-07-15-pilot-prereq-ledgers-opus-review.md`](../../reviews/2026-07-15-pilot-prereq-ledgers-opus-review.md), SHA-256 `bb763b0924cd16196785e9129663531990e6364225a7d57467f0a834e4bf73b4`, and [`2026-07-15-pilot-prereq-ledgers-opus-rereview.md`](../../reviews/2026-07-15-pilot-prereq-ledgers-opus-rereview.md), SHA-256 `cf66e5ffa0efd12e89a61c3a505ee0cd9ba0cefaf80a4476b045517f913134ba`.
