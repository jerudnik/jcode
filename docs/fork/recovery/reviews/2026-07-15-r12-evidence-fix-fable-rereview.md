# R12 corrected package bounded Fable re-review

**Reviewer:** independent Fable-style adversarial bounded re-review  
**Worktree:** `/Users/jrudnik/labs/jcode-fix-r12-evidence`  
**HEAD reviewed:** `f0e77020c20920a8d2e3225f976e5b7a4a1e1512` (`docs: append R12 Fable follow-up rollup`)  
**Base:** `1b9d6e09fe324123ba97b2f627934169484e835b` (`docs(recovery): complete Phase 2 ledger gate`)  
**Range:** `1b9d6e09f..f0e77020c`  
**Prior review inputs read:**

- Prior Fable FAIL: `/tmp/jcode-r12-fix-fable-review.md`.
- Independent Opus PASS: `/tmp/jcode-r12-fix-opus-review.md`.

**Mode:** read-only against repository contents. No edits to repo. No live services, network provider calls, credentials, daemon behavior, cancellation execution, retry execution, or compaction execution. Focused local tests only through `scripts/dev_cargo.sh`.

## Verdict: PASS for the corrected narrow R12 prerequisite package

The previously required corrections are verified:

1. The R12 ledger is restored to the base text byte-for-byte and now only appends a dated rollup over base, with ledger diff `97 0` and `prefix_matches_base True`.
2. Error fixtures now assert strict exact four-event sequence/order, contiguous IDs/sequences, shared turn ID, request/response correlation, provider/model/route identity, stable classification, and `TurnFinished { status: Error }`.
3. The previously missing MPSC `StreamEvent::Error` real persist/replay fixture exists and passed.
4. Provider terminal `error_class` now uses the shared stable classifier instead of raw/truncated provider error text, and fixtures inject secret-like text and assert it is absent.
5. Provider terminal error paths return immediately after emitting the error response, preventing duplicate provider terminal records. The focused R12 fixtures passed 5/5.
6. Cancellation, retry, and compaction remain visibly blocked and are not widened by the amendment.

No CRITICAL or IMPORTANT findings block the narrow corrected package.

## Correction-by-correction verification matrix

| Required correction from prior FAIL / DM | Evidence checked | Result |
|---|---|---|
| Restore base ledger truth verbatim and append only, with zero deletions over base. | `git diff --numstat 1b9d6e09f..HEAD -- docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md` returned `97 0`. Byte-prefix check reported base SHA `c794d74da135daffe20687ab3c31cb04ebe2855cb2657d4e07ddc74893ead127`, current-prefix SHA same, `prefix_matches_base True`, `append_lines 97`. | PASS |
| Strict exact error-event sequence/order. | `crates/jcode-app-core/src/agent_tests.rs:220-335` asserts `events.len() == 4`, sequence `[0,1,2,3]`, and exact kinds at indices 0-3: `TurnStarted`, `ProviderRequest`, `ProviderResponse`, `TurnFinished`. | PASS |
| Strict IDs and correlations. | `agent_tests.rs:240-249` asserts all events share the same `turn_id`; `:263` and `:329` assert no provider request ID on turn start/finish; `:265-286` captures request ID from `ProviderRequest`; `:311-314` asserts `ProviderResponse` uses that same ID. | PASS |
| Strict provider/model/route identity. | `agent_tests.rs:273-278` asserts provider `r12-fixture`, model `r12-fixture-model`, route `r12-fixture-route`, and `tool_count == 0`; `:288-309` asserts response provider/model and error status. | PASS |
| Strict classification and `TurnFinished Error`. | `agent_tests.rs:303-308` asserts provider `error_class` equals expected stable class, is non-empty, and lacks `token=` / `request=`. `:316-326` asserts `TurnFinished.status == Error`, no output, and same class. | PASS |
| Add MPSC `StreamEvent::Error` real persist fixture. | Test `r12_mpsc_stream_event_error_persists_terminal_provider_response` at `agent_tests.rs:542-562` drives `run_once_streaming_mpsc`, reads persisted evidence via `read_session_evidence`, and uses the strict helper. Focused R12 test run passed it. | PASS |
| Stable classifier cannot contain raw provider text. | `crates/jcode-app-core/src/agent/evidence.rs:142-159` now takes `&anyhow::Error` and stores `error_class(error)`. `:211-228` takes the final cause, splits before `:`, trims, caps length, and falls back to `error`. Error fixtures pass messages containing `token=secret request=abc` but expect only prefixes at `agent_tests.rs:490-499`, `:508-518`, `:527-539`, and `:548-561`. | PASS |
| No duplicate terminal provider records. | Source terminal error helper call sites return immediately after emitting: blocking open error `turn_loops.rs:141-148`, blocking raw stream error `:237-244`, blocking event error `:641-648`, MPSC open error `turn_streaming_mpsc.rs:281-288`, MPSC raw stream error `:455-462`, MPSC event error `:916-923`. Success responses are separate post-loop sites, blocking `turn_loops.rs:689-708` and MPSC `turn_streaming_mpsc.rs:967-980`, unreachable after those returns. Tests count `(1,1,1, [Error])` for error paths at `agent_tests.rs:331-334`. | PASS |
| Cancellation/retry/compaction still blocked, not widened. | MPSC cancel before open still returns `Ok(())` at `turn_streaming_mpsc.rs:247-251`; cancellation/retry/compaction grep shows unchanged blocked sites at `turn_streaming_mpsc.rs:257`, `:365`, `:409`, `:859` and `turn_loops.rs:128`, `:202`, `:595`; `status_for_result` still maps only Ok/Error at `evidence.rs:203-209`. Ledger append explicitly blocks these rows at `ledger.md:360-363` and excludes them at `:398-407`. | PASS |

