# Phase 4 Architecture-Gate Independent Review — RECOVERY_PLAN.md

- Reviewer: fresh independent architecture-gate reviewer (verify posture, read-only).
- Fixed commit reviewed: `76ead5607032ef9e574979a779f6fddc60607b23` on branch `recovery/2026-07-15`.
- Repository: `/Users/jrudnik/labs/jcode`.
- Constraints honored: no repository/Git/stash/branch/worktree/daemon/build mutation; no network, credentials, live providers, tools/MCP, memory, publication, install, or reload. Only local read-only Git, `shasum`, grep, awk, sed, and source inspection were used. No Cargo/Nix build ran. No child agent spawned.
- Method: recomputed plan and Fable artifact hashes; diffed the two Fable proposals; read all seventeen seam ledgers directly for their disposition lines rather than trusting README/coordinator prose; cross-checked W0-W7 against R12/R05B/R08A/R10/R09 rollups; verified fix-chain ancestry, decisive source lines, and preservation state.

## VERDICT: PASS

Confidence: high for the sync-posture decision, authority model, disposition arithmetic, workstream-to-ledger mapping, and preservation state (all directly reproduced). Medium only where the plan itself declares medium (W3/W5 effort bounds), and those are carried as named blockers/defers, not assumed. Every decisive claim I checked reproduced; no blocking finding.

---

## 1. Hash recomputation and Fable-artifact preservation (check 1) — PASS

- Committed `RECOVERY_PLAN.md` at HEAD and the working-tree copy both hash `cd1bc5e423ce18c418853216eb5a8786115e2faa4065495b8029917dc8790e76`; `git status` shows the plan clean (not part of the dirty tree). The plan is committed, not floating.
- Preserved Fable artifacts (byte-exact, committed):
  - `reviews/2026-07-15-phase4-fable-initial-plan.md` = `f9d892d96a289606f643a264cf157ccd29dff80f11fdca8e0a0298a494fb1d7f` (committed in `76ead5607`).
  - `reviews/2026-07-15-phase4-fable-corrected-plan.md` = `b0bae9803fa726a489e0560fdc423daefa20bd8478ede0aa2772f7684ea21eb9` (committed in `76ead5607`).
  - `reviews/2026-07-15-g5-g4-evidence-opus.md` = `37f094d26b196612f2171de98d52238abb72bb8b69d59b149e7bb00999db86d3` (matches the hash cited in RECOVERY_PLAN.md line 5, committed earlier in `e2464666d`).
- **Bounded correction history verified.** `diff initial corrected` yields exactly four hunks, and they match the plan's claimed corrections (lines 7, 258) precisely:
  1. Added v2 revision note (points v1 to `/tmp/jcode-phase4-recovery-plan-fable.md`).
  2. §1 item 3: "Thirteen ... retain-fork (…without R04…)" -> "Fourteen ... retain-fork (…with R04…)" plus the R04-authority clarifying sentence.
  3. §3 W1 acceptance: single-request wording -> "one correlated terminal response per emitted request; retry paths may emit multiple requests."
  4. §9 findings: "thirteen retain-fork" -> "fourteen retain-fork."
  No other bytes changed. The correction is exactly the two claimed audit fixes, nothing smuggled.

## 2. Seventeen-responsibility disposition arithmetic (check 2) — PASS

- §5 disposition table lists all seventeen IDs exactly once: R00, R01, R02, R03A, R03B, R04, R05A, R05B, R06A, R07A, R07C, R08A, R09, R10, R11, R12, R13. No duplicates, no omissions.
- Disposition arithmetic verified **directly from each ledger's `Recommended disposition` / `Disposition` line**, not from README prose:
  - `retain-fork` (14): R00, R01, R03A, R03B, R04, R05A, R05B, R06A, R07A, R07C, R09, R11, R12, R13.
  - `compose` (2): R02, R10 (each names narrow upstream candidates — R02 tier/catalog, R10 draft-release finalization).
  - `defer` (1): R08A.
  - 14 + 2 + 1 = 17. Matches the plan.
