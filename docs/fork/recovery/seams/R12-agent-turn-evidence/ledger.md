# R12 Agent turn execution and durable evidence emission: authoritative ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review head | `16921ace18cf5c25368a376357b7636478d3928f` |
| Review mode | `full` |
| Research budget | `8 decisive checkpoints; 8 consumed without expansion` |
| Authority today | `fork` for the evidence implementation; `none` for an unqualified pilot claim |
| Recommended disposition | `retain-fork` |
| Pilot entry verdict | `blocked today`; conditionally admissible only after the strict fixture in this ledger passes |
| Confidence | `high` for topology, fixed-ref provenance, lifecycle control flow, and strict-fixture boundary; `medium-high` for complete writer census; `medium` for untested exceptional behavior |
| Last updated | `2026-07-15T10:34:20Z` |

## Review preservation and integrity

The independent reviews were copied byte-for-byte from their designated external
artifacts before Terra adjudication. They remain evidence rather than substituted
source authority. The source tree was read-only. No live daemon, network,
credentials, stash replay, destructive action, or publication was used.

| Review | External artifact | SHA-256 | Repository copy | Preservation result |
|---|---|---|---|---|
| Opus | `/tmp/jcode-r12-opus-review.md` | `d3c19a9576f21e008b831594c13f09189527a98a20050d044e8d7e908e462a60` | [`opus-review.md`](./opus-review.md) | byte-identical, verified with `cmp -s` |
| Grok | `/tmp/jcode-r12-grok-review.md` | `c12b96cbd935010405a05cd57a6caba7c56a5a0aca904c302ccc2cf6f52555d8` | [`grok-review.md`](./grok-review.md) | byte-identical, verified with `cmp -s` |

R00 fixes the comparison refs and prohibits implicit upstream authority or stash
replay. R11 makes these hashed, append-only artifacts recovery truth. This ledger
is the only R12 adjudication and does not alter either review.

### 2026-07-15 corrective amendment

Fable sign-off identified an overclaim in the original shared
`StreamEvent::Error` matrix row. Terra reproduced that the blocking engine logs
and directly returns `Err` at `turn_loops.rs:586-637` without a correlated
`ProviderResponse`; only the MPSC engine emits that error response at
`turn_streaming_mpsc.rs:912-924`. The matrix, evidence, fix acceptance, negative
findings, and remaining risks below distinguish the engines. This is a ledger-only
amendment: the independent reviews and their hashes are unchanged, and commit
`99e153edf131f42668a0e51361904053108a8357` is not rewritten.

## Scope and invariants

- **Owns:** prompt assembly; provider invocation; streaming and tool-continuation
  ordering; exactly one terminal turn result; correlated per-provider-call
  request/response emission; usage summary; liveness status; and the timing and
  contents of calls into the R06A evidence writer.
- **Excludes:** provider/account/model/entitlement selection (R02), durable
  evidence schema and replay implementation (R06A), compaction policy (R13),
  individual tool authority (R07A), telemetry consent (R07C), and UI rendering.
- **Must preserve:** R02's explicit selected `provider`/`model`/`route` identity;
  one `TurnStarted` and one `TurnFinished` per entrypoint turn; one correlated
  terminal `ProviderResponse` for every emitted `ProviderRequest`; no secret
  credential value in evidence; R06A faithful persistence/replay; and consistent
  agent/session `provider_session_id` state except for the recorded TUI-local
  divergence window.
- **Invariant qualification:** tool continuations may make multiple provider calls
  in one user turn. The cardinality is one response **per provider request**, not
  one provider response per user turn. The strict one-turn no-tool fixture may
  safely assert one request and one response because it intentionally excludes
  tools.

## Divergence at a glance

| Concern | Fork | Upstream | Consequence |
|---|---|---|---|
| Evidence spine | `agent/evidence.rs`, `ProviderRequest`, `ProviderResponse`, and turn bracketing exist. | `agent/evidence.rs` is absent and the reviewed turn files contain no equivalent emission. | The pilot-required feature is fork-only. Retain, do not adopt upstream. |
| Blocking and streaming turn engines | Fork delta from merge base is `88/0` in `turn_loops.rs` and `126/6` in `turn_streaming_mpsc.rs`. | Deltas are `5/0` and `8/6`, respectively. | Fork dominates the operational behavior. |
| Turn entrypoints | Fork delta is `54/9` in `turn_execution.rs`, including evidence bracketing. | Delta is `23/5` with no durable evidence bracket. | Fork is the only candidate authority. |
| Tests | Component/storage tests exist. No test runs a no-tool R12 engine and reads back its complete event stream. | No evidence-spine tests are possible. | Fixture is a hard pilot entry requirement. |

## Eight-checkpoint evidence ledger

