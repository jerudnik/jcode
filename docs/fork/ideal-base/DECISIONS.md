# Ideal-base decisions

Append new decisions. Do not rewrite prior decisions to make the program appear
more linear than it was.

## D001. Archive recovery and normalization in place

**Decision:** `docs/fork/recovery/` and `docs/fork/normalization/` remain at their
existing paths as frozen historical namespaces.

**Reason:** the trees contain 600-plus evidence, review, and seam files with
relative links, checksum manifests, and hash-cited records. Moving them creates
integrity risk without improving execution. The active authority moves to
`docs/fork/ideal-base/`.

**Reopen trigger:** an explicitly authorized archive migration with a complete
link, checksum, and citation rewrite plan.

## D002. Preserve the historical orchestrator prompt byte-for-byte

**Decision:** do not edit `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`.

**Reason:** current records state it was restored to tracked baseline and retained
because many historical documents reference it. Archival warnings live in parent
indexes and the active baseline instead.

**Reopen trigger:** explicit user authorization to break the tracked-baseline
preservation guarantee.

## D003. Use graph structure for execution and repository state for restart

**Decision:** the live deep task graph schedules work and enforces artifacts and
gates. `WORK_GRAPH.json`, `STATE.json`, reachable commits, and evidence provide the
cross-session restart authority.

**Reason:** graph artifacts provide typed dataflow while repository checkpoints
survive coordinator or daemon loss.

**Reopen trigger:** a demonstrated task-graph persistence mechanism that makes the
repository state redundant without weakening recovery.

## D004. Separate implementation from acceptance

**Decision:** foundation-critical implementation requires a distinct verification
node or independent reviewer. A failed verifier injects a fix node and repeats the
same gate.

**Reason:** implementation self-assessment is insufficient for lifecycle,
persistence, packaging, and signoff claims.

**Reopen trigger:** none expected. Any exception requires a written risk decision.

## D005. Keep external gates honest and separate

**Decision:** provider, platform, Apple, credential, publication, and push work is
represented in the graph but cannot execute without the applicable authorization.
`authorization_blocked` is a valid explicit disposition and never means passing.

**Reason:** deterministic foundation work should proceed without silently spending,
publishing, or mutating external systems.

**Reopen trigger:** explicit authorization for the named gate and bounded scope.

## D006. Preserve the observed stale pending activation as F09 reproduction evidence

**Decision:** the stale selfdev `pending_activation` observed at session start on
2026-07-18 (requested 05:45:12Z by dead session
`session_peacock_1784221108198_12fe3e2e04160f62`, with `new_version` equal to
`previous_current_version` `923c6353e-dirty-5a0f07fa7495`) is left untouched.
No promotion, rollback, or reload is performed on it by the coordinator.

**Reason:** it is a live instance of the exact failure class node F09 must
reconcile. Clearing it by hand would destroy the best available real-world
fixture and would mutate runtime state outside the graph. The drift is
classified in `evidence/W0.1/drift.md`.

**Reopen trigger:** F09 implementation lands with reconciliation logic, or the
user explicitly asks for a manual manifest repair first.

## D007. Quarantine the stale persisted swarm plan before railway seeding

**Decision:** the persisted swarm plan keyed to this repository's `.git`
still contained the completed historical recovery program (P*, G*, w3-*
nodes). Seeding W0.2 with `task_graph` merged into that plan, and `run_plan`
resurrected five stale nodes (G4-pilot-execute, P3_gate_recheck, w3-cluster-b,
w3-cluster-c, w3-cluster-d) with fresh workers. Those workers were stopped
within minutes; one had added a partial test to
`crates/jcode-storage/src/active_pids.rs`, preserved as stash
`stale-plan worker (w3-cluster-d/blowfish) ...` rather than deleted. The full
pre-reseed plan snapshot is saved at
`docs/fork/ideal-base/evidence/W0.3/pre_reseed_plan_snapshot.json`. After the
in-flight W0.2 node completes, the stale plan will be cleared
(`swarm:clear_plan`) and the railway graph reseeded cleanly.

**Reason:** the recovery program is a frozen historical namespace; its plan
nodes must not execute again. Clearing the server-side plan does not rewrite
history because all recovery evidence lives in the repository, and the
snapshot preserves the final plan state.

**Reopen trigger:** none. If the stashed worker diff proves useful for F26 it
may be cherry-picked by the F26 owner.

## D008. Apply W0.2 census amendments GN-1, GN-2, GN-5 to the work graph

**Decision:** based on the accepted W0.2 source census
(`evidence/W0.2/source_census.md`, commit `fb00ab840`):

- F06 owned path `src/cli/commands/**/*mcp*` (matched zero files) is replaced
  with `src/cli/mcp_serve.rs` and `src/cli/dispatch.rs`.
- F09 gains owned path `crates/jcode-selfdev-types/src/**` because
  `PendingActivation` lives there.
- F04 gains the explicit acceptance gate "Status-serialization and write
  failures are surfaced, not swallowed".

GN-3 (reuse `OwnedChildPermit`, no second cap counter) and GN-4 (startup PID
sweep pre-exists; F26 starts with a verify of the existing sweep) are recorded
as binding scope guards for F12 and F26 owners rather than graph edits. GN-6
is an observation only.

**Reason:** implementation nodes cannot commit inside their ownership boundary
when the boundary names nonexistent paths, and gates must cover the confirmed
swallowed-error behavior at `background.rs:133`.

**Reopen trigger:** further source drift discovered by any F-node owner.

## D007a. Stale-worker stash resolved by commit

