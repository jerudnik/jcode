# W1 R12 Fable-class independent re-review

- Reviewer posture: adversarial read-mostly verification.
- Worktree: `/Users/jrudnik/labs/jcode-w1-r12`.
- Fixed base: `602709895be96a85a6090690c0b27d5681d17321`.
- Fixed HEAD: `518d0632e9cb24d8b3d7f253d4e70ed8546e3043`.
- Report path: `/tmp/jcode-w1-fable-rereview.md`.

## Verdict: FAIL

W1 mostly implements the requested per-request terminal evidence behavior for deterministic offline fixtures, and the focused `r12_` suite passes. However, I do not pass the package because the shared `error_class` classifier is still not secret-safe for provider errors whose sensitive detail appears before the first colon, or for no-colon errors. W1 reuses that classifier on the newly covered context-limit and cancellation/terminal error paths, so the package still cannot substantiate the requested stable secret-safe error-class claim.

## Findings

### CRITICAL

None found.

### IMPORTANT

#### IMPORTANT-1: `error_class` remains raw-provider-text-derived and can persist secret-prefix/no-colon details

Evidence:

- `crates/jcode-app-core/src/agent/evidence.rs:229-246` implements `error_class(error)` as `error.chain().last().map(|cause| cause.to_string().split(':').next()...take(120))`. This is a prefix heuristic over arbitrary provider error text, not a closed classifier.
- `crates/jcode-app-core/src/agent/evidence.rs:143-160` writes that string into every `ProviderResponse{Error}` through `append_provider_error_response`.
- W1 adds or uses this helper on all newly reviewed terminal-error paths: blocking `turn_loops.rs:129,148,224,251,628,666`; MPSC `turn_streaming_mpsc.rs:252,266,296,395,447,485,908,957`.
- The current tests only prove one message shape where sensitive text follows the first colon: e.g. `agent_tests.rs:837-846`, `855-865`, `874-886`, `895-908`, `972-991`, and `1001-1021` use messages such as `context length exceeded tokens: token=secret request=abc` and assert the prefix. They do not test `token=secret request=abc: context length exceeded tokens`, `invalid API key sk-...`, or no-colon provider errors.
- R12's preserved invariant says evidence must contain “no secret credential value” (`docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md:52-55`). The W1 request explicitly asked for a stable secret-safe error class. A prefix-of-raw-provider-text classifier does not satisfy that property.

Impact:

- A provider/open/stream/context-limit error whose string begins with a secret, request id, URL, token, or other sensitive detail would persist that detail in `ProviderResponse.error_class` and `TurnFinished.error_class`.
- This is not a duplicate/orphan cardinality failure. It is a durable-evidence safety and docs-claim failure.

Expected fix direction:

- Replace prefix extraction with a small enum/allowlist classifier, such as `context_limit`, `turn_interrupted`, `stream_error`, `provider_open_error`, `transport_error`, `unknown_error`, or equivalent, and keep raw text only in logs if logs have their own redaction policy.
- Add adversarial fixtures where the raw error starts with `token=secret request=abc` and where the raw error has no colon.

### MINOR

#### MINOR-1: Retry fixture matrix is source-supported but not fully symmetric across both engines

Evidence:

- Source inspection found both engines now have terminal responses for provider-open retry/final error and stream-event retry/final error: blocking call sites at `turn_loops.rs:129,148,224,251,628,666`; MPSC call sites at `turn_streaming_mpsc.rs:266,296,447,485,908,957`.
- The new tests include one blocking open/context-limit retry fixture, `r12_open_context_limit_retry_persists_terminal_response_per_attempt` (`agent_tests.rs:966-994`), and one MPSC mid-stream context-limit retry fixture, `r12_mid_stream_context_limit_retry_persists_terminal_response_per_attempt` (`agent_tests.rs:995-1022`).
- There is no matching MPSC open/context-limit retry fixture and no matching blocking mid-stream context-limit retry fixture in the four new W1 tests.

Impact:

- I treat this as a scope limit, not the failing reason, because source paths are simple and the W1 acceptance text can be read as requiring representative open and mid-stream retry fixtures rather than a 2x2 engine matrix.
- A follow-up would raise confidence and catch future engine skew.

#### MINOR-2: My focused validation command was not perfectly offline/pure despite tmp-only target/home

Evidence:

- I ran `JCODE_HOME=$(mktemp -d /tmp/jcode-w1-home.XXXXXX) JCODE_NO_TELEMETRY=1 CARGO_TARGET_DIR=/tmp/jcode-w1-cargo-target bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_`.
- Exit: `0`; result: `9 passed; 0 failed; 0 ignored; 1090 filtered out`; duration reported by background task: `333.23s`.
- The script output included: `dev_cargo: cargo not on PATH; re-entering repo Nix dev shell`, `install-git-hooks: installed ...`, and `fork: (remote state refreshing in background; rerun for an updated verdict)`.

Impact:

- The actual test exercised only local in-process fixtures and wrote build/home artifacts under `/tmp`, but the dev-shell wrapper performed hook setup and reported a background remote refresh. That violates the ideal “offline only” review process. I did not rely on remote state for any verdict.
- `git status --short` after the run printed no tracked changes and exited `0`.

## Audit checklist

- RECOVERY_PLAN W1: Reviewed `docs/fork/recovery/RECOVERY_PLAN.md:89-95`. It correctly states per-request, not per-user-turn, terminal response cardinality and permits multiple requests on retry paths.
- R12 ledger: Reviewed `docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md:411-482`. The W1 amendment is append-only over prior blocked truth and does not rewrite earlier matrix rows.
- Three commits: Verified exactly three commits in range: `21304d8e4` fix, `40235bd87` test, `518d0632e` docs. Commit boundaries match source/test/docs separation.
- Both engine evidence paths: Inspected blocking `turn_loops.rs` and MPSC `turn_streaming_mpsc.rs`. Request sites are exactly two: `turn_loops.rs:105`, `turn_streaming_mpsc.rs:224`. Success response sites are `turn_loops.rs:714`, `turn_streaming_mpsc.rs:1009`; shared error response helper at `evidence.rs:143-160`.
- Per-request/per-turn cardinality: The code emits one fresh provider request id per attempt via `provider_evidence_correlation` (`evidence.rs:135-140`) and tests assert 2 requests/2 responses/1 turn finish for retry fixtures (`agent_tests.rs:650-681`). Docs do not overclaim one provider response per user turn.
- Cancellation timing: MPSC cancel-before-open returns after appending a provider error response (`turn_streaming_mpsc.rs:247-259`); mid-stream cancel returns after appending a provider error response (`turn_streaming_mpsc.rs:380-402`). Fixtures assert interrupted turn finish and non-Ok provider response (`agent_tests.rs:618-647`, `912-963`).
- Open/mid-stream context-limit retries: Source appends an error response before continuing retry in open and stream-event context-limit branches. Representative fixtures pass for blocking open retry and MPSC mid-stream retry.
- Correlation/sequence/persistence: Tests assert exact event sequence vectors `0..=3` and `0..=5`, shared `turn_id`, request/response provider_request_id pairing, durable readback, and trailing malformed-line tolerance for the strict success fixture (`agent_tests.rs:392-507`, `509-681`, `685-829`).
- Stable secret-safe error class: FAIL. See IMPORTANT-1.
- Duplicate/orphan risk: No path found that reaches both a helper error response and the success response for the same provider request id. Retry paths use distinct request ids and one response per request. Tests assert counts and distinct retry ids (`agent_tests.rs:667-680`).
- Real-seam fixture quality: Fixtures run through `run_once_capture` and `run_once_streaming_mpsc`, which call the actual turn wrappers and centralized `finish_evidence_turn` (`turn_execution.rs:27-47`, `51-110`). The mid-stream cancel fixture uses a polling stream and fires the actual shutdown signal after stream polling (`agent_tests.rs:940-963`).
- Success-path regression: Existing strict success fixture remains green and validates provider/model/route/tool-count/token-usage and replay (`agent_tests.rs:685-829`).
- R09 honesty: W1 ledger records expected-red R09 gates and no `--update` (`ledger.md:467-477`). I did not rerun R09 in this review.
- Docs overclaim: The W1 per-request cardinality and boundaries are honest. The remaining overclaim is the secret-safe classifier property discussed in IMPORTANT-1.

## Commands and exact outputs/counts

All commands below were run in `/Users/jrudnik/labs/jcode-w1-r12` unless noted.

1. Fixed refs and range inventory:

```text
pwd
git rev-parse HEAD
git rev-parse 602709895be96a85a6090690c0b27d5681d17321
git rev-parse 518d0632e9cb24d8b3d7f253d4e70ed8546e3043
git status --short
git log --oneline --decorate 602709895be96a85a6090690c0b27d5681d17321..518d0632e9cb24d8b3d7f253d4e70ed8546e3043
git diff --stat 602709895be96a85a6090690c0b27d5681d17321..518d0632e9cb24d8b3d7f253d4e70ed8546e3043
git diff --name-status 602709895be96a85a6090690c0b27d5681d17321..518d0632e9cb24d8b3d7f253d4e70ed8546e3043
```