| # | Finding | Evidence and reproduction | Confidence | Decides |
|---:|---|---|---|---|
| 1 | Fixed refs and behavioral baseline are reproducible. | `git merge-base 7ff4fc6be 802f69098` returned `631935dd`; `git merge-base --is-ancestor 7ff4fc6be 16921ace` succeeded; the reviewed R12 implementation paths have no diff from the fork baseline to review head. | H | Comparison authority and no post-baseline implementation claim. |
| 2 | Both supplied reviews are preserved and independently reachable. | `shasum -a 256 /tmp/jcode-r12-{opus,grok}-review.md` returned the hashes above; 165 and 208 lines, respectively. | H | Review integrity. |
| 3 | The evidence topology is fork-only and R12 is its emitter authority. | Upstream `git cat-file -e 802f69098:crates/jcode-app-core/src/agent/evidence.rs` fails. Fork emits the turn bracket in `turn_execution.rs:16,22;38,45;100,106`, request before calls in `turn_loops.rs:103-114` and `turn_streaming_mpsc.rs:222-233`, and success/error responses at the sites below. | H | `retain-fork`; R06A must not be blamed for missing emissions. |
| 4 | Correlation is structurally available but only correct when every exit emits. | `agent/evidence.rs:134-140` stamps current `turn_id` plus a fresh `provider_request_id`; the engines clone it into response writes. `start_evidence_turn` and `finish_evidence_turn` are `:8-53`. | H | Exact request-to-response matching assertion. |
| 5 | Global invariant #4 has under-emission defects in both engines. | Raw stream transport `Err(e)` returns directly without a response: `turn_loops.rs:201-243`, `turn_streaming_mpsc.rs:410-460`. Blocking non-compaction `StreamEvent::Error` also directly returns without a response: `turn_loops.rs:586-637`; MPSC emits its response at `:912-924`. Context-limit retry continues after an emitted request: blocking `:127-140`, `:642-650`; streaming `:253-280`, `:857-895`, `:937`. | H | Global production claim is false until fixed. |
| 6 | Cancellation can fabricate success in streaming mode. | Before open, `turn_streaming_mpsc.rs:222-252` returns `Ok(())` after request with no response. Mid-stream cancellation `:370-385` falls through to success response `:940-988`. `status_for_result` at `agent/evidence.rs:181-186` maps only `Ok` and `Error`, despite schema statuses `Cancelled` and `Interrupted`. | H | Cancellation-inclusive pilot is blocked. |
| 7 | R02, R06A, R07C, and R13 give a safe, bounded fixture envelope. | R02 writes session provider key/route/model in `agent/provider.rs:71-86`; R06A owns append/readback, not emission; R07C requires a fresh `JCODE_HOME` and `JCODE_NO_TELEMETRY=1`; R13 proves one small no-tool turn cannot compact because it needs more than 10 active turns and automatic threshold pressure. | H | Exact fixture observables and scope. |
| 8 | Narrow warmed component tests pass but do not prove end-to-end cardinality. | With fresh `JCODE_HOME` and `JCODE_NO_TELEMETRY=1`, four named tests passed: `finish_evidence_turn_populates_assistant_checkpoint`, `evidence_events_are_searchable_and_distinctly_labeled`, `all_v1_event_kinds_round_trip`, and `messages_for_provider_applies_manual_compaction_in_native_auto_mode`. Search found no `run_once`/`run_turn` plus `read_session_evidence` fixture. | H | Existing tests cannot admit the pilot. |

### Exact terminal-cardinality trace

`TurnFinished` below is the entrypoint terminal from `run_once`,
`run_once_capture`, or `run_once_streaming_mpsc`. “One” means correlated by the
same `provider_request_id`; “none” is an invariant violation, not an R06A replay
loss.

| Lifecycle case | Request | ProviderResponse | TurnFinished | Adjudication and evidence |
|---|---:|---:|---:|---|
| No-tool happy path, blocking or MPSC | 1 | 1 `Ok` | 1 `Ok` | The only candidate strict fixture path. Blocking success is `turn_loops.rs:677-697` and MPSC success is `turn_streaming_mpsc.rs:968-988`; wrapper finish sites are `turn_execution.rs:22,45,106`. |
| Provider open error, no compaction | 1 | 1 `Error` | 1 `Error` | Passes cardinality. Blocking `turn_loops.rs:141-153`; MPSC `turn_streaming_mpsc.rs:281-293`. |
| Raw stream transport `Err(e)` after open | 1 | **0** | 1 `Error` | Global violation in both engines. Direct return sites are blocking `turn_loops.rs:201-243` and MPSC `turn_streaming_mpsc.rs:410-460`. Grok uniquely called out this raw-transport branch; Terra reproduced it. |
| Blocking `StreamEvent::Error`, no compaction retry | 1 | **0** | 1 `Error` | Global violation. `turn_loops.rs:586-637` logs then directly returns `Err(StreamError)` without a `ProviderResponse`. |
| MPSC `StreamEvent::Error`, no compaction retry | 1 | 1 `Error` | 1 `Error` | Passes cardinality: MPSC emits the correlated error response at `turn_streaming_mpsc.rs:912-924` before returning. |
| Cancel before stream open, MPSC | 1 | **0** | 1 **`Ok`** | Violates cardinality and fabricates successful turn completion. `turn_streaming_mpsc.rs:222-252`; no blocking equivalent cooperative pre-open select was identified. |
| Mid-stream cancel, MPSC | 1 | 1 **`Ok`** | 1 **`Ok`** | Cardinality count is one but semantics are false: partial/cancelled work is persisted as success. `turn_streaming_mpsc.rs:370-385`, `:968-988`, and `agent/evidence.rs:181-186`. |
| Open or mid-stream context-limit retry/compaction | 1 per abandoned attempt | **0** per abandoned attempt | 1 only for eventual outer turn | Violates global invariant #4. The retry `continue` discards the old correlation and next attempt mints a new ID. Blocking `turn_loops.rs:127-140,642-650`; MPSC `turn_streaming_mpsc.rs:253-280,857-895,937`. |

**Boundary:** the strict fixture can establish only the first row, plus
emit-to-persist-to-replay. It cannot establish global correctness for raw stream
errors, blocking `StreamEvent::Error`, cancellation, or retry/compaction. The raw
transport defect, blocking event-error defect, and cancelled-success defect make
the unqualified R12 pilot **blocked**, as Grok concludes.
Opus's conditional pass is accepted only as an *entry condition after the fixture
exists*, not as permission to run today.

## R02 handoff, R06A round trip, and provider-session writers

### Required exact R02 route observables

R02 selects and persists `session.provider_key`, `session.route_api_method`, and
`session.model` in `agent/provider.rs:71-86`. R12 receives the selected provider
and echoes only the safe request identity in `ProviderRequest`:

1. Fixture configuration has a symbolic provider/account-source category, fixed
   provider key, model, route API method, and entitlement outcome. It contains no
   credential text, `/v1/me`, or live admission claim.
2. `ProviderRequest.provider`, `.model`, `.route`, `.message_count`, and
   `.tool_count` exactly match the fixture's provider/session/request.
3. Provider stub observation records provider/model plus message/tool counts and
   hashes or lengths of system prompts, never raw secret-bearing content.
4. The ledger joins R02 selected-route evidence to R12's echo. R12 must not mint
   account, entitlement, or credential authority absent from its payload.

### R06A emit-to-persist-to-replay acceptance

Use a fresh temporary `JCODE_HOME` and an in-process `Provider` stub. Run one
blocking `run_once` or deterministic capture path with a no-tool stream:
`TextDelta`, deterministic token usage, and `MessageEnd(end_turn)`. Read the
JSONL with R06A's reader and assert exactly four schema-v1 events in sequence
`0..=3`: `TurnStarted`, `ProviderRequest`, `ProviderResponse{Ok, usage}`,
`TurnFinished{Ok}`. All share the turn ID; request/response share the one provider
request ID; there is exactly one response for the request. Assert no compaction or
route-selection event interleaves. Append one malformed fifth line and assert
R06A replay returns the four valid records, demonstrating safe truncation without
fabricated completion. This composes R12 emission with R06A persistence rather
than transferring R12's emission obligation to storage.

