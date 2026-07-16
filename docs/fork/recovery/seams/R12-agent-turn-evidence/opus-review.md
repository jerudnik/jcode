# R12 Agent turn execution and durable evidence emission: independent Opus full-seam review

| Field | Value |
|---|---|
| Reviewer | Opus (independent full-seam review) |
| Repository | `/Users/jrudnik/labs/jcode-seam-r12` |
| Review head | `16921ace18cf5c25368a376357b7636478d3928f` (branch `recovery/seam-r12-20260715`) |
| Fixed refs | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Baseline check | fork ref is an ancestor of the review head; `git diff --stat 7ff4fc6be HEAD -- crates` is empty. Behavior baseline equals the fixed fork ref. |
| Review mode | `full` (R12 is a six-full-seam per `RESPONSIBILITIES.md`, rank 3, score 15/16) |
| Research budget | 8 decisive checkpoints; 8 consumed |
| Recommended disposition | `retain-fork` |
| Pilot entry verdict | `conditional pass` for the strict no-tool happy path with the named fixture; `blocked` for any cancellation-inclusive or compaction-inclusive variant |
| Confidence | high for emission topology, fork-only provenance, and the happy-path terminal record; medium-high for the invariant-#4 edge-case census |

Source tree read-only. No live daemon, credentials, network, stash replay, or destructive action. No future Grok R12 artifact was read. All binding ledgers (R00, R01, R02, R06A, R07C, R09, R11, R13) and `RESPONSIBILITIES.md` R12 were read before authoring.

## Scope and boundary confirmation

R12 owns prompt assembly, provider invocation, streaming and tool-continuation ordering, exactly one terminal result, correlated turn/request/response evidence, usage summary, and liveness status. It excludes provider selection (R02), evidence storage format (R06A), individual tool authority (R07A), and UI rendering (R08). This review checks only *when and with what content* R12 emits evidence and reaches terminal states; it accepts R06A's storage round trip as separately proven (R06A ledger, `retain-fork`, fixture round trip passing) and R02's route identity as the input R12 must faithfully echo (R02 ledger, `compose`, pilot `blocked`).

Two binding upstream authorities were challenged, not assumed:
- **R06A** proved the storage layer never fabricates a `ProviderResponse` on read and truncates safely. This review confirms the *emitter* is the correlation authority and locates where the emitter can under-emit (below), which R06A explicitly deferred to R12.
- **R13** enumerated `provider_session_id` writers and asserted the pilot avoids compaction. This review independently reproduced the R12 writer set and confirms the compaction-orphan emission gap is real but pilot-unreachable, exactly as R13 scoped it.

## Divergence at a glance (fork vs upstream, at fixed refs)

| Concern | Fork | Upstream | Consequence |
|---|---|---|---|
| Turn-evidence emission (`TurnStarted`/`ProviderRequest`/`ProviderResponse`/`TurnFinished`) | Present throughout both turn engines; `agent/evidence.rs` (206 lines) is the emitter facade | **Absent.** `git show 802f69098:.../turn_loops.rs \| grep -c ProviderRequest` = 0; `agent/evidence.rs` does not exist upstream (`git cat-file -e` fails) | Nothing upstream to adopt. Disposition space is `retain-fork` or `delete`; the pilot needs emission, so `retain-fork`. |
| `turn_loops.rs` (blocking engine) | `base..fork` = 88 insertions | `base..upstream` = 5 insertions | Strongly fork-dominant. |
| `turn_streaming_mpsc.rs` (streaming engine) | `base..fork` = 126/6 | `base..upstream` = 8/6 | Strongly fork-dominant. |
| `turn_execution.rs` (entrypoints, turn bracketing) | `base..fork` = 54/9 | `base..upstream` = 23/5 | Fork-dominant; upstream has post-base activity but no evidence bracketing. |

Method: `git diff --numstat 631935dd1d3b <ref> -- crates/jcode-app-core/src/agent/<file>`; `git show 802f69098:...`. Consistent with the R06A finding that the evidence subsystem is fork-only, and with the Phase 1 adjudication that R12 was "material responsibility omitted by both initial maps."

## Emission-site census (every request/result/terminal writer, non-test)

