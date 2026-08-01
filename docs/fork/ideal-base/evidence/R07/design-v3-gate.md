# R07 design v3 gate: adversarial re-review

Reviewed `automation/r07-design` at `c27b1d8b2` (local, unpushed) from an isolated worktree,
against:

- R07 in `docs/fork/ideal-base/WORK_GRAPH.json` (`all_nodes` list entry `R07`);
- D031 in `docs/fork/ideal-base/DECISIONS.md`;
- the v1 gate (`automation/r07-design-gate:.../design-gate.md`, FAIL);
- the v2 gate (`automation/r07-design-v2-gate:.../design-v2-gate.md`, FAIL); and
- the complete `4d7a01691..c27b1d8b2` diff.

All GitHub probes in this review were GET-only via ambient `gh` auth. No ruleset, branch
protection, pull request, ref, workflow, or repository setting was written. No write token was
used; no write probe was needed, because every finding below is demonstrable read-only.

## Verdict: FAIL

v3's two headline repairs are the right *shape* — an immediately-pre-write assertion against
current `main` (sequence 6), and a transaction-bound maintenance procedure (§4 steps 1-8) — and
the v2 gate's finding 3 (exact comparisons) and finding 4 (residual framing) are genuinely
resolved. But both headline repairs rest on live GitHub API behavior that the author did not
test, and both are broken as written against the real API:

1. **Sequence 6's protected-path assertion fails open.** The compare API truncates `files` at 300
   entries with no truncation flag and no pagination for that array. On this repository I
   reproduced a compare whose true diff touched 875 files including `scripts/fork-health.sh` and
   `scripts/ideal_base_railway.py` — both protected paths — and the API response's `files` array
   omitted both. Sequence 6 would have passed. That is exactly the v2 finding-1 counterexample
   surviving into v3, now hidden behind an assertion that *looks* executable.
2. **Sequence 6 misses renames.** The compare API reports a moved file with `filename` = the new
   path and `previous_filename` = the old path. Sequence 6 asserts only on `filename`. Moving
   `.github/workflows/fork-ci.yml` to `ci-fork.yml.disabled` produces a `filename` outside every
   protected prefix, so the assertion passes while a required context's definition has been
   removed from `main`. design.md:58 explicitly names "rename" as the attack the fix must catch.
3. **Maintenance step 7's exactly-one-commit proof is unsatisfiable.** `GET
   compare/{expected_base_sha}...refs/heads/main` returns every commit in the range, not just the
   merge commit. A one-commit PR merged as a merge commit yields two entries. Verified against
   four real merges on `main`: 2, 4, 9, and 15 commits. Step 7 as written stops on every
   legitimate maintenance run, which means in practice it will be ignored or "fixed" ad hoc — it
   is not the executable no-intervening-merge proof the v2 gate required.

Any one of these is blocking on its own. Together they mean the v2 gate's two blocking findings
are not actually closed: finding 1's lockout counterexample is still constructible, and finding
2's window still lacks a working proof of what merged inside it.

## Findings

### Finding 1 (blocking): sequence 6's `files` assertion fails open past 300 files

`github-governance.proposed.json` sequence 6 asserts "the response's `files` array contains zero
entries whose filename starts with any prefix in `template_variables.protected_paths`."

GitHub's compare endpoint caps `files` at 300 entries. Unlike `GET
/repos/{owner}/{repo}/commits/{sha}` — whose documentation explicitly states that past 300 files
"the response will include pagination link headers for the remaining files, up to a limit of 3000
files" — the compare response provides no such affordance for `files`.

Reproduction (read-only, this repository):

```
$ gh api "repos/jerudnik/jcode/compare/04fa8a3922674c1a71579ccb5956da36e23f06c6...refs/heads/main" \
    --jq '{total_commits, nfiles:(.files|length)}'
{"nfiles":300,"total_commits":222}

