# W1 R12 error-class remediation independent re-review (Opus, offline)

- Reviewer posture: adversarial, read-mostly verification.
- Worktree: `/Users/jrudnik/labs/jcode-w1-r12`.
- Branch: `recovery/fix-r12-terminal-evidence-2026-07-15`.
- Full W1 base: `602709895be96a85a6090690c0b27d5681d17321`.
- Prior review-disagreement HEAD (pre-remediation): `9afe5bdb7d96b0bc30e29a17dec090f469ce75e4`.
- Current remediation HEAD: `c77f5e24628692eab89f5adf49081512ba4d429d`.
- Report path (only artifact written): `/tmp/jcode-w1-remediation-opus-review.md`.
- Constraints honored: offline only; no live provider/daemon, network, credentials, MCP/tools, reload, publication, baseline update, or `--update`. No code or docs mutated. Existing worktree `target/` used; no custom `CARGO_TARGET_DIR`.

## Verdict: PASS (high confidence)

The single prior Fable IMPORTANT blocker is closed. Persisted `error_class` on both
`ProviderResponse{Error}` and `TurnFinished{error_class}` now derive exclusively
from a closed, stable, `'static` allowlist. No path in the in-scope error seam can
persist raw provider text, secret prefixes, no-colon secrets, request ids, tokens,
or URLs. Adversarial secret-prefix and no-colon fixtures cross the real persistence
seam (`run_once_capture` / `read_session_evidence`) and prove no secret substring
persists. Cardinality/correlation, both engines, commit boundaries, append-only
ledger/review preservation, R09 honesty, and declared scope all hold.

## Blocker closure verification (the Fable IMPORTANT-1)

### Classifier is now a closed stable allowlist

- `crates/jcode-app-core/src/agent/evidence.rs:237-258` defines
  `enum EvidenceErrorClass { ContextLimit, ProviderOpen, StreamTransport, StreamEvent,
  TurnInterrupted, Unknown }` with `as_str` mapping to fixed `'static` labels:
  `context_limit`, `provider_open_error`, `stream_transport_error`, `stream_error`,
  `turn_interrupted`, `unknown_error`. No format, split, prefix, or truncation of
  provider text remains.
- Pre-remediation confirmation of the fixed defect: at `9afe5bdb7...:evidence.rs:229-241`
  the old `error_class` used `...split(':').next()...take(120)` over raw error text.
  The remediation replaced this. Confirmed by `git show 9afe...:.../evidence.rs`.

### No persisted `ProviderResponse`/`TurnFinished` error_class can derive raw text

- `TurnFinished.error_class` is set at `evidence.rs:48` via
  `result.as_ref().err().map(error_class_for_error)`; `error_class_for_error`
  (`evidence.rs:274-276`) returns `classify_evidence_error(error).as_str().to_string()`,
  i.e. a closed label only.
- `ProviderResponse{Error}.error_class` is set only inside
  `append_provider_error_response` (`evidence.rs:143-164`), as
  `Some(error_class.as_str().to_string())` from a caller-passed `EvidenceErrorClass`
  enum. The `_error: &anyhow::Error` parameter is intentionally unused (underscore).
- The only three `ProviderResponse` constructor sites are:
  `evidence.rs:153` (the error helper, closed enum) and the two success sites
  `turn_loops.rs:736` and `turn_streaming_mpsc.rs:1033`, both of which carry
  `status: Ok` and `error_class: None` (verified at `turn_loops.rs:752` and
  `turn_streaming_mpsc.rs:1049`). No raw-text error class reaches a ProviderResponse.

### Explicit call-site categories are truthful and wrappers do not lose class

All 14 error-response call sites pass a truthful explicit class matching their branch:
- Blocking (`turn_loops.rs`): ContextLimit open retry `:135`, ContextLimit abandon
  `:148`; ProviderOpen `:158/:163`; ContextLimit stream retry `:238`, abandon `:251`;
  StreamTransport `:269/:274`; ContextLimit late retry `:650/:663`; StreamEvent `:692`.
- MPSC (`turn_streaming_mpsc.rs`): TurnInterrupted cancel-before-open `:258`;
  ContextLimit open retry `:273`, abandon `:286`; ProviderOpen `:307/:312`;
  TurnInterrupted mid-stream cancel `:410`; ContextLimit stream retry `:463`,
  abandon `:476`; StreamTransport `:505/:510`; ContextLimit late retry `:932/:945`;
  StreamEvent `:985`.
