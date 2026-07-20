# F11: Independent verification of F09/F10 reconciliation

Verifier: independent opus-class worker, 2026-07-20. Verified F09 (d5d388028,
`reconcile_stale_pending_activation` in `crates/jcode-build-support/src/lib.rs`)
and F10 (4b66de27c, `reconcile_disconnect_cleanup_records` + durable records in
`crates/jcode-app-core/src/server/client_disconnect_cleanup.rs`).

Gate: all recovery matrices pass with **no false rollback** and **no fabricated
terminal state**.

## Verdict

**PASS.** All 8 F09 reconcile tests and all 16 F10 cleanup tests pass fresh.
Four independent probes (corrupt manifest, fingerprint-less valid candidate,
missing sidecar, idempotent double-sweep) confirm no false rollback and no
fabricated terminal state. One unrelated pre-existing test failure noted below.

## Fresh run transcripts

`scripts/dev_cargo.sh test -p jcode-build-support` (inside `nix develop`):

```text
test result: FAILED. 57 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.92s
```

The single failure is `tests::dirty_source_state_uses_fingerprint_in_version_label`,
which panics at `tests.rs:381` with "source state: Failed to get git hash".
It is **unrelated to F09** (it exercises `current_source_state` on a git repo
fixture; the failure reproduces in isolation and stems from `git` not being on
the dev-shell PATH in this sandboxed environment: `error: tool 'git' not found`).
All 8 `reconcile_*` tests pass:

```text
test tests::reconcile_dead_session_fingerprint_mismatch_rolls_back ... ok
test tests::reconcile_dead_session_missing_candidate_rolls_back ... ok
test tests::reconcile_dead_session_valid_candidate_completes ... ok
test tests::reconcile_fresh_record_is_untouched_even_with_dead_session ... ok
test tests::reconcile_live_initiator_is_left_alone ... ok
test tests::reconcile_no_pending_returns_no_pending ... ok
test tests::reconcile_preserves_live_canary_from_other_session ... ok
test tests::reconcile_rollback_does_not_clobber_newer_publish ... ok
```

`scripts/dev_cargo.sh test -p jcode-app-core --lib client_disconnect_cleanup`:

```text
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 1159 filtered out; finished in 2.32s
```

All 3 `startup_reconcile_*` tests plus the record-lifecycle tests
(`disconnect_lock_timeout_leaves_durable_cleanup_record`,
`disconnect_happy_path_removes_cleanup_record`) pass.

## Combined recovery matrix

| # | Case | Mechanism | Test / probe | Asserted outcome |
|---|------|-----------|--------------|------------------|
| 1 | No pending record | F09 sweep | `reconcile_no_pending_returns_no_pending` | `NoPending`, no action |
| 2 | Crash: dead initiator, valid candidate (binary + matching sidecar) | F09 | `reconcile_dead_session_valid_candidate_completes` | `Completed`; canary=Passed; symlinks untouched |
| 3 | Crash: dead initiator, candidate binary missing | F09 | `reconcile_dead_session_missing_candidate_rolls_back` | `RolledBack`; both symlinks restored; canary=Failed |
| 4 | Crash: dead initiator, sidecar fingerprint mismatch | F09 | `reconcile_dead_session_fingerprint_mismatch_rolls_back` | `RolledBack`; symlinks restored |
| 5 | Live owner: initiator still alive | F09 | `reconcile_live_initiator_is_left_alone` | `InitiatorAlive`; record kept |
| 6 | Fresh record (< min age), even with dead session | F09 | `reconcile_fresh_record_is_untouched_even_with_dead_session` | `StillFresh`; record kept (no false rollback) |
| 7 | Live owner: different live canary session | F09 | `reconcile_preserves_live_canary_from_other_session` | `Skipped`; canary state and symlinks untouched |
| 8 | Restart race: newer publish after stale record | F09 | `reconcile_rollback_does_not_clobber_newer_publish` | `RolledBack` but current stays at newer publish |
| 9 | Corrupt manifest JSON | F09 | **probe 1** | `Err` surfaced; no action; manifest not overwritten; symlinks untouched |
| 10 | Fingerprint-less (legacy) record, valid candidate | F09 | **probe 2** | `Completed`, **not** rolled back (no false rollback) |
| 11 | Candidate binary present but sidecar missing | F09 | **probe 3** | `RolledBack` to previous versions; record cleared |
| 12 | Idempotency: second sweep after completion | F09 | **probe 4** | `NoPending`; no double action |
| 13 | Crash: record for dead session, reason=while_processing | F10 startup sweep | `startup_reconcile_marks_stale_session_terminal_and_deletes_record` | Session -> Crashed; record deleted |
| 14 | Crash: record for dead session, reason=client_disconnected | F10 | `startup_reconcile_maps_clean_disconnect_reason_to_closed` | Session -> Closed (not Crashed); record deleted |
| 15 | Live owner: record whose session is live | F10 | `startup_reconcile_leaves_live_sessions_alone` | Session stays Active; record kept (no fabricated terminal state) |
| 16 | Lock timeout leaves durable intent | F10 | `disconnect_lock_timeout_leaves_durable_cleanup_record` | Record written with session_id + pid |
| 17 | Happy path removes intent | F10 | `disconnect_happy_path_removes_cleanup_record` | Record deleted after terminal persist |
| 18 | Missing file: record for unloadable session | F10 | code path `Session::load` Err arm (lines 285-290) | warn + drop record (uncovered by test, see below) |