$ git diff --name-only 04fa8a39 498249777 | wc -l
875
```

The response's top-level keys are exactly `ahead_by, base_commit, behind_by, commits, diff_url,
files, html_url, merge_base_commit, patch_url, permalink_url, status, total_commits, url` — there
is no `truncated`, `incomplete_results`, or equivalent flag. `per_page`/`page` paginate `commits`
but return an empty `files` array on pages 2+:

```
page=1: {"nc":100,"nf":300}
page=2: {"nc":100,"nf":0}
page=3: {"nc":22,"nf":0}
```

No `Link` header is emitted for this response.

Which protected paths got dropped in that reproduction:

```
protected-path changes in TRUE diff:
  .github/workflows/{ci,docs-impact,fork-ci,fork-health,freebsd-smoke,ios-testflight,
                     nix-update,nix,release,security,sync,windows-smoke}.yml
  scripts/fork-health.sh
  scripts/ideal_base_railway.py
present in API's 300:   the 12 .github/workflows/* entries
MISSING from API:       scripts/fork-health.sh, scripts/ideal_base_railway.py
```

The truncation is not random: `files` is returned in lexicographic path order (verified: the
returned list equals its own sort). The cutoff in the reproduction fell at
`crates/jcode-background-types/Cargo.toml`. That makes the failure mode *attacker-orderable*, not
merely unlucky: `.github/` sorts first and is relatively safe, but `scripts/`, `tests/`, and
`docs/fork/ideal-base/evidence/R07/github-governance.proposed.json` all sort after `crates/`, and
this repository has ~2600 tracked files with a large `crates/` tree. A PR (or an accumulation of
ordinary PRs) that touches enough files sorting before `scripts/` pushes every `scripts/`-side
protected path out of the window. Under the design's own threat model — where `fork-health.sh` is
the live comparator and `ideal_base_railway.py` is the validator — those are precisely the files
whose silent modification matters most.

Consequence against the R07 acceptance gate: sequence 6 can pass while a required workflow has
been deleted or path-filtered on current `main`, and sequence 7 then requires a context that
cannot be emitted. That is "a context that can be absent is never required" violated by the same
mechanism v2 was failed for, with the added hazard that v3 presents it as closed.

A correct assertion cannot be built on this endpoint's `files` array at all. Workable
alternatives (not prescriptions, but existence proofs that a fix is available): compare the two
commits' *tree* SHAs for each protected path via `GET /repos/{o}/{r}/contents/{path}?ref={sha}`
or the git-trees API, which returns per-path blob SHAs and supports a `truncated` flag; or fetch
the two commits locally and run `git diff --quiet <base> <head> -- <protected paths>`, which is
the same computation `governance-root.yml` already performs and has no cap.

### Finding 2 (blocking): sequence 6 does not consider `previous_filename`, so renames evade it

The compare API represents a rename with `status: "renamed"`, `filename` set to the **new** path,
and `previous_filename` set to the old path. Verified read-only on a real rename in this
repository's history:

```
$ git show --name-status --find-renames --format= e59985449
R100  docs/fork/NEXT_SESSION_KICKSTART.md  docs/archive/NEXT_SESSION_KICKSTART.md

$ gh api "repos/jerudnik/jcode/compare/e59985449~1...e59985449" \
    --jq '[.files[]|{status,filename,previous_filename}]'
[{"filename":"docs/archive/NEXT_SESSION_KICKSTART.md",
  "previous_filename":"docs/fork/NEXT_SESSION_KICKSTART.md",
  "status":"renamed"}]
