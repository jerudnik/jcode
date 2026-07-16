# R00 Integration provenance and sync governance: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4` (Phase 1 adjudication baseline; ledger authored at `8848f2d54f67f9a5a1de76bace9666c78036e116`); upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light overlay` (mandatory governance overlay per `RESPONSIBILITIES.md`) |
| Research budget | 8 decisive checkpoints for the R00/R09/R11 overlay batch; 6 consumed |
| Recommended disposition | `retain-fork` |
| Confidence | high for governance-policy ownership; medium for curated-sync semantic completeness (see gaps) |

R00 owns fixed refs, ancestry, curated-sync provenance, equivalence claims, preservation, rollback, and stop budgets. It excludes runtime behavior and implementation authority. This overlay binds every Phase 2 seam ledger and the Phase 3 pilot. Upstream is a comparison input, never an authority: no evidence below, and no future equivalence claim, may treat `upstream/master` content as correct merely because it is upstream.

## Findings

| Finding | Evidence | Consequence |
|---|---|---|
| Fixed refs resolve and the merge base is stable | `git rev-parse --verify 7ff4fc6be8dc^{commit} 802f69098258^{commit} 631935dd1d3b^{commit}` all succeed; `git merge-base 8848f2d54 802f6909825809e882d9c2d575b7e478dce57d3b` = `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`; reproduced 2026-07-15 in this worktree | Every seam comparison is anchored to reproducible refs; drift is detectable |
| Governance machinery is fork-only with zero upstream counterpart | `PRESCREEN.md` R00 row: files F/U/O = 215/0/0, commits 30/0, including 199 `.rerere-cache` paths and `.fork.toml`; `docs/fork/SYNC_MODEL.md` documents monitored curation | There is nothing upstream to adopt; disposition can only be `retain-fork` or `delete`, and deleting governance mid-recovery is self-defeating |
| The curated sync `b3ed82a6b` is a single-parent squash and an ancestry gap | `git show -s --format='%H%n%P' b3ed82a6b` -> one parent `8ed75637a`; `SEAM_LEDGER_TEMPLATE.md` evidence standard names it an explicit gap | Absence of a commit from ancestry proves nothing; seams must search for absorbed behavior |
| No exact patch-ID equivalence exists between fork and upstream ranges | `PRESCREEN.md` patch-equivalence pre-screen: `git patch-id --stable` over 288 fork and 243 upstream non-merge commits, empty `comm -12` intersection | All equivalence claims must be semantic/symbol-level and must record command, refs, options, and assumptions |
| `vendor/upstream` is pinned to the merge base and is not a current mirror | `git rev-parse vendor/upstream` = `631935dd1...` (`BASELINES.md` Phase 0 snapshot; `PROGRESS.md` active blocker) | Any seam citing `vendor/upstream` as current upstream state makes a provenance error; cite `802f69098...` instead |
| Preservation state is intact at authoring time | `git stash list --date=iso-strict` shows the four recorded stashes; `git worktree list` shows all registered worktrees including the three restored `recovery/orchestrator-s{4,5,6}-20260715`; the user's `ORCHESTRATOR_PROMPT.md` diff SHA-256 reproduced as `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00` via `git diff -- docs/fork/recovery/ORCHESTRATOR_PROMPT.md \| shasum -a 256` in the coordinator worktree | The Phase 0 preservation contract holds; the earlier worktree-reclaim incident (`BASELINES.md` "Preservation incident and repair") shows why the check must rerun at every session start |
| Hot-path stashes are evidence, not open work | `PRESCREEN.md` maintenance reconciliation: `a69ef9710`, `3d80eaf34`, `97529fd6c`, `2ccf43fd7`, `17cceb1a1`, `e6ff371c1` are ancestors; the three `fix-config-hotpath-spam` stashes overlap already-integrated fixes | Popping or replaying a stash is a provenance violation and a stop condition |

