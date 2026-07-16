# R12 Pilot-Prerequisite Implementation Review (Independent)

- **Verdict: PASS** (narrow R12 strict-fixture prerequisite; cancellation/retry/compaction correctly remain fail-closed blocked)
- **Reviewer:** independent verify agent (Opus). No prior Fable review was read.
- **Worktree:** `/Users/jrudnik/labs/jcode-fix-r12-evidence`
- **HEAD:** `74aaf1710ff8ef54ce69cc67dc77dd0121acec37` over base `1b9d6e09f`
- **Commits inspected:** `61f241a9d` (fix), `4ed674f14` (test), `74aaf1710` (docs)
- **Disk note:** prior reviewer failed on disk exhaustion; here `df -h .` showed 42Gi free. No exhaustion risk. No full build attempted (out of scope, and no cargo/target present).

## Scope of change (exact, all 6 files)
```
crates/jcode-app-core/src/agent/evidence.rs            | +22   (new append_provider_error_response helper)
crates/jcode-app-core/src/agent/turn_loops.rs          | +14   (2 error emit sites: raw transport, StreamEvent::Error)
crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs | +7    (1 error emit site: raw transport)
crates/jcode-app-core/src/agent_tests.rs               | +341  (4 new deterministic tests + scaffolding)
crates/jcode-base/src/session/evidence.rs              | +/-14 (sequence base 1 -> 0, tests aligned)
docs/.../R12-agent-turn-evidence/ledger.md             | +71/-54 (amendment + in-place matrix/checkpoint updates)
```
`git diff --name-only 1b9d6e09f 74aaf1710` confirms these are the only touched paths. No preserved review artifact was modified.

## Event-matrix verification (both engines)

I enumerated every `ProviderResponse` and `append_provider_error_response` site and confirmed each terminal exit `return`s immediately, so no path can double-emit. Blocking = `turn_loops.rs`, MPSC = `turn_streaming_mpsc.rs`.

| Lifecycle case | Blocking site | MPSC site | Request | Response | Result |
|---|---|---|---:|---:|---|
| No-tool happy path | success `turn_loops.rs:691-711` | success `turn_streaming_mpsc.rs:975-995` | 1 | 1 `Ok` | PASS |
| Provider open error | `turn_loops.rs:141-153` | `turn_streaming_mpsc.rs:281-293` | 1 | 1 `Error` | PASS (pre-existing) |
| Raw stream transport `Err(e)` after open | **FIXED** `turn_loops.rs:242-249` | **FIXED** `turn_streaming_mpsc.rs:460-467` | 1 | 1 `Error` | PASS |
| Blocking `StreamEvent::Error`, no compaction | **FIXED** `turn_loops.rs:644-651` | n/a | 1 | 1 `Error` | PASS |
| MPSC `StreamEvent::Error`, no compaction | n/a | pre-existing `turn_streaming_mpsc.rs:919-931` | 1 | 1 `Error` | PASS |
| Cancel before/mid stream (MPSC) | n/a | `:222-252`, `:370-385` -> success | 1 | 0 or 1 `Ok` | Still defective; correctly **blocked** (slice 3) |
| Context-limit retry/compaction | `:127-140`, `:600-626` | `:253-280`, `:864-901` | 1/attempt | 0/attempt | Still defective; correctly **blocked** (slice 3) |

I confirmed against base `1b9d6e09f` that the three fixed rows were genuine under-emission gaps: base blocking raw-transport (`return Err(e)` with no response), base MPSC raw-transport (same), and base blocking `StreamEvent::Error` (`return Err(StreamError)` with no response). MPSC `StreamEvent::Error` already emitted at base, so it was correctly left unchanged. This matches the ledger census exactly.

### Helper truthfulness (`evidence.rs:142-162`)
`append_provider_error_response` always writes `status: Error`, `output: None`, `usage: None`, `error_class: Some(first 120 chars)`, reusing the same `provider_correlation` cloned from the request. This is a truthful terminal error record with a matching `provider_request_id`. No fabricated success, no synthetic response out of thin air.

