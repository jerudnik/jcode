# R07 design v2 gate: adversarial re-review

Reviewed `automation/r07-design` at `4d7a0169103e39f2bb36edc298e9ea1a29691d5f`
from isolated branch `automation/r07-design-v2-gate`, against:

- R07 in `docs/fork/ideal-base/WORK_GRAPH.json`;
- D031 in `docs/fork/ideal-base/DECISIONS.md`;
- recon ground truth at `automation/r07-recon`;
- the v1 gate at `automation/r07-design-gate`; and
- the complete `26eedab27..4d7a01691` evidence diff.

All GitHub probes in this review were read-only and used ambient `gh` authentication. No
ruleset, branch protection, pull request, ref, workflow, or repository setting was written.

## Verdict: FAIL

The steady-state ruleset shape is corrected, and the state/mapping/archive portions retain the
substance that passed the v1 gate. The revision nevertheless fails the independent no-lockout and
no-bypass gate in two transition states:

1. the bootstrap-to-apply sequence can require a context that was present on the historical
   bootstrap head but has since been removed from current `main`; and
2. later maintenance removes the only governance-path blocker repository-wide without binding
   the bypass window to one reviewed pull request and one expected head SHA.

Both are executable bad/sloppy-actor paths. D031 accepts the owner-admin as the root of trust; it
does not repeal R07's explicit requirements that an absent context is never required and that an
independent reviewer find no bypass, lockout, or false-durability gap
(`WORK_GRAPH.json:1252,1256`).

## Findings

### Finding 1: blocking: bootstrap preflight proves historical emission, not current availability

The design correctly says context definitions must exist on `main` before they are required
(`design.md:304-318`) and explicitly permits another pull request to merge between the bootstrap
merge and ruleset apply (`design.md:320-323`). It claims the apply remains safe because preflight
re-reads live state.

The authoritative apply document does not perform the required read:

- its `apply_only_after` prose infers that because the bootstrap PR merged, all definitions exist
  on `main` (`github-governance.proposed.json:7-11`), but that implication can become stale;
- preflight sequence 2 reads check runs on `${bootstrap_head_sha}`, a historical commit
  (`github-governance.proposed.json:23-28`);
- sequences 3-5 read ruleset identities and classic protection, not current `main` workflow
  definitions (`github-governance.proposed.json:30-53`); and
- sequence 6 immediately requires all four contexts (`github-governance.proposed.json:54-118`).

Counterexample:

1. bootstrap PR emits all four contexts and merges;
2. before sequence 1, another PR merges under the weak pre-R07 regime and removes, renames, or
   path-filters one required workflow/context;
3. sequence 2 still passes because the old bootstrap head still has its check runs;
4. sequences 3-5 still pass because their server-side surfaces are unchanged; and
5. sequence 6 requires the now-absent context, locking normal merges out.

That directly violates the acceptance gate, “a context that can be absent is never required.” It
also contradicts the design's claim that an intervening merge is safe.

A repair needs an executable immediately-pre-write assertion against current `main`, not the old
bootstrap head. It must bind the current main SHA to reviewed workflow bytes/contracts, reject any
protected-path change since the bootstrap merge, and serialize the final assertion and activation
so no intervening merge is treated as harmless.

### Finding 2: blocking: maintenance drops `Governance Root` globally, with no transaction binding

The maintenance procedure opens and reviews a governance PR, then removes `Governance Root` from
required checks for the entire repository, merges, and restores it (`design.md:325-347`). It does
not bind the window to:

- one PR number;
- one source repository/ref;
- one reviewed head SHA;
- one expected base/main SHA;
- an API merge conditioned on that head SHA;
- proof that no other PR merged in the window; or
- a trusted pre-window comparator used after the merge.

This matters because the design itself correctly admits that every remaining check runs from
pull-request-controlled workflow/code and that one PR can change the summary jobs, comparator,
and `governance-root.yml` together (`design.md:175-190`). The integration ID pin does not separate
one GitHub Actions workflow from another. While `Governance Root` is not required, a concurrent or
mistakenly selected governance PR can make the other required contexts green using the same
trusted integration ID and merge.

The restoration steps do not close the evidence gap. Restoring the exact ruleset and running
`fork-health.sh --live` prove final server state, not that only the reviewed PR merged during the
window. If an intervening governance merge changes the manifest, workflow definitions, and
candidate-run comparator consistently, the post-restore candidate comparator is not an
independent witness. A scheduled live run detects the missing check only if it fires during the
window; after exact restoration, ruleset-only drift is gone.