**Turn bracket (exactly one pair per turn, in `turn_execution.rs` entrypoints):**

| Entrypoint | Start | Finish | Path |
|---|---|---|---|
| `run_once` (`:5-25`) | `start_evidence_turn` (`:16`) | `finish_evidence_turn(&result,..)` (`:22`) | blocking, print |
| `run_once_capture` (`:27-48`) | `:38` | `:45` (with output) | blocking, captured (ambient/swarm/debug) |
| `run_once_streaming_mpsc` (`:51-111`) | `:100` | `:106` | streaming (interactive client) |

`start_evidence_turn` (`agent/evidence.rs:8-30`) sets `current_evidence_turn_id = Uuid::new_v4()` and emits `TurnStarted`. `finish_evidence_turn` (`:32-53`) emits `TurnFinished`, then clears `current_evidence_turn_id = None`. Terminal status derives from `status_for_result` (`:181-187`): **`Ok` or `Error` only**.

**Provider request/response emitters (inside the two turn engines):**

| Engine | ProviderRequest | Success ProviderResponse | Error ProviderResponse |
|---|---|---|---|
| Blocking `run_turn` (`turn_loops.rs`) | `:104-114` (before `complete_split`) | `:677-697` (status Ok, usage, output) | `:141-152` (open error, after compaction check) |
| Streaming `run_turn_streaming_mpsc` | `:223-233` | `:968-988` (status Ok) | `:281-292` (open error); `:912-923` (mid-stream `StreamEvent::Error`) |

Correlation: each request calls `provider_evidence_correlation()` (`agent/evidence.rs:134-140`), which stamps the shared `turn_id` **plus a fresh `provider_request_id = Uuid::new_v4()`**, cloned into the matching response. This is the structural basis for invariant #4's request↔response correlation.

**`provider_session_id` writers/resets owned by R12 (reconciled against the R13 census):**

| Site | Copies written | Note |
|---|---|---|
| `turn_loops.rs:503-504` (`StreamEvent::SessionId`) | both (`self.` + `self.session.`) | consistent |
| `turn_streaming_mpsc.rs:790-791` (`StreamEvent::SessionId`) | both | consistent |
| `turn_execution.rs:189` (`clear`) | agent only | benign: whole-state clear |
| `turn_execution.rs:197-198` (`reset_provider_session`) | both | consistent |
| `turn_execution.rs:233-234` (`rewind_to_message`) | both | consistent |
| `turn_execution.rs:250-251` (`undo_rewind` restore) | both | consistent |
| `turn_execution.rs:599` (`restore_session_with_working_dir`) | agent from session | restore provenance |
| `crates/jcode-tui/src/tui/app/turn.rs:724` (TUI-local `SessionId`) | **agent only** | the one divergence window R13 flagged; TUI local engine writes no session copy here |

This exactly reproduces the R13 invariant-#3 census. The `turn.rs:724` single-copy write is confirmed and remains the single R12-owned divergence window; it does not affect the server-side pilot engines (`turn_loops`/`turn_streaming_mpsc`), which write both copies.

## Terminal-record adjudication (invariant #4 challenge)

Invariant #4: *each R12 provider request has exactly one correlated success or error response; R06A must persist and replay it without loss, duplication, or fabricated completion.* I traced every exit of both engines across success, provider error, cancellation, stream close, and no-tool.

### PASS: the strict no-tool happy path (the pilot path)

Blocking `run_turn`: provider opens, stream yields text + `MessageEnd`, loop ends, `tool_calls.is_empty()` → success `ProviderResponse` (`:677`) → `break` (`:872`) → `run_once` emits `TurnFinished{Ok}`. Result: exactly `TurnStarted`, one `ProviderRequest`, one `ProviderResponse{Ok}`, one `TurnFinished{Ok}`, correlated by one `provider_request_id`. **This is clean and satisfies invariant #4.** The streaming engine's no-tool path is structurally identical (`:968` then `:106`).

### CHALLENGE 1 (pilot-unreachable): compaction-retry orphans the request