### Complete R12 `provider_session_id` writer/reset census

| R12 site | Copies written or reset | Classification and fixture treatment |
|---|---|---|
| `agent/turn_loops.rs:503-504`, `StreamEvent::SessionId` | agent and persisted session | App-core blocking writer. If a fixture emits a `SessionId`, assert both immediately. |
| `agent/turn_streaming_mpsc.rs:790-791`, `StreamEvent::SessionId` | agent and persisted session | App-core MPSC writer. Assert both immediately if session ID is in scope. |
| `agent/turn_execution.rs:189`, `clear` | agent only | Whole-agent state clear, not a provider-stream divergence. |
| `agent/turn_execution.rs:197-198`, `reset_provider_session` | both | Reset must remain paired. |
| `agent/turn_execution.rs:233-234`, `rewind_to_message` | both | Rewind reset must remain paired. |
| `agent/turn_execution.rs:250-251`, `undo_rewind` | both | Restore must remain paired. |
| `agent/turn_execution.rs:599`, `restore_session_with_working_dir` | agent from persisted session | Restore provenance, not a provider-returned stream write. |
| `crates/jcode-tui/src/tui/app/turn.rs:723-724` | agent only | The recorded stale divergence window. Later sync is `conversation_state.rs:621-627`. Do not use this TUI-local path for immediate provider-session correctness. |

This reproduces R13's all-writer census. The strict pilot uses app-core blocking
or capture, not TUI-local interactive execution, and normally omits `SessionId`.
If it includes one, it must assert the paired app-core write.

## Adjudication

| Disagreement | Opus position | Grok position | Terra resolution | Deciding evidence |
|---|---|---|---|---|
| Fork disposition | `retain-fork` because upstream has no evidence spine. | `retain-fork` for the same reason. | **Retain fork.** There is no upstream behavior to compose or adopt. | Fixed-ref `git cat-file` absence; fork-dominant numstats; checkpoint 3. |
| Strict no-tool happy-path pilot | Conditional pass if a named fixture proves four-event emit→persist→replay. | Current pilot blocked because that fixture does not exist. | **Blocked today.** After the fixture passes, grant only a narrow conditional entry for one non-interactive no-tool, no-cancel, no-compaction turn. | Existing test search plus checkpoint 8; first matrix row. |
| Cancellation and compaction-inclusive pilot | Explicitly blocked, with cancelled-success and retry-orphan findings. | Blocked and requires terminal status/response fixes. | **Blocked.** No widening without fixed negative fixtures. | Matrix cancellation/retry rows; schema capability versus `status_for_result`. |
| Raw stream transport error | Not the principal extra finding. | Both engines directly return `Err(e)` without `ProviderResponse`. | **Confirmed global invariant violation.** It must be fixed before any broad R12 claim or pilot widening. | `turn_loops.rs:201-243`; `turn_streaming_mpsc.rs:410-460`. |

**Terra reproduction:** ran fixed-ref ancestry/absence/delta commands, then
inspected the emitter and all terminal control-flow exits with line-numbered
source. The decisive result is that upstream lacks the emitter, while fork has
both a valid strict happy path and documented request-orphan/mislabelled-success
branches. That decides `retain-fork` and a blocked current pilot.

## Pilot entry, exit, and stop conditions

### Entry, after implementation only

1. Deterministic no-network/no-secret `Provider` stub, fresh disposable
   `JCODE_HOME`, `JCODE_NO_TELEMETRY=1`, no daemon, tools, MCP, memory recall,
   manual compaction, or cancellation signal.
2. R02 fixture inputs and route observables above are explicit and stable.
3. R06A writer/readback and safe-truncation assertions pass in the same fixture.
4. R13 avoidance is demonstrated by one small no-tool turn and no compaction
   event. R07C remains disabled by fixture environment.
5. R09 trusted classifier/green gates remain green without `--update`; current
   red debt remains visible and attributed.

### Exit observable

The fixture is successful only if readback is exactly four events in this order:
`TurnStarted`, exactly one `ProviderRequest`, exactly one correlated
`ProviderResponse{Ok, usage}`, and exactly one `TurnFinished{Ok}`. Sequences are
contiguous; all events share `turn_id`; route/provider/model/counts match R02;
no credential text is present; safe-truncation returns only the four valid
records. This proves a narrow happy-path emission claim, not the global invariant.

### Immediate rollback or stop

Stop the pilot and revert only the active implementation slice if any of these
occur: a missing or duplicate terminal response; correlation or sequence mismatch;
wrong provider/model/route/count; an event status `Ok` after cancellation;
compaction/retry in the one-turn fixture; telemetry enabled; evidence outside the
temporary home; a live credential/network/daemon dependency; new R09 debt hidden
by an update; or any write outside the three review-record paths for this ledger.
Broader cancellation, transport-error, tool, or compaction scenarios are stop
conditions until their dedicated deterministic fixtures pass.

## Recommendation

- **Disposition:** `retain-fork`.
- **Why:** only the fork has the required durable R12 evidence spine. Removing it
  breaks the R06A-backed pilot record, while upstream offers no replacement.
- **Authority today:** fork is authority for retained implementation provenance,
  not for a claim that every emitted request has a terminal response. R12 is
  globally blocked on the confirmed defects and fixture gap.
- **Cross-seam dependencies:** R00 fixed refs and preservation; R02 route inputs;
  R06A round trip; R07C telemetry opt-out; R09 visible debt; R11 append-only
  hashed record; R13 compaction avoidance and writer/reset census.
- **Upstream opportunity:** none for adoption. A future bounded upstream patch
  could carry a shared terminal-emission helper only after fork fixes and parity
  tests demonstrate it is independently useful.
- **Quality-of-life idea:** unify duplicated blocking/MPSC terminal emission only
  in the separate refactor slice after behavior is pinned. Do not refactor as a
  substitute for tests or mix it with a sync.

## Bounded implementation slices

