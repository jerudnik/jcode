# S02: independent adversarial review at the exact commit

Reviewed source head: `efb730a9a694b1c855ff97d32caa089ccb27f150`
(branch `automation/s01-fix-1`, worktree clean at review time).
Deterministic runtime subject: `356476265ad6164970d2753f24da4dce9bdc89d5`.
Reviewer posture: read-only. The only file written by this node is this review.

## Verdict

**BLOCKED.**

Two blocking findings. Neither is a claim that the runtime work is wrong — the
deterministic matrix result itself reproduces and the external gates are
honestly labeled. Both are integrity defects in the *evidence*: at this exact
commit, a cited artifact does not exist in the tree, and an accepted node's
prose asserts an assertion the shipped fixture no longer makes.

A9 requires that every accepted node cite evidence and that an independent
reviewer report no unresolved blocker omission or false validation claim. These
two findings are exactly that class, so the signoff cannot be taken at this
commit. They are also both narrow and cheaply fixable.

## Findings, ordered by severity

### B1 (blocking). Cited gate-3 evidence is untracked and absent from the commit

`STATE.json` lists five evidence paths for `S01-FIX-1`. The fifth does not
exist at this commit:

```
$ git ls-tree -r HEAD --name-only docs/fork/ideal-base/evidence/S01-FIX-1/
docs/fork/ideal-base/evidence/S01-FIX-1/AMENDMENT.md          # only this
$ git cat-file -e HEAD:docs/fork/ideal-base/evidence/S01-FIX-1/gate3-sweep.log
fatal: path ... does not exist in 'HEAD'
$ git check-ignore -v docs/fork/ideal-base/evidence/S01-FIX-1/gate3-sweep.log
/Users/jrudnik/.config/git/ignore:25:*.log   docs/.../gate3-sweep.log
```

The file is present on disk (1183 bytes, mtime Aug 7 00:45) but is swallowed by
a **global** `*.log` ignore rule in the reviewer's personal git config — not by
anything in the repository. That is why it was never noticed: `git status` is
clean, and the author's own working copy shows the file.

Why this blocks rather than nags: `STATE.json`'s `S01-FIX-1` summary rests its
gate-3 claim on this artifact — "Gate 3 evidence: gate3-sweep.log,
run_lifecycle_matrix.sh 2 with remote builder active, 2 rounds, 18 PASS / 0
FAIL". At this commit that sentence cites a file no one else can read. Anyone
cloning the repo gets a node claiming a gate met, pointing at nothing. A9's
"every accepted node cites a commit, evidence path, validation output" is not
satisfied by a path that resolves only on one machine.

Note the same class of rule did **not** bite the S01 evidence directory: those
`.log` files are tracked because they were added before/despite the ignore
(ignore rules do not affect already-tracked files). So this is an isolated miss,
not systemic — which is also why it is worth fixing rather than rationalizing.

Fix: `git add -f docs/fork/ideal-base/evidence/S01-FIX-1/gate3-sweep.log`, and
add it to that directory's checksum manifest if one is intended.

### B2 (blocking). F03 is `accepted` while its README asserts an assertion the shipped fixture no longer makes

`S01-FIX-1` legitimately owns `evidence/F03/lease_class_fixtures.sh` (confirmed
in `WORK_GRAPH.json` `owned_paths`), so amending the fixture was in scope, and
the amendment is well-argued in `S01-FIX-1/AMENDMENT.md`. I agree with the
substance: design 4.1 classifies `ClientConnection` as C1 abandon-on-drain, so
asserting a full new idle window after release was testing something the
contract never promised. Removing that one assertion is correct.

What was not done is propagate the change to the accepted node's own claims.
`evidence/F03/README.md` was last touched at `cc76fd16f`, *before* the amendment
at `2c7ad7374`, and still reads:

- line 42: `| A. Hold/release per lease class (all 8 classes) | ... STILL alive
  4s after release (full-new-window assertion, review F03-I1); ... |`
- line 50: `"...exit after release": matrix row A, all 8 classes PASS.`

The shipped fixture contradicts this at `lease_class_fixtures.sh:152`:

```sh
if [ "$class" != "client-connection" ]; then      # full-window assert skipped
...
pass "[$class] post-release window not asserted (C1 abandon contract, design 4.1)"
```

And the transcript confirms 7-of-8, not 8-of-8 (`round-A.log:96`):

```
PASS: [client-connection] post-release window not asserted (C1 abandon contract, design 4.1)
PASS: [provider-turn] alive 4s after release (full new idle window enforced)
```

So an `accepted` node's summary table claims strictly more coverage than the
evidence at this commit provides. That is the "support claims exceed evidence"
pattern, and it is the more insidious of the two blockers, because the fixture
still prints `PASS` on that line — a reader skimming for `FAIL` sees green.

