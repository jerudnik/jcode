# R12 pilot-prerequisite implementation review

**Reviewer:** independent Fable-style adversarial review  
**Worktree:** `/Users/jrudnik/labs/jcode-fix-r12-evidence`  
**HEAD reviewed:** `74aaf1710ff8ef54ce69cc67dc77dd0121acec37` (`docs: record R12 evidence fixture qualification`)  
**Base:** `1b9d6e09fe324123ba97b2f627934169484e835b` (`docs(recovery): complete Phase 2 ledger gate`)  
**Range:** `1b9d6e09f..74aaf1710`  
**Mode:** read-only static/narrow checks. No full build. No edits to repo. No daemon, network, credentials, or live service use.

## Verdict: FAIL for acceptance as an R11-compliant prerequisite package

The source changes are narrowly credible for the no-tool/no-cancel/no-compaction R12 prerequisite path and for the newly fixed terminal error branches. However, the package should not be accepted as-is because the R12 ledger change rewrites earlier adjudication text instead of appending a dated amendment, violating the R11 append-only recovery-truth rule.

A narrower source-only judgment would be: **PASS with explicit scope limits** for the strict fixture and the fixed raw-transport/blocking-stream-error rows. Do **not** widen to cancellation-inclusive or retry/compaction-inclusive pilots.

## CRITICAL findings

### CRITICAL-1: R12 ledger rewrites prior recovery truth instead of preserving it append-only

**Evidence:**

- R11 says seam ledgers must accumulate dated amendments and not rewrite earlier decisions: `docs/fork/recovery/seams/R11-documentation-governance/ledger.md:18` and `:26`.
- `PROGRESS.md:3` repeats the same rule for progress history.
- The R12 ledger diff is not append-only: `git diff --numstat 1b9d6e09f..HEAD -- docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md` returned `71 54`, meaning 54 deleted lines.
- Rewritten examples from `git diff --unified=0` include:
  - `State` changed from `adjudicated` to `implemented-narrow-r12`.
  - `Pilot entry verdict` changed from `blocked today` to `qualified for the R12 strict fixture only`.
  - Checkpoints 5-8 and terminal-cardinality matrix rows were replaced in place.
  - The prior blocked strict-pilot adjudication row was replaced by a qualified row.

**Why this matters:** R11 explicitly allows superseding evidence by appended and linked amendment, not by substituting the earlier decision text. The current ledger makes the final state readable, but it collapses the historical blocked decision into the new one.

**Required correction:** Preserve the pre-fix R12 adjudication text as historical truth and append a dated implementation/fix amendment after it. If current summary fields are retained at top, they must clearly be a current rollup and link to preserved prior decision sections. Avoid deleting or replacing earlier matrix rows unless they are explicitly duplicated into a preserved historical section.

## IMPORTANT findings

### IMPORTANT-1: Error-path tests can false-pass on wrong turn-finish semantics and extra events

**Evidence:**

- The shared helper `assert_one_terminal_error_response` counts provider requests/responses/finishes and asserts response status plus request/response ID equality: `crates/jcode-app-core/src/agent_tests.rs:220-248`.
- It does **not** assert:
  - exact event length or event order,
  - a `TurnStarted` event exists,
  - all events share the same `turn_id`,
  - `TurnFinished.status == Error`,
  - `ProviderResponse.error_class.is_some()`,
  - provider/model/route identity,
  - sequence values are contiguous.
- The happy-path fixture is much stronger and does assert most of these: `agent_tests.rs:251-395`.

**Why this matters:** The error tests would pass if an error response were present but the turn finish were incorrectly recorded as `Ok`, or if unrelated evidence events interleaved. That is especially concerning because cancellation already has a known false-`Ok` shape in source.

**Required correction:** Strengthen the error helper to mirror the happy-path strictness: exact event sequence, exact event kinds, shared turn ID, request/response provider ID, provider/model/route, contiguous `0..` sequence, and `TurnFinished{Error}` with non-empty classification.

### IMPORTANT-2: Blocking/MPSC parity is source-supported but not fully fixture-supported

