# Operational / safety review: post-recovery normalization drafts

**Scope:** read-only review of
`docs/fork/normalization/{README.md,BASELINE.md,DEFINITION_OF_DONE.md,NEXT_SESSION_PROMPT.md}`
in `/Users/jrudnik/labs/jcode`.
**Reviewer posture:** adversarial, read-only. No files or host/repo state modified.
**Date:** 2026-07-16.

## Verdict: FAIL (not yet safe/operationally complete as written)

The design intent is strong: the approval gate model, append-only preservation
rule, "clean != erase history" framing, real-provider labeling, and
installer/updater fail-closed boundaries are all correct and would likely stop
most user-regrettable actions. But two concrete defects can cause silent data
loss or a misleading cleanup manifest, so I cannot return PASS. Both are fixable
with small, exact edits below; after they land the package is close to
operationally complete.

All baseline Git facts were revalidated and are accurate (see Evidence).

---

## CRITICAL

### C1. The default rollback archive silently loses 3 of 4 stashes

`NEXT_SESSION_PROMPT.md` N1.1 specifies preservation via "an archive ref and
verified Git bundle." `DEFINITION_OF_DONE.md` D0 requires the four stashes to
"have immutable hashes and at least one verified rollback archive," and D1
requires each of the four stashes to be individually resolved. The obvious and
implied mechanism (`git bundle create ... --all`) does **not** preserve the
stash stack.

Evidence: only `stash@{0}` exists as a real ref (`refs/stash` =
`1f54abc9fbb0190f59af2fe5744e8e8dfb99c67f`). `stash@{1..3}` exist **only** in
the reflog of `refs/stash`. `git bundle --all` bundles `refs/stash` (i.e.
`stash@{0}`) and does not include reflogs, so `stash@{1}`, `stash@{2}`, and
`stash@{3}` (and their index parents) are omitted. After any `stash drop` /
`gc`, those three are unrecoverable. Recording a SHA alone is not enough; the
archive must physically contain the objects.

Exact correction: before any mutation, capture each stash explicitly, not via
`--all`. Add to BASELINE required reproduction and to N1.1:

```bash
for i in 0 1 2 3; do
  git rev-parse "stash@{$i}" ;                     # record immutable hash
  git rev-parse "stash@{$i}^2" 2>/dev/null || true # index commit
done
# Physically preserve objects so drop/gc cannot lose them:
git bundle create /path/rollback-stashes.bundle \
  $(git rev-parse stash@{0} stash@{1} stash@{2} stash@{3})
# and/or format-patch each: git stash show -p "stash@{$i}" > stash-$i.patch
```

DoD D0/D1 should state that the four stash **commit objects** are in the
archive, verified by `git bundle verify` plus a re-list of all four, not merely
that four SHAs were noted.

---

## IMPORTANT

### I1. Host inventory omits the two live PATH binaries; one is Nix-managed and is not a "stale duplicate"

`PATH` resolves **two** distinct jcode binaries:

- `/Users/jrudnik/.local/bin/jcode` -> `~/.jcode/builds/current/jcode`
  (self-dev/updater-managed build).
- `/etc/profiles/per-user/jrudnik/bin/jcode` ->
  `/nix/store/w2wbi1jjm21hjqb9l920c5ph2m733g6n-home-manager-path/bin/jcode`
  (declaratively managed by home-manager).