- **R04 specifically** (the audit-flagged item): ledger line reads `| Recommended disposition | \`retain-fork\` |`, and its authority table says "`retain-fork` is the sole disposition for the fork-owned defense layer." The plan's v2 correction (moving R04 into the retain-fork count) is factually correct; the v1 count of thirteen was the error, correctly caught by the coordinator audit.
- Note (nonblocking): R05B's ledger *disposition* is `retain-fork` while its *state* is `blocked`. The plan represents this exactly (§2 "R05B (source-blocked)"; §5 "Required W2. Remains `blocked`"). No misrepresentation.

## 3. Curated composition supported by structure + pilot, not replay-economics (check 3) — PASS

- The plan's §1 explicitly and repeatedly disclaims the pilot as an economics test: line 23 "It was a **behavior-composition fixture**, not a replay-versus-curated economics experiment. No replay was ever attempted, so 'replay is uneconomical' is a structural inference." This matches the G4/G5 record (G5 review §6: proves only the exact bounded question; G4_RESULT claim-limit section).
- Structural support is real, not asserted: PRESCREEN patch-ID emptiness, the single-parent squash `b3ed82a6b`, and fourteen retain-fork ledgers with no upstream counterpart for the evidence spine (R06A/R12 ledgers confirm `git cat-file` absence upstream). I confirmed the vendor pin `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` equals the merge-base `git merge-base 7ff4fc6be 802f69098`.
- **No broad-sync/replay authority overreach.** §1 posture rules forbid broad merge ("No slice may 'use a broad merge to make the diff disappear'"), require every import to be a named `sync` slice with provenance, and keep `vendor/upstream` pinned. The pilot's composability finding is scoped to "the load-bearing property curated composition needs," not to sync authority. No overreach.

## 4. W0-W7 audit: order, status, one class per commit, seams, tests, stops, rollback, reviewers, gates (check 4) — PASS

- **Dependency order** (§3 mermaid + prose): W0 first (all others depend on it); W1->W3, W1->W7, W2->W7; W2/W5/W6/W4 depend only on W0. This is acyclic and each edge is justified (W3 needs W1 because wait-like results interact with R12 evidence; W7 needs the owning fix pinned first). Consistent throughout §3, §4, §6.
- **Required vs conditional vs optional** (§4): required = W0, W1, W2, W3, W5, W6; conditional = W4 (product-gated, becomes a documented defer absent product truth); optional = W7. Stated consistently in §4 and §5.
- **One class per slice** honored, and where a workstream carries two classes it explicitly splits them into separate commits/slices: W2 separates fixes (`fix`) from the liveness consolidation (`refactor`); W4 separates `sync` from optional `refactor`; W6 is "two slices, strictly separate" — a behavior-fix slice and a sync slice with distinct commits; W5 separates the review `docs` sub-slice from the `fix`. §6 commit-hygiene rule reinforces "sync, fix, refactor, QoL, and docs are always separate commits." No slice mixes classes within one commit boundary.
- **Owning seams, acceptance, stops, rollback, reviewers, external-action gates** are present for every workstream (W0-W7 each carry Owner, Prerequisite, Surface/Acceptance, Tests/observations, Stop, Rollback, Commits, Reviewer, Later external action). External-action gate is correctly set only where warranted: W6 alone carries "Later external action: **yes**" (release/publish/install requires separate authorization); all others "none." §6 restates that reload/daemon/debug-socket/publication/network require separate authorization not granted here.
- No workstream is unsupported: each maps to a named ledger slice (W1->R12 slice 3; W2->R05B fixtures 1-6; W3->R04 items 1-8; W4->R02 checkpoints; W5->R08A escalation; W6->R10 items 4-5). No unnecessarily broad workstream; each surface is a bounded file set.

## 5. W1 vs R12 rollup: per-request terminal cardinality (check 5) — PASS

