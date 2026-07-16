# W1 R12 error-class remediation: independent Fable-class rereview

| Field | Value |
|---|---|
| Verdict | **PASS** |
| Confidence | High for the declared remediation surface; medium for unexecuted engine cells noted below |
| Reviewer class | Fable (adversarial, read-only) |
| Date | 2026-07-16 (UTC) |
| Worktree | `/Users/jrudnik/labs/jcode-w1-r12`, branch `recovery/fix-r12-terminal-evidence-2026-07-15` |
| Prior review-disagreement HEAD | `9afe5bdb7d96b0bc30e29a17dec090f469ce75e4` (verified present) |
| Remediation HEAD | `c77f5e24628692eab89f5adf49081512ba4d429d` (verified `git rev-parse HEAD`) |
| Full base | `602709895be96a85a6090690c0b27d5681d17321` |
| Working tree | clean (`git status --porcelain` = 0 lines) |
| Mode | Offline only. No `--update`, no live provider, no network action, no daemon, no writes outside this report |

## Remediation slice under review

`git log 9afe5bdb7..c77f5e246`, exactly three commits, clean class separation:

- `f14ed5e12` `fix: close R12 error evidence classes` — source only: `agent/evidence.rs`, `agent/turn_loops.rs`, `agent/turn_streaming_mpsc.rs` (133 insertions / 42 deletions).
- `c23bae5e7` `test: cover R12 closed error classes` — test only: `agent_tests.rs` (73/15).
- `c77f5e246` `docs: record R12 error-class remediation` — docs only: R12 ledger (91 insertions, **0 deletions**).

## Adversarial checklist, confirmed point by point

### 1. Closed allowlist only, no raw-text fallback — CONFIRMED

The old `error_class` function (base `602709895` split raw provider text on `:` and truncated to 120 chars) is deleted. The only persisted values now come from `EvidenceErrorClass::as_str()` (`agent/evidence.rs:247-258`), a closed six-value set: `context_limit`, `provider_open_error`, `stream_transport_error`, `stream_error`, `turn_interrupted`, `unknown_error`. The fallback is the fixed literal `unknown_error`, never derived text. `classify_evidence_error` (`evidence.rs:278-291`) resolves only by typed downcast (`TurnInterruptedError`, `ClassifiedEvidenceError`, `StreamError`); it never reads `to_string()`.

Exhaustive writer census for the two remediated fields:
- `ProviderResponse.error_class`: written only in `append_provider_error_response` (`evidence.rs:160`) from the enum, or `None` at the two success sites (`turn_loops.rs:752`, `turn_streaming_mpsc.rs:1049`). Grep found no other writer.
- `TurnFinished.error_class`: written only at `evidence.rs:48` via `error_class_for_error`, which returns only allowlist strings. `finish_evidence_turn` is called only from the three wrappers (`turn_execution.rs:22,45,106`); no engine writes `TurnFinished` directly (zero grep matches, verified as a zero-match result).

### 2. Truthful categories at every ProviderResponse error path — CONFIRMED

All 15 `append_provider_error_response` call sites enumerated and checked against their control-flow context:

- Blocking (`turn_loops.rs`): open context-limit retry `:130-135`, final open error `:153-158` (ProviderOpen), raw transport context retry `:233-238`, final raw transport `:264-269` (StreamTransport), stream-event context retry `:645-650`, final stream-event `:687-692` (StreamEvent).
- MPSC (`turn_streaming_mpsc.rs`): cancel-before-open `:253-258` and mid-stream cancel `:405-410` (TurnInterrupted), open context retry `:268-273`, final open error `:302-307` (ProviderOpen), raw transport context retry `:458-463`, final raw transport `:500-505` (StreamTransport), stream-event context retry `:927-932`, final stream-event `:980-985` (StreamEvent).