Mitigating, and the reason this is B2 rather than B1: nothing is hidden. The
amendment is committed, reasoned, contingency-tested (32-run separation), and
the fixture's own PASS string names the change and cites the contract. This is a
stale document, not a cover-up.

Fix: update `F03/README.md` rows to say 8 classes for held-past-timeout /
exit-44 / residue and 7 for the post-release window, cite the amendment, refresh
`F03/SHA256SUMS`. Whether F03 must return to `verifying` is the coordinator's
call; I do not think a re-run is needed, since the retained assertions are
unchanged and were exercised in both final rounds.

### O1 (non-blocking). Product changes on the branch touch files no node owns

Mapping `main...HEAD` against every `owned_paths` glob, five product files have
no owning node:

```
UNOWNED crates/jcode-app-core/src/agent/utils.rs
UNOWNED crates/jcode-base/src/auth/browser_policy.rs
UNOWNED crates/jcode-base/src/auth/copilot.rs
UNOWNED crates/jcode-base/src/login_qr.rs
UNOWNED crates/jcode-core/src/env.rs
UNOWNED scripts/ambient_roots_allowlist.txt
```

These are the env-flag truthiness consolidation (`51f8f7b9f`), which
`STATE.json` *does* disclose under `S01-FIX-1` ("env-flag truthiness
consolidation, 12 hand-rolled copies to one canonical
`jcode-core::env::flag_enabled`"). So it is declared, not smuggled. But the
protocol says workers "commit only their exact paths", and a 13-file
cross-crate refactor landing under a fix node whose `owned_paths` are three
entries is a scope stretch. Non-blocking because it is disclosed, mechanical,
and covered by both final rounds; worth a retro note on whether refactors
surfaced mid-node should get their own node.

### O2 (non-blocking). `controls.log` was captured against a dirty tree

`controls.log:2` records `warning: Git tree '/Users/jrudnik/labs/jcode' has
uncommitted changes`. This does not undermine the determinism claim: controls
D1–D4 test the *normalizer's* sensitivity against a frozen specimen file, not
the runtime, so tree state is irrelevant to what they prove. Recording it here
only because a reader auditing "clean state" claims will hit that line and needs
to know it was checked and found harmless. The two rounds that *do* carry the
determinism claim both pin the runtime commit in their headers
(`round-A.log:1`, `round-B.log:1`, both `commit=356476265...`).

### O3 (non-blocking). `round-A.log` was rewritten by the checkpoint commit

`git diff --name-status 356476265..efb730a9a` shows `round-A.log` as **M**, not
**A**. The pre-checkpoint version is a different run at a different source
commit (551 lines, `commit=1e356391e3bb...`, raw `1c0341ff...`). I checked
whether this was a silent swap of a bad round for a good one. It is not:
`README.md:25-28` states the final executions "were originally labeled E and F
while the harness was being stabilized; they are preserved here canonically as
`round-A.log` and `round-B.log`", and `cmp` confirms round-A ≡ round-E and
round-B ≡ round-F byte-for-byte. `FINDINGS.md:409` explicitly supersedes the
earlier pair and says why. The rename is disclosed and the superseded attempts
are documented rather than deleted. No finding beyond the observation that a
filename was reused for different content across commits, which makes
`git log -p` on that path misleading to a future reader.

## What I validated, and what it showed

Recomputed rather than read from summaries:

- **Tree state.** `git status --porcelain` empty; HEAD `efb730a9a` on
  `automation/s01-fix-1`.
- **Checksums.** From `evidence/S01/`, `shasum -a 256 -c SHA256SUMS` → all 15
  files **OK** (README, NORMALIZER_SPEC, PREDICTIONS, FINDINGS, normalize.py,
  controls.py, controls.log, prewarm.sh, s01_matrix.sh, round-A, round-B,
  NORMALIZED_SHA256SUMS, specimen-f14, fix1-matrix, repro-f03-cc). An earlier
  run of mine from the repo root reported failures; that was my own path error,
  not an evidence defect — recorded so the contradiction is not left dangling.
  `evidence/F03/SHA256SUMS` also verifies OK (3 files).
- **Round determinism.** Both rounds `N_STEP=18 N_FAIL=0` (`round-A.log:577`,
  `round-B.log:577`), 577 lines each, both headers pinning
  `commit=356476265ad6...`. One `FAIL` token in each file, and in both it is the
  literal `N_FAIL=0` counter — no failing step is present.
- **Round identity.** `cmp` A≡E, B≡F. A vs B differ raw only at line 1 char 5
  (the round label), consistent with the normalizer story.
- **Railway checker.** `python3 scripts/ideal_base_railway.py check` → exit 0,
  "9 roots, 66 child nodes, 75 state records, protected hash intact".
- **Node states.** 69 accepted, 2 implemented (S01, S01-FIX-1), 2 pending (S02,
  S03), 1 in_progress (W5), 1 superseded (F26-FIX-1). No mandatory deterministic
  node is left silently `pending` or `blocked`.
- **Gate honesty (A8).** G02 and G05 each carry a `BLOCKED.md` stating
  `authorization_blocked` with a named reason and next action; G02 additionally
  has `VERIFIED.md` after authorization was granted, and that sweep *reports a
  real provider defect* plus 3 failures rather than claiming clean. G01 took the
  documented downgrade branch and says so in its title. No blocked gate is
  described as passing. **A8 is satisfied.**
- **Evidence tracking.** `git ls-tree` on `evidence/S01-FIX-1/` → B1 above.
- **Ownership.** Every `main...HEAD` non-evidence file mapped against all
  `owned_paths` globs → O1 above. Confirmed `S01-FIX-1` *does* own the F03
  fixture, correcting my own initial suspicion that the amendment was
  out-of-scope.
- **Predictions.** P1–P5 scored in `README.md:58-67`. P1/P2/P5 held on the final
  pair; P3 and P4 are recorded as "confirmed by superseded pairs", i.e. the
  predictions that anticipated failure are marked confirmed *because* they
  failed first and were repaired, with the frozen normalizer explicitly not
  widened. That is the honest direction of fit.
- **Self-correction present.** `FINDINGS.md:179` records a correction where a
  reconcile filter exited 0 against an earlier observation of 97. Evidence
  against green-washing.

## Edge cases I considered

- Whether the superseded C/D rounds were quietly dropped: no, documented in
  `FINDINGS.md` with hashes and the prewarm root cause.
- Whether the A/B relabel hid a failing round: no, byte-identical to E/F.
- Whether the F03 amendment weakened a real safety assertion: no, it removed an
  assertion the design contract never made; held-past-timeout, exit-44, and
  residue are retained for all 8 classes.
- Whether the `*.log` ignore silently ate other evidence: checked, S01's logs
  are tracked; the miss is isolated to `S01-FIX-1`.
- Whether `controls.log`'s dirty-tree warning invalidates determinism: no, the
  controls test the normalizer against a frozen specimen.

## Open questions for the coordinator

1. Does fixing B2's README require F03 to re-enter `verifying`, or is a
   documentation correction with a `SHA256SUMS` refresh sufficient? My read is
   the latter, but the disposition is not mine to set.
2. Should `evidence/S01-FIX-1/` carry its own `SHA256SUMS` once
   `gate3-sweep.log` is tracked, matching the S01 convention?
3. Is the O1 refactor scope acceptable post-hoc, or should it be retro'd into a
   node of its own?

## What I did not check

- **I did not re-run the deterministic matrix.** Every round claim above is read
  from committed transcripts and their checksums, not independently reproduced.
  A transcript that was fabricated wholesale and then self-consistently hashed
  would pass everything I did.
- **I did not build the tree or run any test suite.** No `cargo`, no
  `run_lifecycle_matrix.sh`, no `nix build`. Compilation at this commit is
  unverified by me.
- **I did not read `gate3-sweep.log`'s contents** to confirm it says what
  `STATE.json` claims. B1 is about its absence from the commit, not its
  contents; I deliberately did not launder an untracked file into evidence by
  reading it off disk.
- **I did not verify the G02 provider-doctor numbers** against provider APIs;
  live-credential claims are taken as recorded.
- **I did not audit all 69 accepted nodes.** I audited the gate nodes (G01, G02,
  G05), F03, S01, and S01-FIX-1 in depth, and sampled state records for the
  rest. A stale-evidence defect of the B2 kind could exist in an unaudited node.
- **I did not check the F14 restoration byte-identity claim** beyond the
  transcript's own assertion.
- **I did not evaluate `POST_DISTRIBUTION_ORCHESTRATOR_PLAN.md`**, which is
  outside the signoff question.
- **I did not verify documentation-checker or APM-generation claims** in A9
  (Markdown links, `.apm/` primitive parity).

## Confidence

**Medium-high** on the two blocking findings: both are mechanically verifiable
(`git ls-tree`, `git check-ignore`, and a direct file-vs-prose contradiction at
cited line numbers), and neither depends on judgment about the runtime work.

**Medium** on the verdict as a whole. The blockers are narrow and fixable, and
the underlying engineering looks sound and unusually honest about its own
failures — the superseded-round documentation and the reconcile-filter
correction are both things a green-washing author would have deleted. My
confidence is capped by not having re-run the matrix and not having audited all
69 accepted nodes; a deeper sweep could find more of the B2 pattern.