BASELINE.md records neither. D7 ("only one documented canonical jcode
binary/runtime selection remains" and "remove ... duplicate binaries") plus N5.3
("remove ... duplicate binaries ... aliases, symlinks") could lead the
coordinator to treat the Nix/home-manager binary as a removable duplicate.
Deleting a home-manager path entry by hand is user-regrettable: it does not
persist, breaks the declarative model, and is reverted on next `home-manager
switch`.

Exact correction: BASELINE must enumerate both PATH entries, label the
home-manager one as **declaratively managed, not agent-removable** (only via its
Nix module by the user), and D7/N5 must say "duplicate binary removal excludes
Nix/home-manager-managed paths; those change only through their declarative
source with explicit user action."

### I2. Live user daemons (menubar, hotkey) and their launch agent are unclassified

Live now: pid 84136 `jcode menubar`, pid 95512 `jcode setup-hotkey
--listen-macos-hotkey`, and loaded launch agent `com.jcode.hotkey`
(`ProgramArguments[0] = /Users/jrudnik/.local/bin/jcode`, launchctl-loaded).
These consume the `builds/current` binary that N4/N5 will rebuild/repoint.

D7 requires "no orphan process" and "at most one intended jcode daemon," but the
drafts nowhere state that these two **user-facing** processes exist, are
intentional, and must be gracefully preserved or restarted rather than swept as
"stale." N5's approval gate helps, but the manifest classification guidance is
missing them, risking a dry-run that proposes killing the user's active menubar.

Exact correction: BASELINE must record menubar + hotkey processes and the
`com.jcode.hotkey` agent as **intended, user-facing, retain**. D7/N5 must
distinguish "the sandbox validation daemon (must leave no orphan)" from
"pre-existing user integrations (preserve/restart, do not silently kill)."

### I3. `com.jcode.lesson-library-shadow` is unrelated and must not be swept by "remove stale launch agents"

Loaded launch agent `com.jcode.lesson-library-shadow` runs
`/usr/bin/python3 .../docs/cloudflare-roadmap/scripts/run_lesson_library_shadow_sampler.py`
with `WorkingDirectory = /Users/jrudnik/labs/jcode`. It is not part of the jcode
runtime, only shares the name/path. N5.3's "remove ... stale daemons/launch
agents" plus a name-glob inventory (`grep -i jcode`) would flag it for removal.
That is out-of-scope and user-regrettable.

Exact correction: add to BASELINE as **unrelated project agent, out of scope,
retain**; add an explicit N5/D7 rule that agents merely matching `jcode` by name
or path are not removed unless they are jcode-runtime agents, and that unrelated
agents require separate explicit user decision.

### I4. Rollback-archive scope in N1.1 is narrower than D0 and than the real ref set

N1.1 preserves "the exact recovery line." But the object DB holds substantial
unique work outside that line: 916 commits are unreachable from the recovery
head across local branches (e.g. `orch/w1-control-log` +146, `orch/failure-
scoreboard` +138, `recovery/seam-r04-20260715` +3, `agent/hotpath-
stabilization` +2). BASELINE.md line 64-67 correctly warns non-ancestor tips are
not disposable, and D0 requires "branches ... immutable hashes and ... rollback
archive," but N1.1's recovery-line-only bundle does not satisfy D0 before any
branch deletion.

Exact correction: N1.1 must archive **all refs** before any ref deletion:
`git bundle create rollback-all.bundle --all` (plus the explicit stash bundle
from C1), then `git bundle verify`. State that branch deletion is gated on this
all-refs archive verifying, not just the recovery line.

### I5. No documented, exercised rollback command for the live binary/daemon repoint

D8 requires rollback "from canonical runtime to the archived recovery artifact
... exercised without data loss," and D6/N4 rebuild the binary behind
`~/.jcode/builds/current`. But no draft specifies how to restore the previous
`builds/current` symlink target if promotion misbehaves while the menubar/hotkey
agents point through it. This is the one live runtime coupling most likely to
strand the user.

Exact correction: N5/D8 should record the current `builds/current` symlink
target hash pre-change and give an exact restore command (repoint
`~/.jcode/builds/current` to the recorded prior build, reload agents), and
require it be dry-run exercised.

---

## LOW

- **L1.** Duplicate remotes: `origin` and `github` both point to
  `https://github.com/jerudnik/jcode.git`; `upstream` push is `DISABLED`. D1
  ("remotes ... current") does not mention reconciling the origin/github
  duplicate. Add a one-line remote reconciliation note. (No safety impact; push
  is already gated.)
- **L2.** Some D2 gates are subjective ("small logical stack," history that
  "obscures product history"). Acceptable, but add a measurable proxy (e.g.
  commit count bound + reviewer sign-off + the tree-equivalence proof already in
  D2) so "done" is not reviewer-mood dependent. The tree-equivalence gate (D2)
  is the strong measurable anchor and is good.
- **L3.** `.wrangler/` (written by the lesson-shadow agent into the canonical
  repo working tree) is git-ignored (`git check-ignore` matches), so it will not
  dirty `git status --short`. Fine, but worth a one-line BASELINE note so a
  future "why is there untracked runtime state" question is pre-answered.
- **L4.** BASELINE's oauth material (`~/.jcode/antigravity_oauth.json`,
  `.bak`) is correctly covered by D0/D8 secret-leak rules; just ensure the host
  inventory command set never cats credential files into evidence (the drafts
  already forbid this; keep the inventory commands metadata-only).

