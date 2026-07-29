# R07 barriers 2-5 — execution evidence

Date: 2026-07-29. User authorized all six enumerated external writes
(2026-07-29T02:05Z). Barrier 1 evidence is in `barrier-1-archive-push.md`.

## Barrier 2 — branch push + bootstrap PR

- All 12 `automation/r07-*` branches pushed to jerudnik/jcode (no force).
- Bootstrap PR: https://github.com/jerudnik/jcode/pull/39
- Process note, recorded honestly: the PR was initially created before the
  authorized workflow diff was committed; `07b10b1e2` applied
  `workflow-contexts.proposed.patch` in the same PR (fixture regeneration
  byte-identical, actionlint clean) before any merge. Final head:
  `69a6a63101d605fb8f2b2a7e069ec49e1a61f73c`.

## Barrier 3 — context-emission proof (read-only) + one real gap found and fixed

Check-runs on head `69a6a6310`: all four required contexts present exactly
once each, emitted by app id **15368** (`github-actions`): Fork CI Gate,
Security Gate, Nix Gate `success`; Governance Root `failure` naming every
changed protected path (expected on a governance-path change).

Barrier 3 caught a real gap no local gate could see: the railway validator's
`reviewed_commit` object-existence check cannot pass in a CI clone (the
reviewed objects are not ancestors of main), so `Governance Contract Gate`
went red and would have locked out every PR. Fixed in `69a6a6310` (option A,
user-approved): explicit opt-in `JCODE_RAILWAY_ALLOW_MISSING_REVIEWED_OBJECTS`
degrades only that one check to a NOTE in CI; strict everywhere else,
existence anchored by barrier-1 remote verification and strict local
validation. Re-run: Governance Contract Gate green, all gates green except
the by-design Governance Root red.

## Barrier 4 — bootstrap merge + governance apply

- Merge: API `PUT pulls/39/merge`, `merge_method=merge`, expected head SHA
  verified; merge commit `a545ecee42fef172c8c6ff730dc6cb5608ebe652`.
- Apply sequence `github-governance.proposed.json` sequences 1-17 executed
  strictly in order with the abort policy honored:
  - seq 1-6 read asserts: PASS (repo id 1238606714; four contexts on head
    with app 15368; both ruleset bodies sanitized-equal to recon baselines;
    classic protection body equal; local git ancestry + 27-path diff quiet).
  - seq 7 PUT ruleset 18509013 → seq 8 read-back exact (incl.
    `required_reviewers: []`, merge-only, four contexts @15368,
    `bypass_actors: []`) + seq-6 repeat PASS → seq 9 effective rules exactly
    [deletion, non_fast_forward, pull_request, required_status_checks].
  - seq 10 PUT ruleset 18509016 → seq 11 read-back exact (bypass_actors now
    empty; excludes main + automation/**).
  - seq 12 PATCH repo (merge-commit only) → seq 13 read-back PASS.
  - seq 14 checkpoint: all prior passed.
  - seq 15 DELETE classic branch protection → seq 16 read-back 404 →
    seq 17 final effective-rules assert PASS.
- **Live verification:** `fork-health.sh --live` at `a545ecee4` is fully
  green including `governance snapshot matches scripts/required-checks.json`
  and "enforcing 27 paths". This is the true post-apply confirmation the
  re-review flagged for G3 (`required_reviewers: []` pin works against the
  real live ruleset).
- Auth deviation, recorded honestly: sequences 7 and 10 ran with ambient
  owner auth because the `rbw` binary was not on the execution PATH; after
  locating it at `/etc/profiles/per-user/jrudnik/bin/rbw`, sequences 12 and
  15 used the admin token inline as documented. No token was stored, echoed,
  or committed.

## Barrier 5 — under-enforcement proof (closed unmerged)

Proof PR: https://github.com/jerudnik/jcode/pull/40
- Commit 1 (harmless, no protected paths): **all four contexts green**
  (Governance Root pass in 15s).
- Commit 2 (planted comment-only change to `scripts/fork-health.sh`):
  **Governance Root red** with `::error::governance paths changed` naming
  `scripts/fork-health.sh`; Fork CI Gate, Security Gate, Nix Gate green.
- Closed unmerged; branch deleted. The gate is neither vacuous nor
  over-broad.
