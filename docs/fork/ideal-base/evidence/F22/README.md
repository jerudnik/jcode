# F22 — structured security-advisory ownership

Node: `F22` (implement, parent `W4`, depends on `R07`)
Branch: `automation/w4-f22`, based on `main` `eee5ccc71`

## What this node establishes

Every advisory the fork suppresses now carries a machine-readable ownership
record, and an acceptance that is undocumented, incomplete, stale, expired,
postdated, or blanket fails CI and preflight. Before this node the only
enforcement was a shell loop in `.github/workflows/security.yml` that grepped
two Markdown files for the advisory ID: any passing mention satisfied it, and
it carried no owner, no expiry, and no retirement condition.

The fork suppresses advisories on **two** surfaces, and a suppression is only
as governed as its weakest one:

| Surface | Read by | Governed since |
|---|---|---|
| `.cargo/audit.toml` `[advisories].ignore` | `cargo audit` when run bare | round 1 |
| `scripts/security_preflight.sh` `audit_ignores=()` | **what CI actually executes** (`ci.yml:249`, `security.yml:95`/`:117`, `governance-root.yml:52`) | round 2 |

Missing the second one made the round-1 checker vacuous where it mattered; see
*Round 2* below.

## Design decision: why the record is a separate file

The contract asks for a machine-readable record. `.cargo/audit.toml` cannot
hold one. cargo-audit validates that file against a closed schema and refuses
to run on any unknown key, which I verified directly rather than assuming:

```
$ cat .cargo/audit.toml          # probe tree, not the repo
[advisories]
ignore = ["RUSTSEC-2026-0141"]
owner = "x"
$ cargo-audit audit --no-fetch
error: cargo-audit fatal error: parse error: TOML parse error at line 3, column 1
  |
3 | owner = "x"
  | ^^^^^
unknown field `owner`, expected one of `ignore`, `informational_warnings`, `severity_threshold`
```

A `[[triage]]` table fails the same way (`unknown field 'triage', expected one
of 'advisories', 'database', 'output', 'target', 'yanked'`). So ownership
metadata cannot live beside the ignore list, and comments there are not
machine-readable by any definition worth the word.

The record therefore lives in **`docs/security/advisories.toml`**, and
`scripts/check_advisory_policy.py` proves it agrees with every suppression
surface in both directions: a suppression with no record fails, a record with
no suppression fails, and a suppression present on one surface but not the
other fails as drift. So retiring an advisory cannot be done halfway.

Note the third key in that error message, `severity_threshold`. It is a
blanket suppression of everything below a severity level, and it is now
governed too (probe K).

## Files

| Path | Role | Owned by F22? |
|---|---|---|
| `docs/security/advisories.toml` | The machine-readable record: `id`, `crate_name`, `owner`, `accepted`, `expires`, `affected_surface`, `rationale`, `retire_when` | new file |
| `scripts/check_advisory_policy.py` | The checker | new file |
| `tests/test_advisory_policy.py` | 35 fixtures, each planting one violation | new file |
| `.cargo/audit.toml` | Header rewritten: it is one suppression surface, not the record | yes |
| `docs/SECURITY_DEPENDENCIES.md` | Rewritten and reconciled with reality | yes |
| `.github/workflows/security.yml` | New `advisory ownership policy` job, wired into `Security Gate` | yes (protected path) |
| `scripts/required-checks.json` | `advisory-policy` added to the Security Gate contract | no, PROTECTED (reported) |
| `scripts/preflight.sh` | Two new local gates | no (reported to coordinator) |
| `docs/fork/SECURITY_TRIAGE.md` | De-designated as an enforcement surface | no (reported to coordinator) |

## Round 2: two blockers found by independent review

The first cut of this node shipped two real defects. Both were found by an
independent Opus review, reproduced by the coordinator, and reproduced again
here before fixing. They are recorded rather than quietly patched, because
both are instructive about how a gate can look green and be worthless.

### Blocker 1 — the governance manifest was not updated

Adding `advisory-policy` to `security-gate`'s `needs:` without adding it to
`scripts/required-checks.json` breaks `governance_compare.py --live`:

```
FAIL: 'Security Gate' summary dependencies are ['advisory-policy',
      'dependency-audit', 'detect-dependency-changes', 'secret-scan'];
      manifest requires ['dependency-audit', 'detect-dependency-changes',
      'secret-scan']
```

This would have turned the daily Fork Health live run red *after* merge. The
PR run does not catch it, because fork-ci compares against an embedded
fixture snapshot that predates this node — a live/fixture split worth
remembering: a green PR does not prove a green daily.

`scripts/required-checks.json` is a **protected path**. The one-line addition
is flagged for the coordinator's maintenance window.