**Evidence:**

- New tests cover blocking raw transport error (`agent_tests.rs:399-414`), blocking `StreamEvent::Error` (`:417-433`), and MPSC raw transport error (`:436-453`).
- There is no deterministic test for the existing MPSC `StreamEvent::Error` branch.
- Source shows MPSC `StreamEvent::Error` emits a correlated error response at `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:919-931`.

**Why this matters:** The implementation claims blocking/MPSC parity. Static source says the MPSC stream-event branch is correct, but the regression matrix is asymmetric and could miss future parity regressions.

**Required correction:** Add `r12_mpsc_stream_event_error_persists_terminal_provider_response` with the same strict helper proposed above.

### IMPORTANT-3: `error_class` is still raw/truncated error text, not a stable classification

**Evidence:**

- New helper writes `error_class: Some(error.as_ref().chars().take(120).collect())`: `crates/jcode-app-core/src/agent/evidence.rs:142-161`.
- Existing open-error paths use the same raw string truncation: blocking `turn_loops.rs:141-153`, MPSC `turn_streaming_mpsc.rs:281-293`.
- `TurnFinished` has a separate `error_class(error)` function that extracts the last cause and pre-colon class: `evidence.rs:203-209` and following function.

**Why this matters:** A field named `error_class` should be low-cardinality and non-secret. Provider error text can be high-cardinality and may include request details. The tests do not assert classification semantics.

**Required correction:** Either rename/document this as sanitized error summary, or feed provider terminal errors through a stable classifier. Add tests that forbid obvious secret-bearing/raw request material and assert non-empty classification.

## Source semantics and coverage assessment

### Positive source evidence

- Turn bracketing is at the entrypoints:
  - blocking `run_once`: `turn_execution.rs:16` starts, `:22` finishes.
  - capture `run_once_capture`: `:38` starts, `:45` finishes.
  - MPSC `run_once_streaming_mpsc`: `:100` starts, `:106` finishes.
- Provider request correlation is minted once per provider attempt by `provider_evidence_correlation`: `evidence.rs:134-140`.
- Provider requests are emitted before provider invocation:
  - blocking: `turn_loops.rs:103-114`.
  - MPSC: `turn_streaming_mpsc.rs:222-233`.
- Newly fixed terminal error branches emit correlated `ProviderResponse{Error}` before returning:
  - blocking raw stream transport `Err(e)`: `turn_loops.rs:203-249`.
  - blocking non-compaction `StreamEvent::Error`: `turn_loops.rs:593-651`.
  - MPSC raw stream transport `Err(e)`: `turn_streaming_mpsc.rs:410-467`.
- Success responses remain single terminal responses before normal continuation:
  - blocking success: `turn_loops.rs:691-710`.
  - MPSC success: `turn_streaming_mpsc.rs:975-995`.
- Durable persistence path is real append-to-JSONL, not just in-memory emit:
  - `append_session_evidence_with_correlation` creates a writer and calls `writer.append`: `evidence.rs:96-116`.
  - writer appends JSONL and increments sequence: `crates/jcode-base/src/session/evidence.rs:87-103`.
  - reader stops at a malformed trailing line and sorts by sequence: `session/evidence.rs:115-143`.
  - empty log now starts at sequence 0: `session/evidence.rs:165-170`.

### Strict fixture coverage

The happy-path fixture is strong for the narrow prerequisite:

- It uses disposable `JCODE_HOME` and telemetry opt-out: `agent_tests.rs:253-256`.
- It uses an in-process scripted provider, no tools, deterministic text, token usage, and `MessageEnd`: `:258-270`.
- It calls the real capture entrypoint: `:273-276`.
- It reads persisted evidence from disk: `:279-280`.
- It asserts exactly four events, sequences `0..=3`, schema version, shared turn ID, provider request ID on request/response only, route/model/provider/tool count, token usage, output summaries, and truncation behavior: `:281-395`.

This is real emit-to-persist-to-replay coverage for the strict no-tool/no-cancel/no-compaction path.

### Boundary assessment: cancellation/retry/compaction remain blocked, correctly not widened

