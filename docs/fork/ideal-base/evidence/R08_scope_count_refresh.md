# Critical-path scope count refresh (tui 191 -> 192)

## What failed

`Quality Guardrails` / "Test critical-path budget checker" on PR #90:

    test_expected_counts_match_the_current_tree  ... 'tui': 192 != 'tui': 191
    test_expected_counts_sum_to_the_scanned_total ... 292 != 293

Reproduced locally with `python3 scripts/test_critical_path_budget.py`.

## Cause

R08's poke work extracted `crates/jcode-tui/src/tui/app/commands_poke.rs` out of
`commands.rs` (`d7bcbdca3`, extended by `c383b6962` and `f9579aca9`). That is one
net new in-scope tui file, so the tree holds 192 where `EXPECTED_FILE_COUNTS`
still pinned 191.

## Why this was not merely bookkeeping

The obvious reading is "growth is allowed, so this is a stale number." That
reading is wrong, and the checker's own docstring says why: the pin is the
denominator that stops a *shrink* from reading as cleanup. Measured, not
reasoned about:

    pinned tui = 191   tree tui = 192
    delete 1 real tui file -> count 191 -> flagged as shrink? False

While the pin lagged the tree by one, a genuine deletion of an in-scope tui file
would have landed exactly on the stale expectation and passed silently. The
staleness did not merely annoy the self-test; it opened one file of undetected
shrink slack. That is the same defect shape R08 itself is about: a gate
reporting OK for something it did not actually verify.

## Fix and control

`EXPECTED_FILE_COUNTS["tui"] = 192`, and the `--expect-digest` pin in
`fork-ci.yml` refreshed to
`ed8789f326fcdfdf008b586f312f05677f584ce7ebefa9d3ce11baee095b501e`.

The digest is not optional bookkeeping either: after correcting the count,
`test_workflow_pin_matches_the_current_digest` failed until the workflow pin was
refreshed, which is the checker's own guard against editing counts freely.

Control, the same simulated deletion under both pins:

    corrected pin 192 -> flagged: "tui lost in-scope production files: 192 -> 191"
    stale pin     191 -> not flagged

Self-test 32/32 OK. Enforcement exits 0 with unchanged headroom (lifecycle and
tui each 2 below ceiling), so no ceiling was moved to make this pass.

## Scope note

Both edited files are protected governance paths, so this rides the same R07 §4
maintenance window PR #90 already requires. It raises no new authorization: the
`Governance Root` check was already failing on this branch for
`build-matrix.json`, `nix.yml`, and `test_nix_distribution_policy.py` from
earlier F30 work, none of it mine.

Only a count that had fallen behind the tree was corrected. No ceiling, target,
or repository high-water mark was touched.