`test_required_checks_manifest_lists_the_job` now asserts the manifest and the
workflow agree, so the next person to add a job cannot repeat this. Probe N
shows that test failing when the manifest fix is reverted.

### Blocker 2 — the checker was vacuous for the surface CI executes

This is the serious one. `scripts/security_preflight.sh` carries its **own**
hardcoded `audit_ignores=(--ignore ...)` array at lines 101-114, and that is
what CI actually runs: `ci.yml:249`, `security.yml:95` and `:117 --strict`,
`governance-root.yml:52`. The round-1 checker only ever parsed
`.cargo/audit.toml`, so an ignore added straight to the executed array was
invisible to it:

```
$ # --ignore RUSTSEC-2099-9999 planted in scripts/security_preflight.sh
$ python3 scripts/check_advisory_policy.py
advisory policy: OK          # exit 0
```

So the central claim of this node — "no undocumented ignore, and retirement
cannot be done halfway" — was false exactly where it mattered. I had checked
that `preflight.sh` invoked my checker and concluded the wiring was done,
without asking what the *other* preflight script suppressed on its own
authority.

The checker now treats suppression as a property of a set of surfaces. It
parses both, requires every record to match every surface, and requires the
surfaces to agree with each other, so a half-retired advisory is caught as
drift rather than hidden by a union. Probes H and I demonstrate both.

A single source of truth would still be better than agreement-checking: the
preflight array duplicates audit.toml by hand. `security_preflight.sh` is a
vendor-pristine protected file, so collapsing them is not F22's call, but it
is the right follow-up.

### Two smaller holes, same review

- **Blanket `severity_threshold`.** cargo-audit accepts
  `severity_threshold = "critical"`, which silently drops every advisory below
  that level, including ones nobody has ever seen. Ten carefully owned records
  and one unowned threshold is not ownership. A threshold now requires its own
  record with owner, rationale, expiry, and retirement condition, must match
  the configured level, and expires like any other acceptance (probe K).
- **Postdated acceptance.** Expiry was an interval between two *self-declared*
  dates, so `accepted = "2098-01-01"` with a perfectly legal 151-day window
  parked a suppression for 72 years and passed every check (probe J). The
  checker now rejects an `accepted` date in the future.

## Gate 1 and 2 — non-vacuity in both directions

Full transcript: [`non-vacuity.txt`](non-vacuity.txt). Every probe injects the
date, mutates the tree, observes the verdict, and restores. Summary:

| Probe | Tree | Injected date | Exit | Message |
|---|---|---|---|---|
| A | as committed | 2026-07-29 | **0** | `advisory policy: OK as of 2026-07-29` |
| B | as committed | 2027-06-01 | **1** | 10 × `acceptance expired on 2027-01-29` |
| C | `RUSTSEC-2099-0001` added to `.cargo/audit.toml` | 2026-07-29 | **1** | `suppressed in .cargo/audit.toml but has no record` |
| D | first record's `owner` blanked | 2026-07-29 | **1** | `incomplete record, missing or blank: owner` |
| E | `RUSTSEC-2026-0141` ignore deleted, record kept | 2026-07-29 | **1** | `the suppression surfaces must agree` |
| F | restored | 2026-07-29 | **0** | `advisory policy: OK as of 2026-07-29` |
| G | manifest missing `advisory-policy` | n/a | **1** → **0** | `governance_compare --live` FAIL, then match |
| H | `RUSTSEC-2099-9999` planted in `security_preflight.sh` | 2026-07-29 | **1** | `suppressed in scripts/security_preflight.sh but has no record` |
| I | `RUSTSEC-2026-0190` dropped from preflight only | 2026-07-29 | **1** | `suppressed in .cargo/audit.toml but not in scripts/security_preflight.sh` |
| J | `accepted = 2098-01-01`, 151-day window | 2026-07-29 | **1** | `accepted is dated 2098-01-01, in the future` |
| K | `severity_threshold = "critical"`, no record | 2026-07-29 | **1** | `has no [severity_threshold] record` |
| L | restored | 2026-07-29 | **0** | `advisory policy: OK as of 2026-07-29` |
| N | manifest fix reverted | n/a | **1** → **0** | guarding test red, then green |

A, F, and L bracket every red probe, so the greens are not an artifact of a
broken checker and the reds are not residue. Note B and A are the *same tree*:
only the injected date differs, which is precisely the expiry gate. H, J, and
K all exited **0** under the round-1 checker; they are the three vacuity holes
that review found.

## Gate 4 — expiry fixtures inject the current date

The checker resolves "today" as `--today` > `$ADVISORY_POLICY_TODAY` > system
date. No fixture calls `date`. `tests/test_advisory_policy.py` pins this:

- `test_expiry_is_deterministic` runs the same fixture three times either side
  of the expiry and asserts `[0,0,0]` and `[1,1,1]`.
