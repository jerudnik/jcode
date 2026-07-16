# Independent Grok-style R12 review: agent turn execution and durable evidence emission

Date: 2026-07-15
Worktree: `/Users/jrudnik/labs/jcode-seam-r12`
Reviewed commit: `16921ace18cf5c25368a376357b7636478d3928f`
Constraint note: I did **not** read `/tmp/jcode-r12-opus-review.md` and did not use its conclusions. Source/repo were treated as read-only. No edits, live daemon, credentials, network, stash replay, or destructive actions were used.

Baseline correction: fork baseline `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4` is an ancestor of reviewed HEAD `16921ace18cf5c25368a376357b7636478d3928f`. `git diff --stat 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4..HEAD -- <R12 source/test paths>` showed no changes to the reviewed implementation paths (`turn_loops.rs`, `turn_streaming_mpsc.rs`, `agent/evidence.rs`, `turn_execution.rs`, TUI `turn.rs`, and evidence schema/storage/tests). The only matched changes were recovery docs (`RESPONSIBILITIES.md`, R06A ledger, R13 ledger). Therefore the behavioral findings below assess the baseline implementation, not assumed later source changes.

## Disposition

**Disposition: `retain-fork`, but R12 is `pilot-blocked`.**

The fork is the only side that emits durable turn/provider evidence. Upstream fixed ref `802f6909825809e882d9c2d575b7e478dce57d3b` has no R06/R12 evidence spine, while fork fixed ref `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4` adds R12 emission plus durable evidence modules. Deleting or adopting upstream would remove the pilot-required request/result evidence.

However, the current implementation does **not** support the stronger pilot claim that every production R12 provider request has exactly one correlated terminal `ProviderResponse` under cancellation, stream-error, and retry paths. It also lacks a deterministic fixture-backed no-secret provider turn test that proves no-tool live emission cardinality end-to-end. The bounded pilot should not proceed until those are fixed or explicitly narrowed to a path whose evidence invariants are tested.

## Eight decisive checkpoints

### 1. Fixed refs and scope were established