## Detailed source assessment

### Ledger compliance

The prior blocker was R11 ledger rewriting. The corrected package resolves it over the acceptance base:

```text
git diff --numstat 1b9d6e09f..HEAD -- docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md
97	0	docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md
```

The stronger byte-level check confirms the current ledger begins with the exact base ledger bytes:

```text
base_bytes 26585 current_bytes 33723
base_sha256 c794d74da135daffe20687ab3c31cb04ebe2855cb2657d4e07ddc74893ead127
current_prefix_sha256 c794d74da135daffe20687ab3c31cb04ebe2855cb2657d4e07ddc74893ead127
prefix_matches_base True
append_bytes 7138
base_lines 310 current_lines 407 append_lines 97
```

The appended section starts at `docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md:313` and explicitly states the historical base, blocked verdict, matrix truth, and reviews remain preserved verbatim. This satisfies the required R11 append-only correction.

### Provider terminal error classification

`append_provider_error_response` now has a single provider-error response construction site:

- `crates/jcode-app-core/src/agent/evidence.rs:142-159`: constructs `ProviderResponse { status: Error, output: None, usage: None, error_class: Some(error_class(error)) }`.
- `evidence.rs:211-228`: `error_class` uses the last cause, strips after the first colon, trims, caps at 120 characters, and falls back to `error`.

This is not a perfect taxonomy, but it satisfies the specific correction: the recorded provider terminal class is a stable prefix and cannot include the injected raw suffix after `:` such as `token=secret request=abc`. The strict tests assert this directly.

A grep found two remaining raw/truncated `error_class` assignments at `turn_loops.rs:1142` and `turn_streaming_mpsc.rs:1503`, but both are `ToolFinished` rows, not `ProviderResponse` terminal provider evidence. They are outside this R12 no-tool/no-cancel/no-compaction provider-terminal correction and do not contradict the requested provider-text fix. They may be worth a separate hygiene ticket if tool evidence classification is later in scope.

### MPSC StreamEvent::Error parity

The missing fixture from the prior FAIL now exists:

- `agent_tests.rs:542-562` creates a scripted provider that emits `StreamEvent::Error { message: "r12 mpsc stream event failure: token=secret request=abc", retry_after_secs: Some(2) }`.
- It calls the real `run_once_streaming_mpsc` entrypoint.
- It reads durable evidence via `crate::session::read_session_evidence(&session_id)`.
- It applies `assert_strict_terminal_error_evidence(&events, "r12 mpsc stream event failure")`.

The source path is also corrected/shared:

- `turn_streaming_mpsc.rs:914-923` wraps the stream event error, emits `append_provider_error_response`, and immediately returns `Err`.

### Duplicate terminal record risk

For the corrected error paths, the source emits and returns immediately. That prevents fall-through to the success `ProviderResponse` sites. The test helper additionally rejects duplicates by requiring exactly one request, one response, and one finish, with only one response status.

The no-tool success path remains exactly one terminal `ProviderResponse { Ok }` in the strict happy fixture and passed.

### Blocked boundaries remain blocked

This rereview did not execute cancellation, retry, or compaction, by request. Static source and ledger posture show they remain excluded:

- Cancellation before MPSC stream open can return `Ok(())` after provider request and before provider response: `turn_streaming_mpsc.rs:247-251`.
- Mid-stream MPSC cancellation still uses the graceful shutdown branch around `turn_streaming_mpsc.rs:365` and remains outside the qualified slice.
- Context-limit retry/compaction branches still continue or break before terminal evidence for abandoned attempts: blocking `turn_loops.rs:128`, `:202`, `:595`; MPSC `turn_streaming_mpsc.rs:257`, `:409`, `:859`.
- `status_for_result` still has only Ok/Error mapping at `evidence.rs:203-209`.
- Ledger append keeps MPSC cancellation and retry/compaction rows blocked at `ledger.md:360-363` and acceptance exclusions at `ledger.md:398-407`.

