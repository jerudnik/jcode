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

**Decision:** the persisted swarm plan for `/Users/jrudnik/labs/jcode/.git`
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