| Slice | Class | Change | Acceptance | Rollback or stop condition |
|---:|---|---|---|---|
| 0 | `sync` | No source adoption. Preserve fork-only evidence and recheck fixed refs before any future comparison. | Upstream still lacks equivalent evidence; no behavior import. | Stop if ref/provenance changes without an R00 amendment. |
| 1 | `fix` | Add `no_tool_turn_emits_one_correlated_record`: stub provider, one blocking/capture no-tool turn, R02 assertions, R06A readback/truncation, R07C environment, R13 absence. | Exact four-event exit observable passes deterministically. | Revert test/fixture slice if it needs network, credential, daemon, or broad production behavior. |
| 2 | `fix` | In both engines, turn raw stream transport `Err(e)` into one correlated `ProviderResponse{Error}` before return; also add that response on blocking non-compaction `StreamEvent::Error`; add blocking and MPSC negative fixtures. | Raw transport error in blocking and MPSC, plus `StreamEvent::Error` in blocking and MPSC, each have one request, one error response, and one error finish. | Stop on duplicate response, correlation mismatch, or changed success behavior. |
| 3 | `fix` | Make cancellation emit a correlated non-`Ok` terminal provider response and `TurnFinished{Cancelled|Interrupted}`; make abandoned context-limit attempts terminally represented or delay request emission until non-abandoned; add before-open, mid-stream, and retry fixtures. | Every request has exactly one response and cancelled work is never `Ok`. | Do not widen pilot if any cancellation/retry fixture remains red. |
| 4 | `refactor` | Extract a narrow shared terminal-emission/status helper only after slices 2-3 pin both engines. | Parity tests retain all matrix outcomes with no duplicate IDs. | Revert independently if extraction obscures engine-specific cancellation semantics. |
| 5 | `docs` | Append implementation outcomes, exact commands, debt ownership, and any amendment to this ledger. Preserve reviews and hashes. | Hashes, matrix, and R11 append-only record remain correct. | Stop on review rewrite, hash mismatch, or undocumented scope growth. |

## R09 debt, negative findings, and gaps

### R09 debt posture

No gate baseline was updated and this documentation-only commit does not claim a
source-debt reduction. R09's current visible aggregate debt is: production size
60 violations / +6,604 net LOC; test size 31 / +3,679; panic 46 versus 31 (+15);
and swallowed errors 3,077 versus 2,987 (+90). R09 explicitly assigns
agent-turn-path panic/swallowed debt to R12, but has **not** enumerated the
per-file R12 subset (`R09 ledger:24-30,47-50`). Before any R12 source slice,
record each owned file/count, run the trusted classifier and applicable gates
without `--update`, and keep unrelated inherited debt red and visible.

### Negative findings

- No upstream emitter, response correlation facade, or equivalent end-to-end
  durable evidence behavior was found at the fixed upstream ref.
- No path was found that emits two responses for one provider request ID. The
  demonstrated issue is under-emission, not duplication.
- Blocking non-compaction `StreamEvent::Error` is an additional under-emission
  path: `turn_loops.rs:586-637` logs then directly returns without a
  `ProviderResponse`. MPSC is not equivalent because it emits at
  `turn_streaming_mpsc.rs:912-924`.
- No provider response is fabricated out of thin air. The cancellation defect is
  a false `Ok` response/turn status, not a synthetic response record.
- Server-side blocking and MPSC `SessionId` writes update both copies. The one
  immediate single-copy window is TUI-local and excluded from the strict fixture.
- Existing component tests do not test the full request/response terminal matrix
  or an end-to-end R12 evidence readback.

### Confidence and remaining decisive gaps

- The strict happy-path topology is high confidence from paired source sites, but
  fixture execution is absent, so it cannot pass a gate today.
- Raw transport errors, blocking `StreamEvent::Error`, cancellation, and
  compaction/retry defects are high-confidence control-flow findings but remain
  unobserved in deterministic tests. Slices 2-3 must convert them into regression
  fixtures.
- Compaction is proven unreachable for the strict one-turn fixture by R13, not
  proven correct for general R12 operation. It remains a global blocker.
- TUI-local session-id stale-window behavior, tool-continuation multi-call
  cardinality, and full provider-specific stream semantics were not executed.
  They are outside the strict fixture and must not be inferred from it.

## Validation and sign-off

- **Commands:** `shasum -a 256 /tmp/jcode-r12-{opus,grok}-review.md`; fixed-ref
  ancestry, `git cat-file`, and `git diff --numstat` commands recorded in the
  evidence table; line-numbered `rg`/`nl` lifecycle inspection; then:
  - `JCODE_HOME=$(mktemp -d) JCODE_NO_TELEMETRY=1 bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- finish_evidence_turn_populates_assistant_checkpoint` (pass, 1 test).
  - Same environment, `... evidence_events_are_searchable_and_distinctly_labeled` (pass, 1 test).
  - Same environment, `bash scripts/dev_cargo.sh test -p jcode-session-types --lib -- all_v1_event_kinds_round_trip` (pass, 1 test).
  - Same environment, `... -p jcode-app-core --lib -- messages_for_provider_applies_manual_compaction_in_native_auto_mode` (pass, 1 test).
- **Failure modes checked:** happy path, open error, raw transport error in both
  engines, blocking versus MPSC `StreamEvent::Error`, cancel-before-open,
  mid-stream cancel, retry/compaction, upstream absence, route handoff, durable
  schema round trip, and session-id writers/resets.
- **Remaining risks:** the global emitter invariant remains false on identified
  branches including blocking non-compaction `StreamEvent::Error`, no full fixture
  exists, and R02 itself remains pilot-blocked on its independent
  stale-tier/product fixture conditions.
- **Opus review:** conditional pass only for the strict no-tool fixture after it
  exists; cancellation and compaction variants fail.
- **Grok review:** fail for current pilot, including raw transport errors in both
  engines and missing fixture.
- **Terra adjudication:** `retain-fork`; R12 pilot is blocked today and may enter
  only under the narrow post-fixture conditions above.