- The R12 ledger's governing invariant (lines 54-60) is "one correlated terminal `ProviderResponse` for every emitted `ProviderRequest`; ... one response **per provider request**, not one provider response per user turn." W1's acceptance (plan line 94) states exactly this: "exactly one correlated terminal (non-`Ok` where applicable) `ProviderResponse` per emitted `ProviderRequest`, and exactly one `TurnFinished` per user turn; retry paths may legitimately emit multiple requests." Faithful, not overstated.
- Retry multiplicity handled correctly: W1(b) requires that "every emitted request, including each abandoned attempt, either receives its own correlated non-`Ok` terminal response or has its request emission delayed until non-abandoned, with no orphaned request and exactly one `TurnFinished`." This is verbatim the R12 ledger slice 3 remedy (ledger line 235) and the matrix retry-orphan finding (line 102).
- **No fixed strict-path work reopened or already-fixed slice repeated.** R12 slice 2 (raw transport `Err` / blocking `StreamEvent::Error` correlated error responses) is already integrated — commits `a4d673ffd` (fix), `8bb7afc16`/`8ac1c0f55` (test) are reachable from HEAD, and `2ef1041f9` (the §9-cited chain commit) is an ancestor. W1 scopes only slice 3 (cancellation/retry) and requires "(c) the five existing `r12_*` fixtures stay green" and "(d) no duplicate-response path." The strict no-tool/non-retry path stays qualified; W1's stop condition is "changed success-path behavior." No repetition, no reopening.
- I confirmed the six `append_provider_error_response` call sites (3 in each engine) and the helper at `evidence.rs:142` exist, matching the plan's "reuse the existing helper; prefer no new abstraction."

## 6. W2/W3 (R05B/R04 blockers + overlap), W4 (product blocker), W5 (R08A/R02 + no credential), W6 (R10 + R01 + classes + no publication) (check 6) — PASS

- **W2 vs R05B:** R05B ledger state is `blocked` (line 5) with four adjudicated defects — explicit `Visible` silent headless fallback (BLOCKER, `comm_session.rs:619-691`), stale takeover erasing history, cap-fail overwriting `checkpoint_summary`, duplicated liveness predicates (`swarm.rs:372-377` vs `comm_control.rs:734` vs `swarm_verbs.rs:55`). W2 targets exactly fixtures 1-6 and these four defects, with the R05A entry criterion (reproduce the two control-log tests, closing the flagged verification gap). Correct.
- **W3 vs R04 + overlap:** R04 lifecycle widening remains gated; W3 executes items 1-8 plus the two carried Fable IMPORTANT follow-ups (disconnect terminal-persistence distinguishability; marker-lock liveness edge). Plan explicitly handles W2/W3 path overlap: "not concurrent with W2 if `swarm.rs`/`comm_session.rs` overlap emerges (check paths at branch time; if overlapping, serialize)" (line 107), and §6 lists W2+W3 as "Never concurrent." Overlap risk correctly gated.
- **W4 product blocker:** B1 (line 222) blocks W4 on "a recorded product entitlement decision (owner: user/coordinator; not an agent decision)," matching R02 ledger's "product truth absent" stop condition. Correctly conditional.
- **W5 vs R08A/R02 + no credential:** W5 is "preceded by the ledger-mandated full R08A/R02 joint review (a docs/evidence sub-slice)," matching R08A's retirement condition (ledger line 60: "retires into a full R08A/R02 joint review before onboarding/import is exercised"). Acceptance requires "no credential read occurs without explicit approval"; stop condition forbids "any live credential/import." R02 boundary preserved (R08A collects a selected set only; R02 owns credential validation). No credential exercise. Correct.
- **W6 vs R10 + R01 + classes + no publication:** W6 preceded by "the ledger-mandated full R10 review" (matches R10 retirement condition, ledger line 49). Splits into strictly separate behavior-fix (SHA256SUMS-absent refusal; `install.sh` verify-or-retire; activation delegates to R01) and sync (draft-to-final `release.yml`) slices — matching R10 findings 4/5 and the R01 boundary. Acceptance is "workflow lint/dry-run only (no tag, release, or publication)"; "Later external action: **yes**" gates any real release/install/update. No publication/install/update authorized. Correct.