- Wrapper preservation: `classified_evidence_error` (`evidence.rs:166-171`) boxes a
  `ClassifiedEvidenceError { error_class, error }`. Its `Display` delegates to the
  inner error but it does **not** implement `source()`, so `error.chain()` yields the
  wrapper itself and `classify_evidence_error` (`evidence.rs:278-290`) recovers the
  explicit `error.error_class` at `:283-285`. The class the call site declares for
  `ProviderResponse` is therefore identically what `TurnFinished` records. No wrapper
  loses the explicit class.
- No mislabeling risk in classify order: the loop checks `TurnInterruptedError` first,
  then `ClassifiedEvidenceError`, then `StreamError`, else `Unknown`. `classified_evidence_error`
  is never used to wrap a `TurnInterruptedError` (all 10 call sites wrap context/provider/
  stream errors), and the interrupted paths return `interrupted_turn_error()` directly,
  so the first-match order cannot mislabel.

### Consistency of context-limit/interrupted/open/transport/stream/unknown paths

- ProviderResponse and TurnFinished agree because each terminal-error call site emits
  the ProviderResponse with class C and then returns an error that `classify_evidence_error`
  maps back to C: explicit `ClassifiedEvidenceError` for ContextLimit/ProviderOpen/
  StreamTransport, typed `StreamError` (`evidence.rs:286-288`) for the bare StreamEvent
  return at `turn_loops.rs:695`, and `TurnInterruptedError` for cancel paths.
- `status_for_result` (`evidence.rs:216-224`) maps `TurnInterruptedError` to
  `Interrupted` and everything else Err to `Error`, matching the recorded classes.
- `unknown_error` is the safe default for any unclassified/untyped error, so an
  unexpected raw error degrades to the closed `unknown_error` label rather than
  leaking text.

### Adversarial fixtures cross the real persistence seam

- `agent_tests.rs:931-947` `r12_raw_secret_prefix_transport_error_uses_closed_error_class`
  injects `"token=secret request=abc: transient transport failure"` (secret **before**
  the first colon) via a scripted TransportError, runs `run_once_capture`, reads back
  with `read_session_evidence`, and asserts `stream_transport_error`.
- `agent_tests.rs:949-967` `r12_no_colon_provider_open_secret_uses_closed_error_class`
  injects `"invalid API key sk-secret"` (no colon), runs `run_once_capture`, reads back,
  and asserts `provider_open_error`.
- The shared strict helper `assert_strict_terminal_error_evidence` (`agent_tests.rs:402-515`)
  validates the exact 4-event shape, contiguous sequence `[0,1,2,3]`, shared `turn_id`,
  request/response `provider_request_id` pairing, and calls `assert_stable_error_class`
  (`agent_tests.rs:392-400`) on **both** the ProviderResponse and TurnFinished classes.
  `assert_stable_error_class` asserts the exact expected label and that the persisted
  string does not contain `token=`, `request=`, or `sk-secret`, and is not the raw
  message. This proves the secret text crosses and is dropped at the durable seam.

## Reverified invariants

- Per-request/per-turn cardinality: `provider_terminal_counts` asserted `(1,1,1,[Error])`
  in the strict error helper (`agent_tests.rs:511-514`) and `(1,1,1,[Ok])` in the
  success fixture (`agent_tests.rs:833-836`). Retry helper asserts the 6-event
  two-attempt shape. One TurnFinished per turn; one ProviderResponse per request.
- Correlation / no duplicates / no orphans: fresh `provider_request_id` per attempt via
  `provider_evidence_correlation` (`evidence.rs:135-141`); request/response ids paired
  (`agent_tests.rs:490-493`); TurnStarted and TurnFinished carry no provider_request_id
  (`:445`, `:509`). No path reaches both the error helper and a success ProviderResponse
  for one request id (success sites are `Ok/None`; error sites return before success).
- Success-path behavior: `r12_no_tool_turn_emits_and_persists_exactly_one_terminal_provider_response`
  passes; success ProviderResponse carries `Ok`/`error_class: None`, plus malformed
  trailing-line replay tolerance (`agent_tests.rs:838-848`).
- Both engines: blocking `turn_loops.rs` and MPSC `turn_streaming_mpsc.rs` both covered
  by focused fixtures (blocking + mpsc transport, stream-event, cancel, context-limit).
- Exact focused r12 fixtures: 11 `r12_` tests, all passing (see commands).
- Commit boundaries: exactly three commits, cleanly separated source / test / docs:
  `f14ed5e12` fix (evidence.rs, turn_loops.rs, turn_streaming_mpsc.rs),
  `c23bae5e7` test (agent_tests.rs), `c77f5e246` docs (R12 ledger, +91 lines only).
