# R07 adversarial integration gate

Date: 2026-07-28
Integration reviewed: `automation/r07-integration` at `bbb03555f13dc568455166e56576b2514333844f`
Design baseline: `automation/r07-design` at `886f1a383d8eb5e7cd7d5bbb64f122a61e3cfbde`

## Verdict: PASS-WITH-FIXES

The three implementation merges are faithful, the schema-v2 STATE migration is lossless and validates, the workflow patch applies and passes `actionlint`, both required test suites pass at the expected counts, governance modes fail closed, and the state/archive evidence spot-checks reproduce.

The integration is **not ready for independent review unchanged** because the authoritative apply document did not receive the coordinator's 16 adjudicated protected-path additions. This is a bounded coordinator-applicable fix and does not require redesign, so the verdict is PASS-WITH-FIXES rather than FAIL.

## Required fix before independent review

### F1 - Apply document protects only 7 of the 23 enforced paths

Severity: blocking integration-coherence defect.

`scripts/required-checks.json:103-131`, the patched `governance-root.yml` represented by `workflow-contexts.proposed.patch:35-59`, and the regenerated fixture all contain the same 23 enforced paths. The five deliberately excluded ratchet baselines appear in none of those sets.

However:

- `github-governance.proposed.json:11-19` still lists only the original 7 paths in `template_variables.protected_paths`.
- `github-governance.proposed.json:120-126` hard-codes the same stale 7-path set in sequence 6's executable `git diff --quiet` assertion.
- `integration-adjudication.md:38-40` states that the apply document was extended and that sequence 6 asserts on the full enforced set, but the file contradicts that claim.
- `git diff 886f1a383..bbb03555f -- github-governance.proposed.json` is empty, confirming the integration never changed the apply document.

The 16 missing paths are:

- `scripts/ambient_roots_allowlist.txt`
- `scripts/check_agent_instructions.py`
- `scripts/check_ambient_roots.sh`
- `scripts/check_code_size_budget.py`
- `scripts/check_env_lease_drop_order.py`
- `scripts/check_panic_budget.py`
- `scripts/check_swallowed_error_budget.py`
- `scripts/check_test_size_budget.py`
- `scripts/check_tui_render_lock.py`
- `scripts/check_warning_budget.sh`
- `scripts/docs_impact_advisory.py`
- `scripts/rust_production_filter.py`
- `scripts/security_preflight.sh`
- `scripts/test_docs_impact_advisory.py`
- `tests/test_nix_distribution_policy.py`
- `tests/test_rust_production_filter.py`

Impact: the PR-time `Governance Root` workflow will detect changes to these paths, but the design-v4 sequence-6 assertion will not detect an intervening main change to them between the bootstrap merge and the first ruleset write. That invalidates the adjudication's same-set invariant and the widened form of design v4's current-main protected-byte assertion.

Coordinator-applicable remediation:

1. Add the 16 paths to `template_variables.protected_paths`, preserving the existing directory-slash convention.
2. Add the same paths to sequence 6's hard-coded `git diff --quiet` assertion. Sequence 8 repeats sequence 6 by reference and then inherits the corrected set.
3. Add a regression test that normalizes directory suffixes and compares all four representations: manifest `protected_paths.required`, patched workflow array, apply-document template and executable sequence-6 path list, and fixture workflow text. Existing tests do not read `github-governance.proposed.json`, which is why 73/73 remained green.
4. Re-run the two Python suites, patch application, fixture comparison, and `actionlint`. The embedded ruleset/classic-protection hashes should remain unchanged because no request or baseline body needs to change.

## Verification evidence

### 1. Merge faithfulness

Each merge introduced exactly its implementation branch's changed-path set, and its merge-result bytes equal the implementation tip on those paths:

- Archive merge `917fb7e30`: 2 paths, exact.
- State merge `d3c046b9a`: 3 paths, exact.
- Governance merge `4b346412b`: 8 paths, exact.

Comparing implementation tips to final integration found only coordinator-authored deltas:

- Archive: zero final deltas.
- State: only `tests/test_ideal_base_railway.py`; its diff exactly equals the coordinator commit's diff.
- Governance: only the regenerated fixture, `scripts/required-checks.json`, and `tests/test_governance_compare.py`; their diff exactly equals the coordinator commit's diff.

`DECISIONS.md` and `WORK_GRAPH.json` blob IDs are unchanged from design:

- `DECISIONS.md`: `7d6e7e96206c60992bf03bb485f62267816531ee`
- `WORK_GRAPH.json`: `51d8ddb449b52ad2c01f955510112f7b8ac3472a`

### 2. STATE schema-v2 migration

Independent comparison of design-tip schema v1 to integrated schema v2 found:

- 57/57 node IDs preserved.
- Integrated `STATE.json` exactly equals `evidence/R07/STATE.proposed.json`.
- 0 changes to node `state`, `evidence`, `summary`, or `updated_at` values.
- 35/35 `reviewed_commit` values are full 40-hex prefix-preserving expansions of the previous abbreviated `commit` values.
- 35/35 `published_commit` values exist as commit objects and are ancestors of `498249777c453c1d551aeb01fc45420d8ca0a585`.
- 0 accepted full-SHA format violations.
- 0 non-accepted records with non-null commit identities.
- `last_checkpoint` invariant fields are identical and its key split is lossless.
- Mapping ledger has 35 entries, the expected baseline, and 0 identity mismatches against STATE.
- WORK_GRAPH and STATE node sets match 57/57.