## 7. Observe-only / deferred seams: owner, reason, evidence gap, escalation; R09 debt not converted (check 7) — PASS

- §4's deferral table gives every deferred item an owner, reason, evidence gap, and escalation trigger (R09 red debt; R02 residuals; R01/R03A residuals; R12/R13 turn.rs:724 window; R03B WebSocket; R07A/R07C fixture/isolation; hot-path stashes; W7 refactors; W5 upstreaming; deferred seams R06B/R07B/R08B/C/D). I confirmed R06B/R07B/R08B/C/D are `defer` with named triggers in `RESPONSIBILITIES.md`.
- **R09 expected-red debt is not turned into cleanup or green.** §4 row 1 states expected-red is "attributed, visible, and non-blocking by design; blanket cleanup is prohibited by R09 overlay rule 2," with escalation "`--update` anywhere is an immediate incident." §5 R09: "no `--update`; debt moves only via owning-seam remediation. Not a workstream." §7 checklist item 4: "four expected-red ratchets remain red unless genuinely remediated." This matches R09 ledger rules 2-3 (no blanket update; debt stays visible and seam-attributed). No conversion to blanket cleanup or green evidence. The 60-vs-61 drift is handled as a historical note (W0), consistent with the ledger recording 60 at Phase 0 head and G0/G4 truth 61.

## 8. Concurrency, integration cadence, preservation, no overlapping writers, no live action (check 8) — PASS

- §6: "At most two full slices in flight, and only when file surfaces are provably disjoint." Approved concurrent pairs (W1+W2, W5+W6, W1+W5, W2+W6) are genuinely path-disjoint (W1 = agent turn files; W2 = server/swarm; W5 = tui onboarding + external-auth; W6 = update/install/release workflow). "Never concurrent" set includes W2+W3 and anything+W0. No two overlapping writers.
- Integration cadence: coordinator integrates one slice at a time onto `recovery/2026-07-15` with clean-tree check, cherry-pick, targeted tests, full R09 matrix (no `--update`), doc validation, PROGRESS checkpoint; cross-seam regression floor after each pair. Matches orchestrator Phase 5.
- No unapproved reload/debug/live action: §6 explicitly requires separate authorization for reload/daemon/debug-socket/publication/network; a no-reload `selfdev` build is permitted only to validate, not to reload. Consistent with orchestrator Phase 4 (plan is a user-visible checkpoint, no external action).
- Preservation rules restated in §3 (no stash pops, ref moves, force pushes, prompt edits) and §7 checklist item 1 (branch, sole dirty prompt path, 4 stashes, vendor pin, prompt hash).

## 9. Coordinator audit accuracy + working-tree state (check 9) — PASS

- §10 coordinator audit accurately describes the v1->v2 corrections (R04 count omission; retry overstatement) and introduces **no new authority**: its verdict is "PASS, pending independent review of the fixed plan commit," and it enumerates every still-gated action (live provider, credential, network, daemon/reload, publication/install/update, tool/MCP, memory, swarm, cancellation/retry pilots). It does not grant any external action; it re-affirms W4/W5/W6 blockers.
- **Working-tree state:** `git status --porcelain` shows exactly one modified file, `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`. `git diff` (full tree) and `git diff` of that file alone both hash `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00` — the prompt is the sole dirty content. `git stash list` shows exactly four stashes (fix-config-hotpath-spam parts 3/2/1 + "wip before upstream sync"). Matches every citation in the plan and coordinator audit.

## Supporting verifications (decisive claims independently reproduced)