**Amendment to D007:** the preserved stash ("stale-plan worker
(w3-cluster-d/blowfish) ...") was applied and committed as `715d5fd21`
(test(r04): complete streaming-marker 2x2 replacement matrix) during the F01
window; the stash entry no longer exists. Coordinator verified
`cargo test -p jcode-storage --lib active_pids`: 10/10 pass at that commit.
The change is a bounded test addition consistent with F26's seam and is
retained on main.

## D009. Temporary review-model substitution: OpenAI for Opus-class

**Decision:** Anthropic usage is exhausted as of 2026-07-18T07:17Z (user
notice). Until further notice, "Opus-class" verification/critique nodes run on
the strongest available OpenAI route (`gpt-5.6-sol` at high effort, falling
back to `gpt-5.5`). Review artifacts must name the actual model used.

**Reason:** the railway must keep moving; the review-model requirement is
about independent adversarial capability, not vendor identity.

**Reopen trigger:** Anthropic usage restored; subsequent reviews may return to
Opus-class models. Already-accepted reviews are not re-run solely for vendor
identity.

## D009a. OpenAI model routing by difficulty

**Amendment to D009 (user-specified):** while Anthropic usage is unavailable,
route OpenAI workers as follows:

- `gpt-5.6-sol`: hard, critical implementation and adversarial verification.
- `gpt-5.6-terra`: medium-complexity implementation, review, and investigation.
- `gpt-5.6-luna`: easy/non-critical context retrieval, search, and summarization.

Do not substitute GPT-4o. Use the actual route name in evidence and review
artifacts.

**Reopen trigger:** user changes routing or Anthropic usage returns.

## D010. Revert accidental frozen-recovery mutation from stale scheduler work

**Decision:** stale scheduler work committed `feeef1d4e`, adding
`docs/fork/recovery/evidence/2026-07-18-p3-gate-recheck/README.md` after the
ideal-base session had declared `docs/fork/recovery/` frozen. The coordinator
immediately reverted it with commit `3e479972f`. Tree comparison against the
pre-incident head shows no remaining recovery-tree diff, and the protected
orchestrator prompt hash remains `ca3f1998...eed5b6`.

**Reason:** frozen historical namespaces must remain byte-for-byte historical;
a new commit in that namespace was unauthorized even though it did not alter
the protected prompt. The revert preserves Git history while restoring the
required tree state.

**Reopen trigger:** none.

## D011. Expand model rotation and fail F01 before implementation

**Decision:** user-approved non-Anthropic rotation now also includes Kimi K3
(frontier), Cursor Grok, GLM-5.2, DeepSeek-V4-Pro, and MiniMax M3. D009a remains
the default OpenAI routing: Sol hard/critical, Terra medium, Luna retrieval.
These additional models may provide independent lanes where useful; GPT-4o is
explicitly excluded.

The independent F01 review (`7563a1237`, OpenAI `gpt-5.6-sol`, high effort)
returned FAIL with three blockers and several important contradictions. F01 is
therefore not accepted. F02's Anthropic worker hit 429 after producing an
uncommitted partial implementation; that diff is preserved as stash
`ideal-base F02 aborted partial implementation ...` and will not be applied
until the revised F01 design passes a fresh independent critical review.

**Reason:** implementation cannot proceed from a design with an unimplementable
crate boundary, a self-blocking reload lease, or incomplete provider/MCP turn
coverage. Preserving the partial diff avoids data loss without treating it as
accepted work.

**Reopen trigger:** revised F01 design passes independent review; F02 may then
salvage only compatible pieces from the stash.

## D012. Coordinator recovery: direct F01-R revision, over-decomposed plan cleared

**Decision:** the coordinator session (`fish`) was interrupted after the F01-R
repair fan-out over-decomposed into a 148-node analysis plan and the external
model rotation partially failed (GLM worker crash failed `b2`; Kimi endpoint
4xx; Cursor Grok stream error; user reported "glm seems dead"). A fresh
coordinator (session `monkey`) recovered per EXECUTION_PROTOCOL section 9:

1. Preserved the seven completed typed worker artifacts (`b1`, `i1`, `i2`,
   `F01-R-watchdog-review-lines`, `F01-R-source-seam`,
   `F01-R-entry-families`, `F01-R-reloadhandoff`) from session journals into
   `evidence/F01-R/worker-artifacts/`, and snapshotted the full 148-item plan
   (version 64) into `evidence/F01-R/pre_clear_plan_snapshot.json`.
2. Performed the F01-R design revision directly as coordinator work
   (fable-class design role), producing `evidence/F01/design.md` revision 2
   and `evidence/F01/revision_response.md`, resolving all three blockers,
   all five important findings, and the ten-point revision gate. All new
   file:line citations were mechanically re-verified at `398b51c07`
   (23/23 pass).
3. Amended F02 `owned_paths` in `WORK_GRAPH.json` (both `all_nodes` and the
   W1 expansion) per the chosen `jcode-core` inversion seam:
   `crates/jcode-core/src/activity.rs`, `crates/jcode-core/src/lib.rs`,
   `crates/jcode-base/src/mcp/manager.rs`, `crates/jcode-base/src/mcp/pool.rs`,
   `crates/jcode-app-core/src/tool/mod.rs`.
4. Cleared the over-decomposed 148-item swarm plan (snapshot preserved), the
   same quarantine-then-clear treatment W0.3 applied to the earlier stale
   plan. The unexecuted `b2/b3/i3-i5/gate` analyses are subsumed: the
   revision responds to every review finding directly and F01-V re-validates
   them all independently against source.

**Reason:** a 143-node queued analysis swarm was scaffolding for a revision
that one grounded design pass could produce; reviving it would burn provider
budget on partially dead routes without changing the acceptance bar, which
remains the independent adversarial F01-V re-review.

**Reopen trigger:** F01-V FAIL, which would inject targeted repair nodes
rather than re-growing the analysis tree.

## D013. F01 accepted after three-round independent review convergence

**Decision:** F01 is accepted at design revision 4, commit `a70db3700`, after
the independent architecture critique gate passed in
`reviews/F01-architecture-re-review.md` Round 3 (commit `1a37ba109`, reviewer
OpenAI `gpt-5.6-sol` at high effort per D009/D011).

Review trail: revision 1 FAIL (3 blockers), revision 2 FAIL (2 blockers),
revision 3 FAIL (2 blockers), revision 4 PASS with no blocking, important, or
revision-requiring minor findings. Each round's findings and dispositions are
recorded in the review file and `evidence/F01/revision_response.md`.

Binding design outcomes for F02:
- lease interface in `crates/jcode-core/src/activity.rs` (neutral crate seam);
- `McpCall` guards at both `McpManager::call_tool` and
  `SharedMcpPool::call_tool`;
- `ProviderTurn` guard inside `process_message_streaming_mpsc`, eight
  production call sites across seven caller families incl. startup
  reload-recovery (`server.rs:1009`);
- serialized coordinator executor publishing `Cleaned` (never exiting);
  top-level runner (`src/cli/dispatch.rs:114`) and coordinator-armed watchdog
  are the only two authorized termination sites, made mutually exclusive by
  an atomic Armed/Cancelled handoff;
- F02 `owned_paths` expanded accordingly (jcode-core activity files, MCP
  manager/pool, tool/mod.rs, src/cli/dispatch.rs).

**Reopen trigger:** F02 implementation discovering the design unimplementable
at any specified seam, which injects a repair node and re-runs this gate.

## D014. F02 accepted after three-round independent implementation review

**Decision:** F02 (work-aware activity leases + bounded shutdown coordinator)
is accepted at commit `2b5607882`, verified by the independent implementation
review (`reviews/F02-implementation-review.md`, reviewer OpenAI `gpt-5.6-sol`
high effort): round 1 FAIL (5 blockers), round 2 FAIL (2 blockers), round 3
PASS with no remaining blocking defect and both acceptance gates met.

Notable hardening driven by the review: atomic idle-shutdown claim with
`ClientConnection` leases as the admission gate (refused connections dropped
uncounted), `ScheduledDelivery` lease around the ambient direct-dispatch gap,
reload intake cancellation with refuse-before-publish ordering, all lease
refusals failing closed, adopted-original `AbortHandle` retention so cleanup
aborts rather than detaches, watchdog thread-spawn fallback, off-runtime
executor spawning, StartupRecovery TTL.

The round-3 review flagged a fixture-binary provenance defect (stale build);
the transcript was regenerated from a clean-tree exact-commit build with
three consecutive passing runs (`evidence/F02/exit_mode_fixtures_run.log`).

**Reopen trigger:** F03 fixtures uncovering a lease-class or exit-mode gap;
that injects a repair node against F02's owned paths.

## D015. F03 accepted; review PASS plus post-review harness strengthening

**Decision:** F03 (lease-class and exit-mode verification) is accepted. The
independent review (`reviews/F03-verification-review.md`, OpenAI `gpt-5.6-sol`
high effort) returned PASS at commit `d8c223d29` with both acceptance gates
met and no blocking finding. Its two nonblocking evidence-strength findings
were then implemented rather than deferred: the harness now asserts a
minimum post-release liveness window (F03-I1) and boots a successor over the
forced-exit residue in the same runtime directory (F03-I2). The strengthened
matrix passes 41/41.

F03 additionally caught and fixed a production defect: terminal-outcome
publication via `watch::send` dropped the value when no waiter was
subscribed yet, which could hang `begin_and_wait` forever; now
`send_replace`.

Remaining coverage limitations (recorded by the review, owned by later
nodes): real-provider/MCP/swarm integration fixtures, process-level reload
fixtures, Windows behavior, and owned-descendant cleanup (F06/F08).

**Reopen trigger:** any later node discovering a lease-class or exit-mode
gap the matrix should have caught; that injects a repair node here.

## D016. F04 accepted after three-round independent review convergence

**Decision:** F04 (atomic serialized TaskStatusStore) is accepted at commit
`9c4c99897`, verified by the independent review
(`reviews/F04-implementation-review.md`, OpenAI `gpt-5.6-sol` high effort):
round 1 FAIL (persistence-failure durability B1 plus contract findings),
round 2 FAIL (cancel tombstone / finalize policy R2-B1), round 3 PASS with
all three acceptance gates met.

Key guarantees now in force: temp+rename reader-atomicity, per-task write
serialization, first-terminal-wins precedence (hostile mutations cannot
resurrect Running), spawn fails closed without a durable initial record,
terminal-persistence failure retains a live-map tombstone with a backoff
recovery loop, cancel aborts in place, and shutdown finalize applies an
explicit two-arm failure policy (orphan-sweep recovery vs loudly logged
data loss for the adopted/no-record corner, accepted as the honest bound).

The reviewer's 10-item F05 handoff list (crash durability/fsync, stale temp
cleanup, cross-process writers, task-id collision policy, Windows rename
semantics, persistence-health events, retry lifecycle, delivery-during-
recovery, lock-map growth, targeted publication-count tests) is the F05
work seed.