```

Sequence 6 tests only `filename`. Counterexample, fully within v3's stated threat model:

1. bootstrap PR merges; all four contexts emitted; `bootstrap_merge_sha` recorded.
2. Before the apply, an intervening PR renames `.github/workflows/fork-ci.yml` to
   `ci/fork-ci.yml.bak` (or any path outside the six protected prefixes). `Governance Root` is not
   yet required, so nothing blocks the merge — the design explicitly permits intervening merges.
3. Sequence 6 runs: the entry's `filename` is `ci/fork-ci.yml.bak`, which matches no protected
   prefix. The assertion passes.
4. Sequence 7 writes the ruleset requiring `Fork CI Gate`. That workflow no longer exists at a
   path GitHub will run on `pull_request`. The context can never be emitted. Every subsequent PR
   is unmergeable, and the ruleset that locks the repository is itself only changeable by the
   owner-admin.

This is a hard lockout, not a detection gap. design.md:58 names rename explicitly as one of the
three things the fix must catch ("remove, rename, or path-filter"), so this is a failure against
the design's own stated requirement, not an outside objection.

The same omission applies to `governance-root.yml`'s own check in a narrower way, but there
`git diff --name-only <base> HEAD -- <protected>` does list the deleted old path, so the audit
gate would go red; sequence 6 is where the rename actually escapes.

### Finding 3 (blocking): maintenance step 7's "exactly one commit" assertion can never pass

design.md §4 step 7: "`GET repos/jerudnik/jcode/compare/{expected_base_sha}...refs/heads/main`
and assert `commits` has exactly one entry and its `sha` equals `merge_sha` from step 5."

The compare endpoint returns all commits in the range, including the merged PR's own branch
commits, not only the merge commit. Verified against four real merges on `main`:

```
merge 498249777  commits in base..merge = 4
merge 8d851ea72  commits in base..merge = 2   (PR branch had exactly 1 commit)
merge 78a08e4d4  commits in base..merge = 9
merge 3db42db1f  commits in base..merge = 15
```

API confirmation for the minimal case:

```
$ gh api "repos/jerudnik/jcode/compare/287af80f5...8d851ea72" --jq '[.commits[].sha]'
["50f40588f0826890a9a78ed54a092ee5f937290e","8d851ea7240731551812f7a0190f52bb075ae654"]
```

The floor is two (one branch commit plus the merge commit), and only if the PR is a single commit
and merge-commit strategy is used, which the ruleset mandates. So the assertion as written fires
"governance incident, requires out-of-band investigation" on **every** correctly-executed
maintenance run. An assertion that always fails is not fail-closed; it is noise that will be
suppressed, and it leaves the v2 gate's finding-2 requirement ("proof that no other PR merged in
the window") unmet in practice.

The correct invariant is expressible — e.g. assert the range contains exactly one *merge* commit
and that it equals `merge_sha`, or assert `commits[-1].sha == merge_sha` together with
`merge_sha`'s two parents being `expected_base_sha` and `head_sha` (step 5 already proves the
parents, which is what actually pins the range) — but v3 does not say that. As written the
executable proof the gate asked for does not exist.

### Finding 4 (material): the embedded SHA-256 hashes depend on an unstated canonicalization, and sequence 5's literal comparison does not match a healthy live response

All three embedded hashes are correct, but only under one specific serialization, which the
document never states. I recovered it by brute-forcing 32 combinations of `sort_keys`,
separators, indent, and trailing newline; exactly one reproduces all three:

```
json.dumps(obj, sort_keys=True, separators=(',',':'))   # no indent, no trailing newline
seq 3 -> 8440214dee8621d8a12a9456083a1c3afc82442291fd8a67ddcea7852d239124  ✓
seq 4 -> 1376e3835feca779dd1dd2387e7cb5e1095f34c6de71ae64483c17e52823f99f  ✓
seq 5 -> d20823253081aca9537b632d1b8605a72d8838f520fe0d14defa7dc2d76b4704  ✓
```

The design says "byte-identical (compared as parsed JSON, key order insensitive)", which is
self-contradictory (byte identity is not key-order insensitive) and does not pin the encoder.
Any operator using `jq -S -c` (which emits `,`/`:` without spaces and *with* a trailing newline)
gets a different hash and a spurious stop. This is a reproducibility defect in a document whose
entire purpose is to be machine-checkable by someone other than its author.

More seriously, sequence 5's assertion is wrong as literally written. It says "the response is
byte-identical ... to the embedded object below" — the *response*, with no sanitization step,
unlike sequences 3 and 4 which name their strip list. The live response is not equal to the
embedded object:

```
seq 5 literal parsed-equal: False
  live-only top-level key:  url
  differ: enforce_admins           (live has extra subkey: url)
  differ: required_signatures      (live has extra subkey: url)
  differ: required_status_checks   (live has extra subkeys: url, contexts_url)
  hash(live as-is)  = 14852c2ae440441b9e92...
  expected          = d20823253081aca9537b...