- Current HEAD is `16921ace18cf5c25368a376357b7636478d3928f`, branch `recovery/seam-r12-20260715`; final `git status --short` printed no repo changes. Fork baseline `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4` is an ancestor of HEAD and has no code diff on the reviewed R12 implementation paths.
- `RESPONSIBILITIES.md` anchors Phase 1 to fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`: `docs/fork/recovery/RESPONSIBILITIES.md:5`.
- R12 owns prompt assembly, provider invocation, streaming/tool-continuation ordering, exactly one terminal result, correlated request/response evidence, usage summary, and liveness status: `docs/fork/recovery/RESPONSIBILITIES.md:41`.
- Cross-seam invariant 4 requires each R12 provider request to have exactly one correlated success/error response, and R06A only persists/replays it: `docs/fork/recovery/RESPONSIBILITIES.md:66`.

### 2. Binding ledger constraints that matter to R12

- R02 owns route selection and requires exact R02-to-R12 provider outcome, with explicit provider/model/profile route identity over ambient state: `docs/fork/recovery/seams/R02-config-provider-routing/ledger.md:26-28`.
- R06A explicitly excludes live turn emission. It only owns what happens after R12 calls the writer. It names R12 emission sites and states storage must not be blamed for emission-order defects: `docs/fork/recovery/seams/R06A-durable-session-evidence/ledger.md:12`, `:40`.
- R13 requires a complete `provider_session_id` writer/reset census across R02/R04/R12/R13: `docs/fork/recovery/seams/R13-compaction-context-budget/ledger.md:24-49`.
- R00 forbids stash replay, implicit upstream authority, and broadening when a pilot would need real credentials/live daemon/network or unowned identity writers: `docs/fork/recovery/seams/R00-integration-provenance/ledger.md:26-31` and `RESPONSIBILITIES.md:88`.
- R09 binds every seam to no `--update`, visible red debt, and behavior-owned debt attribution: `docs/fork/recovery/seams/R09-quality-gates/ledger.md:24-30`.

### 3. Request construction and R02 route identity handoff

Supported on the R12 side, with one limitation:

- Non-streaming/blocking turn builds `ProviderRequest` immediately before `complete_split`, recording `provider`, `model`, `route: self.session.route_api_method.clone()`, `message_count`, `tool_count`, and prompt payload: `crates/jcode-app-core/src/agent/turn_loops.rs:103-123`.
- Streaming/MPSC turn does the same with the forked provider handle: `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:222-240`.
- R02 route selection sets the same `session.route_api_method` and session model/provider key before persistence: `crates/jcode-app-core/src/agent/provider.rs:71-87`.
- R12 records `provider.name()` and `provider.model()` at request/response time. It does not independently record the selected account, entitlement, or credential reference. That is consistent with R02 ownership, but the pilot observable must join R02’s selected route/account evidence with R12’s request `route`, provider, and model.

### 4. Happy-path terminal evidence is present, but it is per provider call, not necessarily per user turn

- `run_once`/`run_once_capture` starts a turn, calls `run_turn`, then unconditionally emits one `TurnFinished`: `crates/jcode-app-core/src/agent/turn_execution.rs:15-24` and `:38-46`.
- `run_once_streaming_mpsc` starts a turn, calls `run_turn_streaming_mpsc`, then unconditionally emits one `TurnFinished`: `crates/jcode-app-core/src/agent/turn_execution.rs:98-110`.
- Blocking success emits `ProviderResponse { status: Ok, duration_ms, output, usage }`: `crates/jcode-app-core/src/agent/turn_loops.rs:653-697`.
- Streaming/MPSC success emits equivalent `ProviderResponse { status: Ok, ... }`: `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:940-988`.
- Tool turns loop through provider calls. Tool execution starts after a successful provider response, then tool results are appended and the outer loop continues for the next provider call: `crates/jcode-app-core/src/agent/turn_loops.rs:820-872`, `:900-1151`. Therefore a single user turn with tools can legally contain multiple `ProviderRequest`/`ProviderResponse` pairs before one `TurnFinished`.

Adversarial conclusion: the no-tool bounded pilot can demand exactly one `ProviderResponse` and one `TurnFinished`. The full R12 seam cannot claim exactly one `ProviderResponse` per user turn unless tool-continuation semantics are changed or the claim is reworded to exactly one response per provider request.

### 5. Missing terminal `ProviderResponse` paths

These are pilot blockers unless the fixture excludes and tests around them.

1. **Blocking stream error after stream open misses `ProviderResponse`.**
   - Request is emitted before stream open: `turn_loops.rs:103-114`.
   - `complete_split` open error emits an error `ProviderResponse`: `turn_loops.rs:127-153`.
   - But a non-context `Err(e)` from `stream.next()` logs `stream_error` and directly `return Err(e)` with no `ProviderResponse`: `crates/jcode-app-core/src/agent/turn_loops.rs:201-243`.
   - The wrapper still emits `TurnFinished { status: Error }`, so durable evidence has a `ProviderRequest` and `TurnFinished` but no correlated terminal provider response.

2. **Streaming/MPSC stream error after stream open misses `ProviderResponse`.**
   - Request is emitted at `turn_streaming_mpsc.rs:222-233`.
   - Open-time provider error emits an error response: `turn_streaming_mpsc.rs:253-293`.
   - `StreamEvent::Error` emits an error response: `turn_streaming_mpsc.rs:853-924`.
   - But a transport-level `Err(e)` from the stream itself logs `stream_error` and directly returns `Err(e)` with no `ProviderResponse`: `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:410-460`.

3. **Context-limit retry attempts emit requests without responses.**
   - Blocking open-time context-limit branch increments retry and `continue`s after the already-emitted `ProviderRequest`: `turn_loops.rs:127-140`.
   - Blocking mid-stream context-limit branch breaks/retries without an error `ProviderResponse`: `turn_loops.rs:205-233`, `:642-650`.
   - Streaming/MPSC open-time context-limit branch continues after request: `turn_streaming_mpsc.rs:253-280`.
   - Streaming/MPSC mid-stream context-limit branches retry after request without terminal response: `turn_streaming_mpsc.rs:414-451` and `:857-895`.

This violates `RESPONSIBILITIES.md:66` as written: every provider request must have exactly one correlated success or error response.

### 6. Cancellation/status semantics are not durable enough

- `SessionLogStatus` includes `Cancelled` and `Interrupted`: `crates/jcode-session-types/src/evidence.rs:171-179`.
- `finish_evidence_turn` calls `status_for_result(result)`: `crates/jcode-app-core/src/agent/evidence.rs:32-52`.
- `status_for_result` maps all `Ok` to `Ok` and all errors to `Error`, never to `Cancelled` or `Interrupted`: `crates/jcode-app-core/src/agent/evidence.rs:181-186`.
- Streaming/MPSC cancellation before API stream open returns `Ok(())` immediately after `ProviderRequest`, with no `ProviderResponse`: `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:222-252`.
- Streaming/MPSC cancellation while waiting for a stream event logs `stream_cancelled` and breaks: `turn_streaming_mpsc.rs:370-385`; the function then falls through to success response emission at `:940-988`, so a cancelled/partial turn can become `ProviderResponse { status: Ok }` and `TurnFinished { status: Ok }`.
- Tool interruption has more nuanced `ToolFinished { Interrupted | Ok }` handling for reload/wait-like tools: `turn_streaming_mpsc.rs:1521-1555`, but turn-level `TurnFinished` still cannot reflect `Interrupted` because of `status_for_result`.

Adversarial conclusion: `Cancelled`/`Interrupted` exist in the schema but are effectively unused for R12 turn terminals. The pilot should either avoid cancellation entirely or require a fix and fixture proving cancellation emits a correlated terminal `ProviderResponse` plus `TurnFinished { Cancelled|Interrupted }` as appropriate.

### 7. `provider_session_id` writer/reset ownership

R13’s census is mostly supported, with one R12 stale-window risk that should be observable:

- R12 app-core blocking path sets both agent and persisted session copies together on provider `SessionId`: `crates/jcode-app-core/src/agent/turn_loops.rs:499-504`.
- R12 app-core streaming path sets both copies together and forwards the session id to the client: `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:789-792`.
- TUI local interactive path sets only `app.provider_session_id`: `crates/jcode-tui/src/tui/app/turn.rs:723-724`; the persisted session copy is later synced on quit at `crates/jcode-tui/src/tui/app/conversation_state.rs:621-627`.
- R13 correctly records this as the identified single-copy divergence window: `docs/fork/recovery/seams/R13-compaction-context-budget/ledger.md:32-48`.

For the bounded no-live-daemon pilot, prefer the app-core streaming or capture path and assert both copies immediately after `SessionId`. Do not use the TUI local interactive path for provider-session correctness unless the stale window is explicitly in scope.

### 8. Fork/upstream symbol-level divergence

Baseline-to-HEAD note: the source-level divergence discussed here is fork/upstream at the fixed refs, not a post-baseline source change. The baseline implementation is what HEAD still carries for the reviewed R12 code paths.

- `git diff --stat 802f6909825809e882d9c2d575b7e478dce57d3b..7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 -- crates/jcode-app-core/src/agent/turn_loops.rs crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs crates/jcode-app-core/src/agent/evidence.rs crates/jcode-app-core/src/agent/turn_execution.rs crates/jcode-tui/src/tui/app/turn.rs crates/jcode-session-types/src/evidence.rs crates/jcode-base/src/session/evidence.rs` shows 1,122 insertions and 7 deletions across 7 files.
- Upstream fixed ref has no `crates/jcode-app-core/src/agent/evidence.rs`, `crates/jcode-session-types/src/evidence.rs`, or `crates/jcode-base/src/session/evidence.rs`.
- Base/upstream have `provider_session_id` turn writes but no `ProviderRequest`, `ProviderResponse`, or `finish_evidence_turn` symbols in the reviewed R12 files.
- Fork/HEAD have `ProviderRequest`/`ProviderResponse` emission in `turn_loops.rs` and `turn_streaming_mpsc.rs`, plus `finish_evidence_turn` in `agent/evidence.rs`.

Challenge to authorities: upstream is not authoritative for durable R12 evidence because it lacks the feature. Fork is not yet authoritative for pilot safety because the new evidence emission does not cover all terminal paths and lacks fixture-backed cardinality tests.

## Tests run

Commands were run read-only, no network/credentials/live daemon:

1. `bash scripts/dev_cargo.sh test -p jcode-session-types --lib`
   - Passed: 11/11.
   - Relevant tests included `evidence::tests::all_v1_event_kinds_round_trip` and `evidence::tests::omitted_schema_version_defaults_to_v1`.

2. Attempted as part of a narrow batch:
   - `bash scripts/dev_cargo.sh test -p jcode-app-core --lib finish_evidence_turn_populates_assistant_checkpoint`
   - `bash scripts/dev_cargo.sh test -p jcode-app-core --lib reload_interrupted_`
   - Result: not reached before the 600s background-task timeout because `jcode-app-core` was cold-compiling dependencies, ending at `Compiling aws-config v1.8.16`.
   - I did not broaden or rerun after timeout, per the 8-checkpoint budget and narrow/block instruction.

Test coverage finding:

- Existing in-tree tests mention `ProviderResponse`/`TurnFinished` only in storage/search/schema contexts, not in an end-to-end R12 turn-emission fixture.
- `crates/jcode-app-core/src/agent_tests.rs:1297-1342` tests assistant checkpoint population from `finish_evidence_turn`, not provider request/response cardinality.
- `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:1680-1762` tests helper behavior such as reload-interrupted tool result and wrap-marker detection, not terminal provider evidence.

## Deterministic fixture-backed no-secret provider turn for the bounded pilot

Required fixture shape:

1. Create disposable `JCODE_HOME` with no network and no credentials.
2. Create an in-process mock `Provider` implementing `complete`/`complete_split` from `jcode-provider-core::Provider`.
3. Mock provider records the received:
   - `messages.len()`
   - `tools.len()`
   - `system_static`, `system_dynamic` hashes or lengths, not raw secrets
   - `resume_session_id`
   - provider name/model
4. Configure R02-equivalent route identity in the session before turn execution:
   - `session.provider_key = Some("fixture-provider")`
   - `session.route_api_method = Some("fixture-route")`
   - `session.model = Some("fixture-model")`
   - provider returns `name() == "fixture-provider"`, `model() == "fixture-model"`
5. Run one no-tool turn through `run_once_capture` or `run_once_streaming_mpsc` with a provider stream:
   - `TextDelta("fixture answer")`
   - `TokenUsage { input_tokens: Some(3), output_tokens: Some(2), ... }`
   - `MessageEnd { stop_reason: Some("end_turn") }`
   - optional `SessionId("fixture-provider-session")` if provider-session behavior is in scope
6. Read back the evidence JSONL via R06A reader and assert:
   - exactly four events for no-tool: `TurnStarted`, `ProviderRequest`, `ProviderResponse`, `TurnFinished`
   - monotonic sequences `0..=3`
   - all four share the same `turn_id`
   - `ProviderRequest.correlation.provider_request_id` equals `ProviderResponse.correlation.provider_request_id`
   - request `provider`, `model`, and `route` match the R02/session identity
   - response `status == Ok`, output summary is present, usage is present
   - `TurnFinished.status == Ok`
   - if `SessionId` is emitted, both `agent.provider_session_id` and `session.provider_session_id` are `Some("fixture-provider-session")`
7. Negative fixture variants that should exist before broader pilot:
   - open-time provider error emits `ProviderResponse { Error }` and `TurnFinished { Error }`
   - stream transport error after open emits `ProviderResponse { Error }` and `TurnFinished { Error }`
   - cancellation before open emits `ProviderResponse { Cancelled|Error }` and `TurnFinished { Cancelled }`, not `Ok` and not missing
   - context-limit retry either emits terminal error responses for abandoned attempts or does not emit `ProviderRequest` until the attempt is non-abandoned

This fixture is deterministic, no-secret, no-network, and avoids tools/MCP/memory/UI/live daemon.

## Pilot blockers and observables

Blockers:

1. Missing provider terminal response on stream-level `Err(e)` in both blocking and MPSC paths.
2. Missing provider terminal response on context-limit retry attempts after a request has already been emitted.
3. Cancellation before stream open returns success after `ProviderRequest` with no `ProviderResponse`.
4. Cancellation/interruption statuses are not represented by turn-level `TurnFinished` because `status_for_result` only maps `Ok`/`Error`.
5. No fixture-backed no-secret R12 turn test proves request/response/turn cardinality and R02 route identity.

Required pilot observables:

- R02 route identity: selected provider key, route API method, model, account/source category, entitlement fixture result.
- R12 request identity: `ProviderRequest.provider`, `.model`, `.route`, `.message_count`, `.tool_count`.
- Correlation: shared `turn_id`; request/response shared `provider_request_id`; exactly one response per request.
- Terminals: exactly one `ProviderResponse` and one `TurnFinished` for the no-tool pilot; explicit status values.
- Session id: before/after `provider_session_id` agent copy and session copy if provider emits `SessionId`.
- R06A storage: evidence file path under disposable `JCODE_HOME`, event count, sequence numbers, schema version.
- R07C/telemetry: reporting disabled, no credential or prompt contents emitted outside disposable storage.

## Negative findings

- No R12 production evidence path was found outside `turn_loops.rs`, `turn_streaming_mpsc.rs`, and `agent/evidence.rs` for provider request/response/turn terminal emission.
- No evidence reader path was found that fabricates a missing `ProviderResponse`; R06A storage preserves absence rather than repairing it, consistent with `R06A` ledger lines 27-29.
- No upstream evidence implementation exists to adopt at fixed upstream ref.
- No live network, credentials, or daemon dependency is necessary for the proposed pilot fixture.
- No mutation was needed or performed in the repo; output was written only to `/tmp/jcode-r12-grok-review.md`.

## Confidence and gaps

Confidence: **medium-high** for source-level findings and fork/upstream divergence; **medium** for full behavioral coverage because app-core targeted tests did not finish compiling within the timeout.

Gaps:

- I did not run a live daemon, network provider, credentialed provider, tools/MCP, UI/TUI interactive turn, reload, or cancellation harness.
- I did not inspect `/tmp/jcode-r12-opus-review.md` by instruction.
- I did not rerun broad app-core tests after the cold-compile timeout.
- I did not validate the full R09 gate matrix; this review was source/test narrow and read-only.
- I did not build the proposed fixture because edits were forbidden.