## Happy-path single-emit + persistence
- `run_once_capture` brackets exactly one `start_evidence_turn` + `finish_evidence_turn` (`turn_execution.rs:38-45`); MPSC mirror at `:100-106`.
- `run_turn` breaks after one iteration when `tool_calls.is_empty()` and text is present (`turn_loops.rs:881-886`), so a no-tool turn yields exactly: TurnStarted, ProviderRequest, ProviderResponse{Ok}, TurnFinished{Ok}.
- `StreamEvent::TokenUsage` fields (`crates/jcode-message-types/src/lib.rs`) match the test payload; blocking engine records usage at `turn_loops.rs:413-447`, so the test's 7/3/total=10 assertion is sound.
- `Agent::new` -> `ensure_initial_session_context_message` seeds a message, so the test's `user_message_index > 0` assertion holds.

## Sequence/id/persistence correctness
- Sequence base changed `1 -> 0` in `next_sequence_for_evidence_path` (`jcode-base/src/session/evidence.rs:170`). This is **aligning code to a pre-existing cross-seam contract**, not a drive-by: R06A's own fixture spec already documents `sequences 0..=3` (`R06A-.../ledger.md:37`), authored earlier (`16921ace1`). The happy-path test asserts `[0,1,2,3]`, so the change is required for the fixture to pass.
- No non-test consumer depends on the old base of 1. `grep` across `jcode-app-core` and `jcode-base/session` found only the intended callers. The one remaining `sequence, 1/2` assertion in `jcode-desktop/src/desktop_worker_host.rs:179-180` is a **different** IPC frame counter (`DesktopWorkerIpcWriter`), unrelated to evidence, correctly untouched.
- Correlation: `provider_correlation` (turn_id + fresh provider_request_id) is cloned into request and every response/error write. The test asserts request_id == response_id and that TurnStarted/TurnFinished carry no provider_request_id. Verified against `evidence.rs:119-140`.
- Fail-safe replay: `read_session_evidence_from_path` stops at first unparsable line (`evidence.rs:129-140`); the happy-path test appends a malformed trailing line and asserts readback is unchanged (no fabricated completion). Correct.

## Cancellation / retry / compaction fail-closed
- Cancellation still returns `Ok(())`/success (`turn_streaming_mpsc.rs:222-252`, `:370-385`) and `status_for_result` (`evidence.rs:203-209`) only maps Ok/Error despite schema Cancelled/Interrupted. The ledger keeps these **blocked** as slice 3, does not claim them fixed. Correct posture.
- Context-limit retry `continue` still discards correlation per abandoned attempt; ledger keeps it blocked. Correct.

## Fixtures cross the real seam
`ScriptedEvidenceProvider` drives the real `run_once_capture` / `run_once_streaming_mpsc` entrypoints (not a mock of the evidence layer), writes to a real disposable `JCODE_HOME`, and reads back through the real `read_session_evidence`. `JCODE_NO_TELEMETRY=1`, no tools (`allowed_tools = empty set`), `memory_enabled=false`, no network, no live daemon, no credentials. `ScopedEnvVar` restores env on drop. This genuinely crosses emit -> persist -> replay.

## Test-addition challenge (341 lines)
- Four tests: happy-path (full field assertions + malformed-trailing-line replay), blocking raw-transport error, blocking StreamEvent::Error, MPSC raw-transport error. Each asserts exactly-one request/response/finish with Error status and matching correlation via `assert_one_terminal_error_response`.
- Symbol resolution verified: `crate::storage::lock_test_env` (established pattern, `jcode-base/src/storage.rs:59`, re-exported via `pub use jcode_base::*`), `crate::session::read_session_evidence*` and `session_evidence_path` (jcode-base session module, re-exported), all `StreamEvent` variants and `Agent` private fields (`allowed_tools`, `memory_enabled`, `route_api_method`) are in-crate. The `#[path = "agent_tests.rs"]` module (`agent.rs:1095`) makes private access legal. No compile-blocking issue found by static inspection.
- **Coverage gap (LOW):** there is no MPSC `StreamEvent::Error` test. That path was already correct at base and is source-covered, so this is a completeness nit, not a correctness defect. The ledger explicitly acknowledges MPSC StreamEvent::Error is "covered by source," which is honest.