- `test_expiry_boundary_is_inclusive` asserts `2027-01-28` → 0 and
  `2027-01-29` → 1, so there is no silent last-day grace.
- `test_injected_date_overrides_environment` asserts `--today` beats
  `ADVISORY_POLICY_TODAY`, and that the environment is honored when `--today`
  is absent.

CI uses the runner's date, which is the point: the gate must turn red on its
own when an acceptance ages out, on a PR that changed nothing.

```
$ python3 -m unittest discover -s tests -p 'test_advisory_policy.py' -v
Ran 35 tests in 13.677s
OK
```

`test_each_required_field_is_enforced` drops each of the seven non-`id`
required fields in turn and asserts the checker rejects the result, so the
completeness rule is proven field by field, not in aggregate.

## Gate 3 — records are machine-readable and complete

Ten records, one per suppressed advisory, each with all eight fields.
`test_every_ignore_has_a_record` asserts set equality between the ignore list
and the record IDs by parsing both files with `tomllib`, not by grepping prose.
`test_preflight_array_agrees_with_audit_toml` asserts the same set equality
against the array CI executes, so the governed list and the executed list are
provably the same list.

## RUSTSEC-2026-0217 — fixed, not ignored

`main` was red on a real vulnerability: `tract-nnef 0.21.10`, integer overflow
to out-of-bounds read in the NNEF tensor parser, reached via
`jcode-embedding`'s `tract-onnx = "0.21"`. It would have been the obvious first
customer for a new ignore format. It is not, because a compatible fix exists.

**The in-semver-line bump does not work.** Both patched 0.21 releases are
unreachable:

```
$ cargo update -p tract-nnef --precise 0.21.17
error: failed to select a version for `time`.
    ... required by package `tract-linalg v0.21.17`
versions that meet the requirements `>=0.3.23, <0.3.42` are: 0.3.41, ...
  previously selected package `time v0.3.49`
    ... which satisfies dependency `time = "^0.3.47"` of package `azure_identity v1.0.0`
```

`--precise 0.21.16` fails identically. crates.io confirms the constraint is per
release: 0.21.16 and 0.21.17 declare `time >=0.3.23, <0.3.42`; 0.22.2 and
0.22.3 declare `time ^0.3.23`.

**The 0.22 bump works, with no source changes.** The advisory's own solution
line is `>=0.21.16, <0.22.0 OR >=0.22.2, <0.23.0 OR >=0.23.1`, so 0.22.3 is a
patched version:

```
$ cargo update -p tract-onnx -p tract-hir   # after tract-* = "0.22"
    Updating tract-core   0.21.10 -> 0.22.3
    Updating tract-data   0.21.10 -> 0.22.3
    Updating tract-hir    0.21.10 -> 0.22.3
    Updating tract-linalg 0.21.10 -> 0.22.3
    Updating tract-nnef   0.21.10 -> 0.22.3
    Updating tract-onnx   0.21.10 -> 0.22.3
    Updating tract-onnx-opl 0.21.10 -> 0.22.3
      Adding nom-language v0.1.0, pastey v0.1.1, safetensors v0.6.2
$ cargo check -p jcode-embedding
    Finished `dev` profile [unoptimized] target(s) in 4m 12s
$ cargo test -p jcode-embedding
    Finished `test` profile [unoptimized] target(s) in 23m 34s
running 5 tests
test tests::cosine_similarity_handles_basic_cases ... ok
test tests::cross_encoder_scores_relevant_higher_if_present ... ok
test tests::find_similar_returns_only_top_k_sorted_hits ... ok
test tests::alt_model_related_beats_unrelated_if_present ... ok
test tests::minilm_related_beats_unrelated_if_present ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
$ cargo-audit audit
   (no vulnerabilities; 10 allowed warnings, unchanged)
```

The two `..._beats_unrelated_if_present` tests run real MiniLM inference
through `tract-onnx`, so the 0.22 bump is exercised end to end and not merely
type-checked.

0.23.4 also resolves but does **not** compile: it moved `model_for_path` and
`SimplePlan`, so it is a source migration. 0.22.3 is the correct target.

`Cargo.lock` and `crates/jcode-embedding/Cargo.toml` are outside F22's owned
paths. The bump was verified in this worktree and then reverted; the coordinator
owns that commit.

### Reachability, for the record

Even before the fix, the advisory was not reachable from jcode, and this is
worth recording because it is the kind of claim the new format exists to hold:

- `read_tensor` (`tract-nnef-0.21.10/src/tensors.rs:60`) has exactly one
  non-test caller: `DatLoader::try_load` (`src/resource.rs:91`), gated on
  `path.extension() == "dat"`.
- `DatLoader` is registered in exactly one place: `impl Default for Nnef`
  (`src/framework.rs:31`).
