# Normalization Infrastructure: Operational/Safety Re-Review (Opus, fresh)

Read-only re-review of the corrected normalization authority infrastructure.

## Fixed refs

- Reviewed commit: `1f938b7e537a20aaad133ec300d0cfdc6368bca0`
  ("docs(fork): define post-recovery normalization")
- Parent (pre-infrastructure head): `cdc2cc2b4cea51c185de330c8e15e08615acc46c`
- Repo HEAD at review: `1f938b7e537a20aaad133ec300d0cfdc6368bca0` (branch `recovery/2026-07-15`)
- `main`: `6ca1fcf2ec2366c7abc99664a485c40d60cec80e`
- `vendor/upstream`: `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`
- Authority commit derived from `COMPLETION_STANDARD.md` history: `1f938b7e...` (self-consistent)
- Files reviewed (committed active authority only):
  `docs/fork/normalization/{README.md, BASELINE.md, COMPLETION_STANDARD.md, COORDINATOR_BRIEF.md}`
- Excluded by instruction and honored: everything under `docs/fork/normalization/reviews/`.
  I did not read, open, or infer any prior reviewer's conclusions.

## Verdict

**PASS.** Zero unresolved CRITICAL or IMPORTANT findings. The infrastructure
defines a safe, read-only-first program in which no destructive, credentialed,
live-runtime, `main`-moving, daemon-repointing, or remote-write action can occur
without an explicit approval packet plus a recorded, dry-run-exercised rollback.
Every committed host and Git fact I could independently observe matches live
read-only state exactly.

## Required-item verification (from live facts + committed text)

| Required assurance | Result | Evidence |
|---|---|---|
| All four stash commit + index objects explicitly archived, not via `--all` | PASS | BASELINE gives a per-entry `refs/archive/.../{worktree,index}` update-ref loop and a *separate* `jcode-stashes.bundle`; COMPLETION_STANDARD D0 states `git bundle --all` alone is insufficient because `stash@{1..3}` are reflog-only. Live proof: `stash@{1..3}` commits and `stash@{1..3}^2` index parents are NOT reachable via `git rev-list --all`; only `stash@{0}` and its index are. |
| Restoration-tested, not trust-on-verify | PASS | D0 requires both bundles pass `git bundle verify` AND are restoration-tested in a disposable repo with all four stash commits re-listable; N0 step 4 repeats this. |
| All refs + 916 non-recovery commits archived before deletion | PASS | BASELINE + D0/D2 require a verified all-ref bundle containing branch/tag tips including commits not reachable from recovery; live `git rev-list --all --not recovery/2026-07-15 --count` = 916 (matches). Deletion is gated behind bundle verification. |
| Both PATH binaries classified; HM/Nix state not hand-mutated | PASS | BASELINE classifies `~/.local/bin/jcode` (agent-managed symlink chain) and `/etc/profiles/per-user/jrudnik/bin/jcode` as home-manager-declarative "not agent-removable"; D7 forbids direct edit/delete, requires declarative-source change + approval. Live `type -a` confirms both, HM link -> `/nix/store/w2wbi1jjm21hjqb9l920c5ph2m733g6n-home-manager-path/bin/jcode` (matches). |
| Menubar/hotkey/shared-server/com.jcode.hotkey retained or gracefully restarted | PASS | BASELINE + D6/D7 + N4/N5 mark them retain/preserve-and-restart; explicit "no `kill -9`" and supported-lifecycle restart. Live: all three processes running and `com.jcode.hotkey` LaunchAgent state=running (matches). |
| lesson-library-shadow + `.wrangler` out of scope | PASS | BASELINE + D7 mark `com.jcode.lesson-library-shadow` (a `/usr/bin/python3` job) and `.wrangler/` out of scope; name/path glob classification forbidden. Live: agent program is `/usr/bin/python3`, logs under `.wrangler/` (matches; confirms it is not jcode runtime). |
| Sandbox paths cannot collide with runtime | PASS | D6 + N4 require a pre-start collision check proving disjoint home, runtime, socket, port, pid, marker paths from every pre-existing jcode process/integration before any daemon start. |
| Current/shared links: exact targets, hashes, restore commands | PASS | BASELINE records both link targets, both SHA-256 anchors, and an exact `ln -sfn`/`shasum -a 256` restore core. Live: targets, and hashes `fd6297d9...`/`a4973e8c...` match exactly; restore then graceful integration restart required by D7/D8/N5. |
| No destructive/main/credential/provider/daemon/remote action without approval packet | PASS | README mutation rule + D0/D8 + COORDINATOR safety rules require dry-run manifest, approval, backup, post-check, tested restore; upstream fetch is read-only (no delete/merge/rebase/replay/push); no force/remote push without separate explicit instruction; inventory and deletion never combined. |

## Correction matrix (prior blocking themes -> current committed state)