- **Sol initial sign-off:** `pass` on `99e153edf131f42668a0e51361904053108a8357`; preserved at [`2026-07-15-r12-sol-signoff.md`](../../reviews/2026-07-15-r12-sol-signoff.md). It did not identify the blocking `StreamEvent::Error` overclaim.
- **Fable initial sign-off:** `fail` on `99e153edf131f42668a0e51361904053108a8357` with one IMPORTANT matrix finding; preserved at [`2026-07-15-r12-fable-signoff.md`](../../reviews/2026-07-15-r12-fable-signoff.md).
- **Terra corrective follow-up:** `1db425ef6747611a1902836e8417e5f0f7440b48` corrected the matrix without rewriting the original adjudication commit.
- **Sol bounded re-review:** `pass` on `1db425ef6747611a1902836e8417e5f0f7440b48`; preserved at [`2026-07-15-r12-sol-rereview.md`](../../reviews/2026-07-15-r12-sol-rereview.md).
- **Fable bounded re-review:** `pass` on `1db425ef6747611a1902836e8417e5f0f7440b48` with no new CRITICAL or IMPORTANT findings; preserved at [`2026-07-15-r12-fable-rereview.md`](../../reviews/2026-07-15-r12-fable-rereview.md).

---

## 2026-07-15 implementation amendment and current rollup

This section is an append-only R11 amendment to the historical R12 ledger above.
The base adjudication, blocked pilot verdict, matrix truth, Opus/Grok disagreement,
Sol/Fable sign-offs, and bounded re-reviews remain preserved verbatim as recovery
truth. This amendment supersedes the current operational verdict only by adding new
implementation evidence; it does not delete or replace the earlier blocked state.

### Reviewed range and corrective trigger

- Historical base restored verbatim from `1b9d6e09f`.
- Initial implementation commits:
  - `61f241a9d` `fix: persist terminal provider errors`.
  - `4ed674f14` `test: cover R12 terminal evidence persistence`.
  - `74aaf1710` `docs: record R12 evidence fixture qualification`.
- Independent adversarial follow-up: `/tmp/jcode-r12-fix-fable-review.md`, verdict
  `FAIL for acceptance as an R11-compliant prerequisite package` because the prior
  docs commit rewrote ledger truth instead of appending it, and because error-path
  fixtures/classification needed tightening. Source semantics were otherwise judged
  narrowly credible for the strict no-tool/no-cancel/no-compaction path and fixed
  terminal error branches.
- Corrective implementation commits:
  - `7c6044907` `fix: stabilize provider error evidence classes`.
  - `991e2c165` `test: harden R12 terminal error fixtures`.

### Current rollup verdict

- The R12 strict no-tool/no-cancel/no-compaction prerequisite is qualified only for
  the deterministic fixture shape below.
- The broader pilot remains fail-closed for cancellation and abandoned
  retry/context-limit/compaction attempts. These rows are not widened by this
  amendment.
- Opus PASS and Grok/Fable disagreement are preserved as historical review evidence;
  the current rollup records new implementation evidence, not a rewrite of those
  reviews.

### Before/after terminal evidence matrix

| Path | Historical ledger truth before implementation | Current evidence after commits | Current verdict |
|---|---|---|---|
| Strict no-tool/no-cancel/no-compaction success | Required fixture absent; pilot blocked today. | Deterministic in-process provider fixture persists exactly `TurnStarted`, `ProviderRequest`, `ProviderResponse{Ok}`, `TurnFinished{Ok}` with sequences `0..=3`, shared `turn_id`, correlated request/response ID, provider/model/route/tool-count assertions, token usage, durable readback, and truncation tolerance. | Qualified for strict fixture only. |
| Provider open error without retry/compaction | Existing implementation emitted one request, one error response, and one error finish. | Provider terminal errors now use stable `error_class(error)` classification instead of raw/truncated provider text. | Pass. |
| Blocking raw stream transport `Err(e)` without retry/compaction | Under-emitted terminal provider response. | `append_provider_error_response` emits one correlated `ProviderResponse{Error}` before return; strict error fixture asserts exact ordered four-event shape and stable class. | Fixed and covered. |
| MPSC raw stream transport `Err(e)` without retry/compaction | Under-emitted terminal provider response. | Same stable helper and strict fixture coverage as blocking raw transport. | Fixed and covered. |
| Blocking `StreamEvent::Error` without retry/compaction | Under-emitted terminal provider response, as corrected by earlier Terra/Fable matrix amendment. | Emits one correlated classified `ProviderResponse{Error}` and returns `StreamError`; strict fixture asserts exact event order, turn id, sequence, provider/model/route identity, response correlation, nonempty stable classification, and `TurnFinished{Error}`. | Fixed and covered. |
| MPSC `StreamEvent::Error` without retry/compaction | Historically emitted a correlated provider error response, but implementation matrix lacked parity fixture. | Now uses the shared stable helper and has deterministic parity fixture `r12_mpsc_stream_event_error_persists_terminal_provider_response`. | Covered for parity. |
| MPSC cancel before stream open | Known false shape: `ProviderRequest`, no provider response, `TurnFinished{Ok}`. | Not changed in this slice. | Blocked/fail-closed. |
| MPSC mid-stream cancellation | Known false shape can fall through to success response and `TurnFinished{Ok}`. | Not changed in this slice. | Blocked/fail-closed. |
| Open/context-limit retry or compaction abandonment | Already-emitted requests can be abandoned without a terminal provider response. | Not changed in this slice. | Blocked/fail-closed. |

### Error classification semantics

`ProviderResponse.error_class` is now a low-cardinality classifier derived through
`agent::evidence::error_class(error)`, the same classifier already used by
`TurnFinished.error_class`. It takes the final error cause, strips text after the
first colon, trims, caps length, and falls back to `error`. R12 strict error
fixtures intentionally inject strings containing `token=secret request=abc` and
assert that `ProviderResponse.error_class` and `TurnFinished.error_class` match the
stable prefix without those raw request details.

### Validation after Fable FAIL follow-up

Commands were run without `--update` and without live provider credentials,
network, daemon dependency, cancellation, or compaction in the R12 fixtures.

- `bash scripts/dev_cargo.sh fmt` passed.
- Exact R12 fixtures passed 5/5:
  - `r12_no_tool_turn_emits_and_persists_exactly_one_terminal_provider_response`.
  - `r12_blocking_raw_transport_error_persists_terminal_provider_response`.
  - `r12_blocking_stream_event_error_persists_terminal_provider_response`.
  - `r12_mpsc_raw_transport_error_persists_terminal_provider_response`.
  - `r12_mpsc_stream_event_error_persists_terminal_provider_response`.
- Affected suite attempt: `bash scripts/dev_cargo.sh test -p jcode-app-core --lib`
  built and ran, then failed with two non-R12 tests:
  - `server::comm_session::comm_session_tests::prepare_visible_spawn_session_cleans_session_when_launch_errors`.
  - `tool::selfdev::tests::build_lock_is_removed_on_drop_and_can_be_reacquired`.
  Immediate targeted rerun of exactly those two tests passed 2/2, so this amendment
  records the suite failure honestly as an unrelated full-suite/concurrency failure,
  not as an R12 fixture failure.