- `tract_onnx::onnx()` (`tract-onnx-0.21.10/src/lib.rs:47-51`) returns
  `Onnx { op_register, ..Onnx::default() }` and never constructs an `Nnef`.
  `grep -rn nnef` across `tract-onnx-0.21.10/src/` returns one hit, an
  unrelated `tract_num_traits::Zero` import in `ops/resize.rs:5`.
- jcode's only entry points are `tract_onnx::onnx().model_for_path` at
  `crates/jcode-embedding/src/lib.rs:134` and `:297`, both on an ONNX protobuf;
  the only model fetched is all-MiniLM-L6-v2 over HTTPS from huggingface.co
  (`lib.rs:88-90`).

Not reachable is a reason to be calm about the timeline, not a reason to
suppress. A patched version exists and builds untouched, so the honest
disposition is the fix.

## Documentation reconciled with reality

`docs/SECURITY_DEPENDENCIES.md` carried three stale claims, all corrected:

1. It said ignores live in `scripts/security_preflight.sh`. They live in
   `.cargo/audit.toml`; the preflight array is a vendor-pristine duplicate.
2. It listed `RUSTSEC-2023-0086` (`lexical-core`) as a current advisory. That
   ID is not in `.cargo/audit.toml` and does not appear in current `cargo
   audit` output. The phantom row is removed rather than carried.
3. It contained two near-duplicate "Notes" bullets about the preflight ignore
   list, one a stale subset of the other.

`docs/fork/SECURITY_TRIAGE.md` claimed "the Security workflow fails if an
ignore has no row in either file". That check is gone; the file now says
plainly that it is not an enforcement surface.

## Retired Homebrew host-verification requirement

F22's original contract included "no `StrictHostKeyChecking=no` on the Homebrew
publication path" (`evidence/W0.2/source_census.md:194-200`). `DECISIONS.md:775`
retired that clause when the Homebrew path was removed. Verified absent:

```
$ grep -rn -i 'homebrew\|StrictHostKeyChecking' .github/workflows/release.yml
(no matches; the file is 53 lines and metadata-only)
$ grep -rn -i 'homebrew' docs/SECURITY_DEPENDENCIES.md .cargo/audit.toml \
    .github/workflows/security.yml scripts/security_preflight.sh \
    docs/fork/SECURITY_TRIAGE.md
(no matches)
```

`test_retired_homebrew_host_verification_is_gone` asserts this so the
requirement cannot silently return with a resurrected publication path.
Remaining mentions are in frozen planning and census documents that record the
history, which is correct.

## Commands run

```
cargo-audit audit                                   # before: 1 vulnerability
cargo update -p tract-nnef --precise 0.21.17        # FAILS: time conflict
cargo update -p tract-nnef --precise 0.21.16        # FAILS: time conflict
cargo update -p tract-onnx -p tract-hir             # after tract-* = "0.22": OK
cargo check -p jcode-embedding                      # OK, 4m 12s, no source changes
cargo test  -p jcode-embedding                      # OK, 5 passed / 0 failed
cargo-audit audit                                   # after: 0 vulnerabilities
python3 scripts/check_advisory_policy.py --today ...  # probes A-F, see non-vacuity.txt
python3 -m unittest discover -s tests -p 'test_advisory_policy.py' -v   # 19 tests OK
actionlint .github/workflows/security.yml           # clean
```

## What this node did not check

- The workflow was validated with `actionlint` and with `governance_compare
  --live`, not by a live GitHub Actions run. The `advisory ownership policy`
  job's behavior on a real runner (including that `python3` is present on
  `ubuntu-latest`, which it is by image definition) is unobserved here.
- **Whether any further suppression surface exists.** Round 2 found a second
  one I had missed. I searched for `--ignore` and `RUSTSEC` across `scripts/`
  and `.github/workflows/` and found only the two now governed, but a surface
  that suppresses by some other mechanism (a `deny.toml`, a wrapper that drops
  advisories from output, a vendored database) would not be caught by that
  search and is not covered.
- The 0.22 bump is proven by `cargo check` and `cargo test -p jcode-embedding`
  in this worktree, but it is **not** part of this branch: `Cargo.lock` and
  `crates/jcode-embedding/Cargo.toml` are coordinator-owned and were reverted
  after verification. It landed separately as PR #44. No wider test run
  (workspace-level, or non-darwin) was made under the bump.
- Linux advisory resolution. `cargo audit` was run on aarch64-darwin; the
  `memmap2` record notes that Linux CI observes an additional older transitive
  version.
- Whether the advisory database itself is complete or current beyond the
  fetched snapshot.
- The duplication between `.cargo/audit.toml` and the preflight array is now
  *checked* but not *removed*. A single source of truth would be better;
  `security_preflight.sh` is vendor-pristine and protected, so collapsing them
  was out of scope here.