Live read-only context reduces but does not eliminate the defect:

- the direct collaborator list currently contains only `jerudnik`;
- there were no other open governance-path PRs at the time of the probe; but
- repository Actions default workflow permissions are `write`, with review approval enabled;
- collaborator inventory and workflow-token policy are not part of the manifest or maintenance
  preflight; and
- the requested adversarial case includes a sloppy owner selecting or merging the wrong PR.

D031 accepts that the owner can rewrite rulesets. It does not justify a procedure that temporarily
makes unrelated candidate-controlled governance changes mergeable while claiming no bypass or
false-durability gap. At minimum, maintenance must reuse the v1-quality identity/merge binding:
exact PR/source/head/base capture, expected-head API merge, main-tip/two-parent verification, proof
of no intervening merge, immediate restoration, and post-merge comparison from a reviewed
pre-window implementation as well as the new implementation.

### Finding 3: material: the five preflight reads are ordered before writes, but not all asserted drift is executable

The positive part is sound:

- sequences 1-5 are all `read_assert` operations;
- the first write is sequence 6; and
- `abort_policy` forbids a write before all five pass
  (`github-governance.proposed.json:13-56,214`).

Identifier provenance is now honest. Sequence 1 reads repository identity/id, and sequence 2
reads required check runs and their GitHub Actions app id. Read-only live probes confirmed
`repository_id: 1238606714`, GitHub Actions app id `15368`, and the two current ruleset id/name
bindings. Neither numeric id appears in recon, so treating both as live-verified inputs is the
correct correction.

The fail-closed claim is still incomplete:

- sequence 2 does not assert that `Governance Root` concluded `failure`, although the design says a
  green result on the governance-changing bootstrap PR is a stop (`design.md:312-316`);
- sequences 3 and 4 bind numeric IDs to names/target/source type, but do not compare the full live
  bodies to recon, despite `design.md:287-289` claiming preflight sequences 3-5 do so; and
- sequence 5 says the classic-protection body must equal “the recon baseline recorded in design.md
  section 4” (`github-governance.proposed.json:47-52`), but section 4 contains a summary table, not
  an exact sanitized object or baseline hash.

These should be exact machine comparisons with embedded/referenced reviewed bodies and hashes,
plus the current-main assertion required by Finding 1.

### Finding 4: minor but explicit checklist failure: removed-anchor terms remain in the evidence tree

No `"type": "workflows"` rule or `workflows` parameter remains in the JSON. The deleted transition
templates are gone. There is no normative external-anchor assumption in the surviving design.

However, the requested literal “zero residual references” check is not satisfied. Search found
historical `workflows`-rule, required-workflow, bootstrap-pin, immutable-transition, and trust-root
narrative in `design.md` at lines 12-28, 93-95, 182-185, 274-300, 374-379, and 753-761, plus the
“not a trust root” comment in `workflow-contexts.proposed.patch:6`. These passages reject the old
mechanism rather than reintroducing it, so this is not the reason for FAIL, but it is a direct miss
against the stated re-gate checklist.

## What passed

### Exact steady-state governance target

`github-governance.proposed.json` parses as schema v3 and carries exactly these main rules:

- `deletion`;
- `non_fast_forward`;
- `pull_request` with `allowed_merge_methods: ["merge"]`, zero approvals, and required thread
  resolution; and
- `required_status_checks` with strict policy, four contexts, and integration id `15368`.

Both rulesets have `bypass_actors: []`; `no-stray-branches` retains only its `creation` rule and the
exact `main`/`automation/**` exclusions. Repository settings disable squash and rebase. Classic
protection DELETE remains last, after the new ruleset, effective rules, no-stray ruleset, and merge
settings are read back and checkpointed (`github-governance.proposed.json:120-214`).

### v1 regression invariants

The state, mapping, and archive artifacts are byte-identical to v1:

- `STATE.proposed.json`: 57 records, 35 accepted records with both identities, 22 non-accepted
  records with both identities null;
- `mapping-ledger.proposed.json`: 35 entries, exact identity agreement with STATE, unchanged
  method distribution (`26 unique_patch_id`, `3 merge_payload`, `3 file_tree_at_published_commit`,
  `2 identity`, `1 merge_payload_with_file_tree_split`);
- all 35 proposed published identities remain ancestral to baseline main
  `498249777c453c1d551aeb01fc45420d8ca0a585`;
- `archive-manifest.proposed.json`: 33 heads plus 6 tags; all 39 source objects exist locally and
  all six local stash tags resolve to the manifest objects; and