## R11 append-only governance (docs)
- Preserved reviews are byte-identical: `shasum -a 256` of `opus-review.md` = `d3c19a95...462a60` and `grok-review.md` = `c12b96cb...52555d8`, exactly matching the hashes recorded in the ledger table. The adjudication commit `99e153edf` remains reachable (`git cat-file -e` passes). No review/baseline file was edited by these commits.
- The docs commit adds a dated "2026-07-15 implementation amendment" section (append-only, compliant) **and** edits several existing matrix/checkpoint rows in place (State field, checkpoints 5/6/7/8, matrix rows, slice status, disposition).
- **Finding (LOW/informational):** R11 rule 1 states "No seam rewrites an earlier decision... superseding evidence is appended and linked." The in-place edits of the checkpoint/matrix rows are updates to a *working ledger's live status table*, not rewrites of a preserved review, baseline, or adjudication commit. The prior decision text and the reviews are preserved and hash-verified, and the amendment is dated. This matches the ledger's own precedent (it already carries dated amendments and preserves the adjudication commit unrewritten). I judge it compliant with the spirit and letter of R11, but flag it so the coordinator can confirm the live-status-table edits are intended to be mutable.

## R09 debt posture
Ledger states no gate baseline updated and no source-debt-reduction claim; the fix adds targeted evidence emissions + tests and leaves inherited debt red/visible. Consistent with the diff.

## Commands run (key)
```
git diff --name-only 1b9d6e09f 74aaf1710          # 6 files, reviews untouched
git show 61f241a9d / 4ed674f14 / 74aaf1710         # inspected all three
shasum -a 256 .../opus-review.md .../grok-review.md # match ledger hashes
git cat-file -e 99e153edf...                        # adjudication commit reachable
grep ProviderResponse|append_provider_error_response # 7 total emit sites, mutually exclusive
grep '\.sequence' across crates                     # only intended consumers; desktop IPC is unrelated
sed -n on each cited line range in both engines      # matrix line refs accurate
```

## Confidence and gaps
- **High** confidence: terminal-response cardinality for the four covered rows, single-emit happy path, sequence/id/correlation correctness, R06A `0..=3` contract alignment, review preservation/hashes, fail-closed posture for cancellation/retry/compaction.
- **Medium** confidence: test compilation (verified by static symbol/type resolution only; no cargo available, no full build per scope). All referenced symbols, variants, and private fields resolve; I found no compile blocker, but I did not execute `cargo test`.
- **Gaps not covered by tests (acknowledged in ledger, not regressions):** MPSC `StreamEvent::Error` (source-correct, untested); cancellation and retry/compaction (intentionally left defective + blocked).

## Corrections / required changes
None blocking. Two optional follow-ups:
1. (LOW) Add an MPSC `StreamEvent::Error` regression test for parity with the blocking case.
2. (LOW/informational) Coordinator to confirm the in-place ledger status-table edits are acceptable under R11 (I assess them compliant; preserved artifacts and hashes are intact).

**Bottom line:** The implementation truthfully persists exactly one terminal `ProviderResponse` on the previously-gapped raw-transport (both engines) and blocking `StreamEvent::Error` paths, keeps the happy no-tool turn at exactly one, correctly aligns evidence sequencing to the R06A `0..=3` contract, holds cancellation/retry/compaction fail-closed, crosses the real emit->persist->replay seam in tests, and preserves R11 governance. PASS.
