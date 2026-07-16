# Bounded Fable re-review: R12 Terra follow-up

## Verdict

**PASS** for the bounded re-review of HEAD `1db425ef6747611a1902836e8417e5f0f7440b48`.

The prior Fable IMPORTANT finding is resolved in the ledger as amended. The R12 ledger now correctly distinguishes:

- Blocking `StreamEvent::Error`, no compaction retry: **1 request, 0 ProviderResponse, 1 TurnFinished{Error}**.
- MPSC `StreamEvent::Error`, no compaction retry: **1 request, 1 ProviderResponse{Error}, 1 TurnFinished{Error}**.

No new CRITICAL or IMPORTANT findings were found within the bounded re-review scope.

## Scope and constraints

Bounded scope requested by chipmunk:

- Verify HEAD after Terra follow-up commit.
- Verify only the prior exact-cardinality issue and affected evidence/fix/risk sections.
- Confirm blocking `StreamEvent::Error` is now correctly recorded as 0 responses.
- Confirm MPSC remains correctly recorded as 1 response.
- No mutation or broadening.

I did not run tests, live daemon, network, credentials, source edits, ref edits, stash operations, or publication. I wrote only this `/tmp` report.

## HEAD and diff check

Observed:

```text
HEAD=1db425ef6747611a1902836e8417e5f0f7440b48
parent=99e153edf131f42668a0e51361904053108a8357
subject=docs: Correct R12 stream error matrix.
status=## recovery/seam-r12-20260715
```

Bounded diff from prior sign-off commit:

```text
1 file changed, 36 insertions(+), 16 deletions(-)
docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md
```

This preserves the original commit and reviews. The follow-up is ledger-only.

## Verification of the prior IMPORTANT finding

### Blocking `StreamEvent::Error`: now correctly 0 responses

Source evidence remains:

- `crates/jcode-app-core/src/agent/turn_loops.rs:621-637` logs `stream_event_error` and directly returns `Err(StreamError::new(...).into())`.
- In the inspected `turn_loops.rs:600-640` snippet, no `append_session_evidence_with_correlation` or `ProviderResponse` write occurs on the non-compaction `StreamEvent::Error` branch.

Amended ledger evidence:

- `docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md:32-38` records the corrective amendment and says blocking returns without a correlated `ProviderResponse`, while MPSC emits at `turn_streaming_mpsc.rs:912-924`.
- `ledger.md:81` adds blocking non-compaction `StreamEvent::Error` as an under-emission defect.
- `ledger.md:98` now says: `Blocking StreamEvent::Error, no compaction retry | 1 | 0 | 1 Error`.
- `ledger.md:104-108` includes blocking `StreamEvent::Error` in the boundary/global-blocker discussion.
- `ledger.md:258-261` adds the negative finding that blocking is additional under-emission and MPSC is not equivalent.
- `ledger.md:273-275` includes it in remaining high-confidence but untested control-flow findings.

Conclusion: resolved.

### MPSC `StreamEvent::Error`: remains correctly 1 response

Source evidence remains:

- `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:896-924` logs `stream_event_error`, then calls `append_session_evidence_with_correlation(SessionLogEventKind::ProviderResponse { status: Error, ... }, provider_correlation.clone())`, then returns `Err(StreamError...)`.

Amended ledger evidence:

- `ledger.md:99` now says: `MPSC StreamEvent::Error, no compaction retry | 1 | 1 Error | 1 Error`, citing `turn_streaming_mpsc.rs:912-924`.
- `ledger.md:81`, `ledger.md:258-261`, and `ledger.md:292-297` consistently distinguish MPSC from blocking.
- `ledger.md:234` updates the fix slice acceptance to require raw transport error in blocking/MPSC plus `StreamEvent::Error` in blocking/MPSC each having one request, one error response, and one error finish after future implementation.

Conclusion: still correct.

## Affected section consistency

Checked affected ledger sections:

- Corrective amendment: `ledger.md:32-40`.
- Evidence ledger row 5: `ledger.md:81`.
- Exact cardinality matrix: `ledger.md:93-102`.
- Boundary/risk framing: `ledger.md:104-110`.
- Fix slice 2 acceptance: `ledger.md:230-235`.
- Negative findings: `ledger.md:252-267`.
- Remaining risks/gaps: `ledger.md:269-276`.
- Validation/sign-off notes: `ledger.md:283-304`.

They are internally consistent with the source evidence and with the original top-level adjudication:

- Fork retained for evidence implementation.
- Current pilot remains blocked.
- Strict no-tool future fixture remains the only narrow post-fixture entry path.
- Global invariant remains false until raw transport, blocking event-error, retry/compaction, and cancellation defects are fixed and covered.

## Findings

### No CRITICAL findings

No unsafe pilot-entry approval was introduced. The amended ledger remains conservative.

### No IMPORTANT findings

The prior IMPORTANT finding is resolved. I found no new IMPORTANT issue in the bounded scope.

### Minor notes

- The source code was not changed by this follow-up, so the amended ledger documents current defects rather than fixing them. This is expected and consistent with the requested correction.
- The bounded re-review did not re-run tests or re-open unrelated R12 claims.

## Commands run

Representative read-only commands:

```bash
cd /Users/jrudnik/labs/jcode-seam-r12
git rev-parse HEAD
git status --short --branch
git show -s --format='%H%n%P%n%s%n%cI' HEAD

git diff --stat 99e153edf131f42668a0e51361904053108a8357..HEAD
git diff --name-only 99e153edf131f42668a0e51361904053108a8357..HEAD
git diff --unified=80 99e153edf131f42668a0e51361904053108a8357..HEAD -- docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md

nl -ba crates/jcode-app-core/src/agent/turn_loops.rs | sed -n '600,640p'
nl -ba crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs | sed -n '896,924p'
nl -ba docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md | sed -n '93,110p'
nl -ba docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md | sed -n '230,244p'
rg -n 'StreamEvent::Error|turn_loops\.rs:586|912-924|blocking non-compaction|event-error|blocking versus MPSC' docs/fork/recovery/seams/R12-agent-turn-evidence/ledger.md
```

## Residual gaps

- I did not test or certify future implementation slices.
- I did not re-review unrelated lifecycle rows or cross-seam ledgers beyond the affected sections.
- I did not re-run the full previous sign-off. This was intentionally bounded.

## Final sign-off

**Bounded Fable re-review PASS** at exact HEAD `1db425ef6747611a1902836e8417e5f0f7440b48`.

No new CRITICAL or IMPORTANT findings in the bounded scope.
