# FORK HARDENING — BLUE BRIEF (defense: "the drift was correct; the cut-down model held")

Group: BLUE. Date: 2026-07-08. HEAD `75b20b215`. Vendor `github/vendor/upstream`
= `631935dd1`. Every claim reproducible with the inline commands.

**Thesis:** The model's *conclusion* — this is an addition-dominated fork that needs
`rerere` + `doctor` + additive-seam discipline, not patch-queue/feature-variant
machinery — is not just intact, it is *more* strongly supported at HEAD than when it
was written. RED is right that the doc's specific 2026-06-27 numbers are stale, but
that is a calibration bug in the prose, not a failure of the architecture. The
correct fix is a one-command re-baseline, not a heavier maintenance model.

---

## The one number that settles it: 30:1 additive

```
$ git diff --numstat github/vendor/upstream HEAD -- '*.rs' | awk '{a+=$1;d+=$2} END{print a" added / "d" deleted"}'
   17241 added / 581 deleted            #  ~30 lines added per line deleted
$ git diff --numstat github/vendor/upstream HEAD -- '*.rs' | awk '$2==0' | wc -l
   113                                    #  files with ZERO deletions (pure-additive)
$ git diff --numstat github/vendor/upstream HEAD -- '*.rs' | awk '$2>0'  | wc -l
   73                                     #  files with any deletion
```

The model's *entire* load-bearing claim is one sentence: **"conflicts come from
lines you delete, not lines you add."** At HEAD the fork has grown 6x and the
add:delete ratio is still ~30:1, with 113 of 186 touched `.rs` files deleting
*nothing*. The growth vindicated the prediction: the fork got much bigger and stayed
overwhelmingly additive. That is the model *holding under load*, not breaking.

## Rebuttals to RED's strongest points

**RED#1 "numbers stale 6x–20x" — CONCEDED as prose drift, REJECTED as architecture failure.**
Yes: 180 commits over vendor, not 30; 16 non-test source files delete >5 lines, not
7. But RED measures the wrong axis. Commit count and file count are not the model's
variable; *deletion surface* is. 581 total deletions across a 17k-line diff is a
1.6% conflict surface. The reframe ("routine rebase + a handful of recurring
conflicts") survives its own test: the conflicts `rerere` actually recorded number
100, and the 6h rail is green (RED concedes this). Re-baseline the prose; keep the model.

**RED#2 "invasive swarm/comm subsystem, ungoverned" — the invasiveness is a myth.**
The swarm/comm files *are* upstream-existing (verified), so RED is right they are
edits not new files. But look at the shape:

```
   swarm.rs               435 add /  21 del     (20:1)
   comm_session.rs        220 add /  25 del      (9:1)
   comm_await.rs          248 add /  16 del     (16:1)
   communicate.rs         217 add /  16 del     (14:1)
   session_search.rs      246 add /   7 del     (35:1)   [tool/ — the server/ file is fork-NEW]
```

These are files the fork *massively extends by addition* with a small deletion tail
(sorted-`use` reflow, match-arm edits) — exactly the "insert-heavy edit" the model
says `rerere` absorbs. ~120 deletions across 8 files is the whole "subsystem"
conflict surface. That is a rounding error, not the "deep invasive rewrite" the
model disclaimed. RED's own severity label ("CRITICAL") is not supported by 120
deletions inside 1,900 additions.

**RED#3 "skill.rs regressed 20→0 to 12 deletions" — it's an EXTENSION, not a regression.**
The diff shows the `.agents` block was not reverted; it was folded into a loop that
*adds a new `.apm` source*:

```
+  for dir_name in [".apm", ".agents"] {         # new: .apm support, .agents preserved in-loop
-  let local_agents = Self::project_local_dir(working_dir, ".agents");   # old single-dir form
```

Behavior is preserved (`.agents` still loads) and extended (`.apm` now loads too).
The 12 "deletions" are the single-dir form being replaced by a two-dir loop. RED is
right the seam audit's "byte-identical upstream block" wording no longer holds, but
the *reason* is a deliberate fork feature (`.apm`), not rot. This is the model's
"prefer additive, and when you must edit, keep it a tiny seam" working as intended.

**RED#5 "live fork-health violation (sync.yml on main)" — REAL, and small. CONCEDED.**
This one lands. `main` carries `1f5cc7aac` touching `.github/workflows/sync.yml` over
`distro/nix`. It is a genuine invariant break and belongs on `distro/nix`. But it is
a one-commit misfile, not an architectural failure — the fix is `git cherry-pick` to
the right branch. It argues for making check 5 blocking, nothing heavier.

**RED#6/#7 "two cheap changes became a distro platform / executable ledger returned" — category error.**
RED conflates two different problems. The model governs **fork-vs-upstream
divergence** (rerere/doctor/seams). The workflows RED lists — `fork-ci.yml` budget
ratchets (code-size/panic/swallowed-error), `security.yml`, `nix-update.yml` — govern
**fork-internal code quality**, a separate concern the model never claimed to cover.
A swallowed-error budget is not "an executable *patch* ledger"; it never tracks a
single upstream patch. RED found real machinery and mislabeled it as violating a
model that was never about it. The actual divergence machinery is still just
rerere + the rail + doctor.

## Where reality validated the cut (the deferred heavy options stayed unneeded)

```
$ ls nix/features/ 2>/dev/null           # (nonexistent) — no feature-variant tier built
$ git log --oneline --all | grep -ci jujutsu ; grep -rl 'jj ' .jj 2>/dev/null   # 0 — no jj migration
```

The model deferred jj, Nix feature-variants, and named daemon instances "until a
concrete failure justifies it." Six weeks and 180 commits later: **none was built,
and nothing broke for lack of it.** The escape hatch the model reserved
(`overrideAttrs.patches`) was needed exactly once (Mermaid font-family sanitizer) and
handled by one ledger row, precisely as designed. Deferral was the correct call,
proven by the absence of the failure that would have triggered it.

## The minimal justified hardening (be honest where a fix IS due)

RED surfaced three things that are genuinely worth a small fix — all cheap, none
architectural:

1. **Re-baseline the model's numbers** (one script + a committed table). The prose
   cites 2026-06-27 measurements; make the divergence baseline a regenerable artifact
   (`scripts/fork-divergence-report.sh` → the GREEN scoreboard) so the doc never
   silently decalibrates again. This is the single highest-value fix.
2. **Fix the one fork-health violation and make check 5 blocking.** Move
   `1f5cc7aac`'s workflow change to `distro/nix`; promote fork-health check 5 from
   daily-issue to pre-merge gate. Small, correct.
3. **Extend the ledger + seam-audit coverage to the swarm/comm files** — not because
   they are dangerous (they are 30:1 additive) but because the ledger is cheap
   documentation and "why do we carry this" should be answerable for the fork's
   biggest feature. One paragraph per family, not a governance regime.

## Verdict

- **Top 3 "do NOT build this":**
  1. Do NOT adopt jj / a patch-queue manager. 581 deletions is not a patch stack.
  2. Do NOT build a Nix feature-variant tier. Zero features needed it in 180 commits.
  3. Do NOT turn the swallowed-error/code-size budgets into a formal patch-ledger
     system. They are quality ratchets, a different and already-working tool.
- **Top 3 minimal fixes worth doing:** re-baseline script (H1), fix+gate fork-health
  check 5 (H2), ledger/seam rows for swarm/comm (H3).

The model held. It needs a fresh coat of numbers and one misfiled commit moved, not
a heavier architecture.