- Append-only ledger/review preservation: remediation touches only the ledger (append)
  and no prior review files (`git diff 9afe..c77f --name-only -- .../reviews/` empty).
  The Fable and Opus prior reviews remain byte-preserved.
- R09 honesty: ledger records R09 gates as expected-red with pre-encoded expected exits
  and explicitly no `--update`; the only two `--update` string hits in the range are the
  ledger prose stating none was used. Working tree clean.
- Declared scope: remediation ledger states it addresses only the Fable blocker and does
  not widen scope to live providers, daemon/reload, network, credentials, tools/MCP,
  publication, or baseline updates. Confirmed against the diff.

## Commands, counts, exits

Run in `/Users/jrudnik/labs/jcode-w1-r12`.

1. Refs/range:
```
git log --oneline 9afe5bdb7d96b0bc30e29a17dec090f469ce75e4..c77f5e24628692eab89f5adf49081512ba4d429d
```
Exit 0. Three commits: `f14ed5e12` fix, `c23bae5e7` test, `c77f5e246` docs.

2. Range stat:
```
git diff 9afe..c77f --stat
```
Exit 0. 5 files, 297 insertions, 57 deletions: evidence.rs, turn_loops.rs,
turn_streaming_mpsc.rs, agent_tests.rs, R12 ledger.

3. Focused offline tests (via repo dev shell against existing worktree target):
```
JCODE_HOME=$(mktemp -d /tmp/jcode-r12-home.XXXXXX) JCODE_NO_TELEMETRY=1 \
  bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_ --nocapture
```
Exit 0 (PIPEEXIT=0). Result: `11 passed; 0 failed; 0 ignored; 0 measured; 1090 filtered
out; finished in 1.80s`. One unrelated pre-existing dead-code warning
(`drop_control_log_handle`). All 11 pass, including
`r12_raw_secret_prefix_transport_error_uses_closed_error_class` and
`r12_no_colon_provider_open_secret_uses_closed_error_class`.

4. Static: ProviderResponse constructor sites and error_class surfaces enumerated via
`rg`; success sites `Ok`/`None`; error site closed-enum only. Exit 0.

5. `git status --short` after runs: empty, exit 0.

Note: an initial attempt using `nix develop --offline . --command cargo test ...` stalled
inside Nix cache evaluation (~19 min, no cargo/rustc child, not ENOSPC) and was
terminated per coordination; the accepted test evidence is the `scripts/dev_cargo.sh`
run above against the existing worktree target.

## Findings

### CRITICAL
None.

### IMPORTANT
None. The prior Fable IMPORTANT-1 is closed.

### MINOR / nonblocking
- MINOR-1: The Fable 2x2 engine retry-matrix symmetry note remains a coverage follow-up
  (no matching MPSC open/context-limit retry fixture and no blocking mid-stream
  context-limit retry fixture among the new tests). Source paths for both engines exist
  and are class-consistent; this does not affect the blocker or any invariant. The
  remediation ledger explicitly leaves this visible as nonblocking.
- MINOR-2 (observation, out of scope): Pre-existing `ToolFinished.error_class` sites
  (`turn_loops.rs:1189`, `turn_streaming_mpsc.rs:1568`) still use `e.to_string().take(120)`
  raw text, and `turn_streaming_mpsc.rs:1614` uses a fixed `"interrupted_by_reload"`.
  These are `ToolFinished` events, present at the W1 base, outside the reviewed
  ProviderResponse/TurnFinished error seam and outside the declared W1 scope. Flagged
  for transparency, not a blocker.

## Scope limits / not checked
- Did not run the full `jcode-app-core` suite or the R09 matrix; relied on the ledger's
  recorded expected-red R09 results and ran only focused `r12_` tests offline.
- Did not execute live providers, daemon/reload, tools/MCP, credentials, network,
  publication, or `--update`.
- Did not build a full symmetric 2x2 engine retry fixture matrix.
- Did not audit non-R12 event kinds (e.g. `ToolFinished`) beyond the transparency note.
- Did not mutate any code or docs.

## Confidence
High for PASS. The closed-classifier property is directly provable from source
(`evidence.rs`) and the two adversarial fixtures crossing the real persistence seam.
Cardinality/correlation and commit/ledger discipline are high confidence from source and
the 11 passing focused fixtures.

## Report hash
SHA-256 of this finalized report is reported out-of-band after writing to avoid a
self-referential hash field.