This is the correct bounded posture for the narrow R12 prerequisite. Do not widen this PASS to cancellation-inclusive, retry-inclusive, compaction-inclusive, live-provider, or tool-continuation claims.

## Commands and results

- `cd /Users/jrudnik/labs/jcode-fix-r12-evidence && pwd && git rev-parse HEAD && git rev-parse 1b9d6e09f && git status --short && git diff --name-status 1b9d6e09f..HEAD && git diff --stat 1b9d6e09f..HEAD`
  - Result: clean worktree, HEAD `f0e77020c20920a8d2e3225f976e5b7a4a1e1512`, base `1b9d6e09fe324123ba97b2f627934169484e835b`, six changed files over base.
- `git log --oneline --decorate -8 --no-show-signature`
  - Result: corrective commits over the prior reviewed `74aaf1710`: `7c6044907`, `991e2c165`, and `f0e77020c`.
- `ls -l /tmp/jcode-r12-fix-fable-review.md /tmp/*r12*opus* /tmp/*r12*pass*`
  - Result: prior Fable FAIL and Opus PASS located and read.
- `git diff --numstat 1b9d6e09f..HEAD -- docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md`
  - Result: `97 0`, append-only over base.
- Python byte-prefix ledger comparison against `git show 1b9d6e09f:.../ledger.md`
  - Result: `prefix_matches_base True`; base and current-prefix SHA-256 both `c794d74da135daffe20687ab3c31cb04ebe2855cb2657d4e07ddc74893ead127`.
- `grep -R "append_provider_error_response\|ProviderResponse" -n crates/jcode-app-core/src/agent ...`
  - Result: provider response emissions are the helper error sites plus success response sites cited above.
- `grep -R "error_class: Some" -n crates/jcode-app-core/src/agent crates/jcode-base/src/session crates/jcode-session-types/src/evidence.rs`
  - Result: provider terminal path uses `error_class(error)`; remaining raw app-core sites are `ToolFinished`, not provider responses.
- `git diff --check 1b9d6e09f..HEAD`
  - Result: exit 0, no whitespace errors.
- `scripts/dev_cargo.sh test -p jcode-app-core r12_ -- --nocapture`
  - Result: 5 passed, 0 failed, 1082 filtered out. Passed tests:
    - `agent::tests::r12_mpsc_stream_event_error_persists_terminal_provider_response`
    - `agent::tests::r12_mpsc_raw_transport_error_persists_terminal_provider_response`
    - `agent::tests::r12_blocking_stream_event_error_persists_terminal_provider_response`
    - `agent::tests::r12_no_tool_turn_emits_and_persists_exactly_one_terminal_provider_response`
    - `agent::tests::r12_blocking_raw_transport_error_persists_terminal_provider_response`
- `scripts/dev_cargo.sh test -p jcode-base session::evidence::tests::writer_appends_and_reader_orders_events -- --nocapture`
  - Result: the intended unit test ran and passed, but wrapper exited 97 because integration targets matched zero tests.
- `scripts/dev_cargo.sh test -p jcode-base --lib session::evidence::tests::writer_appends_and_reader_orders_events -- --nocapture`
  - Result: exit 0; 1 passed, 0 failed, 1155 filtered out.
- `git status --short`
  - Result: clean before and after tests. Ignored/generated `./target` and `./.cargo` exist.

## Findings

No CRITICAL findings.

No IMPORTANT findings blocking the corrected narrow package.

### Informational: raw `ToolFinished.error_class` remains out of scope

`turn_loops.rs:1142` and `turn_streaming_mpsc.rs:1503` still use raw/truncated tool error text for `ToolFinished.error_class`. This is not provider terminal evidence and the R12 strict fixtures are no-tool, so it does not fail the requested correction. If future work generalizes evidence classification beyond provider terminal errors, these should be revisited.

## Confidence and gaps

**Confidence:** high for the bounded rereview.

High confidence in ledger append-only compliance, the strict error fixture semantics, MPSC StreamEvent::Error parity coverage, provider terminal classifier stabilization, no duplicate provider terminal records on corrected branches, and retained cancellation/retry/compaction block posture.

Gaps:

- I did not run full app-core or workspace test suites. The bounded R12 app-core fixtures and one base evidence writer test were run.
- I did not execute live provider calls, network, daemon, credentials, cancellation, retry, or compaction behavior.
- I did not review every unrelated evidence event kind beyond checking that remaining raw `error_class` assignments are not `ProviderResponse` rows.

## Acceptance boundary

Accept this package only as the R12 strict no-tool/no-cancel/no-compaction prerequisite plus fixed non-retry terminal provider error branches. Keep cancellation-inclusive, retry-inclusive, context-limit-inclusive, compaction-inclusive, live-provider, and tool-continuation pilots blocked until separately implemented and fixture-covered.
