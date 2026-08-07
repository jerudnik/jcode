# Protected-path source divergence: gate enforced 32, three artifacts declared 31

Date: 2026-08-06
Branch: `automation/protected-path-equality`

## Plain summary

The audit gate enforced 32 protected paths. The manifest and both lists in the
R07 apply-doc declared 31. The check that exists to keep those in agreement was
written one-directional, so it could not see the difference, and did not.

The missing path was `scripts/test_warning_budget.py`. Anyone running
`fork-health.sh --live` was told the protected boundary was 31 paths when the
gate was in fact enforcing 32: a change touching only that path read clean
locally and was then rejected by the gate.

The check is now a set equality in both directions. The three lagging artifacts
were reconciled **up** to the gate's 32.

## Measured divergence (each artifact read independently)

| artifact | n | symmetric difference vs live workflow |
|---|---|---|
| live `.github/workflows/governance-root.yml` `protected=( )` | 32 | (reference) |
| R07 fixture `fixtures/governance-valid.json` workflow copy | 32 | NONE |
| `scripts/required-checks.json` `protected_paths.required` | 31 | `scripts/test_warning_budget.py` |
| apply-doc `template_variables.protected_paths` | 31 | 5 tokens |
| apply-doc sequence-6 `git diff --quiet` assertion | 31 | 5 tokens |

Correction to an earlier claim carried into this window: the fixture was
previously described as lagging at 31. It was not. It is byte-identical to the
live workflow at 32, so only three artifacts lagged, not four.

The apply-doc's 5-token difference is the one real path plus four trailing-slash
spelling differences: it carries `.github/scripts/` and `.github/workflows/`
where the workflow and manifest carry them unslashed.

### Trailing slash: proven equivalent, not assumed

Normalizing `a/b/` to `a/b` is only safe if it does not change what is covered.
Checked rather than assumed:

```
.github/scripts:    n=1   git ls-files with and without slash -> identical digest
.github/workflows:  n=11  git ls-files with and without slash -> identical digest
```

Both queries are non-empty, so the match is a real result and not an empty-set
artifact. Comparison normalizes the trailing slash; reconciliation therefore
added one path, not five.

## Root cause

Two places had the same defect.

`scripts/governance_compare.py:835`

```python
missing = [p for p in protected_paths if p not in source]
```

Manifest-subset-of-workflow. A path the gate enforces but the manifest omits is
structurally invisible to it.

`tests/test_governance_compare.py:619` had the matching shape, a per-path
`assertIn` loop over `required`, while its docstring claimed the set "must be
identical in every artifact" and promised to make "a future one-sided edit fail
loudly". It could not, and did not. Same defect class as the PR #106 false
all-clear.

Substring matching cannot express the missing half: it can tell whether a path
appears somewhere in the file, never whether the file enforces something extra.
The fix parses the gate's own `protected=( ... )` array and compares sets.

## Drift origin

Located by pickaxe, not inferred:

```
git log -S 'scripts/test_warning_budget.py' -- .github/workflows/governance-root.yml
835fca3d0 fix(ci): the warning budget gate counted nothing for its entire life
```

Its stat shows `governance-root.yml +1` and no change to
`scripts/required-checks.json`. A one-sided edit landed green, exactly as the
subset check permits.

## Controls

Every control was planted on disk, asserted present before any exit code was
read, and restored byte-identical (`diff -q` plus a SHA-256 comparison against
the pre-mutation digest). Each fails on a **different** assertion.

| id | mutation | expected | assertion that fired |
|---|---|---|---|
| C1 | none: the real divergence on disk | fail | `enforces protected path(s) [...] that required-checks.json does not declare` (comparator, exit 1) and `manifest/governance-root.yml fixture mismatch` (test) |
| C2 | manifest gains `scripts/preflight.sh` only | fail | `does not name protected path(s)` (the opposite direction from C1) |
| C3 | none, post-reconciliation | **pass** | comparator exit 0, `enforces exactly the 32 protected path(s) the manifest declares`; 74 tests OK |
| C4 | fixture's `protected=( )` emptied | fail | `array is empty; the audit gate enforces nothing` |
| C5a | apply-doc seq-6 drops `scripts/security_preflight.sh` | fail | `manifest/sequence-6 diff assertion mismatch` |
| C5b | live workflow drops a path, fixture left stale at 32 | fail | `manifest/live governance-root.yml mismatch` |

C3 is the acceptance-side control: without it the other five only show the check
can fail, not that it can still pass.

C1 exit codes were read twice. The first read of exit 1 was a `NameError` crash
from a missing `import re`, not a mismatch; exit status alone does not
distinguish the two, so the output was read and the traceback count asserted
zero before C1 was accepted as valid.

C4 exists because an unparseable array would otherwise compare equal to an empty
set and agree with anything. A zero-pattern parse now raises instead.

C5b is the reason the test reads the **live** workflow and not only the fixture.
The fixture is a copy; without that assertion a stale fixture could certify a
drifted gate.

## Falsifier

The user-visible symptom was `fork-health.sh --live` reporting `enforcing 31
paths` against a gate enforcing 32. After reconciliation the comparator reports:

```
NOTE: protected-path additions are adjudicated; enforcing 32 paths
```

Health check and gate now agree.

## Verification

- `tests.test_governance_compare`: 74 tests, OK
- `tests.test_ideal_base_railway`: OK
- `scripts/test_docs_references.py`: 34 tests, OK
- `scripts/test_warning_budget.py`: 7 tests, OK
- `scripts/check_docs_references.py`: exit 0

## Changed files

All four are themselves protected paths, so this change is in scope for R07 §4.

- `scripts/governance_compare.py`: equality check plus `_parse_protected_array`
- `tests/test_governance_compare.py`: set equality; adds the live-workflow assertion
- `scripts/required-checks.json`: 31 -> 32
- `docs/fork/ideal-base/evidence/R07/github-governance.proposed.json`: both lists 31 -> 32

Reconciliation moved the lagging artifacts **up** to the gate's set. Moving the
gate down would have shrunk a governance boundary to make a check pass.
