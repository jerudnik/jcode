# Independent architecture/completeness re-review: post-recovery normalization infrastructure

- Reviewer: Fable (fresh, independent; did not read `docs/fork/normalization/reviews/` per instruction)
- Date: 2026-07-16T14:37Z (UTC)
- Mode: read-only. No file, ref, index, worktree, stash, service, process, runtime link, or host state was modified.

## Fixed refs

| Item | Value |
|---|---|
| Reviewed commit | `1f938b7e537a20aaad133ec300d0cfdc6368bca0` (`docs(fork): define post-recovery normalization`) |
| Parent (pre-infrastructure head) | `cdc2cc2b4cea51c185de330c8e15e08615acc46c` (matches BASELINE) |
| Branch HEAD at review | `recovery/2026-07-15` = `1f938b7e...` (reviewed commit is branch tip) |
| `main` | `6ca1fcf2ec2366c7abc99664a485c40d60cec80e` (ancestor of recovery, 0 unique) |
| `vendor/upstream` | `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Phase 6 closure | `ba45f20aa61fdf597bbe4a1d11e94d1dd43c8c38` (reachable from recovery HEAD) |
| Accepted recovery source head | `51168d16e9c708ae4afff09a6fc6402642d17782` (reachable; only docs paths changed after it up to `cdc2cc2b`) |

Files reviewed at the fixed commit: `docs/fork/normalization/{README,BASELINE,COMPLETION_STANDARD,COORDINATOR_BRIEF}.md`, `docs/fork/recovery/reviews/2026-07-16-w7-review.md`, `docs/fork/recovery/QUALITY_GATES.md`, `docs/fork/SYNC_MODEL.md`. `QUALITY_GATES.md` and `SYNC_MODEL.md` byte-match the working tree (SHA-256 `f877c58d...` and `b7c30775...`).

## Verdict

**PASS.** Zero unresolved CRITICAL or IMPORTANT findings. The normalization program as committed is actionable (each milestone names concrete commands, files, and exit conditions), measurable (binary D0-D9 gates, exact counts, exact hashes, exact expected-red values), safe (archive-before-delete ordering, approval gates on every destructive category, live-integration and Nix-managed-path protections), and sufficient (repo, history, quality, docs, runtime, host, and sign-off surfaces are all covered). Three MINOR findings are recorded below; none blocks.

## Correction-category matrix (verified independently from source)

| # | Category | Verdict | Evidence |
|---|---|---|---|
| 1 | Authority files tracked; dynamic authority-commit rule does not self-trigger drift | VERIFIED | `git ls-files docs/fork/normalization/` lists all four authority files plus the two preserved draft reviews. `git log -1 --format='%H' -- docs/fork/normalization/COMPLETION_STANDARD.md` returns `1f938b7e...`, reachable from branch HEAD (it is HEAD). BASELINE states "The authority commit itself is not drift" and fixes expected post-infrastructure dirty state as only `ORCHESTRATOR_PROMPT.md`; live `git status --short` shows exactly that, and the preserved prompt diff SHA-256 matches `8e8e6a92...`. |
| 2 | N1 ends without moving `main`; N2 promotion follows W7 | VERIFIED | README milestone table: "`main` does not move yet" (N1) and "promotion is the exit criterion of N2 ... not an N1 completion action." COMPLETION_STANDARD D2: "N1 ends when the curated integration branch carries the tree-equivalent recovered product tree ... `main` promotion is the exit criterion of N2, not N1" and "moves only by reviewed fast-forward." BRIEF N1.6 ("Do not move `main` in N1") and N2 closing sentence agree. No contradiction found across the three files. |
| 3 | Remote final state explicit | VERIFIED | D9: final handoff states "an explicitly authorized push ... or the qualified status `local-canonical, remote pending` with its risk recorded." BRIEF N6.4 repeats it verbatim and adds "Do not push merely to satisfy the wording." D2 forbids any push without separate explicit instruction. |
| 4 | Evidence placement bounded | VERIFIED | README: bounded dedicated documentation commits, not interleaved. D2: `docs/fork/normalization/evidence/` in "a bounded number of dedicated `docs(evidence)` commits at the top of the curated stack," named as the explicit exception to the evidence-churn prohibition. BRIEF N1.3 and N6.1 agree. |
| 5 | Branch/tag/stash inventories and all 916 non-recovery commits covered | VERIFIED | Live counts match BASELINE exactly: 40 branches, 138 tags, 29 worktrees (25 labs + 4 tmp), `git rev-list --all --not recovery/2026-07-15 --count` = 916, exactly 4 stashes with all 8 commit/index-parent IDs matching the BASELINE table. D0 requires the all-ref bundle to contain "commits not reachable from recovery"; BRIEF N0.4 names the 916 explicitly; D1 requires per-branch/per-tag keep/archive/delete decisions; the stash-object bundle requirement covers reflog-only `stash@{1..3}` (correctly identified as unprotected by `bundle --all`). |
| 6 | Trusted gates point to concrete command authorities with exact Phase 6 expected-red values | VERIFIED | D4 names `docs/fork/recovery/QUALITY_GATES.md` (reproduction commands exist: `scripts/check_panic_budget.py`, `scripts/check_swallowed_error_budget.py`, `scripts/rust_production_filter.py`, `tests/test_rust_production_filter.py`, dependency-boundary script all present) and the Phase 6 accepted audit driver (exists at `docs/fork/recovery/evidence/2026-07-16-phase6-final-audit/accepted/driver.sh` with manifest and 62-check record). I re-ran both budget gates live: panic `31 -> 48`, swallowed-error `2987 -> 3074`, exactly matching the expected-red values fixed in D4 and the BRIEF. No `--update` needed or invoked. |
| 7 | Sandbox daemon paths disjoint | VERIFIED | D6 requires a pre-start collision proof of disjoint socket/port/pid/marker/home/runtime paths and states the no-orphan rule "does not authorize stopping the pre-existing menubar, hotkey, or shared-server processes." BASELINE and BRIEF N4.2-N4.3 repeat both halves. Live process/launchctl inspection confirms the protected integrations are real and currently running (menubar, shared server, hotkey listener, `com.jcode.hotkey` loaded). |
| 8 | Final reviewers independent | VERIFIED | D9: "Neither reviewer authored, steered, or approved an implementation lane being reviewed." BRIEF: "Final D9 reviewers must be fresh and independent of implementation and steering." Two distinct reviewers (architecture + operational) at the same fixed commit and host manifest are required. |
| 9 | Upstream fetch authority bounded | VERIFIED | README mutation rule: fetch "may update remote-tracking refs but may not delete refs, replay commits, merge, rebase, or push." BRIEF N3.4 adds "after remote verification." Live `git remote -v` confirms `upstream` push is DISABLED, matching BASELINE. |
| 10 | W7d has a policy approver | VERIFIED | D3: "the user or an independent reviewer approves it as a product policy" before the W7d implementation merges, with checkpoint/newest-provenance/omission-marker preservation. BRIEF W7d repeats the propose-then-approve ordering. Consistent with the W7 review's "policy decision, not a casual truncation patch" and its 2 KiB UTF-8-safe recommendation. |
| 11 | Every session re-runs baseline | VERIFIED | BASELINE: "Run this block at the start of **every** normalization session and append the observation before resuming mutation," with a concrete command block (I executed its read-only core; all facts reproduced, including binary link targets and both SHA-256 anchors `fd6297d9...` and `a4973e8c...`). BRIEF opening: re-run, append, reconcile drift before mutation, with the authority files and program commits classified as expected evolution. |
| 12 | Curated commits have validation boundaries | VERIFIED | D2: "Each product commit builds and passes its focused tests, or an explicitly documented inseparable commit group is validated at its group boundary," plus a named-commit stack plan reviewed before integration. BRIEF N1.2 repeats both. |

## Spot checks of factual claims against source

- W7 review's code citations are real: `TurnInterruptedError` with display string `"turn interrupted"` and empty `Error` impl found at `crates/jcode-app-core/src/agent/evidence.rs` (lines ~226-235); the string-comparing fixture exists in `client_lifecycle_tests.rs`. RECOVERY_PLAN lines 130-132 and 413-433 contain the original W7 scope and the five LOW findings/defer table exactly as summarized.
- README's preserved draft-review hashes match: Fable draft `ac708e64...`, Opus draft `a9fe8180...` (recomputed from blob contents; contents themselves not read).
- BASELINE's "no non-document path changed after `51168d16`" verified: `git diff --name-only 51168d16..cdc2cc2b` shows only `docs/fork/recovery/...` paths.
- SYNC_MODEL's relative links resolve at the commit: `docs/fork/patch-ledger.md`, `docs/architecture/FORK_SUSTAINABILITY_MODEL.md`, `docs/architecture/MCP_SERVER_REGISTRATION_GUARDRAILS.md`, `docs/SERVER_LIFECYCLE_INVARIANTS.md` all exist.
- BRIEF's required-reading files (`PROGRESS.md`, `RECOVERY_PLAN.md`, w7-review, SYNC_MODEL) all exist at the commit.
- `main...recovery` = 0/166 at the pre-infrastructure parent (as BASELINE states "at snapshot") and 0/167 at HEAD, the +1 being exactly the authority commit, which BASELINE pre-declares as non-drift.

## New findings

| ID | Severity | Finding |
|---|---|---|
| NF-1 | MINOR | `docs/fork/normalization/README.md` ends with a stray duplicated fragment: the file's last two lines are `...never combined in one step.` followed by the garbage line `tep.` (verified by octal dump of the committed blob). Editing artifact; no normative meaning is altered. Fix in the next docs commit. |
| NF-2 | MINOR | `docs/fork/normalization/COORDINATOR_BRIEF.md` ends with a stray lone backtick after the closing code fence (`` ``` `` then `` ` ``). Cosmetic; could mildly confuse Markdown renderers when the brief is copy-pasted as instructed. Fix alongside NF-1. |
| NF-3 | MINOR | Two forward-looking count interactions are not pre-declared: (a) creating the mandated `refs/archive/normalization/stashes/*` refs will change the session-start `git rev-list --all --not recovery/...` count above 916, and (b) SYNC_MODEL's "190+ fork-only commits" is stale (454 by `vendor/upstream..recovery`). Both are already absorbed by existing mechanisms (append-only drift reconciliation treats program-made changes as expected evolution; README/N3 explicitly require the SYNC_MODEL refresh), so neither blocks, but the next baseline append should note (a) explicitly to avoid a false drift alarm. |

Non-finding worth recording: `QUALITY_GATES.md`'s truth table shows Phase 0 snapshot reds (panic 46, swallowed 3,077) that differ from the Phase 6 expected reds (48, 3,074). This is not a contradiction: QUALITY_GATES is an explicitly historical Phase 0 record used by D4 only as the trusted *command* authority, and D4/BRIEF anchor the *expected-red values* to Phase 6 exactly. The live tree reproduces the Phase 6 values, not the Phase 0 ones.

## Validation performed (all read-only)

- `git cat-file`/`git show` of all seven authority files at the fixed commit; blob hash comparison for the two files shared with the working tree.
- Live reproduction of the BASELINE session-start block core: status, branch, all fixed refs, `main...recovery` counts, branch/tag/worktree/stash/916 counts, stash commit + index-parent IDs, remotes, prompt-diff SHA-256, `type -a jcode`, all four binary links/targets, both binary SHA-256 anchors, process table, and both launchctl agents. Every value matches BASELINE.
- Live execution of the panic and swallowed-error budget gates (read-only scripts, no `--update`): reds reproduce exactly `31 -> 48` and `2987 -> 3074`.
- Existence checks for every command/file authority referenced by D4, the BRIEF reading list, and SYNC_MODEL links.
- Ancestry proofs: `main`, Phase 6 closure, and accepted source head are all ancestors of recovery HEAD.
- Spot verification of W7-review code and RECOVERY_PLAN line citations.

## What was not checked

- The two preserved draft reviews' contents (excluded by instruction; only their SHA-256 preservation was verified).
- Cargo builds, test suites, the 62-check Phase 6 driver re-execution, production/test size gates, dependency/wildcard/warning gates (only the two budget gates were re-run live; the rest were verified as existing command authorities).
- Bundle creation/restoration mechanics (would mutate refs; the BASELINE recipe was reviewed statically and is correct about `bundle --all` not covering reflog-only stashes).
- Contents of credential files, `~/.jcode` internals beyond the four documented links, the home-manager Nix closure, and the full host inventory the BASELINE itself defers to the next session.
- The durable initiative store (`post-recovery-fork-normalization`) and any state outside the repository and the enumerated host facts.
- Remote GitHub state (no fetch performed; remotes inspected by config only).

## Confidence

High for document consistency, gate measurability, safety ordering, and every baseline fact I reproduced live (all matched exactly, including four SHA-256 anchors). Medium for sufficiency of the deferred full host inventory, which the program itself correctly scopes to N0 rather than claiming now.