- the workflow patch applies cleanly to current `main` (which is still the design baseline), and
  all resulting workflows pass `actionlint`.

The v1-to-v2 diff changes only `design.md`, the governance apply JSON, the workflow patch, and the
two deleted transition templates. No state, mapping, or archive weakening rode with the revision.

### D031 consistency

The residual-risk framing correctly states that owner-admin is the accepted root of trust and
that v2 is not self-protecting against that owner. It does not quietly depend on an organization,
enterprise plan, required-workflow anchor, or other external authority.

The inconsistency is narrower but important: statements that no governance change can land
unnoticed and that any owner change is detected (`design.md:66-72,182-190,721-724`) overstate what
final-state live comparison proves during the unbound drop-and-restore window. D031's false-
durability reopen trigger applies to that overclaim.

## Edge cases considered

- An unrelated merge between bootstrap and apply is harmless only if it leaves every protected
  workflow/contract byte intact. The design permits the merge without proving that condition.
- The bootstrap head check-run endpoint is viable: PR #38's head SHA returned its GitHub Actions
  check runs with app id `15368`. The problem is staleness, not endpoint shape.
- A new `pull_request` workflow may run from the PR merge branch; official GitHub documentation
  confirms PR workflows run against `refs/pull/<n>/merge`. No bootstrap failure is claimed merely
  because `governance-root.yml` is new.
- Current direct human access is owner-only and no competing governance PR was open. That is a
  transient observation, not a durable invariant, and it does not protect against operator error
  or candidate workflow capabilities.
- A scheduled fork-health run inside maintenance is expected to fail and would provide a useful
  alarm. A run after exact restoration does not prove no unintended merge occurred inside the
  window.
- The weak classic protection remaining until sequence 14 does not weaken the already-active
  ruleset; GitHub unions the layers. Its last-write deletion order remains sound.
- A bad owner can always rewrite the ruleset under D031. The findings concern additional design
  claims: absence safety, a bounded reviewed transition, and detection of what happened during the
  authorized window.

## Validation performed

- Parsed every v2 JSON artifact with Python.
- Asserted contiguous governance sequences 1-16, five reads before first write, exact rule types,
  exact required contexts/integration IDs, strictness, merge method, and empty bypass lists.
- Searched the full evidence tree for residual rule objects and removed-anchor terminology.
- Diffed `26eedab27..4d7a01691` by name, stat, unified diff, and word diff.
- Proved STATE/mapping/archive files byte-identical to v1 with `git diff --quiet` and recorded
  SHA-256 hashes.
- Rechecked all 35 published identities with `git merge-base --is-ancestor`.
- Rechecked all 39 archive source objects with `git cat-file`; rechecked all six local tag targets.
- Applied `workflow-contexts.proposed.patch` to an archive of `main`; ran `actionlint` over all ten
  resulting workflow files with zero diagnostics.
- Read live repository id, direct collaborators, Actions default workflow permissions, both
  ruleset bindings/bodies, classic protection summary, open governance-path PR inventory, and PR
  #38 head check runs. All probes were GET-only.
- Obtained an independent read-only adversarial review, which independently identified the same
  bootstrap race and repository-wide maintenance bypass.

## What I did not check

- I did not create even a disabled test ruleset or perform any other GitHub write. The remaining
  rule schema was inspected locally, not round-tripped through a live write API.
- I did not open a bootstrap or maintenance PR, so four-context emission for the proposed patched
  workflows was not observed live in this review.
- I did not re-run every historical patch-id, merge-payload, and per-file equality proof. Those
  artifacts are byte-identical to v1, whose gate verified them; I rechecked all published ancestry
  and archive source objects.
- I did not verify the private recovery remote with `ls-remote` or fresh-fetch `fsck`; this review
  performed the requested v1 regression check on the unchanged proposed manifest and local source
  objects only.
- I did not inspect GitHub audit-log retention or test whether it could supply a durable automatic
  record of every merge during the maintenance window. No reviewed artifact makes that log a
  control.
- I did not test an organization/enterprise required-workflow alternative; D031 explicitly places
  that outside the current design.

## Confidence: high

Finding 1 is a direct state-machine counterexample against an explicit R07 acceptance sentence.
Finding 2 follows from the design's own admission that checks are candidate-controlled, combined
with a repository-wide removal of the only governance-path-required context and no PR/SHA/main-tip
transaction binding. The independent reviewer reproduced both findings. The preserved artifact and
workflow checks are machine-verified and do not depend on either conclusion.

## Final verdict: FAIL