Classification is by call-site truth (open vs mid-stream vs event-level vs cancel), with the context-limit branch checked first in every pair, so a context-limit message is never mislabeled as transport/open/stream. Success responses remain `error_class: None` with `Ok` status at the unchanged post-loop sites.

### 3. Explicit class survival through wrappers — CONFIRMED

Every error `return` on a classified branch wraps the error via `Self::classified_evidence_error(e, class)` (14 `return Err` sites checked in both engine files), so the wrapper-level `TurnFinished` derives the identical class as the correlated `ProviderResponse`. The two intentional unwrapped returns are consistent by construction: `StreamError` (classifies to `stream_error`, and stays downcastable for `retry_after_secs` at `client_lifecycle.rs:797,2900` and `client_actions.rs:1137`) and the interrupted sentinel (maps to `TurnFinished{Interrupted}` via `status_for_result` and class `turn_interrupted`).

### 4. Secret-safe persistence — CONFIRMED

On error paths `ProviderResponse.output` and `usage` are hard-coded `None` (`evidence.rs:155-161`) and `TurnFinished.output` is `None` from the wrappers on error. The raw provider message reaches only process logs (`logging::warn`) and the in-memory returned error, never a persisted evidence field. The now-unused `_error: &anyhow::Error` parameter cannot leak (it is not read).

### 5. Adversarial fixture realism — CONFIRMED

- `r12_raw_secret_prefix_transport_error_uses_closed_error_class` injects `token=secret request=abc: transient transport failure`. Under the pre-fix splitter this persisted exactly `token=secret request=abc`; the fixture asserts persisted class equals `stream_transport_error` and contains no `token=`/`request=` substrings, in both ProviderResponse and TurnFinished.
- `r12_no_colon_provider_open_secret_uses_closed_error_class` injects no-colon `invalid API key sk-secret` (realistic OpenAI-style `sk-` key shape). Pre-fix this whole string persisted; the fixture asserts `provider_open_error` and no `sk-secret` substring.
- `assert_stable_error_class` additionally requires exact equality with the expected closed label, so any raw-text regression fails immediately.

Both fixtures drive real engine paths (blocking raw transport via `ScriptedEvidenceProvider`; provider open error via `RetryEvidenceProvider::OpenError`), not synthetic classifier unit calls.

### 6. Cardinality/correlation invariants unchanged — CONFIRMED

The strict four-event helper still asserts exact `0..=3` sequences, shared `turn_id`, one `ProviderRequest` with a `provider_request_id` echoed on exactly one `ProviderResponse`, `TurnFinished` without a request id, and terminal counts `(1,1,1)`. The retry helper still asserts the six-event two-attempt shape with distinct request ids and `(2,2,1)`. The cancel helper still asserts `(1,1,1)` with `Interrupted` finish. Only the expected class strings changed (`turn interrupted` → `turn_interrupted`, raw fixture text → closed labels).

### 7. No duplicate or orphan evidence — CONFIRMED

Each retry `continue`/`break-and-loop` first emits the abandoned attempt's error response, then the loop top mints a fresh `provider_evidence_correlation()` (`turn_loops.rs:104`, `turn_streaming_mpsc.rs:223`) before the next `ProviderRequest`. Every classified error `return` follows exactly one emission for the current request. No path emits two terminal responses for one request id; fixtures enforce this dynamically.

### 8. Success behavior unchanged — CONFIRMED

Diff to the success emission sites is zero beyond context lines. `r12_no_tool_turn_emits_and_persists_exactly_one_terminal_provider_response` still passes with `ProviderResponse{Ok, usage}` and `TurnFinished{Ok}`, `error_class: None` on both.

### 9. Both engines — CONFIRMED

Blocking and MPSC each carry the full class wiring (call-site table in §2) and each has raw-transport and stream-event error fixtures plus the shared closed-class assertions.

### 10. Focused fixture suite green — CONFIRMED BY EXECUTION

