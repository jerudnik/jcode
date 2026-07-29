# F23 — critical-path zero-growth budgets and downward quality targets

Node: `docs/fork/ideal-base/WORK_GRAPH.json` F23 (implement, parent W4, depends on R07).
Acceptance standard: A6, "Critical lifecycle, persistence, updater,
provider-infrastructure, and TUI paths have zero-growth panic/swallowed-error/oversize
budgets plus explicit downward targets."

Worktree `/private/tmp/w4-f23`, branch `automation/w4-f23`, based on `main` at `eee5ccc71`.

## 1. What already existed, and the precise gap

Verified by reading each script and `grep budget .github/workflows/fork-ci.yml`, not assumed.

| Check | Baseline | Zero-growth? | Critical-path aware? | Trend reported? |
|---|---|---|---|---|
| `check_panic_budget.py` | `panic_budget.json` (total 56, 24 files) | Yes, per-file and total | No | No |
| `check_swallowed_error_budget.py` | `swallowed_error_budget.json` (total 3032, 476 files) | Yes, per-file, per-pattern and total | No | No |
| `check_code_size_budget.py` | `code_size_budget.json` (100 files > 1200 LOC) | Yes, per-file | No | No |
| `check_test_size_budget.py` | `test_size_budget.json` (37 files) | Yes, per-file | No | No |
| `check_warning_budget.sh` | `warning_budget.txt` (0) | Yes, single global number | No | No |
| `check_wildcard_reexport_budget.py` | `wildcard_reexport_budget.json` (16) | Yes | No | No. Runs in `ci.yml` and `preflight.sh`, not `fork-ci.yml` |
| `check_startup_budget.sh` | — | N/A, a perf wrapper over `bench_startup.py` | No | No |

So three of A6's four requirements were already met *for the repository as a whole*:
zero-growth is real, and existing debt is genuinely grandfathered. Three things were missing.

1. **No critical path exists.** Not one script distinguishes `crates/jcode-app-core/src/server/`
   from a leaf widget crate. A6 names five domains; nothing in the repository encodes them.
2. **No downward target is recorded anywhere.** Every script says "do not grow" and
   `--update` "only after intentional cleanup". Cleanup is optional and unquantified,
   which is exactly what A6's "plus explicit downward targets" asks for and no script provides.
3. **The trend is neither reported nor one-directional.** Each script prints its own number
   and no artifact aggregates them. More importantly, all five ratchet baselines are
   deliberately *unprotected* (`docs/fork/ideal-base/evidence/R07/integration-adjudication.md`
   left them out of the 27-path protected set so routine tightening needs no maintenance
   window). The accepted cost was that a *raise* is only "visible in review". A ratchet whose
   baseline can be freely raised bounds one working tree, not the trend.

I did not rewrite any working checker. `check_critical_path_budget.py` is additive.

## 2. What was implemented

New files (both previously nonexistent, both unprotected):

- `scripts/check_critical_path_budget.py` — the gate and the trend reporter.
- `scripts/test_critical_path_budget.py` — 20 deterministic tests of its logic.

Modified: `.github/workflows/fork-ci.yml` (three steps added to the `quality` job).

### 2.1 The critical-path set, and why these paths

`CRITICAL_PATHS` maps each A6 domain to concrete prefixes in this crate layout. Prefixes
are matched in declaration order and every file is attributed to exactly one domain, so
totals cannot double count. `scripts/test_critical_path_budget.py` asserts each declared
prefix exists in the tree and resolves to its own domain.

