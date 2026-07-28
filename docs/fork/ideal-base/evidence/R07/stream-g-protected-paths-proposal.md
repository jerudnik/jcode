# R07 Stream G — protected-paths residual and proposed additions

Status: **proposed, not enforced.** For adjudication at the integration gate.

## Summary

The design's protected-path set (§4) protects the workflow definitions themselves
(`.github/workflows`, `.github/scripts`) and the governance rails
(`scripts/required-checks.json`, `scripts/fork-health.sh`,
`scripts/ideal_base_railway.py`, `tests/test_ideal_base_railway.py`,
`docs/fork/ideal-base/evidence/R07/github-governance.proposed.json`).

It does **not** protect the scripts those workflows execute. A required check can
therefore be made vacuous without touching any protected path: edit the gate
script, not the workflow that calls it. The required context still reports green
because the job still runs and the script still exits 0 — it just no longer
checks anything.

This is the non-blocking finding raised by the v4 gate. Stream G confirmed it is
real, enumerated the exposure, and encoded the remedy as *pending* additions
behind `protected_paths.additions_adjudicated: false` in
`scripts/required-checks.json`. While that flag is false the comparator
**reports** the pending additions and does not fail on their absence, so this
stream does not unilaterally widen a governance boundary. Flipping the flag to
`true` (a coordinator decision) makes the additions enforced.

## Execution evidence

Every path below is reached from a workflow that produces a required context, on
the paths that actually run (not `workflow_dispatch`-only paths). Line numbers
are against the current `automation/r07-design` tree.

### `fork-ci.yml`, `quality` job

| Line | Invocation |
| --- | --- |
| 235 | `scripts/check_warning_budget.sh` |
| 238 | `python3 scripts/check_code_size_budget.py` |
| 241 | `python3 scripts/check_test_size_budget.py` |
| 244 | `python3 scripts/check_agent_instructions.py` |
| 247 | `python3 scripts/test_docs_impact_advisory.py` |
| 253 | `python3 scripts/check_panic_budget.py` |
| 260 | `python3 scripts/check_tui_render_lock.py` |
| 268 | `python3 scripts/check_env_lease_drop_order.py` |
| 279 | `scripts/check_ambient_roots.sh` |
| 282 | `python3 scripts/check_swallowed_error_budget.py` |

### `security.yml`

| Line | Invocation |
| --- | --- |
| 73 | `scripts/security_preflight.sh` |
| 111 | `scripts/security_preflight.sh --strict` |

### `nix.yml`, `validate` job

| Line | Invocation |
| --- | --- |
| 129 | `nix build .#checks.<system>.nix-distribution-policy` → `flake.nix:281` → `tests/test_nix_distribution_policy.py` |

### Transitive and data dependencies

These are not invoked directly by a workflow but determine whether the above
gates can fail:

- `scripts/rust_production_filter.py` and `tests/test_rust_production_filter.py`
  — the production-code filter shared by the budget checks. Widening its
  exclusion set silently shrinks what every budget measures.
- `scripts/docs_impact_advisory.py` — the module under test at line 247.
- `scripts/ambient_roots_allowlist.txt` — the allowlist consulted by
  `check_ambient_roots.sh`.
- The ratchet baselines: `scripts/code_size_budget.json`,
  `scripts/panic_budget.json`, `scripts/swallowed_error_budget.json`,
  `scripts/test_size_budget.json`, `scripts/warning_budget.txt`. Raising a
  baseline is the intended escape valve for these gates, so these are the
  weakest members of the set — see "Adjudication notes" below.

## Proposed additions (21)

```
scripts/ambient_roots_allowlist.txt
scripts/check_agent_instructions.py
scripts/check_ambient_roots.sh
scripts/check_code_size_budget.py
scripts/check_env_lease_drop_order.py
scripts/check_panic_budget.py
scripts/check_swallowed_error_budget.py
scripts/check_test_size_budget.py
scripts/check_tui_render_lock.py
scripts/check_warning_budget.sh
scripts/code_size_budget.json
scripts/docs_impact_advisory.py
scripts/panic_budget.json
scripts/rust_production_filter.py
scripts/security_preflight.sh
scripts/swallowed_error_budget.json
scripts/test_docs_impact_advisory.py
scripts/test_size_budget.json
scripts/warning_budget.txt
tests/test_nix_distribution_policy.py
tests/test_rust_production_filter.py
```

All 21 exist in the tree at the time of writing; the comparator's schema check
rejects a manifest that names a path which does not.

## Deliberately excluded

`ci.yml` is `workflow_dispatch:`-only and produces no required context, so the
scripts reachable only through it are out of scope for *this* residual:
`check_dependency_boundaries.py`, `check_config_env_lease.py`,
`check_wildcard_reexport_budget.py`, `check_real_home_isolation.sh`,
`check_startup_budget.sh`, `check_web_mobile.sh`. If `ci.yml` ever gains a
`pull_request` trigger these become in-scope and should be revisited.

`.github/scripts/run_with_timeout.py` is invoked by `fork-ci.yml` but needs no
addition — it is already covered by the `.github/scripts` prefix in the required
set.

## Adjudication notes

Two honest caveats for whoever decides this:

1. **The ratchet baselines are routinely and legitimately edited.** Protecting
   `warning_budget.txt` et al. means every legitimate ratchet movement needs the
   protected-path ceremony. That is arguably the point — a baseline raise is
   exactly the silent-weakening move worth reviewing — but it is real friction,
   and it is the reason I did not simply enforce this. The coordinator may
   reasonably split the set: enforce the check *scripts*, leave the *baselines*
   unprotected, and rely on review.
2. **This closes one hole, not the class.** Protected paths cannot stop a gate
   from being weakened through its own inputs indefinitely; a sufficiently
   determined change can still route around any fixed list (e.g. by editing a
   library the filter imports). The durable fix is the required-context contract
   in §5 plus the comparator's vacuous-gate detection, which fails when a job
   declares a dependency it never reads. Treat this list as defence in depth.

## Enforcement mechanics

`scripts/governance_compare.py` reads `protected_paths` from the manifest:

- `required` — absence is a mismatch (exit 1).
- `proposed_additions` — while `additions_adjudicated` is `false`, absence is
  reported to stdout as a pending item and does not affect the exit code. When
  the flag is `true`, they are treated exactly like `required`.

Covered by `ProtectedPathAdjudicationTests` in
`tests/test_governance_compare.py`, which asserts both polarities of the flag.
