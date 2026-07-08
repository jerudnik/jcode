# FORK HARDENING — RED BRIEF (adversarial: "the model drifted, the fork is under-hardened")

Group: RED. Date: 2026-07-08. HEAD `75b20b215`. Vendor `github/vendor/upstream`
= `631935dd1`. All numbers reproducible with the inline commands.

**Thesis (proven below):** The committed `FORK_SUSTAINABILITY_MODEL.md` describes a
fork that no longer exists. Its four load-bearing measurements ("30 mostly-additive
commits", "7 rewrite-files", "agent.rs is additive-only", "skill.rs converted
20->0") are each false against ground truth. The "two cheap changes" grew into a
multi-workflow distro-maintenance layer with a live invariant violation, and a whole
invasive **swarm/comm subsystem** was carved into upstream files with **zero**
ledger, seam-audit, or model coverage. The fork is more invasive and less documented
than the doc asserts.

---

## Ranked failure modes

### 1. [CRITICAL] The model's headline divergence numbers are stale by 6x–20x
The entire "you don't have a patch-stack problem" argument rests on "30
mostly-additive commits / 7 rewrite-files". Ground truth:

```
$ git rev-list --count github/vendor/upstream..HEAD          # 180  (model: 30)
$ git log --oneline --since=2026-06-29 github/main | wc -l    # 619
$ git diff --name-status github/vendor/upstream HEAD | awk '{print $1}' | sort | uniq -c
    311 A    3 D    186 M                                       # model: 47 new / 60 mod
$ git diff --numstat github/vendor/upstream HEAD -- '*.rs' | awk '$2>5' | grep -viE 'test|tests' | wc -l
    16                                                          # non-test source files, model: 7
$ git diff --numstat github/vendor/upstream HEAD -- '*.rs' | awk '{d+=$2} END{print d}'
    581                                                         # total upstream .rs lines DELETED
```

**Why it contradicts the model:** The doc's central reframe ("routine rebase with a
handful of recurring conflict points, not a patch-queue problem") was justified *by
these exact numbers*. At 180 commits over vendor, 186 modified upstream files, and
16 non-test source files deleting upstream lines (581 deletions total), the premise
that killed the tiered/feature-variant machinery is no longer supported by
measurement. The conclusion may still be right, but it is now **asserted, not
measured** — the doc was last calibrated at 1/6th the current scale.

### 2. [CRITICAL] A whole invasive swarm/comm subsystem edits upstream files, untracked by ledger/seam-audit/model
The largest new source of upstream deletions is an entire multi-agent
"swarm/comm" subsystem the model never names. These are **edits to upstream
files** (verified they exist at `github/vendor/upstream`), not new files:

```
$ git diff --numstat github/vendor/upstream HEAD -- '*.rs' | awk '$2>5' | grep server
   51  40  server/client_comm_message.rs
  220  25  server/comm_session.rs
  435  21  server/swarm.rs
  248  16  server/comm_await.rs
  189  15  server/swarm_persistence.rs
  111   6  server/comm_control.rs
   #  and tool/communicate.rs (217/16), tool/session_search.rs (246/7)
```

Every one of these files is confirmed `EDIT-UPSTREAM` (not additive new file). And
**none** appear anywhere in the governance surface:

```
$ for kw in swarm comm_ communicate compaction session_search conversation_state client_comm; \
    do echo -n "ledger '$kw': "; grep -c "$kw" docs/fork/patch-ledger.md; done
   swarm 0 / comm_ 0 / communicate 0 / compaction 0 / session_search 0 / conversation_state 0 / client_comm 0
```

`FORK_REWRITE_SEAM_AUDIT.md` still lists only the original seven files and never
mentions swarm/comm. **Why it contradicts the model:** the doc's core promise is
"every upstream line you delete is a future merge conflict" and the seam audit is
supposed to walk *every* file deleting >5 upstream lines. A subsystem contributing
~120 upstream deletions across 8 files was added with no seam analysis, no ledger
row, no retire condition. This is precisely the "deep invasive edits" the model
claimed the fork does not have.

### 3. [HIGH] The `skill.rs` "converted to additive 20->0" headline has regressed
The model (§"Done (Change 3)") and `FORK_REWRITE_SEAM_AUDIT.md` both headline
`skill.rs` as **converted to a 0-deletion additive loop**. Ground truth:

```
$ git diff --numstat github/vendor/upstream HEAD -- crates/jcode-base/src/skill.rs
   40  12  crates/jcode-base/src/skill.rs           # 12 deletions, not 0
```

The diff shows the fix regressed: upstream's `.agents` load block is **deleted from
its original position** in *two* functions (`load_project_local_dirs` and the count
variant) and re-added inside a prepended `[".apm", ".agents"]` loop — reintroducing
exactly the upstream-line deletions the conversion was meant to remove. The seam
audit's own success criterion ("upstream's `.jcode`/`.claude` blocks stay
byte-identical") is violated: the `.agents` block no longer exists in upstream's
position. **Why it contradicts the model:** the single named success story of Change
3 silently rotted, and no doc reflects it. If the flagship seam conversion cannot
stay converted across rebases, "additive seams" as a durable discipline is unproven.

### 4. [HIGH] `agent.rs` now deletes real upstream logic — the model swore it was additive-only
The model's table asserts agent.rs/prompt.rs/session.rs are "touched by **adding**
lines (hooks/registrations), not rewriting them." Ground truth:

```
$ git diff --numstat github/vendor/upstream HEAD -- crates/jcode-app-core/src/agent.rs
   118  19  agent.rs
```

The 19 deletions are the upstream **compaction-dispatch block** (`ensure_context_fits`,
`CompactionAction::{BackgroundStarted,HardCompacted,None}`) rewritten to gate on a
new `native_compaction_mode()=="auto"` path. This is a behavioral rewrite of a
hot-loop agent decision, not a registration line. **Why it contradicts the model:**
agent.rs was the doc's marquee example of "invasive-looking but actually additive."
It is now genuinely invasive, and this is the highest-churn file in the agent loop
where a bad rebase resolution is most dangerous.

### 5. [HIGH] Live fork-health invariant violation: `main` modifies a workflow over `distro/nix`
`scripts/fork-health.sh` check 5 forbids `main` from carrying any
`.github/workflows/` diff over `distro/nix`. It fails **right now** on the real
GitHub refs:

```
$ git diff --name-only github/distro/nix github/main -- .github/workflows/
   .github/workflows/sync.yml
$ git log --oneline github/distro/nix..github/main -- .github/workflows/sync.yml
   1f5cc7aac fix(ci): harden sync-blocked alert so labeling can never suppress it
```

A CI-policy fix landed on `main` instead of `distro/nix`, breaking the "CI owned by
distro/nix" invariant the fork built a daily workflow to enforce. **Why it
contradicts the model:** the fork's own hardening machinery is red in production.
Either fork-health.yml is not actually gating (silent), or the violation was merged
past it. Both are hardening failures.

### 6. [MED] The "two cheap changes" quietly grew into a distro-maintenance platform
The model's TL;DR: "rerere + doctor + seams is the whole model… no new files in the
build, no new concepts." Reality is a fleet of fork-only machinery, most added
2026-07-07:

- **6 fork-owned workflows**: `sync.yml`, `fork-ci.yml`, `fork-health.yml`,
  `security.yml`, `nix-update.yml`, plus `nix.yml` (model said sync lives *in*
  `nix.yml`; it was extracted to a dedicated `sync.yml`).
- **A three-branch rail** (`vendor/upstream → distro/nix → main`) with a dedicated
  **invariant checker** (`fork-health.sh`, 5 checks, 140 lines) and issue-bot
  auto-open/close on drift.
- **8 fork scripts**: `fork-health.sh fork-rail-status.sh fork-touched-clippy.sh
  branch-model-status.sh rerere-cache.sh rerere-rebase.sh sync-local.sh
  fork-nudge.sh`.
- **A git-hooks installer** (`scripts/install-git-hooks.sh` + `git-hooks/pre-push`
  branch-rail guard).

**Why it contradicts the model:** the doc explicitly rejected "an executable patch
ledger", "named daemon instances", and heavyweight rail machinery as "machinery for
a problem this fork does not have." The fork then built a distro layer, a rail
invariant DSL, a pre-push guard, and issue automation. The machinery may be
justified — but the model that says it isn't needed is now the *governing document*,
so the machinery is running **against its own stated architecture** with no doc
reconciling the two.

### 7. [MED] A de-facto "executable ledger" now exists — the exact thing the model REJECTED
The model's prior-art table marks "executable patch ledger" as deleted machinery.
The fork now ships six CI-enforced budget ledgers:

```
scripts/code_size_budget.json (5.3K)   scripts/swallowed_error_budget.json (62K)
scripts/panic_budget.json              scripts/test_size_budget.json
scripts/wildcard_reexport_budget.json  scripts/warning_budget.txt
```

`fork-ci.yml` gates every push on `check_code_size_budget.py`,
`check_test_size_budget.py`, `check_panic_budget.py`,
`check_swallowed_error_budget.py`, `check_warning_budget.sh`, and `cargo machete`.
A 62KB machine-maintained swallowed-error ratchet is an executable ledger by any
definition. **Why it contradicts the model:** the concept the doc named and killed
came back at scale, undocumented in the sustainability model.

### 8. [MED] The patch-ledger does not keep pace with the divergence it governs
The ledger last moved 2026-07-04 (`4dd64d650`) and is 85 lines / ~19 rows. It
tracks ~7 patch families. But the fork now has **16 non-test source files** deleting
upstream lines. The entire swarm/comm subsystem, `compaction.rs`+agent.rs compaction
gating, `session_search.rs`, and `conversation_state.rs` have **no rows** (finding
#2 evidence). **Why it contradicts the model:** the ledger is the doc's designated
"why each downstream change exists / when it retires" index. With ~half the invasive
files unlisted, the fork cannot answer "what do we carry and when does it retire" —
the ledger has become a partial view, not the index.

### 9. [LOW] The rerere cache is unbounded with no GC or provenance policy
```
$ ls .rerere-cache/ | wc -l   # 100 entries
$ du -sh .rerere-cache/       # 7.3M ; git ls-files .rerere-cache | wc -l = 199
```
All 100 entries have both preimage and postimage (99/99 resolved), so it is
*healthy*, not stale — but it grows monotonically (recorded per sync,
`5fcf28053 fork: record rerere resolution`) with **no pruning, no expiry, no
staleness audit**. A resolution recorded against a since-deleted upstream hunk
replays forever and can silently mis-resolve a superficially-similar future
conflict. **Why it matters:** the model calls rerere "resolve each conflict once,
ever" but never specifies a policy for retiring recordings whose target code is
gone. At 100 entries and climbing, that debt is now worth a bound.

### 10. [LOW] The model doc mis-states its own CI wiring
The model (§Change 1) says CI sync "lives in `.github/workflows/nix.yml`". It was
extracted into a standalone `sync.yml` (three-rail rebase + fork-health + validation
dispatch). Small, but it means the primary architecture doc points at the wrong file
for the fork's single most important automation. Doc drift on the rail itself.

---

## Rail health verdict (the one thing that is genuinely fine)
The 6h rebase rail *exists and is structurally sound*: `sync.yml` cron `17 */6 * * *`,
`--force-with-lease`, rerere import before rebase, `rerere-rebase.sh` aborts non-zero
on a genuinely new conflict, and opens a `sync-blocked` issue. Ancestry holds
(`vendor ⊆ distro/nix ⊆ main` verified). This is real hardening and RED does not
dispute it. The problem is not the rail; it is that **everything the rail carries has
outgrown the model that describes it**, and one invariant (finding #5) is red.

---

## Top 3 hardening demands

1. **Re-baseline the model or stop citing its numbers.** The "30 commits / 7 files /
   agent.rs-additive / skill.rs 20->0" claims are all false at HEAD (180 commits, 16
   source files, agent.rs deletes 19, skill.rs deletes 12). Either recompute and
   commit a fresh divergence baseline that the seam-audit walks in full, or demote
   the model from "governing architecture" to "historical rationale." A governing
   doc whose four load-bearing measurements are stale is a liability.

2. **Bring the swarm/comm subsystem under governance and re-convert skill.rs.**
   Add ledger rows (class/retire/validation) for the 8 swarm/comm files,
   `compaction.rs`/agent.rs compaction gating, `session_search.rs`, and
   `conversation_state.rs`; run the seam audit over all 16 offenders, not 7.
   Re-apply the skill.rs additive conversion so upstream's `.agents` block is
   byte-identical again (it regressed to 12 deletions).

3. **Make fork-health enforcing, not advisory, and fix the live violation.** `main`
   currently carries a `sync.yml` diff over `distro/nix` (check 5 fails today). Move
   that commit to `distro/nix`, and make the fork-health invariant a *blocking*
   pre-merge gate on `main` (not just a daily issue-bot), so CI-policy drift cannot
   land past the very check built to forbid it. While there, bound the rerere cache
   with a prune/expiry policy so recordings for deleted upstream hunks retire.