```
JCODE_HOME=$(mktemp -d /tmp/jcode-r12-fable-rereview.XXXXXX) JCODE_NO_TELEMETRY=1 \
  bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_ --nocapture
```
Exit `0`; `11 passed; 0 failed; 1090 filtered out; finished in 3.65s`. All eleven `r12_` fixtures listed by name in the output, including both new adversarial ones. One pre-existing unrelated `drop_control_log_handle` dead-code warning, matching the ledger's honest note. Used the existing worktree target; no custom `CARGO_TARGET_DIR`; build completed with 208 GiB free (prior ENOSPC not reproduced).

### 11. Clean commit classes — CONFIRMED (§ slice table above; fix/test/docs strictly separated by file class)

### 12. Append-only honest ledger — CONFIRMED

`git diff 9afe5bdb7..c77f5e246 -- docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md`: 91 insertions, **0 deletions** (the single `-` line is the diff header). The amendment preserves the prior FAIL verdict, records the failed bare `cargo` exit-127 attempt before claiming Nix-based validation, keeps review hashes untouched, and explicitly demands fresh independent rereview rather than self-approving. Ledger validation claims cross-checked by rerun (below) and all matched.

### 13. No R06A/R13 ownership crossing — CONFIRMED

`git diff 602709895..c77f5e246 -- crates/jcode-session-types crates/jcode-base` is empty (0 lines): no evidence schema or storage change. `agent/compaction.rs` untouched; the only compaction interaction remains the pre-existing `try_auto_compact_after_context_limit` predicate call. Touched files are exactly the three declared agent files, the test file, two preserved review artifacts, and the ledger.

## Validation commands, counts, exits

| Command | Exit | Result |
|---|---:|---|
| `bash scripts/dev_cargo.sh test -p jcode-app-core --lib -- r12_ --nocapture` (fresh `JCODE_HOME`, `JCODE_NO_TELEMETRY=1`) | 0 | 11 passed / 0 failed / 1090 filtered |
| `nix develop --offline . --command rustfmt --edition 2024 --check` on the four touched Rust files | 0 | clean |
| `bash scripts/dev_cargo.sh fmt -- --check` | 0 | clean workspace fmt |
| `python3 -m unittest discover -s tests -p 'test_rust_production_filter.py'` | 0 | 17 tests OK |
| `python3 scripts/check_panic_budget.py` | 1 (expected red) | 31 → 46, no baseline update |
| `python3 scripts/check_swallowed_error_budget.py` | 1 (expected red) | 2987 → 3074, no baseline update |
| `python3 scripts/check_code_size_budget.py` | 1 (expected red) | includes `turn_loops.rs` 1251→1314, `turn_streaming_mpsc.rs` 1774→1840 |
| `python3 scripts/check_test_size_budget.py` | 1 (expected red) | includes `agent_tests.rs` 1321→2309 |
| `python3 scripts/check_wildcard_reexport_budget.py` | 0 | total 16 |
| `bash scripts/check_warning_budget.sh` | 0 | current 0, baseline 0 |
| `bash -n scripts/*.sh` | 0 | clean |
| `git diff --check` | 0 | clean |
| `git status --porcelain` | 0 lines | clean tree at `c77f5e246` |

Actual file line counts confirmed independently: `turn_loops.rs` 1314, `turn_streaming_mpsc.rs` 1840, `agent_tests.rs` 2309 — matching the ledger exactly. No `--update` was run anywhere.

## Findings

### CRITICAL

None.

### IMPORTANT

None within the declared remediation surface. The single prior IMPORTANT blocker (raw secret-prefix/no-colon provider text persisted as `error_class`) is fixed at the source, closed by type, and pinned by adversarial fixtures.

### MINOR

