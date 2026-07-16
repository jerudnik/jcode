# Fable sign-off: R12 authoritative ledger

## Verdict

**FAIL for the committed R12 authoritative ledger at commit `99e153edf131f42668a0e51361904053108a8357`.**

This is **not** a pilot-entry PASS. The ledger is directionally safe on pilot blocking, but it fails Fable sign-off because one named lifecycle row overclaims exact request/response cardinality: blocking `StreamEvent::Error` appears to emit no `ProviderResponse`. The ledger correctly says:

- `retain-fork` for the R12 evidence implementation.
- Unqualified R12 pilot is **blocked today**.
- Only a narrow future strict fixture may be admitted after it proves one no-tool, no-cancel, no-compaction emit-to-persist-to-replay path.
- Global production cardinality remains false for raw transport errors, retry/compaction attempts, and cancellation semantics.

I did not read any future Sol sign-off. I read the committed R12 ledger, the two preserved R12 reviews, the binding R00/R01/R02/R06A/R07C/R09/R11/R13 ledgers, and relevant source/tests at the checked-out commit. I performed only read-only evidence reproduction and wrote this file under `/tmp`.

## Exact commit

- Repository: `/Users/jrudnik/labs/jcode-seam-r12`
- Commit checked: `99e153edf131f42668a0e51361904053108a8357`
- Branch observed: `recovery/seam-r12-20260715`
- `git status --short --branch`: clean except branch line only, observed as `## recovery/seam-r12-20260715`

## Severity-ranked findings

### No CRITICAL findings

No unsafe pilot-entry approval was found. The ledger remains conservative at the top level: it records fork authority for retained evidence code while explicitly blocking current pilot entry.

### IMPORTANT-1: exact lifecycle cardinality table overclaims blocking `StreamEvent::Error`

The ledger table at `docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md:87` says `StreamEvent::Error`, no compaction retry, has one `ProviderResponse{Error}` and passes cardinality. My reproduced source evidence supports that for MPSC but not for the blocking engine. In blocking `turn_loops.rs`, the `StreamEvent::Error` non-compaction branch logs and directly returns `Err(StreamError...)` at `crates/jcode-app-core/src/agent/turn_loops.rs:621-637`; no `append_session_evidence_with_correlation(ProviderResponse { ... })` is present in that branch.

Impact: this is a material overclaim in the exact lifecycle matrix and evidence coverage. It does not make the pilot unsafe because the ledger still blocks current pilot entry and the strict no-tool fixture excludes this path, but it prevents full sign-off of the authoritative ledger as written.

### Checked areas

1. Exact request/response cardinality on named lifecycle cases.
2. Fork/upstream authority.
3. Pilot blocked today versus narrow post-fixture entry.
4. Cross-seam preconditions.
5. Fixture sufficiency and missing fixture gap.
6. Negative findings.
7. R09 debt posture.
8. Rollback/stop policy.

### MINOR / residual evidentiary limits

1. **Tests were not rerun by me.** The ledger records several passing narrow tests at lines 267-273. I reproduced decisive static evidence and fixture absence, but did not rerun app-core or session-type tests because the task requested only decisive read-only reproduction and forbade broadening. This does not affect the verdict because the ledger’s key conclusion is blocked pilot entry due to missing end-to-end fixture and static terminal-path defects.
2. **The future fixture remains unimplemented.** The ledger is correct to require it. My sign-off does not certify the future fixture, only the committed ledger’s adjudication and evidence coverage.
3. **R02 remains independently pilot-blocking.** The R12 ledger accurately records R02 as a cross-seam blocker. I did not re-adjudicate R02 beyond reading its binding ledger and verifying the R12 echo sites.

## Evidence coverage by challenged area

### 1. Review preservation and integrity

The committed ledger records preserved Opus and Grok review hashes at `docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md:23-26`.

Reproduced evidence:

- `shasum -a 256 docs/fork/recovery/seams/R12-agent-turn-evidence/opus-review.md docs/fork/recovery/seams/R12-agent-turn-evidence/grok-review.md`
  - Opus: `d3c19a9576f21e008b831594c13f09189527a98a20050d044e8d7e908e462a60`
  - Grok: `c12b96cbd935010405a05cd57a6caba7c56a5a0aca904c302ccc2cf6f52555d8`