On `complete_split` error where `try_auto_compact_after_context_limit` returns true, both engines `continue` **without** emitting a `ProviderResponse` for the already-emitted `ProviderRequest`:
- Blocking: `turn_loops.rs:128-139` emits request at `:104`, then `continue` at `:139` (the `ProviderResponse` at `:141` is only reached on the non-compaction branch).
- Streaming: `turn_streaming_mpsc.rs:257-279` `continue` at `:279`.
- Also the mid-stream `StreamEvent::Error` retry-after-compaction: blocking `:618 break` → `:650 continue`; streaming `:882-894 break` → `:937 continue`. The request emitted for that iteration gets no response.

The next loop iteration mints a **new** `provider_request_id`, so a compaction-retried turn persists N requests but < N responses. This is a genuine invariant-#4 "loss" at the emitter, not the store. **Pilot-unreachable:** R13 proved arithmetically that one no-tool fixture turn cannot trigger compaction (`should_compact_with` needs >10 active turns and ~160k tokens; 413 needs a payload-too-large error). I reproduced the `continue`-before-response control flow directly. Recorded as an escalation trigger, not a pilot blocker.

### CHALLENGE 2 (pilot-relevant if cancellation is added): cancellation fabricates success

Two cancellation exits record a success-shaped terminal record for a turn that did not complete:

- **Cancel before stream opens** (streaming only, `turn_streaming_mpsc.rs:247-251`): after `ProviderRequest` is emitted at `:223`, a `graceful_shutdown` notification returns `Ok(())`. No `ProviderResponse` is emitted, and `run_once_streaming_mpsc` then calls `finish_evidence_turn(&Ok, ..)` → `TurnFinished{Ok}`. Result: a `ProviderRequest` with **no** correlated `ProviderResponse`, closed by a **success** `TurnFinished`. This is both an invariant-#4 loss and a fabricated completion, reachable without compaction.
- **Cancel mid-stream** (streaming, `:370-384`): `graceful_shutdown.notified()` `break`s the stream loop with `retry_after_compaction == false`, so control falls through to the **success** `ProviderResponse{Ok}` at `:968` and then `TurnFinished{Ok}`, even though the turn was interrupted mid-generation. A cancelled turn is recorded as `Ok`.

The schema *has* `SessionLogStatus::Cancelled` and `Interrupted` (`jcode-session-types/src/evidence.rs:171-176`), and the checkpoint formatter (`agent/evidence.rs:78-83`) handles all four, but `status_for_result` (`:181-187`) collapses every non-error terminal to `Ok`. The only non-test use of `Interrupted` is a *tool* record under server reload (`turn_streaming_mpsc.rs:1540`); no turn-level path ever emits `Cancelled`/`Interrupted`. **Consequence:** the terminal turn status cannot distinguish a completed turn from a cancelled one. For the strict single non-interactive pilot turn this is not exercised, but any pilot variant that adds cancellation would record fabricated success, which invariant #4 forbids.

### PASS: provider error and stream-close

Provider open-error (non-compaction) emits `ProviderResponse{Error}` then `return Err` (`turn_loops.rs:141-153`; `turn_streaming_mpsc.rs:281-293`), and the entrypoint's `finish_evidence_turn(&Err,..)` records `TurnFinished{Error}`. Mid-stream `StreamEvent::Error` (non-compaction) does the same (`turn_streaming_mpsc.rs:912-924`). Stream EOF without error falls through to the success `ProviderResponse`. One request, one response, one terminal. Correct.

## Streaming / non-streaming parity

The two engines are ~1,000-line siblings with duplicated emission logic that produces the **same evidence shape** (same `ProviderRequest`/`ProviderResponse` field set, same correlation stamping). Confirmed parity of the happy path and the provider-error path. **One material asymmetry:** the blocking engine has no cooperative mid-stream cancellation (`grep tokio::select turn_loops.rs` inside the stream loop = none; the only `graceful_shutdown` uses are the turn-cancel guard registration `:20` and tool contexts `:566`,`:1004`), whereas the streaming engine has two cancel `select!`s (`:243`, `:365`). This means Challenge 2 is streaming-specific; the blocking engine used by ambient/swarm/`run_once_capture` cannot be cancelled mid-stream and therefore cannot fabricate a mid-stream success, but it also cannot be interrupted cooperatively. The duplication itself is a maintainability risk (an emission fix must be applied twice) but is not a correctness divergence for the pilot.

