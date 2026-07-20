# F15 audit review (adversarial, opus-class)

Reviewed evidence at bb53da147. Verdict: **PASS**. Census total (58),
bucket classifications, CI structure claims, fix commits, and Gate 1
reason/exit-condition comments all verified against source.

## Important findings

Naive `rg '#\[ignore'` recount gives 62; delta fully reconciled: 6 are
doc/comment matches, plus 2 cfg_attr macOS-conditional ignores the naive
pattern misses (56+2=58). Full reconciliation performed: zero
unclassified ignores (gate 2 holds).

## Minor findings

README line refs drift by a few lines vs current workflow files (content
matches exactly). Fix-hash verification: 8 of the cited commits verified
by --stat; the rest existed but were not individually diffed in budget.

## Gates executed

Gate 1 sampled twice: macOS lib-test advisory (continue-on-error with
patch-ledger citation + clean-week exit condition) and Linux Tests
(schedule-conditional blocking with reason + weekly exit). Gate 2 full
recount. CI claims verified (ci.yml workflow_dispatch-only, --no-run lib
tests, no --ignored anywhere). Spot-checks: binary_integration.rs:138
genuinely promotable; bash_tests cfg_attr verbatim; three other entries
match file:line and bucket.

## Not checked

Whether ignored tests actually pass when un-ignored; the patch-ledger
citation contents; ci.yml Windows job details; promotion-order value
judgments beyond prerequisite plausibility.