- `wc -l` returned 165 lines for Opus and 208 lines for Grok, matching ledger line 67.
- `/tmp/jcode-r12-opus-review.md` and `/tmp/jcode-r12-grok-review.md` existed and `cmp -s` matched the committed copies.

Result: supported.

### 2. Fork/upstream authority

Ledger claim: fork is authority for retained R12 implementation because upstream lacks the evidence spine, but fork is not authority for a broad pilot claim. See ledger lines 53-60 and 197-210.

Reproduced evidence:

- `git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b` returned `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`.
- `git merge-base --is-ancestor 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 HEAD` succeeded.
- `git cat-file -e 802f6909825809e882d9c2d575b7e478dce57d3b:crates/jcode-app-core/src/agent/evidence.rs` failed because the path does not exist upstream.
- Upstream `ProviderRequest` count in the two turn engines was `0`.
- `git diff --numstat 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 -- ...` returned fork additions including:
  - `206 0 crates/jcode-app-core/src/agent/evidence.rs`
  - `54 9 crates/jcode-app-core/src/agent/turn_execution.rs`
  - `88 0 crates/jcode-app-core/src/agent/turn_loops.rs`
  - `126 6 crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs`
  - `342 0 crates/jcode-base/src/session/evidence.rs`
  - `340 0 crates/jcode-session-types/src/evidence.rs`

Result: supported. There is no upstream evidence implementation to adopt.

### 3. Turn bracketing and correlation

Ledger claim: each entrypoint emits one `TurnStarted` and one `TurnFinished`, while provider calls get per-request `provider_request_id` correlation. See ledger lines 41-50 and 75-90.

Source evidence:

- `crates/jcode-app-core/src/agent/evidence.rs:8-30`: `start_evidence_turn` creates a turn id and appends `TurnStarted`.
- `crates/jcode-app-core/src/agent/evidence.rs:32-53`: `finish_evidence_turn` appends `TurnFinished`, records checkpoint, then clears current turn id.
- `crates/jcode-app-core/src/agent/evidence.rs:134-140`: `provider_evidence_correlation` stamps the current turn id plus a fresh `provider_request_id`.
- `crates/jcode-app-core/src/agent/turn_execution.rs:15-22`: `run_once` starts and finishes evidence around `run_turn`.
- `crates/jcode-app-core/src/agent/turn_execution.rs:38-45`: `run_once_capture` starts and finishes evidence around `run_turn`.
- `crates/jcode-app-core/src/agent/turn_execution.rs:100-106`: `run_once_streaming_mpsc` starts and finishes evidence around `run_turn_streaming_mpsc`.

Result: supported.

### 4. Exact lifecycle cardinality matrix

The ledger’s matrix at lines 82-90 is mostly supported, but one named row fails for the blocking engine.

#### No-tool happy path, blocking or MPSC

- Blocking request: `crates/jcode-app-core/src/agent/turn_loops.rs:103-114` emits `ProviderRequest` before `complete_split`.
- Blocking success response: `turn_loops.rs:677-697` emits one `ProviderResponse { status: Ok, usage, output }` with the cloned correlation.
- MPSC request: `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:222-233` emits `ProviderRequest`.
- MPSC success response: `turn_streaming_mpsc.rs:968-988` emits one `ProviderResponse { status: Ok, usage, output }`.
- Wrappers emit one `TurnFinished`: `turn_execution.rs:22,45,106`.

Adjudication: supported for the strict no-tool fixture path, but only after an actual end-to-end fixture exists.

#### Provider open error, no compaction

- Blocking non-compaction open error: `turn_loops.rs:127-153` emits `ProviderResponse { status: Error }` then returns `Err`.
- MPSC non-compaction open error: `turn_streaming_mpsc.rs:253-293` emits `ProviderResponse { status: Error }` then returns `Err`.
- Wrappers finish with `TurnFinished { Error }` via `status_for_result`.

Adjudication: supported.

