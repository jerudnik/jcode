# Independent Read-Only Review: W1 R12 Cancellation/Retry Terminal Evidence

- **Reviewer:** opus verify agent (adversarial, read-mostly)
- **Repo:** `/Users/jrudnik/labs/jcode-w1-r12`
- **Branch:** `recovery/fix-r12-terminal-evidence-2026-07-15`
- **Fixed base:** `602709895be96a85a6090690c0b27d5681d17321`
- **Fixed HEAD:** `518d0632e9cb24d8b3d7f253d4e70ed8546e3043`
- **Date:** 2026-07-15
- **Constraints honored:** no repo edits; only this file written; offline only; no live provider/daemon/network/credentials/tools/MCP/reload/publication/baseline update/`--update`.

## Verdict: PASS (high confidence)

The 3-commit W1 package correctly closes the R12 cancellation and open/mid-stream
context-limit retry terminal-evidence defects for deterministic offline engine
fixtures, with clean commit boundaries, append-only ledger truth, preserved
reviews, real-seam test coverage, and all focused offline tests green. I found no
blocking defect. Nonblocking risks are scope gaps that the ledger itself
explicitly declares out of scope.

## Package structure and boundaries (verified)

Exactly 3 commits, exactly 5 files, clean class separation:

| Commit | Class | Files | Boundary check |
|---|---|---|---|
| `21304d8e4` fix: record R12 abandoned provider attempts | fix | `evidence.rs`, `turn_loops.rs`, `turn_streaming_mpsc.rs` | source-only, no tests/docs |
| `40235bd87` test: cover R12 cancellation retry evidence | test | `agent_tests.rs` (+460, `-0`) | pure additions; 5 pre-existing fixtures untouched |
| `518d0632e` docs: record R12 W1 terminal evidence | docs | `ledger.md` (+75, `-0`) | append-only, no rewrite |

- `git diff --name-only base..HEAD` = 5 files; `git diff --check` clean; working tree clean.
- Ledger diff is `75/0` (pure append). Preserved reviews (`opus-review.md`,
  `grok-review.md`, `reviews/*`) have zero diff in the package.

## Declared surface vs actual

The ledger's W1 amendment writer inventory matches the source:

- Blocking `ProviderRequest` at `turn_loops.rs:105`; success `ProviderResponse{Ok}` at `:714` (single).
- Blocking error responses via `append_provider_error_response` at `:129, :148, :224, :251, :628, :666`. Verified by grep: retry sites `:129/:224/:628` are the W1 additions; final-error sites `:148/:251/:666` pre-existed at base.
- MPSC `ProviderRequest` at `turn_streaming_mpsc.rs:224`; success `ProviderResponse{Ok}` at `:1009` (single).
- MPSC error responses at `:252` (cancel-before-open), `:266/:296` (open retry/final), `:395` (mid-stream cancel), `:447/:485` (raw stream retry/final), `:908/:957` (stream-event retry/final).
- `TurnFinished` centralized in `evidence.rs:42`, called only by wrappers `turn_execution.rs:22, :45, :106`. `grep -c TurnFinished` in both engine files = 0. Verified: engines never write finish directly.

## Adversarial writer/terminal trace (both engines)

### Correlation model
- `provider_evidence_correlation()` (`evidence.rs:135-141`) mints `{turn_id, provider_request_id: new Uuid}` per call; both engines build one per outer-loop iteration (`turn_loops.rs:103`, `turn_streaming_mpsc.rs:222`), so each provider attempt gets a fresh request id. The request emission clones it; every terminal response on that attempt clones the same id. Retry mints a new one -> distinct correlation per attempt (asserted `assert_ne!` in `assert_two_attempt_retry_evidence`).
- `finish_evidence_turn` uses `current_turn_evidence_correlation()` = turn_id only, no `provider_request_id` (asserted `is_none()` in `assert_turn_finished`).

### Cancellation
- **Cancel before open (MPSC):** base returned `Ok(())` after request with no response (confirmed at base `:247-252`). W1: constructs `interrupted_turn_error()`, emits one correlated `ProviderResponse{Error, class="turn interrupted"}` (`:252`), returns `Err`. Wrapper maps `TurnInterruptedError` -> `TurnFinished{Interrupted}` via `status_for_result` (`evidence.rs:208-216`). Result: 1 request, 1 correlated non-Ok response, 1 Interrupted finish. **Proven.**
- **Mid-stream cancel (MPSC):** base did `break` which fell through to the success `ProviderResponse{Ok}` and `Ok` finish (confirmed at base `:379`, false success). W1: emits one correlated error response (`:395`) then `return Err(interrupted)`. Success site at `:1009` is unreachable after early return -> no duplicate. Result: 1 request, 1 non-Ok response, 1 Interrupted finish. **Proven.**
- Blocking engine has no cooperative pre-open/mid-stream cancel select (confirmed: `graceful_shutdown` only used for guard registration and subagent plumbing, not a stream select). No cancellation regression; correctly out of scope.

### Mid-stream / open context-limit retry
- Both engines: each abandoned attempt emits one correlated `ProviderResponse{Error}` before `continue` (blocking `:129/:224/:628`, MPSC `:266/:447/:908`). `context_limit_retries` incremented; the `continue` re-enters the outer loop, re-mints correlation and re-emits the request. Success attempt hits the single success response, then one outer `TurnFinished{Ok}`. Result for the covered two-attempt shape: 2 requests, 2 responses (Error then Ok), 1 finish. Abandoned attempt terminally represented, not orphaned. **Proven for covered fixtures.**