**Reopen trigger:** F05 fixtures uncovering a store defect; that injects a
repair node against F04's owned paths.

## D017. Wave-2 delegation routing (user-specified)

**Decision:** from F05/F06 onward, execution runs through the native swarm
task DAG with this routing:

- Implementation/coding nodes: OpenAI `gpt-5.6-sol` at high effort.
- Independent review nodes rotate across: Anthropic `claude-opus-4-8`
  (usage restored per user), Kimi `k3`, Cursor `cursor-grok-4.5-high`, and
  `MiniMax-M3`. Reviews must name the model actually used; if a route
  fails (429/dead endpoint), fall to the next in the rotation and record it.
- The coordinator (this session) seeds a SMALL node set, forbids worker-side
  node expansion (the F01-R over-decomposition lesson, D012), accepts
  artifacts, and checkpoints.

**Reason:** user instruction 2026-07-18T19:15Z; spreads review across
independent vendors while concentrating implementation on the strongest
verified implementation route.

**Reopen trigger:** route availability changes or user re-routes.

## D018. F05 accepted; first cross-vendor delegated node

**Decision:** F05 (background status durability verification) is accepted at
commit `9f4d34d11`, the first node executed under the D017 delegation
routing: implementation by an OpenAI `gpt-5.6-sol` high-effort swarm worker
through the native task DAG, independent review by Anthropic
`claude-opus-4-8` (`reviews/F05-verification-review.md`, first-round PASS,
zero blocking findings, both gates met).

Hardening delivered: fsync durability in `write_atomic` (temp-file sync
before rename, parent-directory sync after, surfaced errors), stale
`*.json.tmp.*` sweep on the startup reconcile path with live-writer
protection, task-ID collision policy documented and tested, and the F05
verification matrix (cross-instance concurrency, crash-interruption/torn
write, malformed-file matrix, orphan re-verification).

Review follow-ups (nonblocking): F05-I1 cross-process last-writer-wins on
non-terminal fields remains an honest deferral (production topology is a
single global manager); test naming could be tightened.

Process note: the DAG driver's deep-mode gate auto-expanded the review node
into 30+ analysis children after the implementation completed; per the D012
lesson the coordinator snapshotted (`evidence/F05/plan_snapshot_before_prune.json`)
and cleared the plan, then ran the review as a directly-routed cross-vendor
session instead.

**Reopen trigger:** F08's integrated gate or later store work uncovering a
durability defect.

## D019. F06 accepted; review-route availability findings