| Domain | Prefixes | Justification |
|---|---|---|
| `lifecycle` | `crates/jcode-app-core/src/server/`, `crates/jcode-core/` | A0/A1. The `server/` module holds `shutdown.rs`, `lifecycle.rs`, `client_lifecycle.rs`, `runtime.rs`, `reload.rs`, `socket.rs` — the bounded shutdown authority and the lease-bearing code. `jcode-core` holds `process.rs`, `panic_util.rs`, `activity.rs`. |
| `persistence` | `crates/jcode-app-core/src/restart_snapshot.rs`, `crates/jcode-storage/`, `crates/jcode-session-types/`, `crates/jcode-background-types/`, `crates/jcode-telemetry-core/` | A2/A7. Durable background/recovery state, the storage root that A7's ambient-root defense routes through, and the telemetry active-session markers A7 names. |
| `updater` | `crates/jcode-app-core/src/update.rs`, `crates/jcode-app-core/src/tool/selfdev/`, `src/cli/selfdev.rs` | A5. `update.rs` is the updater; selfdev performs the same acquire/activate/rollback role in-tree, and A2 explicitly names "stale selfdev pending activation". |
| `provider_infrastructure` | `crates/jcode-provider-core/`, `crates/jcode-provider-env/`, `crates/jcode-provider-metadata/`, `crates/jcode-auth-types/` | Shared transport, selection, failover, retry, auth. **Vendor adapters are deliberately excluded**: `jcode-provider-openai`, `-anthropic` and the other ~16 are leaf integrations, not infrastructure. A6 says "provider-infrastructure", not "providers". The exclusion is pinned by a test so a prefix typo like `crates/jcode-provider-` cannot silently swallow them. |
| `tui` | `crates/jcode-tui/`, `crates/jcode-tui-core/`, `crates/jcode-tui-render/` | The TUI surface this fork ships plus its core and render primitives. Leaf widget crates (`-markdown`, `-mermaid`, `-usage-overlay`, ...) stay on the repository-wide ratchets. |

290 production Rust files are in scope. Test files are excluded by the existing shared
`rust_production_filter`, so critical-path counting matches the other ratchets exactly.

### 2.2 Ceilings: grandfathered, zero-growth

`CEILINGS` records the observed count per domain per dimension at this commit. Existing
debt is grandfathered wholesale — **no cleanup is demanded to land this node**, which is
F23's explicit "without demanding an all-at-once cleanup" constraint. Growth past a
ceiling fails. Because ceilings are per-domain, debt also cannot be shuffled between
domains to stay under an aggregate.

### 2.3 Downward targets: explicit, recorded, and never a blocker

`TARGETS` records a per-domain target per dimension plus a `rationale` string explaining
why that value and not zero. Targets are **reported, never gated**: every run prints
`distance_to_target` and the report JSON carries `at_or_below_target`. This is deliberate.
A gated target would demand the all-at-once cleanup F23 forbids.

Current distance to target: 20 panics, 580 swallowed errors, 20 oversize files.

Tests pin that targets are strictly downward (`target <= ceiling` everywhere, with at
least one strictly below, so a target set equal to the ceiling everywhere cannot pass
while demanding nothing) and that every target carries a rationale.

### 2.4 Trend: a report artifact plus a one-directional ratchet