The infrastructure text itself enumerates the drivers of the prior FAILs; I
verified each is now materially addressed in the committed active files (I did
this from README's own summary and the D-gate text, without reading the reviews):

| Prior theme | Now addressed by | State |
|---|---|---|
| Tracked-authority requirement | README/BASELINE/COORDINATOR: all four files tracked, authority commit derived from `COMPLETION_STANDARD.md` history and must be reachable from HEAD | RESOLVED (live: 4 files tracked, authority = HEAD) |
| Explicit stash-object + all-ref archives | BASELINE archive loop + D0 dual-bundle + restoration test | RESOLVED (live `--all` gap proven) |
| N1/N2 promotion ordering (`main` not moved in N1) | README milestones, D2, N1/N2 explicitly make `main` fast-forward the N2 exit criterion | RESOLVED |
| Exact host/runtime classifications | BASELINE binaries/processes/agents + D7 | RESOLVED (all live facts match) |
| Live-binary rollback anchors | BASELINE hashes + `ln -sfn` core + D7/D8 dry-run | RESOLVED (hashes match) |
| Evidence placement | README + D2 bounded `docs(evidence)` commits at stack top | RESOLVED |
| Remote-state labeling | BASELINE remotes + D9 "local-canonical, remote pending" | RESOLVED |
| Independent final-review rules | D9 two fresh reviewers, no self-review, zero CRITICAL/IMPORTANT | RESOLVED |
| PM-surface hook rejection (checkbox/tracking-named files) | Corrected into normative `COMPLETION_STANDARD.md` + operational `COORDINATOR_BRIEF.md`; no history movement | RESOLVED |

## New findings

No CRITICAL or IMPORTANT findings.

- INFORMATIONAL-1 (silent-loss edge, already covered): `stash@{3}`
  (`29d49b25...`) is a `-u` stash with a **third parent** `7c68ef5f5935...`
  holding its untracked-file payload. This object is NOT reachable via
  `git rev-list --all`. The BASELINE archive pattern references only
  `stash@{i}` (worktree) and `stash@{i}^2` (index). I verified the 3rd parent
  IS transitively reachable from the `stash@{3}` commit, so bundling the
  `.../3/worktree` archive ref captures it, and `git bundle verify` enforces
  prerequisite completeness. No loss occurs. Optional hardening: the restoration
  test in D0/N0 asserts "all four stash commits can be re-listed" but does not
  explicitly assert recovery of the untracked payload; adding a one-line check
  that `7c68ef5f` restores from the stash bundle would make the guarantee
  self-evident. Non-blocking.

- INFORMATIONAL-2 (snapshot drift, expected): BASELINE records `main...recovery`
  as 166 unique-to-recovery; live is now 167, and `rev-list --all --not recovery`
  is 916 (matches). The +1 is exactly this authority commit, which BASELINE and
  COORDINATOR pre-declare as expected evolution ("the authority commit itself is
  not drift"). Correctly anticipated; not a finding.

- INFORMATIONAL-3: Duplicate `origin`/`github` remotes both point to
  `jerudnik/jcode.git`; `upstream` push is DISABLED. D1/N3 already require an
  explicit retain/remove rationale, and no push is authorized. Correctly scoped.

## Validation performed (all read-only)

- `git log/show/rev-parse/cat-file` on the fixed commit; confirmed the 6-file
  commit, parent, and that all four authority files are tracked.
- `git stash list` + per-entry `rev-parse stash@{i}` and `^2`: all commit and
  index IDs match BASELINE's table exactly; `refs/stash` = `stash@{0}` only.
- Reachability adversarial test: proved `stash@{1..3}` commits and index parents
  are unreachable via `--all`, empirically confirming the BASELINE hazard and the
  necessity of the separate stash-object bundle.
- `stash@{3}` 3rd-parent traversal test (INFORMATIONAL-1).
- Counts: 40 branches, 138 tags, 916 non-recovery commits, 29 worktrees (25 labs
  + 4 /private/tmp) all match.
- Key refs (`HEAD`, `main`, `recovery`, `vendor/upstream`), ancestry of Phase 6
  closure, accepted source, and `main`-ancestor-of-recovery: all match.
- Prompt diff SHA-256 `8e8e6a92...` matches; `git status --short` shows only the
  expected preserved prompt edit.
- Binaries: `type -a jcode`, both symlink chains, both targets, both SHA-256
  anchors, and HM `/nix/store` path all match BASELINE.
- Processes/agents: menubar, shared-server serve, setup-hotkey listener live;
  `com.jcode.hotkey` running; `com.jcode.lesson-library-shadow` is a python job
  under `.wrangler/` (out of scope). All match.
- No `refs/archive/*` pre-exist (clean starting point for the archive plan).

## Confidence

High for everything independently checkable from the current repository and host:
every committed Git and host fact matched live read-only observation with no
discrepancy, and the one non-obvious silent-loss vector (reflog-only stash
objects) was empirically confirmed to be exactly the vector the design guards.

## What I did NOT check

- Any file under `docs/fork/normalization/reviews/` (excluded by instruction).
- Contents of `~/.jcode` credential files (metadata policy; not opened).
- Actual execution of any bundle/restore/rollback (design is read-only-first;
  those steps are approval-gated and were not run).
- Referenced-but-not-in-scope docs (`recovery/PROGRESS.md`, `RECOVERY_PLAN.md`,
  `reviews/2026-07-16-w7-review.md`, `SYNC_MODEL.md`, `QUALITY_GATES.md`);
  their internal correctness is outside this four-file authority review.
- The 916 non-recovery commits' individual dispositions (an N1/N2 execution task,
  not an infrastructure-definition task).
- Upstream remote reachability/identity (a live N3 fetch step, not authorized as
  part of this read-only review beyond confirming the configured URLs).