---

## What is correct and should be kept

- Approval gates (README "Mutation rule"; N1.5; N5.1-2; prompt safety rules)
  correctly force dry-run manifest + rollback + explicit user approval before
  moving `main`, deleting refs, removing worktrees, dropping stashes, using real
  credentials, installing/updating, or publishing. This prevents the major
  regrettable classes.
- `main` promotion is fast-forward-only and technically valid: `main` is an
  ancestor of recovery (`main...recovery = 0 / 166`, merge-base = `main`), and
  the curated line is built from `main`, so reviewed fast-forward is sound.
- Recovery archive is explicitly immutable and not rewritten (README "Core
  distinction"; D2). Good forensic discipline.
- Real-provider labeling (D6 / N4.5: "core-runtime validated" vs "fully runtime
  validated") and installer/updater fixture-only boundary (D6) are exactly right
  and fail-closed.
- Append-only preservation of failed/superseded attempts (D0; prompt rules)
  prevents evidence laundering.
- No-force-push / no-remote-push without separate instruction (D2; prompt) is
  correct.

## Measurability of final state

Largely measurable and reproducible: D1/D4/D6/D7 gates map to concrete commands
(git status, ref hashes, worktree prune check, cold-shell binary resolution,
tree-equivalence proof, sandbox daemon lifecycle). The main measurability gaps
are the mechanism-level ones above (C1 stash objects, I4 all-refs archive, I5
binary-repoint rollback), not the pass/fail phrasing.

## Evidence (revalidated, read-only)

- `git status --short`: only `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` + the
  untracked `docs/fork/normalization/` dir. Matches BASELINE.
- HEAD `cdc2cc2b...`, `main` `6ca1fcf2...`, `recovery/2026-07-15` `cdc2cc2b...`,
  `vendor/upstream` `631935dd...`. All match BASELINE/prompt.
- `main...recovery` = `0 / 166`; merge-base = `main`. Matches.
- Prompt diff SHA-256 =
  `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`. Matches.
- Stashes: exactly 4; `refs/stash` = `1f54abc9...` (stash@{0} only);
  stash@{1..3} reflog-only. -> C1.
- Worktrees: 29 (25 labs, 4 `/private/tmp`). Matches BASELINE.
- Referenced authority files all exist (PROGRESS.md, RECOVERY_PLAN.md,
  w7-review.md, SYNC_MODEL.md).
- Non-ancestor unique commits: 916 across local branches. -> I4.
- PATH binaries: `~/.local/bin/jcode -> ~/.jcode/builds/current/jcode`;
  `/etc/profiles/.../bin/jcode -> /nix/store/...home-manager-path/bin/jcode`.
  -> I1.
- Live: pid 84136 `jcode menubar`, pid 95512 `jcode setup-hotkey`; launchctl
  loaded `com.jcode.hotkey`, `com.jcode.lesson-library-shadow`. -> I2, I3.

## Required to flip to PASS

Land C1 (stash object preservation), I1-I3 (BASELINE host inventory of the two
PATH binaries + Nix-managed flag, the two live daemons, and the unrelated launch
agent, with retain/out-of-scope classifications), I4 (all-refs archive gate
before any ref deletion), and I5 (recorded binary-repoint rollback command).
LOW items are optional polish. After those, the approval-gated plan is safe and
operationally complete.
