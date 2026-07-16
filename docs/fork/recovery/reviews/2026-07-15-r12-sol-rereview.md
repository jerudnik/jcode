# Sol bounded re-review: R12 corrective HEAD

- **Verdict:** PASS.
- **Exact HEAD reviewed:** `1db425ef6747611a1902836e8417e5f0f7440b48`
- **Parent:** `99e153edf131f42668a0e51361904053108a8357`
- **Subject:** `docs: Correct R12 stream error matrix.`
- **Scope:** bounded delta review only. I reviewed the corrected blocking vs MPSC `StreamEvent::Error` matrix, evidence table, fix slice, negative findings, unchanged pilot verdict, and review hashes. I accounted for the recorded Fable issue only as the reason for this correction. I did not broaden into a new full R12 sign-off.
- **Mutation:** none to source, refs, stashes, or worktree files. Only this `/tmp` report was written. Final `git status --short` was clean.

## Findings

### Pass findings

1. **HEAD is the expected non-rewriting corrective commit.**  
   `git rev-parse HEAD` returned `1db425ef6747611a1902836e8417e5f0f7440b48`. `git merge-base --is-ancestor 99e153edf HEAD` succeeded. `git show --no-patch` reports parent `99e153edf131f42668a0e51361904053108a8357`, so the prior authoritative commit was not rewritten.

2. **Commit scope is bounded to the R12 ledger.**  
   `git diff --name-status 99e153edf..HEAD` shows only `M docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md`. `git diff --stat` reports one file changed, 36 insertions, 16 deletions. This matches a documentation-only corrective amendment.

3. **Review hashes and preservation remain unchanged.**  
   The repository copies still hash to Opus `d3c19a9576f21e008b831594c13f09189527a98a20050d044e8d7e908e462a60` and Grok `c12b96cbd935010405a05cd57a6caba7c56a5a0aca904c302ccc2cf6f52555d8`. `cmp -s` against `/tmp/jcode-r12-opus-review.md` and `/tmp/jcode-r12-grok-review.md` reported both identical. Ledger lines 23-30 still record the same hashes and say neither review is altered.

4. **The Fable-identified overclaim is corrected.**  
   Ledger lines 32-40 now record the corrective amendment: the prior shared `StreamEvent::Error` row overclaimed blocking behavior, and Terra reproduced that blocking logs and directly returns `Err` at `turn_loops.rs:586-637` without a correlated `ProviderResponse`, while MPSC emits at `turn_streaming_mpsc.rs:912-924`.

5. **Evidence table now matches source.**  
   Ledger line 81 now says raw transport `Err(e)` under-emits in both engines, blocking non-compaction `StreamEvent::Error` also under-emits, and MPSC `StreamEvent::Error` emits its response. Source confirms this: blocking `turn_loops.rs:586-637` logs `stream_event_error` then returns `Err(StreamError...)` with no `append_session_evidence_with_correlation`; MPSC `turn_streaming_mpsc.rs:912-924` appends `SessionLogEventKind::ProviderResponse { status: Error, ... }` before returning.

6. **Terminal-cardinality matrix is now exact for the requested row.**  
   Ledger lines 97-99 split the old row into:
   - raw stream transport `Err(e)`: request 1, response 0 in both engines;
   - blocking `StreamEvent::Error`: request 1, response 0, finish error;
   - MPSC `StreamEvent::Error`: request 1, response 1 error, finish error.
   This exactly matches the source lines above.

7. **Fix slice was updated to cover the corrected blocking event-error defect.**  
   Ledger line 234 now requires slice 2 to add an error response for raw transport `Err(e)` in both engines and for blocking non-compaction `StreamEvent::Error`, with blocking and MPSC negative fixtures. The acceptance condition requires one request, one error response, and one error finish for raw transport and `StreamEvent::Error` in both engines. This is the correct future implementation boundary.

