# Fork Sustainability — Hardening Findings & Backlog (Swarm Synthesis)

Date: 2026-07-08. HEAD `75b20b215`. Vendor `github/vendor/upstream` = `631935dd1`.
Method: adversarial three-group analysis (RED attack / BLUE defense / GREEN
referee) against `FORK_SUSTAINABILITY_MODEL.md`. Full briefs:
`FORK_HARDENING_RED_BRIEF.md`, `FORK_HARDENING_BLUE_BRIEF.md`,
`FORK_HARDENING_GREEN_SCOREBOARD.md`.

## Verdict: the model HELD; the doc DECALIBRATED; one invariant is BROKEN

The architecture the model chose — `rerere` + `doctor` + additive-seam discipline,
with jj / Nix-feature-variants / named-daemons deferred — is validated by ground
truth, *more strongly* than when written:

- The fork grew ~6x (30→180 commits over vendor) yet stayed **≈30:1 additive**
  (17,241 `.rs` lines added vs 581 deleted); **113 of 186** touched `.rs` files
  delete nothing. The model's one load-bearing claim ("deletions conflict, additions
  don't") is the thing that held.
- Every deferred heavy option stayed unbuilt and **nothing broke for its absence**
  (no `nix/features/`, no jj). The escape hatch (`overrideAttrs.patches`) was used
  exactly once (Mermaid) as designed.
- The 6h rebase rail, rerere replay (100 recorded resolutions), and `jcode doctor`
  are all live and healthy; the fork gate is green.

But three things genuinely drifted or broke and are worth fixing. None requires new
architecture; all are cheap.

## Ranked hardening backlog

| ID | Severity | Finding | Fix | Cost |
|---|---|---|---|---|
| **H1** | **BROKEN** | `main` carries a `.github/workflows/sync.yml` diff over `distro/nix` (`1f5cc7aac`); `fork-health.sh` check 5 fails on live refs today. The invariant the fork built a daily workflow to enforce is red. | Move the CI-policy commit to `distro/nix`; promote check 5 from daily-issue-bot to a **blocking pre-merge gate** on `main` so CI drift can't land past its own guard. | script + branch op |
| **H2** | HIGH | The model's four headline measurements are false at HEAD (180 not 30 commits; 16 not 7 rewrite files; agent.rs deletes 19; skill.rs deletes 12). A governing doc citing stale numbers is a liability. | Add `scripts/fork-divergence-report.sh` that regenerates the GREEN scoreboard; replace the model's hardcoded 2026-06-27 table with "run the report" + a committed snapshot. Doc can never silently decalibrate again. | 1 script |
| **H3** | HIGH | Governance gap: agent.rs compaction-gating (19 del), the swarm/comm subsystem (8 upstream-edited files, ~120 del), `compaction.rs`, `session_search.rs`, `conversation_state.rs` have **no** patch-ledger rows and are outside `FORK_REWRITE_SEAM_AUDIT.md`. Ledger governs ~half the real invasive surface. | Add ledger rows (class / retire-condition / validation cmd) for each family; extend the seam audit to walk all 16 offenders, not 7. Documentation, not refactor. | doc, ~1 para each |
| **H4** | MED | skill.rs "converted additive 20→0" claim is false (12 deletions). It is a deliberate `.apm` extension (`.agents` preserved in-loop), not rot — but the doc's flagship success story is stated wrong. | Correct the model + seam-audit prose to describe the `.apm`/`.agents` two-dir loop; either re-shape as a true zero-deletion prepend or annotate why 12 deletions are the minimal seam. | doc (+ optional small refactor) |
| **H5** | LOW | rerere cache is unbounded (100 entries / 199 tracked files / 7.3M), no prune or staleness policy. Healthy now, but a recording against a since-deleted upstream hunk replays forever and could mis-resolve a lookalike conflict. | Add `scripts/rerere-cache.sh prune` (drop recordings whose preimage no longer matches any live conflict over N syncs) and document the bound. Defer if H1–H3 fill the cycle. | small script |
| **H6** | LOW/noise | RED flagged the budget ratchets (`swallowed_error_budget.json` etc.) and extra workflows as a returned "executable patch ledger." GREEN/BLUE adjudication: these govern **fork-internal code quality**, not upstream patches — a different, already-working concern. | No action. Optionally add one sentence to the model clarifying the two scopes (divergence vs internal quality) so this doesn't get re-litigated. | 1 sentence |

## Keep / cut verdict (unchanged from the model, now re-confirmed)

- **KEEP:** rerere + rail + doctor + additive-seam discipline. Validated at 6x scale.
- **CUT (stay deferred):** jj / patch-queue managers, Nix feature-variant tier,
  named daemon instances. Zero features needed them in 180 commits; 581 deletions is
  not a patch stack.
- **The escape hatch (`overrideAttrs.patches`) remains the right home** for the rare
  dependency-graph patch (Mermaid precedent).

## Recommended execution order

1. **H1** (BROKEN, cheap, highest confidence) — restore the invariant + make it block.
2. **H2** (stops the doc re-rotting) — the regenerable baseline underwrites everything.
3. **H3** (closes the real governance gap) — ledger/seam rows for the invasive surface.
4. **H4** (correctness of the flagship claim), then **H5** if cycle remains. **H6** is
   a one-line clarification.

None of these is a new subsystem. The swarm's cross-examination confirms the fork is
**well-architected but under-documented at its current scale**, with exactly one live
invariant break. Fixing H1–H3 restores the doc as an accurate governing artifact.

---

### Process note (for the operator)

This analysis was run via the SDK `Agent` subagent mechanism, not the native
in-jcode `swarm` tool. The native `swarm`/`communicate` tool is only mounted in
jcode's own agent-loop sessions (registered as `"swarm"` in
`crates/jcode-app-core/src/tool/mod.rs:219` = `CommunicateTool`); this `/loop`
self-dev session is Claude-Agent-SDK-hosted and exposes the harness toolset instead,
so `swarm` was never in-schema here — an environment boundary, not a regression in
the (separately hardened) swarm subsystem. Two subagent waves were also killed
mid-flight by server auto-reloads onto the dirty selfdev binary (~210s in); the final
briefs were completed directly against captured ground truth.