- R09 matrix without `--update`:
  - classifier tests: 17/17 passed.
  - warning budget: passed at 0.
  - wildcard re-export budget: passed at total 16.
  - expected-red ratchets remained red: `panic=1 swallowed=1 prod_size=1 test_size=1`.

### Acceptance boundary retained

This amendment qualifies only the strict deterministic no-tool/no-cancel/no-compaction
R12 prerequisite and the non-retry terminal error branches listed as fixed above.
It explicitly does not qualify cancellation-inclusive, retry-inclusive,
context-limit-inclusive, compaction-inclusive, live-provider, tool-continuation, or
full-pilot claims. Those require a separate slice with truthful cancelled or
interrupted terminal evidence and fixtures for before-open, mid-stream, open
context-limit, and mid-stream context-limit behavior.

---

## 2026-07-15 W1 cancellation/retry terminal-evidence amendment

This is an append-only W1 amendment over source base `602709895`. It preserves the historical blocked rows above as prior recovery truth and records the new behavior-fix evidence for the previously excluded cancellation and context-limit retry paths. No earlier review text, matrix row, or claim limit is rewritten.

### Commits in this W1 slice

- Source fix: `21304d8e4e1e6c0b6c53294fbc3a04157ab7d331` (`fix: record R12 abandoned provider attempts`).
- Fixtures: `40235bd87633e75836e0cb2c1d0379362e6e4662` (`test: cover R12 cancellation retry evidence`).
- This ledger amendment is intentionally docs-only and follows those two commits.

### Required inventory and zero-match checks

- No in-repository `AGENTS.md` was present in `/Users/jrudnik/labs/jcode-w1-r12`; `find .. -name AGENTS.md -print` only found sibling-repository files.
- Existing strict R12 fixtures before W1 were exactly five, all in `crates/jcode-app-core/src/agent_tests.rs`:
  - `r12_no_tool_turn_emits_and_persists_exactly_one_terminal_provider_response`.
  - `r12_blocking_raw_transport_error_persists_terminal_provider_response`.
  - `r12_blocking_stream_event_error_persists_terminal_provider_response`.
  - `r12_mpsc_raw_transport_error_persists_terminal_provider_response`.
  - `r12_mpsc_stream_event_error_persists_terminal_provider_response`.
- Current writer inventory after W1:
  - Blocking engine emits `ProviderRequest` at `turn_loops.rs:105` and success `ProviderResponse{Ok}` at `turn_loops.rs:714`.
  - Blocking engine now emits terminal provider error responses through `append_provider_error_response` at `turn_loops.rs:129` and `:148` for provider-open context retry/final open error, `:224` and `:251` for raw stream context retry/final raw error, and `:628` and `:666` for stream-event context retry/final stream-event error.
  - MPSC engine emits `ProviderRequest` at `turn_streaming_mpsc.rs:224` and success `ProviderResponse{Ok}` at `turn_streaming_mpsc.rs:1009`.
  - MPSC engine now emits terminal provider error responses through `append_provider_error_response` at `turn_streaming_mpsc.rs:252` for cancel-before-open, `:266` and `:296` for provider-open context retry/final open error, `:395` for mid-stream cancel, `:447` and `:485` for raw stream context retry/final raw error, and `:908` and `:957` for stream-event context retry/final stream-event error.
  - `TurnFinished` remains centralized in `agent/evidence.rs:42`, called only by the turn wrappers at `turn_execution.rs:22`, `:45`, and `:106`; neither engine writes `TurnFinished` directly. Grep for `TurnFinished` in both engine files returned zero matches and is a zero-match inventory result, not a pass-by-omission.

### Behavior changes and invariant evidence

- Cancellation now returns a typed interrupted turn error from MPSC after request emission. `agent/evidence.rs:165-168` constructs the sentinel error and `status_for_result` at `agent/evidence.rs:208-218` maps only that sentinel to `TurnFinished{Interrupted}`. Other error results still map to `Error`; successful results still map to `Ok`.
- `cancel before open` and `mid-stream cancel` each persist exactly four events: `TurnStarted`, one `ProviderRequest`, one correlated non-`Ok` `ProviderResponse{Error}` with `error_class="turn interrupted"`, and one `TurnFinished{Interrupted}` with no provider request id. The new fixtures are:
  - `r12_mpsc_cancel_before_open_persists_interrupted_terminal_response`.
  - `r12_mpsc_mid_stream_cancel_persists_interrupted_terminal_response`.
- Open and mid-stream context-limit retry each persist exactly six events for the deterministic two-attempt shape: `TurnStarted`, first `ProviderRequest`, first correlated `ProviderResponse{Error}`, second distinct `ProviderRequest`, second correlated `ProviderResponse{Ok}`, and one outer `TurnFinished{Ok}`. The abandoned attempt is terminally represented rather than orphaned. The new fixtures are:
  - `r12_open_context_limit_retry_persists_terminal_response_per_attempt`.
  - `r12_mid_stream_context_limit_retry_persists_terminal_response_per_attempt`.
- Success-path provider response emission remains at the existing post-loop success sites. The fix adds early terminal error responses only on cancellation and context-limit retry/final-error branches, and those branches return or retry before reaching the success response site, preventing duplicate terminal responses.

### Validation performed for W1

All commands below were run without `--update`, live provider, daemon, network action, credentials, reload, publication, MCP/tool exercise, baseline update, stash, ref, worktree, or prompt edits.

- Source pre-commit focused validation:
  - `scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_ --nocapture`
  - Exit `0`; result `5 passed; 0 failed; 1090 filtered out` before adding the W1 fixture commit.
- Fixture development failure preserved:
  - First nine-fixture run exited `101`; result `8 passed; 1 failed`; the failed assertion incorrectly required `TurnFinished{Ok}.output` for MPSC, even though `run_once_streaming_mpsc` intentionally closes evidence with `output=None`. The assertion was narrowed to cardinality/status, then rerun.
- Fixture validation after correction:
  - `scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_ --nocapture`
  - Exit `0`; result `9 passed; 0 failed; 1090 filtered out`.