- Fix-chain ancestry at `76ead5607`: `a371fe758`, `eab42e1b5` (R04 marker fix), `2ef1041f9` (R12 chain, §9 citation) are all ancestors; R12 slice-2 fix/test commits (`a4d673ffd`, `8bb7afc16`, `8ac1c0f55`) reachable.
- `crates/jcode-app-core/src/server/client_api.rs:99` is exactly `runtime_identity: None,` (invariant-1 residual, §2/§4).
- W0 drift claims all true: R04 ledger line 300 still says "a fresh independent source-fix sign-off is still [required]" while the marker-fix Opus (`7a8f2449...`) and Fable (`1ec0ceb5...`) reviews both PASS; the five "Coordinator approval: pending" lines exist (R03B/R05A/R07A/R08A/R10) while the remaining-light-ledgers Opus review (`b537bc56...`) passes all five; R00/R09/R11 carry "Fable review: pending independent Phase 4 architecture review." These are genuine docs-closure items, not evidentiary holes.
- Pilot log has exactly one standalone `PILOT_OBSERVATION` line; vendor pin and merge-base both `631935dd1...`.

## Blocking findings

None.

## Nonblocking limitations / observations

1. W6 is described as a single workstream carrying two slice classes (behavior-fix + sync). This does not violate the one-class-per-commit rule because the plan strictly separates them into distinct slices/commits with distinct reviewers-of-record and an explicit external-action gate on the sync side. It is a labeling choice, not a class mix. Worth watching at execution time that the two slices are branched and reviewed independently.
2. The plan's §9 "Validation performed" cites `2ef1041f9` as the R12 fix chain; the ledger rollup also references `61f241a9d`/`4ed674f14` as slice commit IDs that are not ancestors of HEAD under those exact short hashes (the integrated equivalents are `a4d673ffd`/`8bb7afc16`/`8ac1c0f55`). This is a ledger-internal provenance-labeling nuance, not a plan defect: the R12 slice-2 behavior is demonstrably integrated and the plan's own cited commit (`2ef1041f9`) is an ancestor. Nonblocking; flagged for the W0 record-consistency pass.
3. R09 count 60-vs-61 and swallowed 3,074/3,077 appear across artifacts at different HEADs. The plan handles this as an append-only historical note; internally consistent, but any future citation must state the HEAD. Nonblocking (the plan already says so).

## What I did NOT check

- I ran no Cargo/Nix build and executed no fixture, driver, or test; pass counts (classifier 17/17, r12 fixtures, R05A control-log tests) are taken from preserved manifest-verified logs and ledgers, not fresh compilation.
- I did not exercise any daemon, network, provider, credential, tool/MCP, telemetry, reload, install, or release path.
- I did not line-audit the full Opus/Grok per-seam review corpus beyond the adjudicated ledger content, the R04 marker-fix reviews, the remaining-light-ledgers review, and the G5 review; I trusted preserved review verdicts where their hashes reproduced.
- I did not audit the `b3ed82a6b` squash contents, `.rerere-cache`, the desktop crate, deferred seams' source (R06B/R07B/R08B/C/D), upstream's divergent commit set, or the immutable build-artifact hash on disk.
- I did not independently re-derive G2/G3 authority beyond confirming the G5 review and ledger citations; I did not re-run the pilot driver's unit tests.
- I did not verify W3/W5 effort bounds (the plan itself carries these as medium-confidence).

## Overall

The plan is evidence-backed, reversible, informed by (and honest about the limits of) the bounded pilot, and does not mix synchronization, fixes, refactors, or QoL work within a commit boundary. Disposition arithmetic (14/2/1) reproduces from the ledgers, including the R04 correction. W1 faithfully encodes the R12 per-request terminal-cardinality invariant with retry multiplicity. Every observe-only and deferred seam has owner/reason/gap/trigger; R09 debt stays red and attributed. Concurrency, preservation, and external-action gates obey the orchestrator, and no authority is transferred or external action authorized. Working tree is the sole prompt diff `8e8e6a92...` with four stashes. **PASS.**