**Decision:** F06 (pooled MCP child ownership, bounded pre-exit reap,
mcp-serve owner-liveness) is accepted at commit `84dc0aa2b`. Implementation
by `gpt-5.6-sol` high via the task DAG (light mode, D017 routing). The
independent review (`reviews/F06-implementation-review.md`) is a first-round
PASS with zero blocking findings; both acceptance gates independently
reproduced, including the real-process spoof-resistant ownership test and
the TERM-resistant reap test.

Review-route availability (recorded for D017 rotation): Kimi `k3` fails on
a tool-schema incompatibility (rejects the swarm tool's `anyOf` JSON
schema); `cursor-grok-4.5-high` and `MiniMax-M3` are rejected as unknown
model IDs on the Cursor route in headless sessions. The review therefore
fell to Anthropic `claude-opus-4-8`, the only currently working reviewer in
the rotation.

Nonblocking follow-ups from review: PID-reuse hardening for mcp-serve
owner-liveness (start-time/token cross-check) and single-reaper routing for
the ECHILD fallback; both are F07/F08-window candidates.

**Reopen trigger:** F08's integrated gate finding a descendant-survival
case.

## D020. Reload incident of 2026-07-18 21:38; repair nodes R01/R03 opened

**Decision:** The attempted selfdev reload exposed three architectural faults,
investigated read-only by three parallel `gpt-5.6-terra` sessions
(`evidence/reload-incident-2026-07-18/`):

1. `jcode server reload` never selects a binary; it re-execs whatever the
   `shared-server` channel points at. Only `debug reload`/the selfdev tool
   publish + smoke + repoint first. The signal's `hash=` is the *running*
   build's compile-time hash (log noise, not a target).
2. `install_binary_at_version` hard-links `target/selfdev/jcode` into the
   "immutable" versions dir, so a concurrent cargo rebuild truncates the
   published artifact through the shared inode (observed: zero-byte
   `versions/a87c5f271/jcode`, smoke test EOF).
3. Client identity is not bound to the resumed agent session across exec
   handoff (debug commands route raw session ids against the in-memory map;
   recovery eligibility keys on swarm `running` status, not live attachment),
   and TUI terminal-mode ownership is disarmed before exec with no successor
   guard (kitty/SGR-mouse left enabled: the red report-garbage screenshot).

**Action:** Opened W1 repair nodes `R01` (atomic publish + explicit target
selection + identity binding; merged because their owned paths overlap) and
`R03` (terminal-mode ownership), both `depends_on` F03+F06, review by Opus
per D017. Both are runnable now and context-disjoint from F07, which stays
next for the MCP track.

**Upstream note:** The user granted explicit license to rewrite inherited
upstream subsystems for sanity rather than mirror them; the reload/build
subsystem is the first beneficiary.

**Reopen trigger:** F08's integrated gate finding a reload-path regression.

## D021. Advertised subagent/Task tool surface is dead (fix queued behind R01)

**Finding (2026-07-19, this session):** The Agent-tool dispatch failure
("Unknown tool: subagent") is an inherited surface/registry inconsistency,
not a routing blip:

1. The Claude identity toolset still advertises **Task/Agent**
   (`crates/jcode-provider-anthropic/src/lib.rs` `claude_code_identity_tools`),
   and runtimes map `Task <-> subagent`
   (`crates/jcode-provider-claude-cli-runtime/src/lib.rs:1106,1136`,
   `crates/jcode-provider-core/src/anthropic.rs:284,300`).
2. But the backing tool was deliberately deregistered:
   `crates/jcode-app-core/src/tool/tests.rs:84` asserts "the deprecated
   direct subagent tool must not be exposed; use swarm instead."
3. Every model call to Agent/Task therefore dies in
   `registry.execute` -> "Unknown tool" (`crates/jcode-app-core/src/tool/mod.rs:567`).
4. The `/subagent` slash command is equally dead: `handle_run_subagent`
   (`crates/jcode-app-core/src/server/client_actions.rs:251`) still builds
   `tool_name = "subagent"` and executes via the registry, so it can only
   error. The tool was removed upstream but its three call surfaces were not.

**Decided fix (bridge option):** register a thin `subagent` tool that
delegates to the existing swarm spawn path (`run_swarm_task` in
`crates/jcode-app-core/src/server/swarm.rs:1725` already has the exact
shape: description + subagent_type + prompt -> forked worker session).
This makes the advertised surface honest and revives `/subagent` for free.
The alternative (stop advertising Task and delete `handle_run_subagent`)
loses a capability models actively try to use.

**Sequencing:** touches `crates/jcode-app-core`, which R01's worker
(lizard) currently owns. Act immediately after R01 lands, before or
alongside its review round. Do not start while lizard holds those paths.

**Reopen trigger:** any provider identity-toolset change that adds or
removes advertised tools without a registry round-trip test. Follow-up
candidate: a test asserting every advertised identity tool resolves in the
registry (would have caught this).

## D022. R01 and R03 accepted; reload subsystem repaired

**Decision:** R01 accepted at `e3736e7fb` after a full FAIL->fix->re-review
cycle: implementation by Sol (lizard), adversarial review by Opus 4.8 (hog,
FAIL on BLOCKING-1: exec-stage refusal hardcoded force=true and could exit a
drained daemon on a routine non-forced reload), fix by Sol (vole,
`923bba4aa` force threading + `293384c53` alias GC), re-review by Opus 4.8
(dragon, PASS). R03 accepted at `a0676f781`, first-round PASS by Opus 4.8
(dromedary) with 4 non-blocking hardening notes (recorded in the review).

Route incidents this cycle: stallion (first R01 reviewer) was cancelled by
the coordinator based on a session listing that wrongly showed the live
agent as absent during TUI attach churn (build-mismatch bounce). Lesson
recorded: verify via logs before cancelling "zombie" jobs; the listing
desync is an R01-adjacent observability bug. Separately, a broken tldraw
MCP tool schema (project .mcp.json) bricked three consecutive Sol jobs with
provider 400s before any work; removed from config. Follow-up candidate: a
provider-boundary JSON-schema sanitizer generalizing the Kimi flattener.

**Reopen trigger:** F08 integrated gate finding a reload-path regression.

## D023. D021 implemented; daemon reloaded onto reviewed selfdev build