- Formatting and whitespace:
  - `nix develop --command rustfmt --edition 2024 --check crates/jcode-app-core/src/agent/evidence.rs crates/jcode-app-core/src/agent/turn_loops.rs crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs crates/jcode-app-core/src/agent_tests.rs` exited `0`.
  - `git diff --check` exited `0`.
  - A workspace `cargo fmt` style check was attempted and failed because unrelated pre-existing files outside W1 still differ from current rustfmt output; those paths were not edited or staged by W1.
- Focused post-commit R12 validation:
  - `scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_ --nocapture`
  - Exit `0`; result `9 passed; 0 failed; 1090 filtered out`.
- R09 non-pilot matrix, encoded expected exits and no `--update`:
  - `classifier`: expected `0`, actual `0`; 17/17 tests passed.
  - `panic`: expected `1`, actual `1`; red debt remained visible at `31 -> 46`.
  - `swallowed`: expected `1`, actual `1`; red debt remained visible at `2987 -> 3074`.
  - `code_size`: expected `1`, actual `1`; red debt remained visible. This W1 source slice contributes touched-file growth in `crates/jcode-app-core/src/agent/turn_loops.rs` (`1251 -> 1292 LOC`) and `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs` (`1774 -> 1816 LOC`). No baseline was updated.
  - `test_size`: expected `1`, actual `1`; red debt remained visible. This W1 fixture slice contributes touched-file growth in `crates/jcode-app-core/src/agent_tests.rs` (`1321 -> 2251 LOC`). No baseline was updated.
  - `wildcard`: expected `0`, actual `0`; total `16`.
  - `warning`: expected `0`, actual `0`; current `0`, baseline `0`.
  - `shell_syntax`: expected `0`, actual `0`.
  - `diff_check`: expected `0`, actual `0`.

### Current W1 verdict and boundaries

W1 closes the R12 cancellation and open/mid-stream context-limit retry terminal-evidence defects for deterministic offline engine fixtures. The current invariant is: every emitted provider request in the covered cancellation/retry paths has exactly one correlated terminal provider response, cancellations finish the turn as `Interrupted` rather than `Ok`, retry paths may emit multiple requests, abandoned attempts are not orphaned, and each fixture has exactly one `TurnFinished` for the user turn.

This amendment does not qualify live providers, daemon/reload behavior, tools/MCP, credentials, publication, generic compaction beyond the deterministic context-limit retry fixtures, or any R06A storage-schema change. R13 remains observe-only; W1 did not add a new `provider_session_id` writer/reset site.

## 2026-07-16 W1 independent review amendment

The first independent Opus review of fixed W1 HEAD `518d0632e9cb24d8b3d7f253d4e70ed8546e3043` returned **PASS** with high confidence. The byte-preserved report is [`../../reviews/2026-07-15-w1-opus-review.md`](../../reviews/2026-07-15-w1-opus-review.md), SHA-256 `155d96232a888752fde5d1750351c40a2af077cf85df4907ede0b98bcffdec0a`.

The first Fable attempt failed at the Anthropic API and produced no artifact or verdict. A retry through a separate provider route returned **FAIL** with one IMPORTANT durable-evidence finding. The byte-preserved report is [`../../reviews/2026-07-16-w1-fable-review.md`](../../reviews/2026-07-16-w1-fable-review.md), SHA-256 `3d75e8735a9110c2637145250ffd1ba9722daf791c22560e9926a1bdd464cd1e`.

The cardinality/correlation implementation and nine focused fixtures remain accepted as supported evidence, but W1 is not approved: `error_class` still derives a persisted prefix from arbitrary provider text and can retain a secret/request/URL prefix or a no-colon secret. The earlier W1 closure claim is superseded. A separate remediation must replace raw-prefix extraction with a closed stable classifier and add adversarial secret-prefix/no-colon fixtures before fresh independent rereview. The Fable review's minor 2x2 engine retry-matrix asymmetry remains visible as nonblocking follow-up.

## 2026-07-16 W1 error-class remediation amendment

This is an append-only remediation over the preserved W1 review-disagreement commit
`9afe5bdb7d96b0bc30e29a17dec090f469ce75e4`. It addresses only the
Fable IMPORTANT blocker recorded above: persisted `error_class` could still derive
an arbitrary prefix from raw provider text. It does not alter the W1
cardinality/correlation acceptance boundary, does not widen scope to live
providers, daemon/reload, network, credentials, tools/MCP, publication, baseline
updates, or compaction beyond the deterministic R12 fixtures.

### Commits in this remediation slice

- Source fix: `f14ed5e1239e6803c83e0462efacddf75f0080ab` (`fix: close R12 error evidence classes`).
- Fixtures: `c23bae5e77aee7f1fdbe83d91cd8dbb2d6835f6a` (`test: cover R12 closed error classes`).
- This ledger amendment is intentionally docs-only and follows those two commits.

### Error classification behavior after remediation

`ProviderResponse.error_class` and `TurnFinished.error_class` now use the same
closed stable classifier instead of splitting or truncating raw provider text.
The persisted allowlist is:

- `context_limit` for explicitly detected context-limit retry/abandonment paths.
- `turn_interrupted` for W1 cancellation/interruption paths.
- `provider_open_error` for provider stream-open failures that are not classified
  as context-limit.
- `stream_transport_error` for raw stream transport `Err(e)` failures that are not
  classified as context-limit.
- `stream_error` for provider `StreamEvent::Error` failures that are not
  classified as context-limit.
- `unknown_error` for errors without a typed or explicit call-site classification.

The classifier no longer persists raw strings, prefixes, tokens, URLs, request
ids, credential-like values, or provider message text as an error class. The W1
turn wrappers carry explicit error classes through the returned error when the
same error also drives `TurnFinished`, keeping `ProviderResponse` and
`TurnFinished` consistent.

### Fixture delta and cardinality preservation

The pre-remediation W1 R12 fixture set had nine `r12_` tests. This remediation
preserves those nine and adds two adversarial deterministic fixtures, for eleven
focused R12 fixtures total:

- `r12_raw_secret_prefix_transport_error_uses_closed_error_class` injects raw text
  beginning `token=secret request=abc:` and asserts the persisted provider and
  turn classes are `stream_transport_error` with no secret/request substring.
- `r12_no_colon_provider_open_secret_uses_closed_error_class` injects raw
  no-colon text `invalid API key sk-secret` and asserts the persisted provider
  and turn classes are `provider_open_error` with no raw secret substring.