`--report` writes `critical-path-debt-trend.json`; `fork-ci.yml` uploads it as the
`critical-path-debt-trend` artifact with `if: always()`, so the trend is published on red
runs too. It carries per-domain current/ceiling/headroom/target/distance, the named
oversize files, critical totals, repository totals, and the critical share of repository
debt (currently 35.7% of panics, 38.3% of swallowed errors, 45.0% of oversize files — the
critical scope is 290 of the repository's production files but holds roughly 40% of its debt).

`REPOSITORY_CEILINGS` holds high-water marks for all six repository-wide numbers, read
from the five ratchet baselines rather than rescanned (the per-ratchet scripts already
prove the tree matches its baseline; this checks the one thing they cannot, namely whether
the *recorded budget* moved up). A baseline may be lowered freely. Raising one fails.

### 2.5 Why routine tightening still needs no maintenance window

This is the governance constraint the node called out, and the design satisfies it exactly.

- The five ratchet baselines stay **unprotected and unmodified** by this node. A cleanup
  followed by `check_panic_budget.py --update` lowers `panic_budget.json`, which lands
  *under* its high-water mark, so `check_critical_path_budget.py` stays green with no edit
  to it and no edit to the workflow. **No maintenance window.**
- Critical-path cleanup likewise needs no edit: ceilings are high-water marks, so removing
  debt just opens headroom, reported as "N below its ceiling".
- **Weakening** requires editing `scripts/check_critical_path_budget.py` *and* the
  `--expect-digest` pin in `.github/workflows/fork-ci.yml`. Both are protected governance
  paths, so a ceiling raise, a scope narrowing, a target relaxation, or a threshold loosening
  turns `Governance Root` red and lands in a reviewed maintenance window. That asymmetry —
  tightening frictionless, loosening reviewed — is the whole point of the digest.

The digest covers `oversize_threshold_loc`, `critical_paths`, `ceilings`, `targets` and
`repository_ceilings`. A test asserts `set(pinned_data()) == set(DIGEST_FIELDS)` and that
mutating each field individually changes the digest, so the pin cannot go vacuous by
someone adding a sixth field and forgetting to hash it.

One residual is closed separately: `code_size_budget.json`'s `threshold_loc` is
unprotected, so raising it to 5000 there would retire the oversize dimension without
touching anything protected. The checker asserts the baseline threshold equals its own
pinned `OVERSIZE_THRESHOLD_LOC` and fails on drift (plant 8).

### 2.6 Protection: the original claim was wrong, and is now fixed

**The first version of this section was incorrect and is retained here as the record of
the error.** It said protection needed a four-artifact edit F23 did not own, and concluded:
"The digest pin in the already-protected workflow delivers the same reviewed-weakening
property without them."

That is false. The digest lives in the workflow, but the code that *compares* it lives in
the checker. If the checker is unprotected, one edit can raise a ceiling and disable the
comparison at the same time, and the two agree with each other. A checker cannot attest to
itself. An independent review demonstrated it with two lines, reproduced verbatim in §3.3.

Both scripts are now in `protected_paths.required`. The coordinated edit spans four
artifacts, and `tests/test_governance_compare.py` enforces that they move together:

| Artifact | Edit |
|---|---|
| `scripts/required-checks.json` | both scripts added to `protected_paths.required` |
| `.github/workflows/governance-root.yml` | both added to the `protected=(...)` array |
| `docs/.../github-governance.proposed.json` | `template_variables.protected_paths` **and** the sequence-6 `git diff` assertion |
| `docs/.../fixtures/governance-valid.json` | regenerated via `scripts/generate_governance_fixture.py` |

The earlier 7 failures came from doing only the first. Doing all four: **74 passed.**

This commit therefore touches protected paths and needs the recorded ruleset maintenance
procedure. That is the intended cost of making the checker non-self-attesting.

## 3. Non-vacuity: both directions, every gate

Per DECISIONS.md D029, "never trust a new gate that has not been observed failing". Eight
plants, each planted → observed → reverted → observed green. Full transcript with output:
`non-vacuity-log.txt`. Harness: `plant_harness.sh` (not committed to `scripts/`; it is
evidence, and it `assert_clean`s the tree between plants). The harness ran in two phases
because a full pass exceeds the 600 s tool cap; phase 1 covered plants 1-3, phase 2 the rest.

| # | Plant | Where | Planted | Reverted |
|---|---|---|---|---|
| 1 | new `.expect()` | `crates/jcode-core/src/util.rs` (lifecycle) | **RED** exit 1 | GREEN exit 0 |
| 2 | new `let _ =` | `crates/jcode-app-core/src/update.rs` (updater) | **RED** exit 1 | GREEN exit 0 |
| 3 | new 1301-LOC file | `crates/jcode-provider-core/` (provider-infra) | **RED** exit 1 | GREEN exit 0 |
| 4 | all three, non-critical | `crates/jcode-fuzzy/` | critical gate **GREEN**; repo panic/swallow/code-size ratchets all **RED** | GREEN exit 0 |
| 5 | raise `panic_budget.json` total 56→57 | unprotected baseline | **RED** exit 1 | GREEN exit 0 |
| 6 | raise `tui` panic ceiling 8→99 | pinned block | **RED** on digest mismatch | GREEN exit 0 |
| 7 | drop `crates/jcode-tui/` from scope | pinned block | **RED** on digest mismatch | GREEN exit 0 |
| 8 | raise `code_size_budget.json` threshold 1200→5000 | unprotected baseline | **RED** exit 1 | GREEN exit 0 |

Each red run *names* the defect. Plant 1:

```
Critical-path budget exceeded (acceptance standard A6):
  - lifecycle/panic grew past its zero-growth ceiling: 11 -> 12
      contributor: crates/jcode-app-core/src/server/shutdown.rs (8)
      ...
      contributor: crates/jcode-core/src/util.rs (1)      <- the plant
```

Plant 3 names the planted file with its size:
`contributor: crates/jcode-provider-core/src/f23_planted_oversize.rs (1301 LOC)`.

### 3.1 The documented non-critical policy, proved (plant 4)

Policy: **this gate does not fire outside the critical scope; the repository-wide ratchets
do.** `jcode-fuzzy` is a leaf crate in no critical domain. Planting a panic, a `let _ =`,
and a 1301-LOC file there produced exactly that:

```
plant4/noncritical-critical-gate-GREEN        -> EXIT=0
plant4/noncritical-repo-panic-ratchet-RED     -> EXIT=1
plant4/noncritical-repo-swallow-ratchet-RED   -> EXIT=1
plant4/noncritical-repo-codesize-ratchet-RED  -> EXIT=1
```

New debt is refused everywhere. The difference is *how*: outside the critical scope it can
be accepted by an `--update` to an unprotected baseline; inside it, the ceiling holds and
raising it needs a maintenance window.

### 3.2 The digest is load-bearing, not decorative (plant 6)

Plant 6 records the failure mode the pin exists to prevent. Raising the `tui` panic ceiling
8→99 lets the checker pass on its own — `plant6/ceiling-raise-hides-plant-without-pin ->
EXIT=0`, a silently weakened gate. With the workflow's `--expect-digest` it is red:

```
Critical-path budget scope digest mismatch.
  expected (pinned in .github/workflows/fork-ci.yml): 2eafa153...
  actual   (scripts/check_critical_path_budget.py):   <different>
```

Both plants 6 and 7 also modify a protected path, so in CI they would independently turn
`Governance Root` red. Two mechanisms, as R07 established for `governance_compare.py`.

### 3.3 The self-attestation defect, reproduced and closed (review finding, CRITICAL)

The attack is two lines in the checker and nothing else:

```
line 142:  "tui": {"panic": 8, ...}   ->   "tui": {"panic": 999, ...}
line 512:  if args.expect_digest is not None and ...   ->   if False:
```

The checker still passes, because it is the thing that was edited:

```
$ python3 scripts/check_critical_path_budget.py --expect-digest b3b7fc25...
  tui                      panic                 8    999       0      8
  - tui/panic is 991 below its ceiling
EXIT=0
$ git diff --name-only    # the whole attack surface
scripts/check_critical_path_budget.py
```

So the only thing that can catch it is `Governance Root`. Both directions were run against
the *same* commit, differing only in which `protected=()` array is in force (parsed out of
the workflow by the simulator, not restated, so it cannot drift):

```
BEFORE (c49bbaf19 array, does not name the checker: 0 hits)
  GOVERNANCE ROOT: PASS (exit 0)  -- attack UNDETECTED
AFTER  (this commit's array)
  ::error::governance paths changed; use the recorded ruleset maintenance procedure
  scripts/check_critical_path_budget.py
  GOVERNANCE ROOT: FAIL (exit 1)
```

### 3.4 The shrinking denominator, reproduced and closed (review finding, HIGH)

`git mv crates/jcode-app-core/src/server/shutdown.rs` out of the critical set, changing no
budget file. The old checker treats the vanished debt as progress:

```
BEFORE (c49bbaf19 checker)
  production files in scope: 289
  lifecycle   panic   3   11   0   3
  Critical-path budget OK, with headroom from prior cleanup:
    - lifecycle/panic is 8 below its ceiling
  EXIT=0
```

`lifecycle/panic` fell 11 -> 3 and the gate called it cleanup. With `EXPECTED_FILE_COUNTS`:

```
AFTER (this commit)
  production files in scope: 289
    lifecycle   current=  61  expected=  62  (-1)
  - lifecycle lost in-scope production files: 62 -> 61. Debt that leaves the critical
    set is not debt that was fixed. ...
  EXIT=1
```

Only a *decrease* fails. Growth stays green (`test_added_files_are_not_a_regression`):
adding files to a critical domain is normal work and the debt it brings is still bounded by
the ceilings. The counts are inside the digest, so they cannot be edited without also
editing the workflow pin, which is a protected path.

## 4. Validation performed

| Check | Result |
|---|---|
| `python3 scripts/check_critical_path_budget.py --report ...` at baseline | exit 0, both gates green |
| `python3 -m unittest ... test_critical_path_budget.py` | **30 passed** (was 20) |
| `actionlint .github/workflows/fork-ci.yml` | clean (run twice: after the budget step, after the test step) |
| `pytest tests/test_governance_compare.py` with my changes | **74 passed, 13 subtests passed** |
| `tests.test_governance_compare` with all four artifacts updated | **74 passed** (partial edit = 7 failed; see §2.6) |
| `tests.test_ideal_base_railway` | **18 passed** |
| `check_panic_budget` / `check_swallowed_error_budget` / `check_code_size_budget` | all exit 0 |
| Review defect 1 (self-attestation) both directions | old array PASS/undetected, new array FAIL (§3.3) |
| Review defect 2 (scope shrink) both directions | old checker exit 0 "headroom", new checker exit 1 (§3.4) |
| Non-vacuity harness, 8 plants both directions | all as tabled; `git status` clean after every plant |
| `python3 scripts/check_panic_budget.py` after plant 4 revert | exit 0, no residue |

The 20 unit tests cover what the plants cannot cheaply observe: domain attribution
(including that `update.rs` does not capture a hypothetical `update_helpers.rs` by stem,
and that vendor adapters stay out of provider-infrastructure), digest sensitivity to each
pinned field, target-direction coherence, repository-trend comparison in both directions,
and that the workflow pin is not stale. That last one means a stale pin is a CI failure
rather than a silently unenforced digest.

CI wiring runs the tests via `python3 -m unittest discover -s scripts -p
'test_critical_path_budget.py'`, matching the existing `test_rust_production_filter.py`
precedent, so a semantic neutering of the checker is caught by executed tests as R07's
`rereview-remediation.md` requires.

## 5. Files

Committed (all within F23's owned paths):

- `scripts/check_critical_path_budget.py` — new, **protected**
- `scripts/test_critical_path_budget.py` — new, **protected**
- `scripts/required-checks.json` — modified, **protected**
- `.github/workflows/governance-root.yml` — modified, **protected**
- `docs/fork/ideal-base/evidence/R07/github-governance.proposed.json` — modified, **protected**
- `docs/fork/ideal-base/evidence/R07/fixtures/governance-valid.json` — regenerated, **protected**
- `.github/workflows/fork-ci.yml` — modified, **protected**
- `docs/fork/ideal-base/evidence/F23/README.md` — this file
- `docs/fork/ideal-base/evidence/F23/debt-trend-report.json` — the trend report artifact
- `docs/fork/ideal-base/evidence/F23/debt-trend-console.txt` — console form
- `docs/fork/ideal-base/evidence/F23/non-vacuity-log.txt` — full 8-plant transcript
- `docs/fork/ideal-base/evidence/F23/plant_harness.sh` — the harness, for re-running

Seven files, six of them protected, so this node needs **one maintenance window** covering
the whole set. The unowned paths (`required-checks.json`, `governance-root.yml`, and the
two R07 artifacts) were touched on explicit direction from the coordinator after the
review, purely to add the two scripts to the protected set.

No ratchet baseline was modified. No unowned path was touched.

## 6. Open questions and what was not checked

- **Not checked: CI execution.** Every gate was proved locally on macOS/aarch64. The
  workflow steps are `actionlint`-clean and use only `python3` plus `actions/upload-artifact@v4`
  (already used elsewhere in this repo's workflows), but they have not run on a GitHub runner.
- **Not checked: cross-platform line counting.** `rust_file_line_count` counts `\n`, matching
  `check_code_size_budget.py` exactly, so any CRLF behaviour is identical to the existing gate.
- **Resolved: the two new scripts are now protected.** The earlier answer here ("the digest
  pin covers the weakening case meanwhile") was wrong; see §2.6 and §3.3.
- **Not checked: whether `EXPECTED_FILE_COUNTS` drifts noisily.** Any legitimate file
  removal in a critical domain now needs a protected-path edit. If critical-path files are
  deleted often, that cost recurs. It did not come up across the plants, but the repo's
  decomposition work may move files more than usual, and if so the right fix is probably a
  per-domain allowance rather than dropping the gate.
- **Open: target review cadence.** Targets are recorded but nothing schedules a review.
  Roughly-halve was chosen as a defensible first pass; a follow-up node could tie them to
  the decomposition program's milestones.
- **Not checked: whether targets are achievable.** `lifecycle/panic` target 0 assumes the 11
  remaining `.unwrap()`/`.expect()` calls in `server/` are all removable. I did not audit
  them individually. Since targets never gate, an unachievable target costs a stale report
  line, not a red build.
- **Not checked: `check_wildcard_reexport_budget.py` and `check_startup_budget.sh`.** Neither
  is invoked by `fork-ci.yml` (the former runs in `ci.yml` and `preflight.sh`). Wiring them
  into the fork gate is out of F23's scope and would change which checks block `main`.