```

Stripping `url` and `contexts_url` recursively makes it match exactly and reproduces the embedded
hash. So the intended comparison is sound; the written one stops the apply on a perfectly healthy
repository. Sequences 3 and 4 *do* match live once their stated strip list is applied — I verified
both against the live API, parsed-equal `True` and hash-equal — so the defect is specific to
sequence 5's missing sanitization clause.

### Finding 5 (material): protected-path list omits `.github/scripts/`, which executes inside required workflows

`template_variables.protected_paths` and `governance-root.yml`'s `protected` array both cover
`.github/workflows` but not `.github/scripts`. That directory exists and its contents are executed
by the workflow that produces a required context:

```
$ ls .github/scripts
run_with_timeout.py
$ grep -rn "\.github/scripts" .github/workflows/ | wc -l
(many; e.g. fork-ci.yml:347,354,369,378,383,389,397,434,441
 — `python3 .github/scripts/run_with_timeout.py ...`)
```

`fork-ci.yml` itself already treats `.github/scripts/**` as a change-relevant path
(`fork-ci.yml:81`). A PR that modifies `run_with_timeout.py` alters what the `Fork CI Gate`
context actually executes and reports, without touching any path `Governance Root` guards or
sequence 6 asserts on. Under the design's own framing — the audit gate detects "an unreviewed
governance-path change on the PR that makes it" — this is an uncovered governance path.

Severity is material rather than blocking because it degrades detection rather than creating a
lockout, and because D031 accepts the owner-admin as root of trust. But it is a real coverage gap
in a list the design presents as complete, and it interacts with Finding 1: `.github/scripts/`
sorts adjacent to `.github/workflows/`, so it would be cheap to add.

### Finding 6 (minor): residual TOCTOU between sequence 6 and sequence 7 is real but small, and is not acknowledged

Sequence 6 reads current `main`; sequence 7 writes the ruleset. A PR merging between those two
API calls is not covered, and nothing in the document serializes them (no expected-SHA
precondition on the write, no re-read after the write). Grep for `serializ`, `TOCTOU`, or `race`
in the v3 artifacts finds no acknowledgement of this window.

I rate this minor, not blocking, because: the window is seconds, the operator controls when it
opens, and the failure mode (a required context becomes absent) is detected on the next PR rather
than being silent. But the v2 gate explicitly asked to "serialize the final assertion and
activation so no intervening merge is treated as harmless," and v3 neither serializes them nor
says why it does not need to. A cheap mitigation exists: re-run sequence 6 immediately after
sequence 8's read-back and treat a change as a rollback trigger, since the ruleset can still be
reverted at that point.

## What v3 genuinely fixed

- **v2 finding 3 (exact comparisons): resolved.** Sequences 3 and 4 now embed full sanitized
  bodies with hashes, and both verify against the live API today (parsed-equal `True`, hashes
  reproduce). Sequence 2 now asserts `Governance Root` concludes `failure` on the bootstrap head,
  closing the gap between design.md's prose stop-condition and the executable document. Sequence
  5 embeds an exact object instead of pointing at a prose table (though see Finding 4 for its
  sanitization defect).
- **v2 finding 4 (residual framing): resolved.** §0 and §0a are explicitly labelled history; the
  detection overclaims the v2 gate named are qualified — §3 now says "None of these is a
  continuous audit log, and none proves that every historical governance change..." and §12 says
  detection is "sample-point" rather than instantaneous. The `workflows`-rule/trust-root/
  immutable-transition mentions that remain are all inside explicit history framing or explicit
  rejection.
- **Honest disclosure of the maintenance residual.** §4's "What this does and does not close"
  correctly states that a second ordinary PR can merge during the drop and is only detectable,
  not preventable. That framing is acceptable under D031 *in principle* — the owner-admin is the
  root of trust and controls the window. It is not acceptable *as delivered*, because the
  detection it relies on is step 7, and step 7 does not work (Finding 3). If step 7 is repaired,
  I would accept this residual as correctly scoped rather than overclaimed.

## Regression checks: all clean

- `STATE.proposed.json`, `mapping-ledger.proposed.json`, `archive-manifest.proposed.json` are
  byte-identical to v2 (`4d7a01691`), confirmed by SHA-256:
  `e1c4e8bb...`, `88a1fdfd...`, `20f1a0dc...` — unchanged.
- The `4d7a01691..c27b1d8b2` diff touches exactly three files: `design.md` (+323/-120 overall),
  `github-governance.proposed.json`, and `workflow-contexts.proposed.patch`. No state, mapping,
  or archive weakening rode along.
- The workflow-patch change is a 4-line comment rewrite in `governance-root.yml`'s header only;
  no executable line changed.
- The patch still applies cleanly to current `main` (`git apply --check` plus a real apply in a
  disposable worktree), producing `governance-root.yml` alongside the nine existing workflows.
- Steady-state ruleset shape unchanged from v2 and correct against the R07 contract: `deletion`,
  `non_fast_forward`, `pull_request` (0 approvals, thread resolution required,
  `allowed_merge_methods: ["merge"]`), `required_status_checks` (strict, 4 contexts, all
  `integration_id: 15368`); `bypass_actors: []` on both rulesets; `no-stray-branches` keeps only
  `creation` with the exact `main`/`automation/**` exclusions; repository PATCH disables squash
  and rebase; classic-protection DELETE remains last.
- Governance JSON is valid, `schema_version` 4, steps 1-17 contiguous with no gaps or duplicates,
  first write at sequence 7, all six preflight reads before it, checkpoint at 14 gating the
  DELETE at 15, and `abort_policy` updated consistently ("No write may run before sequences 1-6
  have all passed. Never continue to sequence 15 unless sequence 14 passed.").

## Edge cases considered

- **Is the 300-file cap actually reachable in practice here?** Yes. The repository has ~2600
  tracked files; a 222-commit range produced 875 changed files. The bootstrap-to-apply window is
  intended to be short, which reduces but does not eliminate exposure — and the same sequence 6 is
  the design's answer for a *re-run* after a legitimate maintenance window, where the range can be
  arbitrarily long. The design places no bound on how far `bootstrap_merge_sha` can lag `main`.
- **Does `refs/heads/main` work as a compare head?** Yes, verified unencoded; that part of
  sequence 6 is fine.
- **Does `merge_base_commit.sha == bootstrap_merge_sha` correctly detect a rewrite?** Yes, that
  sub-assertion is sound and I found no counterexample.
- **Could the hash canonicalization be inferred as "obvious"?** No. `jq -S -c`, Python's default
  `json.dumps`, and `sort_keys=True` with default separators all give three different digests. A
  document that stops the apply on mismatch must state the encoder.
- **Is step 7's flaw just imprecise prose?** I considered reading "commits" as "merge commits."
  The text says "`commits` has exactly one entry," naming the response field directly, and the
  surrounding sentences ("If more than one commit appears, stop") reinforce the literal reading. A
  reviewer executing this document as written gets a false stop every time.
- **Does the rename gap also break `governance-root.yml`?** No — `git diff --name-only <base> HEAD
  -- <protected>` lists the removed old path, so the audit gate goes red. Only sequence 6 is
  fooled. That asymmetry is worth noting because it means the rename attack requires the
  bootstrap-to-apply window specifically, which is exactly the window v2 finding 1 was about.
- **Does D031 excuse any of these?** No. D031 accepts the owner-admin as root of trust, i.e. it
  excuses the *absence of prevention* against that owner. Findings 1-3 are failures of
  *detection and safety* against ordinary merges and against the operator's own tooling, which is
  what R07's acceptance gates require and what D031 explicitly does not repeal.
- **Blast radius if executed anyway:** Findings 1 and 2 produce a repository lockout that only the
  owner-admin can undo (by rewriting the ruleset). Finding 3 produces a false alarm, not damage.
  Finding 4's sequence 5 defect fails closed (spurious stop before any write), which is the safe
  direction.

## Validation performed

- Parsed all JSON artifacts; confirmed step numbering contiguous 1-17, kinds/methods/endpoints
  enumerated, first write at 7, checkpoint at 14, DELETE at 15.
- Brute-forced 32 JSON serialization variants to recover the canonicalization reproducing all
  three embedded hashes; confirmed exactly one matches.
- Live GET of both rulesets; applied the document's stated strip list; confirmed parsed-equality
  and hash-equality with the embedded bodies for sequences 3 and 4.
- Live GET of classic protection; confirmed literal inequality with sequence 5's embedded object,
  identified the exact extra keys, and confirmed that recursive `url`/`contexts_url` stripping
  restores both equality and the embedded hash.
- Live compare probes: a 222-commit / 875-file range (300-file truncation, no flag, no Link
  header, empty `files` on pages 2+, lexicographic ordering, two protected paths dropped); a
  small range (sanity); `refs/heads/main` as an unencoded compare head.
- Live compare of a real rename commit, confirming `filename`/`previous_filename` semantics.
- `git rev-list --count` and live compare on four real `main` merges, establishing the
  commits-in-range floor of two.
- Cross-referenced GitHub's REST commits documentation, which states the 300-file/3000-file
  pagination behavior for *Get a commit* and provides no equivalent for *Compare two commits*.
- Diffed `4d7a01691..c27b1d8b2` by name-status and full unified diff; SHA-256 compared the three
  preserved artifacts across both commits.
- Applied `workflow-contexts.proposed.patch` to a disposable worktree of `main`; confirmed clean
  apply and the expected ten-workflow result.
- Grepped the evidence tree for removed-anchor terminology and confirmed every hit is inside
  explicit history/rejection framing.
- Enumerated `.github/scripts` and its references from required workflows.

## What I did not check

- I ran no write probes. Every finding above is demonstrable read-only, so a disabled test
  ruleset would have added nothing; the v1 gate already established that the surviving rule types
  round-trip. The steady-state ruleset body is therefore still not field-by-field round-tripped
  through a live write, same caveat as the v1 and v2 gates.
- `actionlint` is not installed on this machine and Nix is unavailable here, so I did not re-run
  it over the patched workflows. The v2 gate ran it clean, and the v3 patch changes only comment
  lines, so I judge the risk negligible — but I did not independently re-confirm it, and PyYAML
  was also unavailable for a syntax-only fallback.
- I did not open a bootstrap or maintenance PR, so four-context emission for the patched
  workflows remains unobserved live (same as v2).
- I did not re-verify the historical patch-id / merge-payload / file-tree equivalence proofs
  behind the mapping ledger; those artifacts are byte-identical to v2 and v1, whose gates checked
  them.
- I did not verify the private recovery archive remote with `ls-remote` or a fresh-fetch `fsck`.
- I did not determine whether the compare endpoint's 300-file cap is documented anywhere as a
  stable contract versus an undocumented implementation limit. It reproduces consistently here,
  and the absence of a truncation flag is the operative problem either way, but a design that
  depends on this endpoint should not depend on the cap's exact value.
- I did not enumerate every path executed by every required workflow, so Finding 5 may not be the
  only protected-path coverage gap; `.github/scripts/` is the one I found by inspection.
- I did not evaluate whether GitHub's audit log could supply the no-intervening-merge proof
  Finding 3 needs. No reviewed artifact makes it a control.

## Required to reach PASS

1. Replace sequence 6's `files`-array assertion with a mechanism that has no silent truncation
   and that compares per-path content identity (tree/blob SHAs, or a local `git diff --quiet` over
   the protected paths). Handle renames explicitly, whichever mechanism is chosen.
2. Repair maintenance step 7 into an assertion that passes on a correct run and fails on an
   intervening merge (e.g. exactly one merge commit in the range equal to `merge_sha`, combined
   with step 5's existing two-parent proof).
3. State the hash canonicalization explicitly, and add the sanitization clause to sequence 5 so
   its comparison matches a healthy live response.
4. Add `.github/scripts/` to both protected-path lists, or state why it is excluded.
5. Either serialize sequence 6 with sequence 7, or state the residual window and why it is
   accepted (Finding 6).

Items 1 and 2 are blocking. Items 3-5 are the difference between PASS and PASS-WITH-FIXES once
1 and 2 land.

## Confidence: high

Findings 1, 2, and 3 are each reproduced against the live GitHub API on this exact repository,
with concrete commands and outputs, and each contradicts an explicit claim the design makes about
itself. Finding 4's canonicalization is established by exhaustive enumeration and its sequence-5
mismatch by direct live comparison. Finding 5 is established by direct file listing and grep. The
regression checks are machine-verified and independent of all findings.

## Final verdict: FAIL