The strict error helper still asserts the exact four-event shape for terminal
error fixtures. The retry helper still asserts the exact six-event two-attempt
shape. The cancellation helper still asserts one request, one correlated terminal
provider response, and one `TurnFinished{Interrupted}`. Thus per-request
terminal provider-response cardinality and one `TurnFinished` per turn remain the
same evidence contract as W1.

### Validation after remediation

Commands were run without `--update`, without live provider credentials, without
a daemon, and without publication/reload. The commands used `nix develop
--offline` for the Rust toolchain because `cargo` was not on the ambient PATH.
A preliminary direct `cargo check -p jcode-app-core --lib` attempt failed with
exit `127` (`cargo: command not found`) before any validation claim; the offline
Nix reruns below are the accepted validation evidence.

- `nix develop --offline . --command rustfmt --edition 2024 --check crates/jcode-app-core/src/agent/evidence.rs crates/jcode-app-core/src/agent/turn_loops.rs crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs crates/jcode-app-core/src/agent_tests.rs` exited `0`.
- `nix develop --offline . --command cargo check -p jcode-app-core --lib` exited `0`.
- `JCODE_HOME=$(mktemp -d /tmp/jcode-r12-home.XXXXXX) JCODE_NO_TELEMETRY=1 nix develop --offline . --command cargo test -p jcode-app-core --lib -- r12_ --nocapture` exited `0`: `11 passed; 0 failed; 1090 filtered out`. The run emitted one unrelated existing warning for `drop_control_log_handle` dead code.
- `python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'` exited `0`: `17 tests`, `OK`.
- R09 matrix, expected exits encoded before invocation and no `--update`:
  - `python3 scripts/check_panic_budget.py`: expected `1`, actual `1`; panic debt remained red at `46` versus baseline `31`.
  - `python3 scripts/check_swallowed_error_budget.py`: expected `1`, actual `1`; swallowed-error-like debt remained red at `3074` versus baseline `2987`.
  - `python3 scripts/check_code_size_budget.py`: expected `1`, actual `1`; production-size debt remained red and listed W1-touched `turn_loops.rs` (`1251 -> 1314 LOC`) and `turn_streaming_mpsc.rs` (`1774 -> 1840 LOC`) among existing oversized-file growth.
  - `python3 scripts/check_test_size_budget.py`: expected `1`, actual `1`; test-size debt remained red and listed W1-touched `agent_tests.rs` (`1321 -> 2309 LOC`).
  - `python3 scripts/check_wildcard_reexport_budget.py`: expected `0`, actual `0`; total `16`.
  - `bash scripts/check_warning_budget.sh`: expected `0`, actual `0`; current `0`, baseline `0`.
- `bash -n scripts/*.sh` exited `0`.
- `git diff --check` exited `0`.

### Current remediation verdict and remaining boundaries

The single Fable IMPORTANT blocker is remediated for the declared W1
source/test/docs surface: persisted error classes are closed stable labels, not
raw-provider prefixes, and adversarial secret-prefix/no-colon fixtures prove the
secret strings are not persisted in either `ProviderResponse` or `TurnFinished`.
The nonblocking Fable 2x2 retry-matrix symmetry note remains visible as a test
coverage follow-up; no source behavior change was made for it in this slice.
Fresh independent rereview is still required before declaring W1 fully approved.

## 2026-07-16 W1 error-class remediation rereview amendment

Fresh independent rereview is complete for the fixed source/test/docs HEAD
`c77f5e24628692eab89f5adf49081512ba4d429d`. Both reviewers independently
returned **PASS** after the earlier disk-failed attempts were discarded without
artifacts or verdicts. The byte-exact completed reports are preserved in the
separate review-only commit `7abc8642cebcbabb634458c06979dd60354a9c00`:

- Opus rereview: [`../../reviews/2026-07-16-w1-remediation-opus-rereview.md`](../../reviews/2026-07-16-w1-remediation-opus-rereview.md), SHA-256 `6be07ab6a4c360b414105046555c72f9ba7a1e6f28589903fab37a44f541206f`, verdict **PASS**, high confidence.
- Fable rereview: [`../../reviews/2026-07-16-w1-remediation-fable-rereview.md`](../../reviews/2026-07-16-w1-remediation-fable-rereview.md), SHA-256 `bd32b46d57aa2b345f0fa4d1c82315b5b394f7498d9b1d82d802aa7e1912fd43`, verdict **PASS**, high confidence on the declared remediation surface and medium only for the two statically verified but unexecuted retry-matrix cells and excluded reload-checkpoint path.

Both rereviews independently executed the focused offline R12 suite against the
existing worktree target. Each run exited `0` with `11 passed; 0 failed; 1090
filtered out`. They confirmed that `ProviderResponse.error_class` and
`TurnFinished.error_class` derive only from the closed six-label allowlist, the
secret-prefix and no-colon fixtures cross the real persistence seam, explicit
call-site classifications survive the turn wrapper, request/response
cardinality and correlation remain exact, both engines retain their success
behavior, commit classes remain separated, prior disagreement is preserved, and
no R06A schema or R13 writer boundary was crossed.

The prior Fable IMPORTANT finding is therefore closed. W1 is approved only for
the exact deterministic offline boundary recorded above: cancellation and
open/mid-stream context-limit retry terminal evidence, the existing strict
non-retry paths, and the closed provider/turn error-class surface. This does not
approve live providers, daemon/reload behavior, tools/MCP, credentials, network,
publication, generic compaction, a schema change, or any widened pilot.

The following nonblocking observations remain visible and are not silently
absorbed into W1:

- the two unexecuted cells in the 2x2 engine retry-fixture matrix remain a
  coverage follow-up with no concrete correctness defect found;
- pre-existing `ToolFinished.error_class` sites still truncate raw tool error
  text and require a separately owned follow-up if remediated;
- `ClassifiedEvidenceError` currently severs the deeper anyhow source chain,
  with no current in-scope consumer break demonstrated;
- the pre-existing MPSC TextDelta reload-checkpoint path remains outside this
  cancellation review and was not exercised.

The earlier FAIL, the earlier Opus PASS/Fable disagreement, the remediation
record, and the disk-failed no-artifact attempts remain append-only history. No
prior text or review artifact is deleted or rewritten by this amendment.