**Decision:** D021 is implemented at `607d3cbad` after `16646d9f4`
registered a real `subagent` bridge and `6c633b785` aligned the schema with
app-core tool conventions. The bridge delegates to the shared swarm-worker
helper, keeps the child-worker recursive blocklist intact, and revives both
provider-advertised Task/Agent calls and `/subagent`.

Validation recorded in `evidence/D021/IMPLEMENTATION.md`: app-core suite
passed (1138 passed, 0 failed, 23 ignored), selfdev build passed, and the
registry round-trip test now checks the hardcoded Claude identity-tool names
against app-core registry resolution. Post-reload live smoke in a fresh
selfdev session confirmed `subagent` is registered alongside `swarm`.

The daemon was then reloaded via `selfdev build-reload` onto
`607d3cbad-dirty-03aa34bf0344` (dirty only because ignored/untracked drawio
artifacts remain in the worktree). `jcode doctor` reports client/server SAME
and shared-server now points at the new build. This activates the accepted
R01/R03 reload/terminal repairs and D021.

Route notes: a cancelled pre-reload D021 partial stash remains as
`stash@{0}` for safety but is obsolete relative to the committed D021 work;
do not resurrect it unless investigating the dispatch history.

**Next:** resume the ideal-base railway at F07 (dead/hung MCP detection,
cache eviction, bounded reconnect), with D021 available for future delegation.

## D024. R04 injected: reload drain vs accept-loop-exit race; safety net added

**Decision:** A live incident (2026-07-19 23:02 local, `evidence/R04/`)
showed that any reload issued while a drain-blocking lease is active
aborts the exec handoff: the drain's intake cancellation stops the accept
loops, `Server::run`'s select misreads that as a crash, upgrades
`reload -> accept-loop-failure`, and the daemon exits 45 with no
successor. D022's reopen trigger fired early via live use rather than
F08. R04 is injected as a W2 child (W1 is at its 10-child budget and F10
already owns `server/**`, so F10 now depends on R04 to serialize
ownership). R04's F03/F06 dependencies are accepted, so it is immediately
delegable alongside F07 without path overlap.

Independent of daemon code, two coordinator-owned safety nets were added
under `scripts/` and validated in sandboxes: `server_sentinel.sh` (launchd
agent; socket liveness probe, shared-server rescue, quit-vs-crash
discrimination via the durable shutdown marker) and `jcode_emergency.sh`
(rollback to smoke-tested stable/nix binaries, then rate-limited external
agent summon). These reduce the blast radius of future reload-seam work;
they are not a substitute for the R04 fix.

Also landed on main while unblocking scouting: alias resolution in
`validate_tool_allowed` (9e786069e) and disallowed-tool calls surfaced as
tool-result errors instead of turn aborts (a55dec21f). Both carry
regression tests.

**Reopen trigger:** F08 finding any exit mode in which a reload with held
leases fails to hand off, or a genuine accept-loop crash that no longer
exits 45.

## D025. R05 injected: multi-client session contention; W3 CI boundary

**Decision:** A live incident (2026-07-20 11:43, `evidence/R05/`) showed
two TUI clients attached to one session fighting via stall-guard
cancels, with stranded-interrupt recovery replaying 18 duplicate user
messages. R05 is injected as a W4 child (dual-attach policy, queued
duplicate collapse, truthful stall-guard labeling). W1/W2 closed fully;
W3 is 2/7 with F17/F18/F21 blocked at an authorization boundary: their
gates require real CI runs (pushing to the fork, consuming runners),
which the coordinator does not do without explicit user authorization.

**Reopen trigger:** any further duplicate-delivery or dual-attach
incident before R05 lands.

## D026. F17 TUI test-rail strategy: block on green, ignore stale with reasons

**Decision:** Four independent triage workers (mushroom/clover/sunflower/
hibiscus, headless, claude-fable-5) classified all 39 failing jcode-tui
tests against a scrubbed `env -i` + temp HOME/JCODE_HOME baseline. **Zero
product bugs found.** Breakdown: ~7 env-sensitive (macOS `⌥` vs Linux
`Alt` label widths in `ui_viewport.rs`; `TERM=dumb` 256-color
quantization collapsing shimmer blends; real Keychain/dotfile probes via
`/usr/bin/security` that escape the `JCODE_HOME` sandbox), ~9 broken/stale
assertions that never caught deliberate product changes (Ctrl+B→Ctrl+O
set-default remap `65e1bc30f`; `FULL_PREP_CACHE_MAX_BYTES` 8→24MB
`f6bc28e64`; seeded synthetic Session-Context `<system-reminder>` in
`App::new`; `side Pinned` title casing; prompt-jump landing on line 0),
and ~3 order-dependent flakes that pass singly but poison on process-global
`OnceLock`/auth-cache/thread-local pollution in full-suite runs.

The TUI test rail blocks on the 1822 passing tests. The 39 are
`#[ignore = "..."]`-tagged with per-test reasons naming the causing
commit or environment dependency, then burned down as a separate
non-blocking backlog. Rationale: leaving 39 persistent reds desensitizes
the gate (the exact failure mode the railway exists to prevent), while
blocking the epic on cosmetic assertion churn trades a real durability
objective for lint. Honest classification beats papering over: the
ignores are documented, attributed, and reversible.

**Reopen trigger:** any ignored test whose triage reason cites a
*deliberate change* turning out to mask a real regression, or the passing
count dropping below 1822 on the rail.

## D027. Distributed swarm workers: post-epic direction, not this epic

**Decision:** The seams for cross-host swarm workers already half-exist
(`scripts/remote_build.sh` + `remote_config.sh` remote-build path;
`JCODE_SWARM_ID` decoupling identity from filesystem; client
`--socket`/remote-working-dir tolerance; the local `omlx` model fleet
topology). Genuinely missing: a shared session/task store (today all
`~/.jcode` on one disk), file-ownership coordination across checkouts
(the owned-paths model needs a shared FS or git-branch-per-worker
discipline), and result collection. Honest sequencing: (1) fix single-host
build-lock contention cheaply (per-worker `CARGO_TARGET_DIR` or a shared
prebuilt test binary — the four triage workers just demonstrated the
contention cost), (2) remote *build* offload via the existing script,
(3) true remote workers. Filed as a post-epic direction so it survives.

## D028. F17 burndown found a real production memory bug (delegated, adversarially reviewed)