Covered: 17 of 18 enumerated cells by test or probe. Uncovered: 1 (cell 18)
plus the interaction cells listed below.

## Probe results (source in this dir would exceed scope; transcript below)

Standalone binary linking `jcode-build-support` directly, temp `JCODE_HOME`
per probe, run on this machine:

```text
PROBE1 corrupt-manifest: result=Err("Corrupt JSON at .../builds/manifest.json: ...") current_before=Some("prev-current") current_after=Some("prev-current")
PROBE1 PASS: error surfaced, no symlink movement, manifest untouched
PROBE2 no-fingerprint-valid-candidate: outcome=Completed("cand-nofp") current=Some("cand-nofp")
PROBE2 PASS: no false rollback for fingerprint-less valid candidate
PROBE3 missing-sidecar: outcome=RolledBack("cand-nosc") current=Some("prev-current") shared=Some("prev-shared")
PROBE3 PASS: unverifiable candidate rolled back to previous, record cleared
PROBE4 idempotent: first=Completed("cand-idem") second=NoPending
PROBE4 PASS: second sweep is a no-op
ALL F09 PROBES PASS
```

Gate prohibitions checked directly:

- **No false rollback**: cells 2, 5, 6, 10 (probe 2 is the sharpest: a legacy
  record with `source_fingerprint: None` and a healthy candidate completes,
  it does not roll back). Fresh records and live initiators are never touched.
- **No fabricated terminal state**: cell 15 (F10 live session stays Active,
  record retained); cells 5, 7 (F09 never touches live-owner state). F10's
  persist-failure arm (lines 274-280) keeps the record for retry instead of
  claiming success, which is the correct non-fabricating behavior.

## Uncovered cells and risk assessment

1. **F10 record for unloadable/deleted session file** (cell 18): no test.
   Code inspection shows the Err arm warns and drops the record without
   touching any session state, so no fabrication risk. Low risk; worth a test.
2. **F10 corrupt/empty record JSON**: `read_to_string(...).ok().and_then(parse.ok())`
   yields `reason=None`, so reconcile maps to Crashed (conservative) and the
   record is still deleted. Behavior is safe but untested. Low risk.
3. **F09+F10 interaction, both records for the same session**: the two sweeps
   are independent `spawn_blocking` tasks over disjoint state (build manifest
   vs session records); no shared mutable state, ordering is irrelevant.
   Untested as a combined scenario. Low risk.
4. **F09 persist failure mid-rollback** (e.g. read-only symlink dir): rollback
   would `Err` out of the sweep; record survives for the next startup. Not
   tested. Low risk (retry semantics, same shape as F10's persist-failure arm).

## Findings

- No findings contradicting the F09/F10 acceptances. Both callers in
  `server.rs` (lines 1185-1215) wire liveness to
  `observe_session_pid_markers(sid).active_marker_is_live()` and run
  best-effort in `spawn_blocking`, matching the reviews.
- Pre-existing unrelated failure: `dirty_source_state_uses_fingerprint_in_version_label`
  fails in this environment because `git` is unavailable inside the dev shell
  invocation used here. Not an F09/F10 regression (test predates both and
  exercises source-state, not reconciliation).

## Verdict line

F11 VERIFY: PASS. 8/8 F09 reconcile tests, 16/16 F10 cleanup tests, 4/4
independent probes green. No false rollback observed; no fabricated terminal
state observed. Uncovered cells are all no-action or conservative paths.