8. **Negative findings and remaining risks now carry the correction.**  
   Ledger lines 252-267 add that blocking non-compaction `StreamEvent::Error` is an additional under-emission path and explicitly distinguish MPSC because it emits at `turn_streaming_mpsc.rs:912-924`. Lines 269-281 list raw transport errors, blocking `StreamEvent::Error`, cancellation, and compaction/retry defects as high-confidence but untested deterministic gaps. Lines 292-299 update validation/remaining risks to include blocking versus MPSC `StreamEvent::Error` and keep the global invariant false.

9. **Pilot verdict remains unchanged and correctly blocked.**  
   Ledger line 12 still says `Pilot entry verdict` is `blocked today`. Lines 103-108 state the strict fixture cannot prove raw errors, blocking `StreamEvent::Error`, cancellation, or retry/compaction, and the unqualified pilot remains blocked. Lines 304-305 retain `R12 pilot is blocked today` with only narrow post-fixture entry.

### Remaining findings

- No new source or test fix exists at HEAD, by design. The rereview passes the ledger correction only. The underlying production defects remain: raw transport under-emission in both engines, blocking `StreamEvent::Error` under-emission, MPSC cancellation success fabrication, retry/compaction abandoned-request under-emission, and absence of a full emit→persist→replay R12 fixture.
- Existing component tests and previous narrow test evidence are unchanged by this docs-only commit. I did not rerun the full four-test set because this request was a bounded ledger delta review and the corrective commit changes no test or source file.
- R02 remains independently pilot-blocked on stale-tier/product-fixture conditions, as preserved in ledger lines 296-299.

## Commands reproduced

```bash
cd /Users/jrudnik/labs/jcode-seam-r12

git rev-parse HEAD
# 1db425ef6747611a1902836e8417e5f0f7440b48

git log -1 --format=%s
# docs: Correct R12 stream error matrix.

git merge-base --is-ancestor 99e153edf HEAD
# exit 0

git status --short
# clean

git diff --name-status 99e153edf..HEAD
# M docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md

git diff --stat 99e153edf..HEAD
# 1 file changed, 36 insertions(+), 16 deletions(-)

git diff --unified=80 99e153edf..HEAD -- \
  docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md
# reviewed amendment, evidence table, matrix, fix slice, negative findings, validation tail

sha256sum \
  docs/fork/recovery/seams/R12-agent-turn-evidence/opus-review.md \
  docs/fork/recovery/seams/R12-agent-turn-evidence/grok-review.md
# d3c19a9576f21e008b831594c13f09189527a98a20050d044e8d7e908e462a60  opus-review.md
# c12b96cbd935010405a05cd57a6caba7c56a5a0aca904c302ccc2cf6f52555d8  grok-review.md

cmp -s /tmp/jcode-r12-opus-review.md \
  docs/fork/recovery/seams/R12-agent-turn-evidence/opus-review.md
cmp -s /tmp/jcode-r12-grok-review.md \
  docs/fork/recovery/seams/R12-agent-turn-evidence/grok-review.md
# both exit 0

git show 16921ace18cf5c25368a376357b7636478d3928f:crates/jcode-app-core/src/agent/turn_loops.rs \
  | nl -ba | sed -n '580,640p'
# blocking StreamEvent::Error logs and returns Err without ProviderResponse

git show 16921ace18cf5c25368a376357b7636478d3928f:crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs \
  | nl -ba | sed -n '905,928p'
# MPSC StreamEvent::Error appends ProviderResponse{Error} before returning

git diff --check 99e153edf..HEAD -- \
  docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md
# clean

git show --no-patch --format='commit %H%nparents %P%nsubject %s' HEAD
# commit 1db425ef6747611a1902836e8417e5f0f7440b48
# parents 99e153edf131f42668a0e51361904053108a8357
# subject docs: Correct R12 stream error matrix.
```

## Final rereview sign-off

Sol re-signs **PASS** for HEAD `1db425ef6747611a1902836e8417e5f0f7440b48` as a bounded, documentation-only correction. The ledger now accurately distinguishes blocking and MPSC `StreamEvent::Error` behavior, updates the evidence table, terminal-cardinality matrix, fix slice, negative findings, validation tail, and remaining risks, preserves Opus/Grok review hashes unchanged, does not rewrite `99e153edf`, and keeps the current R12 pilot **blocked today**.