**Decision:** Delegated the 38-test jcode-tui burndown to two parallel
workers (Terra-Max `gpt-5.6-terra` on ~27 UI/picker/cache/cosmetic; a
Sol-lane worker that fell back to `gpt-5.5` on the 11 remote-state
tests). Coordinator verified every claim independently by rebuilding the
test binary and running each target in true isolation (fresh
HOME/JCODE_HOME): **38/38 pass isolated, 0 fail.** The full-suite count
(13-16 "fails") is order-dependent global-state pollution (the D026 flake
class: shared OnceLock/auth-cache/thread-locals), NOT the fixes, several
"failures" are tests that were never in the target set and pass singly.

**Two edits exceeded pure assertion-fixes; both upheld after scrutiny:**
1. **Real production bug (ui_memory_estimates.rs).**
   `estimate_prepared_chat_frame_bytes` charged only
   `sections.capacity() * size_of::<PreparedSection>()` and never recursed
   into each section's `Arc<PreparedMessages>` content. This estimator is
   live admission control for the FullPrepCache (ui.rs:1175 insert,
   ui.rs:1210 eviction vs the 24MiB budget), so a multi-MiB transcript
   frame was accounted as ~metadata and admitted to the normal cache;
   several such frames could persist while the 24MiB budget falsely read
   satisfied -> unbounded resident memory on long transcripts. Fix sums
   `estimate_prepared_messages_bytes` per distinct Arc pointer (dedups
   shared sections, does not loosen policy). This is a "STOP and FLAG"
   find the worker fixed instead; accepted, committed separately as a
   reviewable production change.
2. **Flaky perf guard (side-panel latency bench).** The `< 16.0ms` p95
   assertion timed a real debug-build `terminal.draw()` path; measured
   debug p95 is 44ms (release ~3x faster), so the guard only ever passed
   by variance. A worker relaxed it to an arbitrary 250ms; coordinator
   replaced that with `cfg!(debug_assertions)` -> 150ms debug / 16ms
   release, preserving the strict 60fps guard where it is meaningful.

The remaining stale-assertion updates match deliberate fork commits
(ff5e6a262 remote-queue deferral, 945846c6d retry-copy rewrite, 65e1bc30f
Ctrl+B->Ctrl+O, f6bc28e64 cache 8->24MiB). Notably the workers *fixed* the
env-sensitive copy_badge/shimmer tests rather than ignoring them, so the
D026 "ignore the ~7 env-sensitive" fallback was not needed: the honest
hard-fork outcome (repair, not silence) was achievable for all 38.

**Swarm-tooling notes for F27:** spawn `working_dir` must equal the
coordinator's swarm root (`/Users/jrudnik`) or workers land in an
unreachable `.git` sub-swarm; `start`/`wake` can't drive an inline-task
worker (DM it to begin); completion reports lag/omit from coordinator
status; Cursor route `gpt-5.6-sol-high` silently fell back to `gpt-5.5`
(use the OpenAI-routed `gpt-5.6-sol` next time). Shared-worktree edits by
two workers on one file (remote_events_reload_04.rs) triggered the R05
overlap warning; coordinator serialized ownership by DM.

## D0xx — F20c: retire distribution state, not the release workflows

**Context.** F20c's declared `owned_paths` named `.github/workflows/release.yml`,
`windows-smoke.yml`, and `freebsd-smoke.yml`. Editing them from `main` violates
the branch model: `docs/BRANCHING.md` gives `distro/nix` sole ownership of
`.github/workflows/**`, and `scripts/fork-health.sh` check 5 fails when `main`
carries a workflow diff. (That check already FAILs on 8 pre-existing files; the
right response is not to add a 9th.)

**Decision.** Amend F20c to drop the three workflow paths and instead own
`.github/scripts/verify_windows_install.ps1`. `depends_on` gains `F17`, which
owns `.github/scripts/**`, to serialize the ownership overlap the railway
validator correctly flagged.

**Why this is the honest scope, not a dodge.** The thing F20c must remove is
*state*, not CI. Nothing in the three workflow YAMLs referenced the version
store, the channel symlinks, or the in-binary acquisition path; they build and
publish artifacts, which is orthogonal to how a machine installs them. The one
real coupling was `.github/scripts/verify_windows_install.ps1`, which asserted
`builds/versions/<v>/jcode.exe` and `builds/stable/jcode.exe` exist after
install. That is exactly the class of failure F20c exists to prevent: a checker
pinning a layout no resolver reads. It now asserts the single fixed publish path
and that the retired layout is NOT recreated.

**Consequence.** Whether the fork keeps cutting GitHub releases is a
distribution-layer question, decidable on `distro/nix` independently and at any
time. F20c leaves that lever untouched and only guarantees that *if* a release
is installed, it lands on the one fixed path.

## D029. The jcode-tui "flake" was two real bugs plus one measurement error

**Context.** During F20c verification the workspace suite alternated between
fully green and ~16 `jcode-tui` failures. The initial reading was that this
was another instance of the D026/D028 order-dependent global-state pollution,
i.e. pre-existing and not F20c's problem.

**That reading was wrong, and the way it was wrong is worth recording.** The
evidence for "pre-existing" was that stashing the F20c changes reproduced
failures on `main`. That is necessary but not sufficient: it shows `main` also
fails, not that it fails *for the same reason*. Running `-p jcode-tui --lib`
alone came back 4/4 green at 1867 tests, which falsified the intra-process
ordering hypothesis outright. The failures only appeared in whole-workspace
runs, where many test binaries execute concurrently against one real `$HOME`
and one real `~/.jcode`.

**What was actually there.** Two independent defects, both real, neither
caused by F20c but one of them fixed by it:

1. `minimax_token_plan_keys_resolve_to_china_endpoint...` had no isolated
   `JCODE_HOME` and read the developer's real stored MiniMax credential.
   Measured 12/12 FAIL on `main`, 0/12 on the F20c branch (already fixed).
2. `global_config_cache_reloads_after_manual_file_edit` raced any test that
   mutated one of the 147 `CONFIG_ENV_KEYS` without the environment lease.
   Measured ~1/12 on both branches. Fixed in `65e76e7b3`.