#### Raw stream transport `Err(e)` after open

- Blocking raw stream error: `turn_loops.rs:201-242` logs and directly `return Err(e)` without any `ProviderResponse` on the non-compaction branch.
- MPSC raw stream error: `turn_streaming_mpsc.rs:410-460` logs and directly `return Err(e)` without any `ProviderResponse` on the non-compaction branch.

Adjudication: supported. The ledger correctly marks this as a global invariant violation.

#### `StreamEvent::Error`, no compaction retry

- Blocking `StreamEvent::Error`: `turn_loops.rs:621-637` directly returns `Err(StreamError...)` without emitting a `ProviderResponse`.
- MPSC `StreamEvent::Error`: `turn_streaming_mpsc.rs:896-924` emits `ProviderResponse { status: Error }` before returning.

This is the sign-off blocker. The ledger matrix line 87 says this row passes cardinality, but the blocking branch returns without a visible `ProviderResponse`. The MPSC side passes. The authoritative ledger should amend this row to `blocking: 0 ProviderResponse / 1 TurnFinished{Error}; MPSC: 1 ProviderResponse{Error} / 1 TurnFinished{Error}`, or otherwise cite a response emission site I did not find.

Severity: **IMPORTANT**, because exact request/response cardinality on every named lifecycle case was a central sign-off criterion.

#### Cancel before stream open, MPSC

- `turn_streaming_mpsc.rs:222-233`: emits `ProviderRequest`.
- `turn_streaming_mpsc.rs:243-252`: graceful shutdown before stream open returns `Ok(())` with no `ProviderResponse`.
- `turn_execution.rs:105-106` then finishes the turn with `Ok` status.

Adjudication: supported. The ledger correctly blocks cancellation-inclusive pilot variants.

#### Mid-stream cancel, MPSC

- `turn_streaming_mpsc.rs:365-385`: graceful shutdown while waiting for stream event logs cancellation and `break`s.
- `turn_streaming_mpsc.rs:968-988`: control falls through to `ProviderResponse { status: Ok }`.
- `agent/evidence.rs:181-186`: `status_for_result` maps only `Ok` and `Error`, never `Cancelled` or `Interrupted`, despite schema support at `crates/jcode-session-types/src/evidence.rs:171-175`.

Adjudication: supported.

#### Open or mid-stream context-limit retry/compaction

- Blocking open-time retry: `turn_loops.rs:127-140` continues after the request has already been emitted.
- Blocking mid-stream retry: `turn_loops.rs:201-233` sets retry and breaks, then `turn_loops.rs:642-650` continues.
- MPSC open-time retry: `turn_streaming_mpsc.rs:253-280` continues after request.
- MPSC mid-stream retry: `turn_streaming_mpsc.rs:414-451` and `turn_streaming_mpsc.rs:857-895` set retry, then `turn_streaming_mpsc.rs:929-937` continues.

Adjudication: supported. These abandoned attempts orphan provider requests.

### 5. R02 route handoff and non-secret request identity

Ledger claim: R02 owns provider/model/route selection and R12 echoes safe identity without minting account, entitlement, or credential authority. See ledger lines 101-115.

Source evidence:

- `crates/jcode-app-core/src/agent/provider.rs:71-86`: `set_route_selection` writes `session.provider_key`, `session.route_api_method`, and `session.model` and persists the session.
- `turn_loops.rs:103-114`: `ProviderRequest` records `provider`, `model`, `route`, `message_count`, `tool_count`, and prompt summary.
- `turn_streaming_mpsc.rs:222-233`: same for MPSC.
- `crates/jcode-base/src/session/evidence.rs:146-163`: payload summaries contain SHA-256 and byte length, not raw payload bytes.
- R02 ledger lines 24-29 bind R12 to exact selected route identity and forbid credential emission as an observable.

Result: supported.

### 6. R06A storage, replay, truncation, and fixture sufficiency

Ledger claim: R06A persists and replays what R12 emits, but does not repair missing emissions. The future fixture must prove exact four-event emit-to-persist-to-replay and malformed-line truncation. See R12 ledger lines 117-129 and 177-184.