1. **`ToolFinished.error_class` still persists raw truncated error text** (`turn_loops.rs:1189`, `turn_streaming_mpsc.rs:1568`, `Some(e.to_string().chars().take(120).collect())`). Pre-existing at full base `602709895` (same lines 1142/1503 there), unchanged by this slice, and outside the ledger's carefully-scoped claim ("ProviderResponse.error_class and TurnFinished.error_class now use the same closed stable classifier"). It is the same risk class as the fixed blocker but for tool errors rather than provider text. Recommend a follow-up slice; not a defect of this remediation and not grounds to fail it.
2. **`ClassifiedEvidenceError` does not implement `source()`**, so the anyhow cause chain is severed at the wrapper. Consequences: (a) `format_error_chain` (used for `ServerEvent::Error`) now shows only the top message for wrapped provider-open/transport errors, dropping deeper causes from user-facing errors; (b) a typed inner error would be invisible to downstream `downcast_ref`. I verified no current consumer breaks: `StreamError` is constructed only inside the engines and is returned *unwrapped* on the stream-event paths where `retry_after_secs` is consumed, and the `TurnInterruptedError` sentinel is likewise never wrapped. Adding `source() -> Some(inner)` would be safe for the classifier (the per-cause loop matches `ClassifiedEvidenceError` before reaching inner causes). Cosmetic/robustness follow-up only.
3. **2x2 retry-matrix asymmetry (reassessed as instructed, not elevated).** Open-context-limit retry is fixture-tested on the blocking engine only; mid-stream-context-limit retry on MPSC only. I statically verified the two untested cells (MPSC open retry `turn_streaming_mpsc.rs:266-296`; blocking mid-stream retry `turn_loops.rs:233-251`) use the identical helper, class constants, and re-emission structure as their tested twins, and found no concrete correctness issue. Remains a nonblocking coverage note, exactly as the ledger records.
4. **`_error` parameter of `append_provider_error_response` is now dead.** Harmless (cannot leak, never read); could be dropped in the deferred refactor slice.

## Scope limits and what was not checked

- **MPSC TextDelta-processing shutdown checkpoint** (`turn_streaming_mpsc.rs:598-608`): a graceful shutdown observed *while processing an already-received text event* checkpoints the partial response and proceeds to `ProviderResponse{Ok}`/`TurnFinished{Ok}`. This reload-handoff path is pre-existing, is explicitly excluded by the W1/remediation boundary ("does not qualify daemon/reload behavior"), persists no raw error text, and was not exercised by fixtures. Noted so it is not silently assumed covered by the cancel fixtures (which cover the select-arm await paths only).
- Live providers, daemon/reload, tools/MCP, credentials, network behavior, tool-continuation multi-call turns, and generic compaction beyond the deterministic context-limit fixtures were not executed (per mandate and ledger boundary).
- I did not byte-verify the persisted JSONL for absence of secrets outside the asserted fields at runtime; that claim rests on static analysis of the writer (error paths hard-code `output: None`, `usage: None`) plus the fixture assertions on `error_class`. Confidence high.
- The full `jcode-app-core --lib` suite (1101 tests) was not rerun end to end; the prior amendment records two unrelated concurrency-flaky failures there. Only the focused `r12_` suite plus the static/gate matrix above were executed.
- Commits are unsigned (`%G?` = N); no signing requirement is recorded for this repo, so not treated as a finding.

## Verdict

**PASS.** The remediation is real, minimal, and honest: persisted `error_class` values for `ProviderResponse` and `TurnFinished` are now a closed six-label allowlist resolved purely by typed classification, with no raw-text derivation or fallback anywhere on the R12 provider/turn surface; the adversarial secret-prefix and no-colon fixtures reproduce the exact pre-fix leak shapes and pass; cardinality, correlation, success behavior, both engines, commit hygiene, append-only ledger truth, and seam ownership are all intact. The prior Fable IMPORTANT blocker is closed. Remaining items are pre-existing or cosmetic follow-ups listed above, none blocking.

Confidence: **high** on the remediation surface (source read of every writer/return site plus 11/11 executed fixtures plus matching gate matrix); **medium** only for the two statically-verified but unexecuted retry cells and the excluded reload-checkpoint path.