**Method that resolved it.** The assertion printed `left: 8 right: 7` and
nothing else, which is unactionable: it reports that a reload happened, not
what caused it. The fix was to make the failure self-diagnosing first, by
recording reload reasons under `cfg(test)` and printing them from the
assertion. The next reproduction named the key
(`JCODE_MEMORY_EMBEDDING_BACKEND:added`) and the search collapsed from a
32-file audit to one test. Diagnosis before bisection.

**Generalization.** A one-test fix would have left the defect class intact:
nothing prevented the next test from mutating a fingerprint key without a
lease, and the failure would resurface in an unrelated test on someone else's
machine. `scripts/check_config_env_lease.py` now makes the invariant
structural. Building it surfaced two further latent instances
(`JCODE_COPILOT_PREMIUM`, `HOME`) that had never been observed failing.

**A gate that passes everything is worse than no gate.** The first version of
that checker passed the tree, including the known bug. Transitive helper
resolution had admitted 16142 names such as `Config`, `env`, and `set_var`, so
essentially every test looked leased. It was caught by deliberately reverting
the fix and confirming the gate went red. Every gate added here was proven
non-vacuous in both directions before being trusted.

**Standing rules.** (a) "It also fails on main" is a claim about a symptom;
attribute per-test with measured pass/fail counts before concluding
pre-existing. (b) When a global-state assertion fails, make it report its
cause before hunting the culprit. (c) When a flake is fixed, ask what class it
belongs to and gate the class. (d) Never trust a new gate that has not been
observed failing.

## D030. Post-distribution graph amendment: W4R/R07 governance barrier, R06/F30/D01, and validator support

**Context.** The Nix-only distribution transition merged through PR #36
(`78a08e4d4`), satisfying the activation condition the post-distribution
orchestrator plan was waiting on. The plan's first mandated mutation is one
graph-amendment pull request adding a recovery/governance barrier, closing the
sticky-server defect lane, verifying the distribution handoff independently,
and preparing documentation reconciliation. Pre-amendment counts re-audited as
6 roots, 46 children, 52 state records; post-amendment counts are 7 roots, 50
children, 57 state records, matching the plan.

**Decisions.**

- New root `W4R` (depends on W3, alongside W4) carries `R07`: required-check
  contexts in one machine-readable file, merge-commit-only ruleset hardening,
  the `STATE.json` reviewed/published commit schema split, ancestor-of-HEAD
  validator semantics, and private recovery-archive ratification. R07 is the
  publication barrier for every still-pending W4 implementation.
- New W4 children: `R06` (sticky-server process-group signaling repair) and
  `F30` (independent Nix-only / native-iOS retirement verification). New W5
  child: `D01` (documentation reconciliation); `S01` now depends on `D01`.
- Revised contracts: `F22` drops retired Homebrew host verification for
  structured advisory ownership; `F24` owns no release workflow; `G03`/`G04`
  become deterministic; `G05` becomes a Nix/Cachix acquisition smoke with no
  release API. Audit items A16 and A23-A25 were reworded to match; A26 (D01)
  and A27 (F30) were added.

**Validator amendments forced by the graph shape** (the plan named A26 but did
not enumerate these mechanics; recorded here rather than silently deviating):

1. Per-wave expansion budget raised 10 to 12. W4 now has eleven children (R06
   and F30 are explicitly parented to W4 by the plan), and the budget exists to
   keep deep-gate reviews bounded, not to cap wave size at ten.
2. Ordered audit IDs extended A01..A25 to A01..A27, and coverage may cite
   D-prefix nodes. The plan maps A26 to D01, which the F/G-only citation rule
   rejected. F30, as an F node, required coverage the plan did not name, so A27
   covers it.
3. `R06` depends on `F29` in addition to the plan's R01/R04/R07: F29 owns
   `src/cli/commands.rs`, which R06 must edit for the signal call sites, and
   the ownership validator requires same-wave overlaps to be dependency-ordered.
   F29 is accepted, so this orders ownership without delaying R06.
4. `D01` owns concrete documentation paths rather than `docs/**`: a
   shallow-prefix glob would overlap W5 sibling evidence/review ownership
   without dependency ordering.

**Reopen triggers.** A future wave needing more than twelve children must raise
the budget with its own decision; a documentation node beyond D01 must revisit
the citation-prefix rule; a defensible legacy commit mapping failure reopens
the affected node per R07 rather than weakening the ancestry rule.

## D031. R07 proceeds without an external trust root: owner-admin is the accepted root of trust on a personal repository

**Context.** The R07 adversarial design gate (evidence
`docs/fork/ideal-base/evidence/R07/design-gate.md`, verdict FAIL) proved with
live API probes, GitHub documentation, and a GitHub staff statement that the
repository-level `workflows` ruleset rule is organization/enterprise-scoped
and cannot attach to a repository-level ruleset on `jerudnik/jcode`, a
personal user-owned repository. The first R07 design had adopted that rule as
the sole external anchor making zero-required-approval governance
"self-protecting," and failed its own stop-condition when the rule proved
unavailable.

**Decisions.**

- The R07 contract in `WORK_GRAPH.json` is unchanged. It never required an
  external trust root: it requires ruleset hardening (deletion and
  non-fast-forward protection, pull-request-only changes, zero required
  approvals, review-thread resolution, merge commits only, strict required
  checks, no silent administrator bypass), machine-readable required checks,
  the STATE schema split, ancestor-of-HEAD validator semantics, and private
  recovery-archive ratification. The external anchor was a design-layer
  addition, not a contract requirement.
- On a personal repository the owner-admin is accepted as the root of trust;
  this was already true de facto, since the owner can rewrite or delete any
  ruleset. R07's "self-checking governance" property is delivered through
  auditability instead: governance fixtures that prove rule shape without
  GitHub access, a live read-only comparison mode, and fork-health full
  comparison that fails closed on any drift between the machine-readable
  manifest and the live ruleset.
- The R07 design is revised (v2) to remove the `workflows` rule and its
  trust-root narrative, and to fix the unsupported `repository_id` citation
  chain the gate also flagged. Design v2 must pass a fresh adversarial gate
  before implementation begins.

**Reopen triggers.** If the repository ever moves into an organization or
enterprise plan, revisiting the `workflows`-rule external anchor is a new
decision; if GitHub makes repository-level required-workflow rules generally
available, the same. A false-durability finding in the R07 independent review
reopens this trade-off rather than weakening the auditability controls.

## D032. R07 executed: server-side governance is live, audited, and self-checking

