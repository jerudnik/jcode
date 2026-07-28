# Stream G evidence transcripts

Generated 2026-07-28T22:50:50Z from branch automation/r07-impl-governance.
All GitHub access read-only. No writes of any kind were performed.

## 1. Test suite

```
$ python3 -m unittest tests.test_governance_compare
----------------------------------------------------------------------
Ran 73 tests in 29.188s

OK
```

## 2. Fixture mode (offline, valid surface) — expect exit 0

```
$ ./scripts/fork-health.sh --fixture docs/fork/ideal-base/evidence/R07/fixtures/governance-valid.json
=== Fork health: jerudnik/jcode (governance source: fixture) ===
OK:   fork-point (631935dd1d3b) is an ancestor of main
OK:   docs/BRANCHING.md documents every workflow
OK:   no Windows CI jobs (issue #19)
OK:   repository id 1238606714 matches the manifest
OK:   repository merge methods are merge-commit only
OK:   effective rules on 'main' are exactly ['deletion', 'non_fast_forward', 'pull_request', 'required_status_checks']
OK:   classic branch protection is absent
OK:   maintained rail 'main' is present
OK:   required context 'Governance Root' is uniquely defined at .github/workflows/governance-root.yml:governance-root
OK:   required context 'Fork CI Gate' is uniquely defined at .github/workflows/fork-ci.yml:fork-ci-gate
OK:   required context 'Security Gate' is uniquely defined at .github/workflows/security.yml:security-gate
OK:   required context 'Nix Gate' is uniquely defined at .github/workflows/nix.yml:nix-gate
NOTE: 21 proposed protected-path addition(s) are pending adjudication and are reported, not enforced (see docs/fork/ideal-base/evidence/R07/stream-g-protected-paths-proposal.md)
=== Governance: snapshot matches the manifest ===
OK:   governance snapshot matches scripts/required-checks.json
INFO: main payload: 596 commit(s) over fork-point

=== Fork health: all invariants hold ===
exit=0
```

## 3. Live mode against the real repository — expect exit 1 (pre-apply)

The design has not been applied, so the live surface is expected to be red.
This transcript is the pre-apply baseline: every FAIL below is a state the
apply document (design.md section 8) is intended to change. A green live run
before apply would mean the comparator is not actually checking anything.

```
$ ./scripts/fork-health.sh --live
=== Fork health: jerudnik/jcode (governance source: live) ===
OK:   fork-point (631935dd1d3b) is an ancestor of main
OK:   docs/BRANCHING.md documents every workflow
OK:   no Windows CI jobs (issue #19)
FAIL: repository setting allow_rebase_merge is True; manifest requires False
FAIL: repository setting allow_squash_merge is True; manifest requires False
FAIL: ruleset 'no-stray-branches' bypass_actors is [{"actor_id":5,"actor_type":"RepositoryRole","bypass_mode":"always"}]; manifest requires []
FAIL: ruleset 'protect-fork-rails' is missing required rule 'non_fast_forward'
FAIL: ruleset 'protect-fork-rails' is missing required rule 'pull_request'
FAIL: ruleset 'protect-fork-rails' is missing required rule 'required_status_checks'
FAIL: effective rules on 'main' are missing 'non_fast_forward'
FAIL: effective rules on 'main' are missing 'pull_request'
FAIL: effective rules on 'main' are missing 'required_status_checks'
FAIL: classic branch protection still exists alongside the ruleset (a contradictory second layer): {"allow_deletions":{"enabled":false},"allow_force_pushes":{"enabled":false},"allow_fork_syncing":{"enabled":false},"block_creations":{"enabled":false},"enforce_admins":{"enabled":false},"lock_branch":{"enabled":false},"required_conversation_resolution":{"enabled":false},"required_linear_history":{"enabled":false},"required_signatures":{"enabled":false},"required_status_checks":{"checks":[{"app_id":null,"context":"Detect changes"}],"contexts":["Detect changes"],"strict":false}}
FAIL: required context 'Governance Root' has no job definition in any workflow
FAIL: required context 'Fork CI Gate' has no job definition in any workflow
FAIL: required context 'Security Gate' has no job definition in any workflow
FAIL: required context 'Nix Gate' has no job definition in any workflow
=== Governance: 14 mismatch(es) against the manifest ===
OK:   repository id 1238606714 matches the manifest
OK:   maintained rail 'main' is present
NOTE: 21 proposed protected-path addition(s) are pending adjudication and are reported, not enforced (see docs/fork/ideal-base/evidence/R07/stream-g-protected-paths-proposal.md)
FAIL: governance comparison found mismatches (listed above)
INFO: main payload: 596 commit(s) over fork-point

=== Fork health: 1 invariant violation(s) ===
exit=1
```