## Deterministic tests run (narrow, no network)

`JCODE_HOME=$(mktemp -d) bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- <names>` on the review head:

```
test tool::session_search::session_search_tests::evidence_events_are_searchable_and_distinctly_labeled ... ok
test tool::session_search::session_search_tests::evidence_events_obey_role_filters ... ok
test agent::tests::finish_evidence_turn_populates_assistant_checkpoint ... ok
test result: ok. 3 passed; 0 failed; 1079 filtered out; finished in 1.05s
```

These prove: the writer facade produces searchable, role-labeled evidence events (`ProviderRequest` shape included), and `finish_evidence_turn` populates the assistant checkpoint with the derived status. **They do not drive a full turn engine and read back the emitted stream** (see the gap below). R06A's own `jcode-session-types` 11-test and `jcode-base` writer round-trips are accepted as the storage-side proof.

## Negative findings

- No emitter path was found that emits **two** `ProviderResponse` records for one `provider_request_id`. Duplication risk is absent; the risk is under-emission (Challenges 1 and 2), not duplication.
- No path fabricates a `ProviderResponse` out of thin air; the fabrication in Challenge 2 is a mislabeled *terminal turn status*, not a synthesized provider response.
- No turn-engine code writes `provider_session_id` to only one copy except the TUI-local `turn.rs:724`, matching the R13 census exactly; the server engines (the pilot's engines) always write both copies together.
- No test exercises the cancellation or compaction terminal-status paths at the turn level, so the two challenges are code-path findings, not test-observed regressions.
- No secret, credential value, or route API method is emitted as anything other than the R02-owned `route`/`provider`/`model` strings that R02's fixture contract already governs; R12 echoes them and does not mint a competing identity (`turn_loops.rs:106-108` reads `self.provider.name()/.model()` and `self.session.route_api_method`).

## Pilot prerequisites and the smallest fixture-backed no-tool turn

The R06A ledger defined the *storage-side* four-event round trip. R12 owes the *emission-side* proof that a live no-tool turn actually emits those four events in order with correct content. **This test does not exist today** (`grep` for tests calling `run_once`/`run_turn` that then `read_session_evidence*` returns none; the closest is `finish_evidence_turn_populates_assistant_checkpoint`, which calls only the finish helper, not a turn). This is the R12 pilot blocker.

The smallest fixture-backed, non-secret, no-network no-tool turn required for Phase 3:

1. **Build/route observables (from R02, echoed by R12, no network):** an in-process test `Provider` stub (the existing `agent_tests.rs` `DelayedProvider`/`NativeAutoCompactionProvider` pattern) with `name() = "openai"`, a fixed `model()`, and `session.route_api_method = Some("openai-api")`. Symbolic credential category only; no `/v1/me`, no key text. `JCODE_HOME` = disposable temp dir; `JCODE_NO_TELEMETRY=1` per R07C.
2. **Request observable:** the stub's `complete`/`complete_split` returns a stream of `TextDelta("...")` then `MessageEnd{stop_reason: "end_turn"}` and no `SessionId`, no tool calls. Assert the emitted `ProviderRequest` carries `provider="openai"`, the fixed `model`, `route=Some("openai-api")`, `message_count`/`tool_count` matching the constructed turn, and a `provider_request_id`.
3. **Result observable:** drive `agent.run_once(prompt)` (blocking engine, deterministic, no cancel select) to completion, then `read_session_evidence_from_path` in the temp home. Assert exactly four events in order: `TurnStarted`, `ProviderRequest`, `ProviderResponse{status: Ok, usage}`, `TurnFinished{status: Ok}`; sequences contiguous; **exactly one** `ProviderResponse` for the one `ProviderRequest` (match by `provider_request_id`); schema_version 1 on every event; no `Compaction`/`RouteSelected` interleaved (proves R13 avoidance held for the run).
4. **No network / no daemon:** the stub never opens a socket; assert no telemetry log line per R07C exit check.

This fixture composes with R06A's truncation probe (append a garbage 5th line, assert read-back still returns the four valid events) to give the joint emit→persist→replay proof the pilot needs.

## Cheapest tests to add (hill-climbable, deterministic)

1. **`no_tool_turn_emits_one_correlated_record`** (the fixture above): the single required pilot-exit test. Blocking engine, temp home, stub provider, evidence readback. This is the smallest test that would flip R12 from `blocked` to `pass` for the happy path.
2. **`cancel_before_stream_records_non_ok_terminal`** (guards Challenge 2): streaming engine, stub that blocks, fire `graceful_shutdown` before the stream opens, assert the persisted `TurnFinished` status is not `Ok` (currently fails: it is `Ok`). This test is the acceptance gate for any cancellation-inclusive pilot variant and pins the fabricated-success defect.
3. Reuse R13's `messages_for_provider_applies_manual_compaction_in_native_auto_mode` as the compaction-avoidance canary; no new R12 test needed for Challenge 1 in the pilot since it is unreachable.

## Recommendation

- **Disposition: `retain-fork`.** Turn-evidence emission is entirely fork-only at the fixed refs (upstream has zero emission and no `agent/evidence.rs`). There is nothing upstream to adopt or compose; deleting emission would break the pilot's request/result record. The fork's correlation model (shared `turn_id` + per-request `provider_request_id`) is structurally sound and satisfies invariant #4 on the happy, provider-error, and stream-close paths.
- **Pilot entry: `conditional pass` for the strict single no-tool non-interactive turn**, contingent on adding test (1). The happy path provably emits exactly one correlated request/result and one terminal record.
- **Pilot entry: `blocked` for any variant that adds cancellation or compaction.** Challenge 2 (cancellation → `TurnFinished{Ok}` and, mid-stream, `ProviderResponse{Ok}`) is a fabricated-completion violation of invariant #4 reachable without compaction; Challenge 1 is a request-orphan loss reachable only via compaction. Both must be fixed (route cancellation to `Cancelled`/`Interrupted` and emit a terminal `ProviderResponse` for every orphaned request) before those variants enter Phase 3.
- **Cross-seam dependencies:** R00 fixed refs and no stash replay (honored); R02 supplies the exact route identity R12 echoes, so R02's `blocked` pilot verdict transitively gates any R12 pilot that resolves a live route; R06A round-trips the emitted stream (accepted); R07C keeps telemetry disabled for the fixture (accepted); R09 debt below; R13 owns the compaction avoidance proof (accepted, reproduced); R11 append-only evidence.
- **Upstream opportunity:** none in R12. Emission has no upstream counterpart.

## R09 debt ownership (binding, no `--update`)

R09 attributes agent-turn-path panic/swallowed/size debt to R12. This documentation-only review changes no source and no gate. Before any R12 implementation slice (e.g. fixing Challenge 2), R12 must enumerate the panic, swallowed-error, production-size, and test-size entries in the concrete diff for `turn_loops.rs`, `turn_streaming_mpsc.rs`, `turn_execution.rs`, and `agent/evidence.rs`, keep existing red debt visible, and rerun the R09 gate matrix. The two engines' size is already large (`turn_streaming_mpsc.rs` 1,776 LOC; `turn_loops.rs` 1,256 LOC); a cancellation-status fix must not grow them without a bounded refactor, and should be applied to both engines to preserve parity.

## Gaps and residual risk

- No end-to-end turn→evidence-readback test exists yet; test (1) closes it and is the pilot blocker.
- The `b3ed82a6b` curated squash was not searched for an absorbed upstream emission predecessor; upstream at `802f69098` has no emission, making absorption implausible but not disproven (same bound R06A recorded).
- Challenges 1 and 2 are static control-flow findings; only test (2) would observe Challenge 2 dynamically. The happy-path fixture does not touch either.
- The blocking/streaming duplication is a maintainability hazard, not a correctness divergence; recorded so a future fix touches both engines.
- TUI-local `turn.rs` engine emits no evidence and writes only the agent `provider_session_id` copy at `:724`; it is out of the server-engine pilot scope but is the standing R13 divergence window and should not be conflated with the pilot engines.