**Context.** R07 (W4R) was the post-distribution governance and durability
barrier gating every remaining W4 node. Its design loop failed twice
adversarially (v2: bootstrap race and unbound maintenance window; v3: untested
GitHub API behavior around compare-API truncation, rename evasion, and rev-list
floors) before design v4 passed with independent live reproduction. The Opus
independent review of the integrated implementation found three executed
blocking gaps (an unprotected comparator chain, a missing `docs/BRANCHING.md`
CI-table row for `governance-root.yml`, and a `required_reviewers` manifest/live
mismatch), all remediated and re-reviewed before any external write.

**Decisions (record of what was executed 2026-07-29).**

- Barrier 1 (durability): 39/39 refs pushed to the private
  `jerudnik/jcode-recovery-archive` with exact SHA equality, giving the six
  reflog-only commits (F17 through F20c) durable remote refs. User authorization
  for the enumerated external writes preceded every push.
- Barriers 2-3 (bootstrap): PR #39 proved the four required contexts
  (`Governance Root`, `Fork CI Gate`, `Security Gate`, `Nix Gate`, all emitted
  by GitHub Actions app 15368) and that `Governance Root` goes red on
  governance-path changes by design. A CI-only gap was found and fixed: the
  railway validator's `reviewed_commit` object-existence check cannot hold in
  CI clones (reviewed objects are not ancestors of `main`), so the fork-ci
  governance-contract job sets `JCODE_RAILWAY_ALLOW_MISSING_REVIEWED_OBJECTS=1`
  to degrade only that check; strict validation still runs locally at
  coordinator checkpoints.
- Barrier 4 (apply): governance was applied in the strict design.md sequence:
  ruleset 18509013 (`protect-fork-rails`) updated with `deletion`,
  `non_fast_forward`, `pull_request` (merge commits only,
  `required_reviewers: []` pinned, review-thread resolution), the four required
  status checks, and empty bypass actors; ruleset 18509016
  (`no-stray-branches`) bypass actors emptied; the repository restricted to
  merge commits; classic branch protection deleted last. Live fork-health
  comparison is green against `scripts/required-checks.json`.
- Barrier 5 (proof): PR #40 demonstrated the gate is non-vacuous (a planted
  comment-only change to `scripts/fork-health.sh` turned `Governance Root` red,
  naming the path) and non-over-broad (a harmless change stayed green).
- Barriers 6-7 (maintenance + closeout): PR #41 landed through the first
  transaction-bound maintenance window (67 seconds, ruleset restored
  canonical-hash identical, exactly one merge inside the window); PR #42 landed
  with `Governance Root` correctly green on an evidence-only change.
  `STATE.json` records R07 and W4R accepted (reviewed `69a6a6310`, published
  `a545ecee4`).
- Consequence for all future work: the 27 protected governance paths enumerated
  in `docs/fork/ideal-base/evidence/R07/github-governance.proposed.json` are
  self-checking. Changing any of them requires the transaction-bound
  maintenance procedure in design.md section 4. The five ratchet baselines
  (`code_size_budget.json`, `panic_budget.json`, `swallowed_error_budget.json`,
  `test_size_budget.json`, `warning_budget.txt`) are deliberately unprotected
  so quality-gate tightening never requires a maintenance window.

**Reopen triggers.** An organization or enterprise migration (revisits D031's
external-anchor trade-off); any GitHub ruleset or compare-API behavior change
(the comparator pins current behavior and fork-health fails closed on drift); a
false-durability finding in any future audit of the archive or the live
rulesets.

## D033. W4 wave 1 accepted; false-green gates and local-build fallback are not acceptable evidence

**Decision.** Accept W4 wave 1 as six landed nodes: R05, R06, F22, F23, F26,
and F30. W4 remains `in_progress` at 8 of 11 accepted children because F24,
F25, and F27 remain. The durable identities and evidence paths are recorded in
`STATE.json`; `python3 /tmp/w4-remaining.py` independently reports 0 of 6
wave-1 nodes remaining.

Acceptance is at the final verified boundary, not the first implementation
commit or an agent's self-report:

- R05 includes the follow-up notify repair in PR #52. The independent
  adjudicator found that `apply_and_announce_working_dir` discarded the only
  user-visible trace of an undeliverable working-directory notice. The first
  regression test was itself vacuous because its session-id fixture contained
  the asserted substring; the corrected test fails against the planted defect
  and passes after restoration.
- F22 and F23 each had an independent DO-NOT-MERGE round whose blockers were
  reproduced by the coordinator before repair. F23 additionally exposed a
  plant harness that treated exit 2 crashes as proof and a proposed
  baseline-derived ceiling that would have made every repository comparison
  `value > value` and reported no breach. The ceilings remain independent
  literal high-water marks, with tests preventing that weakening.
- F23 PR #49 landed through the R07 section-4 maintenance procedure in a
  six-second window: exact reviewed head, all non-governance required checks
  green, one exact-parent merge, literal ruleset restoration with canonical
  hash `43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b`,
  and live fork health green at both boundary commits. The protected-path set
  intentionally grew from 27 to 29 so the critical-path checker and its tests
  cannot attest to their own weakening.
- F30 is verify-only and retains four bounded guard-strength follow-ups
  (`F30-FIX-1..4`); those gaps do not invalidate the landed Nix-only transition,
  but FIX-1 remains load-bearing and must not be silently dropped.

**Build-loop correction.** PR #53 fixed a separate defect found while measuring
landing efficiency. `remote_build.sh` excluded `.git/` as a directory, but Git
worktrees carry `.git` as a file containing an absolute `gitdir:` pointer. That
file was copied to the remote builder, failed flake evaluation there, and the
default local fallback silently compiled every worktree on the laptop. The
exclude now covers the file, local fallback prints a loud host/cwd banner, and
the maintained workflow requires batched CI submissions plus verification that
the intended remote host performed the build. The fix was proved in both
directions and ran `cargo check -p jcode-base` on SCO in 1m32s. The bounded
before/after transcript and live remote-cache check are retained at
`evidence/R07/remote-build-worktree-proof-2026-07-29.md`.

**Reopen triggers.** Any wave-1 node's non-vacuity mutation stops failing its
gate; `fork-health.sh --live` reports RED or governance drift; a required check
is removed or ceases to instantiate on every PR; the R07 literal-restore hash
cannot be reproduced; or an F30 follow-up is incorrectly treated as already
closed.