### Notes on the live findings

- `allow_rebase_merge` / `allow_squash_merge` are still enabled; the manifest
  requires merge-commit only.
- `protect-fork-rails` currently carries only the `deletion` rule.
- Classic branch protection still exists alongside the ruleset and requires the
  stale `Detect changes` context. This matches recon section 2.2 and design.md
  line 345; it must be deleted only *after* the replacement ruleset is read back.
- The four required contexts have no job definitions because
  `workflow-contexts.proposed.patch` is coordinator-owned and deliberately not
  applied in this stream.
- `no-stray-branches` has a non-empty `bypass_actors` (RepositoryRole 5, always).

## 4. Fail-closed behaviour (section 6 exit codes)

Exit 0 = matches, 1 = classified mismatch, 2 = unclassifiable. An unreadable
or malformed surface must never be reported as a pass.

### 4a. gh not on PATH
```
$ FORK_HEALTH_GH=definitely-not-gh ./scripts/fork-health.sh --live
OK:   fork-point (631935dd1d3b) is an ancestor of main
OK:   docs/BRANCHING.md documents every workflow
OK:   no Windows CI jobs (issue #19)
ERROR: live governance acquisition failed at endpoint: gh
      definitely-not-gh is not on PATH
error: governance comparison could not be completed (exit 2)
exit=2
```

### 4b. malformed fixture
```
$ ./scripts/fork-health.sh --fixture /tmp/r07-bad.json
OK:   fork-point (631935dd1d3b) is an ancestor of main
OK:   docs/BRANCHING.md documents every workflow
OK:   no Windows CI jobs (issue #19)
ERROR: snapshot is not valid JSON: /tmp/r07-bad.json: Expecting value: line 1 column 1 (char 0)
error: governance comparison could not be completed (exit 2)
exit=2
```

### 4c. missing fixture
```
$ ./scripts/fork-health.sh --fixture /tmp/r07-does-not-exist.json
=== Fork health: jerudnik/jcode (governance source: fixture) ===
OK:   fork-point (631935dd1d3b) is an ancestor of main
OK:   docs/BRANCHING.md documents every workflow
OK:   no Windows CI jobs (issue #19)
error: fixture not found: /tmp/r07-does-not-exist.json
exit=2
```

### 4d. no governance source selected (mode is mandatory)
```
$ ./scripts/fork-health.sh
error: one of --fixture PATH or --live is required (see --help)
exit=2
```

### 4e. --repo disagreeing with the manifest fails closed
```
$ ./scripts/fork-health.sh --repo someone/else --fixture .../governance-valid.json
error: --repo 'someone/else' disagrees with the manifest's repository 'jerudnik/jcode'
exit=2
```

## 5. Mutation detection (non-vacuity spot checks)

The valid fixture passes, so each mutation below must flip it to exit 1.
Full coverage of the section 7 matrix lives in tests/test_governance_compare.py;
these are hand transcripts of representative rows.

### 5a. Drop a required context
```
FAIL: required context 'Nix Gate' is not required by ruleset 'protect-fork-rails'
=== Governance: 1 mismatch(es) against the manifest ===
exit=1
```

### 5b. Spoof the integration_id on a required context
```
FAIL: required context 'Governance Root' is pinned to integration_id 99999; manifest requires 15368 (an unpinned context is spoofable)
=== Governance: 1 mismatch(es) against the manifest ===
exit=1
```

### 5c. Re-enable squash merge
```
FAIL: repository setting allow_squash_merge is True; manifest requires False
=== Governance: 1 mismatch(es) against the manifest ===
exit=1
```

### 5d. Grant a bypass actor
```
FAIL: ruleset 'no-stray-branches' bypass_actors is [{"actor_id":5,"actor_type":"RepositoryRole","bypass_mode":"always"}]; manifest requires []
=== Governance: 1 mismatch(es) against the manifest ===
exit=1
```

### 5e. Resurrect classic branch protection
```
FAIL: classic branch protection still exists alongside the ruleset (a contradictory second layer): {"required_status_checks":{"contexts":["Detect changes"]}}
=== Governance: 1 mismatch(es) against the manifest ===
exit=1
```

### 5f. Remove bypass_actors entirely (unauthorized read, not an empty list)
```
ERROR: ruleset 'no-stray-branches' has no bypass_actors key; the credential cannot see bypass actors, so this read is unauthorized, not empty
exit=2
```