Static source still shows known blocked boundaries:

- MPSC cancellation before stream open returns `Ok(())` after `ProviderRequest`, with no provider response: `turn_streaming_mpsc.rs:247-251`.
- MPSC mid-stream cancellation breaks the stream loop at `turn_streaming_mpsc.rs:370-385`, then falls through to success response emission at `:975-995`.
- `status_for_result` maps only `Ok` and `Error`, not `Cancelled` or `Interrupted`: `evidence.rs:203-209`.
- Context-limit retry/compaction can abandon an already emitted request without a terminal response:
  - blocking provider-open retry: `turn_loops.rs:127-140`.
  - blocking raw stream retry: `turn_loops.rs:207-233`.
  - MPSC provider-open retry: `turn_streaming_mpsc.rs:257-280`.
  - MPSC raw stream retry: `turn_streaming_mpsc.rs:414-451`.
  - MPSC event-error retry: `turn_streaming_mpsc.rs:864-901`.

The ledger does call these blocked, so this is not a new source regression. It is a hard scope boundary: no cancellation-inclusive or retry/compaction-inclusive pilot should pass.

## Commands and results

- `pwd && git rev-parse HEAD && git rev-parse 1b9d6e09f && git status --short && git diff --name-status 1b9d6e09f..HEAD`
  - Result: HEAD `74aaf1710ff8ef54ce69cc67dc77dd0121acec37`, base `1b9d6e09fe324123ba97b2f627934169484e835b`, clean worktree, changed files limited to evidence/turn source, tests, session evidence, and R12 ledger.
- `git log --oneline --decorate -5 --no-show-signature`
  - Result: HEAD `74aaf1710`, with implementation commits `61f241a9d` and `4ed674f14` over base.
- `git diff --stat 1b9d6e09f..HEAD`
  - Result: 6 files changed, 462 insertions, 61 deletions.
- `git diff --check 1b9d6e09f..HEAD`
  - Result: exit 0, no whitespace errors.
- `git diff --numstat 1b9d6e09f..HEAD -- docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md`
  - Result: `71 54`, showing non-append-only ledger rewrite.
- `git grep -n "append_provider_error_response\|ProviderResponse" ...`
  - Result: provider terminal emissions found at the cited source sites.
- `cargo test -p jcode-app-core r12_ -- --nocapture`
  - Result: exit 127, `cargo: command not found`. No build was run.
- `command -v cargo; command -v scripts/dev_cargo.sh; test -x scripts/dev_cargo.sh`
  - Result: no `cargo` on PATH; `scripts/dev_cargo.sh` exists and is executable. I did not invoke it to avoid compile/disk-expansion risk under the requested no-full-build/static preference.
- `git status --short`
  - Result: clean worktree after review.

## Confidence and gaps

**Confidence:** medium-high overall.

High confidence in static control-flow findings, request/response correlation semantics, the strict happy-path fixture shape, persistence path, and R11 ledger rewrite finding.

Gaps:

- I did not run the new tests because `cargo` is unavailable on PATH, and I avoided invoking the dev cargo wrapper to respect the no-full-build/disk-exhaustion constraint.
- I did not exercise live daemon, network, credentials, cancellation, retry, or compaction behavior.
- I did not perform a full writer census outside the changed R12/session-evidence paths.

## Required correction before acceptance

1. Make the R12 ledger R11-compliant: restore/preserve the prior adjudication and matrix as historical text, then append the implementation outcome as a dated amendment without deleting/replacing earlier decisions.
2. Strengthen error-path tests so they cannot pass with `TurnFinished{Ok}`, missing turn ID, extra/interleaved events, absent `TurnStarted`, non-contiguous sequence, or missing provider/model/route/error classification.
3. Add the missing MPSC `StreamEvent::Error` regression fixture for parity.
4. Decide and test `ProviderResponse.error_class` semantics: stable class or explicitly sanitized summary. Do not leave raw provider error text untested under a field named `error_class`.
5. Keep cancellation and retry/compaction pilots blocked until their source semantics and deterministic fixtures are fixed.