Exit `0`. Counts: 3 commits, 5 changed files, 625 insertions, 6 deletions. Changed files are `evidence.rs`, `turn_loops.rs`, `turn_streaming_mpsc.rs`, `agent_tests.rs`, and the R12 ledger. `HEAD` matched requested `518d0632e9cb24d8b3d7f253d4e70ed8546e3043`. Initial `git status --short` printed no tracked changes.

2. Static counts:

```text
find . -name AGENTS.md -print | wc -l
git rev-list --count $BASE..$HEAD
git diff --name-only $BASE..$HEAD | wc -l
git diff $BASE..$HEAD -- crates/jcode-app-core/src/agent_tests.rs | rg '^\+async fn r12_' | wc -l
rg -n '^async fn r12_' crates/jcode-app-core/src/agent_tests.rs | wc -l
rg -n 'append_provider_error_response\(' crates/jcode-app-core/src/agent/{turn_loops.rs,turn_streaming_mpsc.rs} | wc -l
rg -n 'SessionLogEventKind::ProviderRequest' crates/jcode-app-core/src/agent/turn_{loops,streaming_mpsc}.rs | wc -l
rg -n 'SessionLogEventKind::ProviderResponse' crates/jcode-app-core/src/agent/turn_{loops,streaming_mpsc}.rs crates/jcode-app-core/src/agent/evidence.rs | wc -l
```

Exit `0`. Results: AGENTS in repo `0`; commit count `3`; changed files `5`; new W1 `r12_` tests `4`; total `r12_` tests `9`; error-response helper call sites in engines `14` (`6` blocking, `8` MPSC); provider request sites `2`; explicit provider response constructor sites `3` including shared helper and two success sites.

3. Hashes and diff check:

```text
shasum -a 256 docs/fork/recovery/RECOVERY_PLAN.md
shasum -a 256 docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md
git diff --check 602709895be96a85a6090690c0b27d5681d17321..518d0632e9cb24d8b3d7f253d4e70ed8546e3043
```

Exit `0`. `RECOVERY_PLAN.md` hash `e8a2df0eb4a2252a042143a343b86bde38861b9f3e3ad093882adaec587db3db`; R12 ledger hash `26157d7717d27cbae21cddb8ca250f259d88178cc78f1b2aa4acfd7a5012ff2d`; diff-check output line count `0`.

4. Focused tests:

```text
JCODE_HOME=$(mktemp -d /tmp/jcode-w1-home.XXXXXX) \
JCODE_NO_TELEMETRY=1 \
CARGO_TARGET_DIR=/tmp/jcode-w1-cargo-target \
bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_
```

Exit `0`. Result: `9 passed; 0 failed; 0 ignored; 0 measured; 1090 filtered out; finished in 1.48s`. Build/test task duration `333.23s`. One unrelated warning: `drop_control_log_handle` dead code in `server/control_log_sync.rs:263`. Caveat: the wrapper re-entered the Nix dev shell and printed hook setup plus a remote-refresh message, so this was not a perfectly offline wrapper invocation even though the test itself was local.

5. Final tracked status:

```text
git status --short
```

Exit `0`; output empty.

## Gaps and scope limits

- I did not rerun the full `jcode-app-core` suite or R09 matrix. I relied on W1 ledger's recorded R09 results and ran only focused `r12_` tests.
- I did not execute live providers, daemon/reload behavior, tools/MCP, credentials, publication, real network workflows, or `--update`.
- I did not test a full symmetric retry matrix across both engines.
- I did not attempt to mutate code or docs.
- Process caveat: despite the instruction to write only this report, I generated temporary diff/test artifacts under `/tmp` while reviewing and used `/tmp` for `JCODE_HOME` and `CARGO_TARGET_DIR`. No tracked worktree changes remain.

## Confidence

Medium-high for the FAIL verdict. The cardinality/correlation behavior is high confidence from source and passing focused tests. The failing secret-safety finding is high confidence for the classifier as written, but medium confidence on real-world exploitability because I did not inspect every provider's exact error string shapes.

## Report hash

The SHA-256 of the finalized report file is reported out-of-band after writing to avoid a self-referential hash field.