### Success / error non-duplication
- Success response sites are singular per engine (`turn_loops.rs:714`, `turn_streaming_mpsc.rs:1009`). Every early error/cancel branch `return`s or `continue`s before reaching them, so no path emits two terminal responses for one request id. The ledger's original "no duplication, only under-emission" finding still holds; W1 converts under-emission to exactly-one without introducing duplication.

### Stable error class / secret redaction
- `error_class` (`evidence.rs:229-246`) takes the root cause, splits on `':'`, keeps the head, truncates to 120 chars. The retry fixtures inject `"context length exceeded tokens: token=secret request=abc"`; the persisted class is `"context length exceeded tokens"` (the `token=secret` tail is stripped). Interrupted class is the stable literal `"turn interrupted"`. No secret-bearing text persists.

## Persistence and sequence correctness
- Fixtures read back via `crate::session::read_session_evidence` (real R06A reader, not an in-memory shortcut). `assert_common_r12_sequence` asserts exact contiguous `sequence` (`0..=3` cancel, `0..=5` retry), uniform `schema_version == SESSION_LOG_EVENT_SCHEMA_VERSION`, and shared `turn_id` across all events. This is real-seam coverage, not a false-pass helper.

## Test-quality inspection (false-pass hunt)
- Providers are real `Provider` trait impls driving the true engine code paths: `MidStreamCancelProvider` + `NotifyingStream` deterministically synchronizes cancel *after* first poll via a oneshot (`polled_rx.await` then `stop_signal.fire()`), so the mid-stream branch is genuinely exercised, not raced. `RetryEvidenceProvider` scripts per-attempt open-error/stream. `DelayedProvider` (5s open) with pre-fired `request_graceful_shutdown` deterministically hits before-open cancel.
- Assertions check status, correlation id equality/inequality, provider/model/route/tool_count, output presence rules (Ok has output, non-Ok none), and exact terminal counts via `provider_terminal_counts`. No assertion is trivially true; helpers `panic!` on wrong event kinds.
- One preserved development caveat in ledger: an initial assertion wrongly required `TurnFinished{Ok}.output` for MPSC (which closes with `output=None`); narrowed to cardinality/status. The final `assert_turn_finished` only asserts `output.is_none()` for non-Ok, which is consistent with the MPSC `finish_evidence_turn(&result, .., None)` call. Not a false pass.

## Commands run (offline, no --update), counts, exits
- `JCODE_HOME=$(mktemp -d) JCODE_NO_TELEMETRY=1 bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_` -> **exit 0**, `9 passed; 0 failed; 1090 filtered out`. The 9 = 5 pre-existing strict fixtures (verified present at base via `git show 602709895:...agent_tests.rs`) + 4 new.
- `python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'` -> **exit 0**, `Ran 17 tests ... OK` (matches ledger's 17/17 classifier claim).
- R09 gate exits (recorded, no `--update`): `check_code_size_budget.py`=**1**, `check_test_size_budget.py`=**1**, `check_panic_budget.py`=**1**, `check_swallowed_error_budget.py`=**1**, `check_wildcard_reexport_budget.py`=**0**. All match the ledger's encoded-expected matrix; red debt remains visible and no baseline updated.
- `git diff --check 602709895 518d0632e` -> clean; `git status --short` -> clean.

## Ledger append-only and preservation (verified)
- W1 amendment is a new trailing section (`+75/-0`); no earlier matrix row, review text, hash, or claim limit rewritten.
- Opus/Grok preserved reviews and `reviews/*` sign-offs untouched in the package.
- The W1 amendment's self-reported validation (9 passed, fmt/diff-check clean, R09 matrix) reproduces against my independent runs.

## Nonblocking risks (all ledger-declared out of scope)
1. Blocking engine has no before-open/mid-stream cooperative cancel path, so cancellation coverage is MPSC-only. This is faithful to the code (no such blocking seam exists), not a gap the fix could close.
2. Retry fixtures cover the deterministic two-attempt shape only; the >MAX_CONTEXT_LIMIT_RETRIES exhaustion branch (`return Err(anyhow!)` after emitting the abandoned response) is not fixture-covered end-to-end. Control-flow inspection shows it still emits one correlated error response per abandoned attempt before the final give-up return, but this is unobserved in tests.
3. Live providers, daemon/reload, tools/MCP, generic compaction beyond the context-limit fixtures, and R06A schema changes are explicitly excluded by the amendment.
4. R13 census unchanged: W1 added no `provider_session_id` writer/reset site (verified: diffs touch only evidence/error-response emission, not session-id writers).

## What I did not check
- Live/end-to-end turn against a real provider, daemon, or network (prohibited and out of scope).
- Full workspace `cargo fmt`/`clippy`/build beyond the compiled `-p jcode-app-core` test build (the lib test compiled with only one pre-existing unrelated dead-code warning).
- Non-R12 test suites and unrelated inherited debt internals beyond gate exit codes.
- TUI-local session-id divergence window and tool-continuation multi-call cardinality (out of strict scope).
- Byte-for-byte re-hash of preserved external review artifacts (originals in `/tmp` not required for this W1 delta; package diff shows zero change to the repository copies).

## Bottom line
Every covered path satisfies the R12 invariant: one request, one correlated
terminal response per emitted request, cancellations finish `Interrupted` (never
false `Ok`), retry paths emit multiple requests with each abandoned attempt
terminally represented, and exactly one `TurnFinished` per user turn. Success and
error paths do not duplicate responses. Correlation, sequence, stable error class
(with secret redaction), and persistence are correct. The five strict R12
fixtures remain green. Commit boundaries, ledger append-only truth, and review
preservation hold. **PASS, high confidence.**