Production validation passed:

```text
python3 scripts/ideal_base_railway.py check --published-ref 498249777c453c1d551aeb01fc45420d8ca0a585
ideal-base railway OK: 7 roots, 50 child nodes, 57 state records, protected hash intact
```

### 3. Protected-path adjudication and workflow patch

Independent normalized set comparison produced:

```text
manifest: 23 paths, ratchets []
workflow patch: 23 paths, ratchets []
fixture workflow: 23 paths, ratchets []
apply document: 7 paths, ratchets [], missing the 16 adjudicated additions
```

Additional checks:

- `proposed_additions` is empty and `additions_adjudicated` is true.
- The fixture's `governance-root.yml` text exactly equals the patched workflow text.
- Patch hunk header is `@@ -0,0 +1,61 @@`; the applied file has exactly 61 lines.
- `git apply --check` and real application both succeeded against workflows archived from baseline main `498249777...`.
- `/nix/var/nix/profiles/default/bin/nix shell nixpkgs#actionlint --command actionlint .gate-tmp/main/.github/workflows/*.yml` exited 0.

### 4. Tests and fork-health modes

Required suites passed at the expected counts:

```text
Ran 73 tests in 25.132s - OK
Ran 25 tests in 65.244s - OK
```

The coordinator's test edits retain mechanism coverage:

- The two governance tests now synthesize one real missing path and prove both flag polarities: pending/unadjudicated reports but exits 0; adjudicated enforces and exits 1 naming the path.
- The state test now asserts the integrated v2 premise, while surrounding tests still cover full-SHA rejection, commit-object existence versus ancestry, unresolved refs, non-main-ancestral reviewed identities, shallow-repository fail-closed behavior, proposed-state validation, and checkpoint semantics.

Fork-health behavior:

- Valid fixture: exit 0.
- No mode, malformed fixture, missing fixture, repository mismatch, and missing `gh`: exit 2.
- Live mode with `gh` supplied by Nix: exit 1 with the expected 14 pre-apply governance mismatches.

### 5. Apply-document design-v4 invariants

Passing checks:

- Step sequences are contiguous 1 through 17.
- Sequences 1 through 6 are reads/assertions before the first write at sequence 7.
- The authoritative apply document is otherwise byte-identical to design v4.
- Embedded and freshly read live hashes match exactly:
  - sequence 3: `8440214dee8621d8a12a9456083a1c3afc82442291fd8a67ddcea7852d239124`
  - sequence 4: `1376e3835feca779dd1dd2387e7cb5e1095f34c6de71ae64483c17e52823f99f`
  - sequence 5: `d20823253081aca9537b632d1b8605a72d8838f520fe0d14defa7dc2d76b4704`

Failing check: the widened protected-path set is absent from both the template and executable sequence 6, as described in F1.

### 6. Stream-evidence sanity

State ledger:

- Method counts reproduce the design: identity 2, unique patch ID 26, merge payload 3, merge payload with file-tree split 1, file-tree at published commit 3.
- Re-derived complete path sets and historical blob equality for F18, F20c, F21, and F28 all pass.
- F18's split publication boundary `163e6e0d7...` is an ancestor of `767afae9c...`.

Archive:

- Manifest has 39 unique refs: 33 reviewed heads and 6 stash tags.
- 39/39 objects exist and are commits.
- Stream A's quoted refspecs equal the manifest exactly, 39/39.
- All six local stash tags resolve to the manifest objects and retain 2 or 3 parents.
- Exactly six reviewed objects are unreachable from current refs; all six remain present in reflogs.
- The 33 archive heads exactly equal accepted reviewed identities that are not main-ancestral.
- Read-only `ls-remote` found zero refs in either managed archive namespace, consistent with pre-write status.

Governance transcript claims were reproduced by the required suite, fixture mode, live mode, and fail-closed cases. The only stale statement is the adjudication's claim that it updated the apply-document protected paths.

### 7. Cross-stream interference sweep

- No direct import/call coupling exists between `governance_compare.py` and `ideal_base_railway.py` or their test modules.
- The implementation streams changed disjoint owned path sets at merge time.
- Coordinator edits changed only documented integration premises and regenerated governance data.
- The principal integration blind spot is cross-artifact rather than runtime: tests compare the manifest to the workflow fixture but never compare either to the authoritative apply document, allowing F1 to pass both suites.

## What I did not check

- No GitHub write, ruleset mutation, workflow application, pull-request creation, merge, or protection deletion was performed.
- I did not prove the archive repository is private, authorize or execute the atomic push, or perform post-write fresh-fetch/fsck verification.
- I did not execute the bootstrap PR/context-emission and post-enforcement proof-PR barriers because the integration is still pre-apply and F1 must be fixed first.
- I did not independently reproduce all 26 unique patch IDs or all three complete merge-payload mappings. I reproduced all published-commit ancestry, all weak file-tree rungs, the F18 split, and the full archive object/refspec surface.
- I did not run unrelated workspace-wide Rust/Nix/preflight suites; the changed integration surface is Python, JSON, shell, workflow patch, and evidence, and the requested targeted gates were run.

## Confidence

**High, 98%.** The blocking finding is a direct parsed-set and executable-command mismatch with precise file evidence. All other requested minimum checks either passed independently or are explicitly listed above as not checked.