Source evidence:

- `crates/jcode-base/src/session/evidence.rs:87-103`: writer appends events with schema version, sequence, correlation, and kind.
- `crates/jcode-base/src/session/evidence.rs:115-144`: reader reads JSONL, stops at first unparsable line, sorts surviving events, and does not synthesize terminal events.
- `crates/jcode-base/src/session/evidence.rs:303-328`: test covers empty/malformed trailing-row behavior and preserves only valid existing events.
- R06A ledger lines 12 and 25-29 explicitly assign emission ordering/correlation to R12 and record that no fabricated completion path was found.

Result: supported. The missing fixture remains a true pilot blocker.

### 7. Missing end-to-end R12 fixture

Ledger claim: no current test drives a no-tool R12 engine and reads back exact event cardinality. See R12 ledger lines 72-73, 117-129, 217-223, and 249-250.

Reproduced evidence:

- A file-level scan for files containing both `run_once`/`run_turn` and `read_session_evidence`/`read_session_evidence_from_path` returned no files.
- Existing evidence references are storage/search/schema or the helper test `finish_evidence_turn_populates_assistant_checkpoint`, not a full turn-to-evidence-readback fixture.

Result: supported. This is the decisive reason the pilot is blocked today.

### 8. R13 compaction avoidance and provider-session writer census

Ledger claim: one small no-tool fixture cannot trigger compaction, and the strict pilot should avoid the TUI-local provider-session stale window. See R12 ledger lines 131-146 and R13 ledger lines 18-23, 24-49.

Source evidence:

- `crates/jcode-compaction-core/src/lib.rs:6-19`: token budget is 200,000; threshold is 0.80; recent turns to keep is 10.
- `crates/jcode-base/src/compaction.rs:857-873`: compaction requires `active.len() > RECENT_TURNS_TO_KEEP` in the reactive/proactive/semantic paths.
- `crates/jcode-app-core/src/agent_tests.rs:426-476`: manual native auto compaction test asserts both `agent.provider_session_id` and `session.provider_session_id` become `None` after compaction.
- `turn_loops.rs:499-504` and `turn_streaming_mpsc.rs:789-792`: app-core `SessionId` writes update both copies.
- `crates/jcode-tui/src/tui/app/turn.rs:723-724`: TUI local path writes only `app.provider_session_id` immediately.
- `crates/jcode-tui/src/tui/app/conversation_state.rs:621-627`: later quit handling syncs the session copy.

Result: supported. The strict fixture must use app-core blocking/capture or explicitly assert both copies if it includes `SessionId`.

### 9. R07C telemetry precondition

Ledger claim: a future fixture must run with fresh `JCODE_HOME` and `JCODE_NO_TELEMETRY=1`, preventing telemetry/content sharing. See R12 ledger lines 167-173 and R07C ledger lines 32-44.

Source evidence:

- `crates/jcode-telemetry-core/src/lib.rs:284-296`: `is_enabled()` returns false if `JCODE_NO_TELEMETRY`, `DO_NOT_TRACK`, or `$JCODE_HOME/no_telemetry` is set.
- `crates/jcode-telemetry-core/src/lib.rs:309-321`: content sharing is false when telemetry is disabled and otherwise requires a marker.
- `crates/jcode-storage/src/lib.rs:75-82`: `JCODE_HOME` controls the jcode storage root.

Result: supported.

### 10. R09 debt and no `--update` posture

Ledger claim: R12 source slices must keep red debt visible, avoid `--update`, and enumerate owned debt before implementation. See R12 ledger lines 226-237 and R09 ledger lines 24-30.

Read evidence:

- R09 ledger line 20 records visible red debt: production size 60 violations/+6,604 LOC, test size 31/+3,679, panic 31 -> 46, swallowed 2,987 -> 3,077.
- R09 ledger lines 24-30 require no blanket baseline update, behavior-owned debt attribution, and trusted gate semantics.
- R12 ledger lines 230-237 repeats the visible debt numbers and says R12 has not enumerated the per-file subset yet.

Result: supported. No implementation slice should proceed before R12-owned debt is enumerated.

