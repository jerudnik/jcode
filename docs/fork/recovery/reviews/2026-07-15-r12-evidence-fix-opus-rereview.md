# R12 Corrected Package Re-Review (Independent, Bounded)

- **Verdict: PASS.** Commits `7c6044907`, `991e2c165`, and `f0e77020c` fully resolve the Fable CRITICAL-1 and IMPORTANT-1/2/3 findings without regressing the prior Opus-verified terminal-cardinality behavior. Cancellation/retry/compaction remain correctly fail-closed.
- **Reviewer:** independent verify agent (Opus), bounded re-review. Read-only, no edits.
- **Worktree:** `/Users/jrudnik/labs/jcode-fix-r12-evidence`
- **HEAD:** `f0e77020c20920a8d2e3225f976e5b7a4a1e1512` (clean) over base `1b9d6e09f`
- **New commits reviewed:** `7c6044907` (fix: stabilize error classes), `991e2c165` (test: harden error fixtures), `f0e77020c` (docs: append Fable follow-up rollup)
- **Prior artifacts read and preserved:** `/tmp/jcode-r12-fix-opus-review.md` (PASS, narrow) and `/tmp/jcode-r12-fix-fable-review.md` (FAIL for R11-compliance; PASS-with-scope-limits for source). Their disagreement is retained below.
- **Infra:** disk 31-32Gi free throughout, no exhaustion. Toolchain via `scripts/dev_cargo.sh` (Nix dev shell, rustc 1.96.0). No network/daemon/credentials.

## Preserved prior disagreement
- **Opus (first pass):** PASS for the narrow strict prerequisite; flagged only LOW items (add MPSC StreamEvent::Error test; confirm ledger status-table edits under R11).
- **Fable:** FAIL for acceptance as an R11-compliant package due to CRITICAL-1 (ledger rewrote prior recovery truth: `71 54` numstat, 54 deletions), plus IMPORTANT-1 (weak error helper), IMPORTANT-2 (missing MPSC stream-event fixture), IMPORTANT-3 (raw/untested `error_class`). Both agreed the source behavior was narrowly credible and that cancellation/retry/compaction must stay blocked.
This re-review adjudicates whether the three new commits close the Fable gap. They do.

## CRITICAL-1 (append-only ledger) — RESOLVED
- **Cumulative diff is now zero-deletion.** `git diff --numstat 1b9d6e09f..f0e77020c -- .../R12-agent-turn-evidence/ledger.md` returns `97 0` (97 insertions, **0 deletions**). `git diff ... | grep '^-[^-]'` returns empty.
- **Mechanism is correct, not a trick.** `f0e77020c` first restores the intermediate in-place edits (State, verdict, checkpoints 5-8, matrix rows, disagreement rows, R09 posture, validation) back to their verbatim base text, then appends a new dated section `## 2026-07-15 implementation amendment and current rollup`. Net effect versus base: the entire original adjudicated ledger is byte-preserved and a dated amendment is added below it. This is exactly the R11 rule (`R11 ledger:18,:26`: accumulate dated amendments, never rewrite earlier decisions).
- **Blocked history retained.** The restored body still carries checkpoint 5/6 "under-emission defects" and "cancellation can fabricate success", the matrix `**0**` rows, and the "blocked today" verdict as historical truth. The appended rollup explicitly states "The broader pilot remains fail-closed for cancellation and abandoned retry/context-limit/compaction attempts" and lists those rows as `Blocked/fail-closed` in its before/after table.
- **Preserved reviews intact.** `opus-review.md`/`grok-review.md` copies and their SHA-256 in the ledger are unchanged; only `ledger.md` was touched among docs.

## IMPORTANT-1 (weak error helper) — RESOLVED
`assert_one_terminal_error_response` was replaced by `assert_strict_terminal_error_evidence(events, expected_error_class)` (`agent_tests.rs`, commit `991e2c165`). It now asserts, mirroring the happy-path strictness:
- exact `events.len() == 4` and exact contiguous sequences `[0,1,2,3]`;
- schema version on all events; shared `turn_id` across all four;
- `events[0]` is `TurnStarted` (image_count 0, input present, user_message_index > 0), no provider_request_id;
- `events[1]` is `ProviderRequest` with provider `r12-fixture`, model `r12-fixture-model`, route `r12-fixture-route`, tool_count 0, prompt present;
- `events[2]` is `ProviderResponse{Error}` with `output.is_none()`, `usage.is_none()`, `error_class == expected`, and `provider_request_id == request_id`;
- **`events[3]` is `TurnFinished{Error}`** with `output.is_none()` and `error_class == expected`, no provider_request_id;
- `provider_terminal_counts == (1,1,1,[Error])`.
This closes the false-`Ok`/interleaved-event gap Fable identified: a `TurnFinished{Ok}`, missing `TurnStarted`, extra event, or non-contiguous sequence now fails.