## Mandatory overlay obligations on every seam

1. **Fixed refs.** Every ledger states fork, upstream, and merge-base SHAs and reruns its decisive commands against them. Refreshing refs requires a new dated `BASELINES.md` append, never an edit.
2. **Provenance of equivalence.** Every adopt/compose/patch-equivalence claim records the exact command, refs, options, and assumptions. Content inside `b3ed82a6b` may be imported upstream behavior, conflict resolution, or fork adaptation; the claim must say which and how it was distinguished. Upstream provenance is never sufficient grounds for adoption.
3. **Preservation.** No seam may pop stashes, delete or move branches/worktrees/refs, or touch the preserved `ORCHESTRATOR_PROMPT.md` edit (diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`, user-controlled). External notes used as evidence record absolute path plus content hash in the ledger.
4. **Rollback and stop budgets.** Every implementation slice names an explicit rollback or stop condition before it starts (template "Bounded implementation slices" column is mandatory, not optional). Per `RESPONSIBILITIES.md` pilot prerequisites, work stops rather than broadens when it would need real credentials, the live user daemon, a baseline `--update`, an unowned identity writer, or more conflict/time/semantic rewrite than budgeted. Exceeded research budgets block or narrow the seam; they are never extended silently.

## Reproduction

```bash
git rev-parse --verify 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4^{commit} \
  802f6909825809e882d9c2d575b7e478dce57d3b^{commit} \
  631935dd1d3b2e31e167e2b12ad463e54bcf4b8d^{commit}
git merge-base HEAD 802f6909825809e882d9c2d575b7e478dce57d3b
git show -s --format='%H%n%P%n%s' b3ed82a6b
git rev-parse vendor/upstream   # must remain 631935dd1..., not current upstream
git stash list --date=iso-strict   # four preserved stashes, do not pop
git worktree list
git -C /Users/jrudnik/labs/jcode diff -- docs/fork/recovery/ORCHESTRATOR_PROMPT.md | shasum -a 256
# expect 8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00
```

## Explicit gaps

- Semantic equivalence hidden inside `b3ed82a6b` was not resolved here; it is an explicit per-seam research question (`PROGRESS.md` active blockers), not an R00 conclusion.
- This ledger did not audit `.rerere-cache` entry-by-entry; rerere recordings are preserved history, and any seam relying on one must cite the specific resolution commit.
- The `test-reload-hash` pending canary activation recorded in `BASELINES.md` was not re-inspected; it belongs to R01 evidence.

## Disposition and conditions

- Recommended disposition: `retain-fork`. The governance machinery has no upstream counterpart, the curated-sync model is deliberate (`docs/fork/SYNC_MODEL.md`), and the recovery program depends on it. Retention does not endorse every recorded resolution; seams re-examine content on the merits.
- Acceptance or retirement condition: this overlay retires only when Phase 6 sign-off confirms all dispositions were made against the fixed refs with recorded provenance, preservation checks pass at final state, and no unlogged ref/stash/worktree mutation occurred. Until then it is accepted as binding when the coordinator approves this ledger.
- Escalate to full review if: any fixed ref or preserved hash fails reproduction; a seam asserts equivalence without a recorded command; anyone proposes replay/merge/rebase of the curated sync or popping a stash; `vendor/upstream` is moved; or the preserved prompt diff hash changes without an explicit user decision.
- Coordinator approval: pass, 2026-07-15. Fixed refs, merge base, stale `vendor/upstream`, four stashes, eleven registered worktrees, and the preserved prompt hash reproduced.
- Fable review: pending independent Phase 4 architecture review; this ledger was authored by Fable and cannot self-approve.

## 2026-07-15 combined integration preservation amendment

- Fixed coordinator source: `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3` on `recovery/2026-07-15`.
- Preservation recheck passed before and after G0: four stashes, all recovery worktrees/branches including parser branch `c53022f4d4135b43fc86337c9c689a9e73c27807`, sole dirty path `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`, and unchanged diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`.
- Byte-exact log copies and SHA-256 manifests are under [`../../evidence/`](../../evidence/): combined prerequisites `41ece4820891461de774dbc5ab06d8e8a66c00630be62274d00dc1f5a9952291`, final R09/infrastructure attempts `113817813b49815d00a10b716e66ab3ed094b28ff6d02fcc60c6d8584c70940a`, and G0 R09 rerun `eadb5441bfdf5aef353a2356b2f04454a33912924a07c8eb7e207146ba992614`.
- The R02 coordinator chain is `3063fe0fa` through `cb924b3ae`; the R01/R03A chain is `615ab1d9a` through `6c6a4f2c8`. Exact source/test/docs/review linkage and intentionally absent categories are in the evidence index.
- No stash, ref, worktree, history, cache, daemon, activation, or publication mutation was performed by G0/G1. Pilot authorization remains OPEN pending independent G2 adjudication.

## 2026-07-15 W0 Phase 4 review amendment

The stale Fable-pending line is discharged. Corrected cross-seam Fable plan SHA-256 `b0bae9803fa726a489e0560fdc423daefa20bd8478ede0aa2772f7684ea21eb9` retained R00 as a binding `retain-fork` provenance/preservation overlay; independent fixed-plan Opus review SHA-256 `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92` returned **PASS** with no blocking findings.

R00 remains active until Phase 6. This amendment does not retire fixed-ref, provenance, prompt, stash, worktree, or no-broad-sync obligations.

## 2026-07-16 Phase 6 coordinator-audit amendment

At source head `51168d16e9c708ae4afff09a6fc6402642d17782`, fixed refs and merge base
reproduced, `vendor/upstream` remained `631935dd1...`, exactly four stashes
remained untouched, and the sole dirty path remained the user-controlled prompt
with diff SHA-256 `8e8e6a92...`. Before/after process projections matched and
showed only the same pre-existing SSH mux; no active build or remote builder was
observed. All 17 recovery evidence manifests verify. The accepted evidence is
[`../../evidence/2026-07-16-phase6-final-audit/`](../../evidence/2026-07-16-phase6-final-audit/),
`SHA256SUMS` SHA-256
`9af58f1563f266066edd6da9208983da62eeb0b1997ec78f9c26318221dcd2a3`.

The coordinator judges the R00 retirement conditions mechanically satisfied.
R00 remains active only until the required independent reviews and joint
Sol/Fable sign-off are preserved.

## 2026-07-16 spot-check metadata correction

The candidate package hash above remains preserved as reviewed. The spot
checker returned PASS and identified only a LOW wording distinction: 62 real
checks versus 76 TSV physical lines. Corrected metadata has package
`SHA256SUMS` SHA-256
`ca8ff5b9f3b6c09dc0ff05de9b3c1c426fc2373706eeeca26cad87126f2e14d8`.
No preservation fact changed.

## 2026-07-16 final retirement amendment

**Status: retired as a special Phase 6 overlay.** Joint Sol/Fable sign-off at
fixed head `17586246a` returned PASS with zero unresolved IMPORTANT or CRITICAL
findings. Exact report SHA-256 values:

- Sol: `228f5937dd7eafa6570ed857b3a8db43a1ed43c0a3c9ad6dcaf6e2d29ef8ebe4`;
- Fable: `7da9ca6810bde9db1035b68e1d2a46f3c0966c6610db7c19553acc96cacc13d3`.

Retirement conditions are satisfied: fixed refs and merge base reproduce;
`vendor/upstream` remains pinned; exactly four stashes remain untouched; the
sole user-controlled prompt diff retains SHA-256
`8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`;
no broad replay/rebase, ref move, stash mutation, worktree mutation, or history
rewrite occurred. R00's preservation rules continue as normal integration
governance; only the special recovery overlay gate is closed.