### 11. Rollback and stop policy

Ledger claim: immediate stop/revert conditions are explicit. See R12 ledger lines 186-195 and bounded implementation slices at 215-224.

Read evidence:

- Stop conditions include missing/duplicate terminal response, correlation/sequence mismatch, wrong provider/model/route/count, `Ok` after cancellation, compaction/retry in the one-turn fixture, telemetry enabled, evidence outside temp home, live credential/network/daemon dependency, hidden R09 debt, writes outside review-record paths, and broader scenarios until dedicated fixtures pass.
- R00 ledger lines 26-31 require fixed refs, no stash/replay, no destructive ref/worktree changes, and explicit rollback/stop conditions.

Result: supported.

## Commands run

Representative read-only commands used:

```bash
cd /Users/jrudnik/labs/jcode-seam-r12
git rev-parse HEAD
git rev-parse 99e153edf
git status --short --branch

git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b
git merge-base --is-ancestor 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 HEAD
git cat-file -e 802f6909825809e882d9c2d575b7e478dce57d3b:crates/jcode-app-core/src/agent/evidence.rs
(git show 802f6909825809e882d9c2d575b7e478dce57d3b:crates/jcode-app-core/src/agent/turn_loops.rs; \
 git show 802f6909825809e882d9c2d575b7e478dce57d3b:crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs) | grep -c ProviderRequest

git diff --numstat 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 -- \
  crates/jcode-app-core/src/agent/turn_loops.rs \
  crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs \
  crates/jcode-app-core/src/agent/turn_execution.rs \
  crates/jcode-app-core/src/agent/evidence.rs \
  crates/jcode-session-types/src/evidence.rs \
  crates/jcode-base/src/session/evidence.rs

shasum -a 256 docs/fork/recovery/seams/R12-agent-turn-evidence/opus-review.md \
  docs/fork/recovery/seams/R12-agent-turn-evidence/grok-review.md
cmp -s /tmp/jcode-r12-opus-review.md docs/fork/recovery/seams/R12-agent-turn-evidence/opus-review.md
cmp -s /tmp/jcode-r12-grok-review.md docs/fork/recovery/seams/R12-agent-turn-evidence/grok-review.md

nl -ba crates/jcode-app-core/src/agent/evidence.rs | sed -n '1,60p;130,190p'
nl -ba crates/jcode-app-core/src/agent/turn_execution.rs | sed -n '1,115p'
nl -ba crates/jcode-app-core/src/agent/turn_loops.rs | sed -n '100,155p;195,245p;642,700p'
nl -ba crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs | sed -n '220,295p;360,390p;405,462p;857,940p;965,990p'
python3 - <<'PY'
from pathlib import Path
for p in list(Path('crates/jcode-app-core/src').rglob('*.rs'))+list(Path('crates/jcode-base/src').rglob('*.rs')):
    s=p.read_text(errors='ignore')
    if ('run_once' in s or 'run_turn' in s) and ('read_session_evidence' in s or 'read_session_evidence_from_path' in s):
        print(p)
PY
```

No command intentionally mutated source, refs, stash, worktree, daemon state, credentials, network state, or publication state.

## Residual gaps

- I did not run a live daemon, network provider, credentialed provider, UI/TUI turn, cancellation harness, compaction harness, or publication path.
- I did not build or add the missing strict fixture.
- I did not rerun the full test matrix or R09 gates.
- I did not read any future Sol sign-off.
- The ledger’s `StreamEvent::Error` row must be amended or re-evidenced for blocking-engine parity because the reproduced blocking snippet returns `Err` without a visible provider response, while MPSC emits one. This is the reason for FAIL.

## Final sign-off

**Fable sign-off: FAIL for the R12 authoritative ledger at `99e153edf131f42668a0e51361904053108a8357`.**

Reason: the ledger is directionally safe and correctly blocks current pilot entry, but its exact lifecycle cardinality table overclaims the blocking `StreamEvent::Error` case. Amend that row, or provide a missing blocking `ProviderResponse{Error}` emission site, before treating the ledger as Fable-signed. All other challenged areas I checked were supported by the read-only evidence above.