## IMPORTANT-2 (MPSC StreamEvent::Error parity) — RESOLVED
New fixture `r12_mpsc_stream_event_error_persists_terminal_provider_response` (`991e2c165`) drives the real `run_once_streaming_mpsc` entrypoint with a scripted `StreamEvent::Error` and applies the same strict helper. The regression matrix is now symmetric: blocking raw-transport, blocking StreamEvent::Error, MPSC raw-transport, and MPSC StreamEvent::Error are all fixture-covered, plus the strict happy path (5 total).

## IMPORTANT-3 (unstable/secret error_class) — RESOLVED
- **Source unification (`7c6044907`):** `append_provider_error_response` now takes `&anyhow::Error` and sets `error_class: Some(error_class(error))`, using the shared low-cardinality classifier (`evidence.rs:211-228`: last cause, split on first `:`, trim, cap 120, fallback `error`). All six provider-error emission sites in both engines were converted from the inline `e.to_string().chars().take(120)` raw truncation to this single helper (provider-open, raw-transport, and StreamEvent::Error in both engines). For StreamEvent::Error, a `StreamError` is wrapped in `anyhow::Error` so the classifier applies uniformly; `StreamError`'s Display is `{message}` (single cause), so the classifier strips everything after the first colon.
- **Secret-bearing test evidence:** each error fixture now injects `"...failure: token=secret request=abc"` and asserts the stored `error_class` equals the pre-colon prefix (`"r12 raw transport failure"` etc.) and does **not** contain `token=` or `request=`. Both `ProviderResponse.error_class` and `TurnFinished.error_class` are asserted equal to that stable prefix, proving the raw request material is stripped and the two classifiers agree.

## Tests executed (safe, focused)
```
dev_cargo.sh test -p jcode-app-core --lib -- r12_
```
Result: **5 passed, 0 failed** (finished 1.41s):
- `r12_no_tool_turn_emits_and_persists_exactly_one_terminal_provider_response`
- `r12_blocking_raw_transport_error_persists_terminal_provider_response`
- `r12_blocking_stream_event_error_persists_terminal_provider_response`
- `r12_mpsc_raw_transport_error_persists_terminal_provider_response`
- `r12_mpsc_stream_event_error_persists_terminal_provider_response`

Other commands:
```
git diff --numstat 1b9d6e09f..f0e77020c -- .../ledger.md   # 97 0 (zero deletions)
git diff 1b9d6e09f..f0e77020c -- .../ledger.md | grep '^-[^-]'  # empty
git diff --stat 1b9d6e09f..f0e77020c   # 6 files, 616 ins / 38 del (source-only dels)
shasum -a256 .../opus-review.md .../grok-review.md   # unchanged vs ledger table
git show 7c6044907 / 991e2c165 / f0e77020c   # inspected fully
```

## No-regression checks
- Terminal single-emit behavior preserved: every error path still `return`s immediately after one `append_provider_error_response`; success paths unchanged. Grep confirms only the six provider-error sites route through the helper.
- Happy-path fixture unchanged and still green (part of the `r12_` run).
- Cancellation/retry/compaction: no source touched in these branches; the restored ledger body and the appended rollup both keep them `Blocked/fail-closed`. Verified `status_for_result` still maps only Ok/Error (historical blocked note intact).
- Rollup honesty: `f0e77020c` records that a full `-p jcode-app-core --lib` run hit two **unrelated** failures (`comm_session...cleans_session_when_launch_errors`, `selfdev...build_lock_is_removed_on_drop`) that passed on targeted rerun, recorded as concurrency/full-suite flakiness, not R12. This matches the known-flaky nature of those tests and is not an R12 fixture defect.

## Gaps / non-blocking observations
- **[INFO, out of scope]** Two `ToolFinished` events still use raw `e.to_string().chars().take(120)` for `error_class` (`turn_loops.rs:1142`, `turn_streaming_mpsc.rs:1503`). These are tool-execution errors, not `ProviderResponse` provider errors, and are outside R12's terminal-provider-error scope and outside the no-tool strict fixture (tool_count 0). Pre-existing pattern, not a Fable finding, not a regression. Worth a future consistency pass but does not block this package.
- **[INFO]** I did not run the full app-core suite (bounded re-review; the rollup already recorded the unrelated flakes and I confirmed the R12 subset green). No cancellation/retry/compaction execution attempted (correctly blocked).
- **Confidence:** High. All four Fable findings are closed with source I inspected and fixtures I executed myself; the append-only property is verified by cumulative numstat and a zero-match deletion grep; preserved-review hashes match.

## Bottom line
The corrected package resolves Fable's blocking CRITICAL-1 by making the cumulative R12 ledger diff append-only (zero deletions) while preserving both the blocked historical adjudication and the dated implementation rollup, and closes IMPORTANT-1/2/3 with a strict ordered error-evidence helper (including `TurnFinished{Error}`, shared identities, contiguous sequences), a new MPSC StreamEvent::Error parity fixture, and a stable non-secret `error_class` classifier proven by secret-injection assertions. All 5 R12 fixtures pass. Cancellation/retry/compaction remain fail-closed. **PASS.**
