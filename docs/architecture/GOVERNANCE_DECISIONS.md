# Ideal-base decisions

Append new decisions. Do not rewrite prior decisions to make the program appear
more linear than it was.

## D001. Archive recovery and normalization in place

**Decision:** `docs/fork/recovery/` and `docs/fork/normalization/` remain at their
existing paths as frozen historical namespaces.

**Reason:** the trees contain 600-plus evidence, review, and seam files with
relative links, checksum manifests, and hash-cited records. Moving them creates
integrity risk without improving execution. Current source, tests,
reproducible commands, Git state, and runtime evidence remain outside this
retired tree; `docs/fork/ideal-base/` stays as retired historical record.

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

## D034. G10A reversed F23's protected-path growth, and the log said otherwise

**Status:** recorded, not adjudicated. This entry exists so the reversal is
visible; choosing the replacement control is separate work.

**What happened.** Commit `621f4d44d` ("Change local governance definition",
Modernization-Node G10A, 2026-08-08) cut the Governance Root protected set from
32 entries to 5:

```
.github/workflows            scripts/governance_compare.py
scripts/required-checks.json scripts/generate_governance_fixture.py
scripts/fork-health.sh
```

The 27 dropped entries were mostly guard scripts and their tests:
`check_panic_budget.py`, `check_warning_budget.sh`, `check_code_size_budget.py`,
`check_critical_path_budget.py`, `check_tui_render_lock.py`,
`check_env_lease_drop_order.py`, `rust_production_filter.py`,
`security_preflight.sh`, `.github/scripts`, and the matching
`test_*` / `tests/test_*` files.

This directly reverses the F23 decision recorded above in this same file, which
states in the present tense that "the protected-path set intentionally grew from
27 to 29 so the critical-path checker and its tests cannot attest to their own
weakening." After G10A that sentence describes a property the repository no
longer has. The stale claim is the more damaging half of this finding: it is the
document a later reader would trust.

**What is genuinely unchanged.** The rails held. Live ruleset
`18509013 protect-fork-rails` still requires `Governance Root` and `PR Gate`,
still sets `strict_required_status_checks_policy: true`, still carries
`deletion` and `non_fast_forward`, still allows only `merge`, and
`bypass_actors` is still empty — verified 2026-08-17 by authenticated read of
`repos/jerudnik/jcode/rulesets/18509013`, where the key is present and `[]` and
`current_user_can_bypass` is `never`. The *list* endpoint `/rulesets` omits the
key entirely; per `scripts/governance_compare.py` an omitted key is an
unauthorized read and must never be recorded as emptiness. G10A's acceptance criteria asked for exactly
that, and it delivered it.

**What is partly compensated.** The guard scripts still execute: `PR Gate` →
`.github/workflows/fork-ci.yml:51` → `just check` → `justfile:7`, which invokes
`check_critical_path_budget.py --expect-digest <sha256>`. Weakening a ceiling,
shrinking scope, or relaxing a target changes `pinned_data()`, so the digest
moves and the author must also edit the pin. That is a visible one-line diff.

**What is genuinely weaker.** `scope_digest()` hashes `pinned_data()` only: the
scope, ceiling and target block. It does not cover the checker's comparison
logic. A PR may relax a comparison inside the checker, leave the digest
untouched, edit the checker's own test in the same commit, and trip no
governance trip-wire. That is precisely the F23 scenario, and before G10A both
files were protected, so touching either forced the maintenance window.

**Why this matters more on this repository than the coverage delta suggests.**
`required_approving_review_count` is `0`. On a single-maintainer fork the
governance window *was* the review forcing function: the only mechanism that
stopped a change from attesting to its own correctness. Removing 27 paths from
that window removed the forcing function without substituting another.

**Not adjudicated here.** Reverting to the pre-G10A 32-path list is one option and probably
the worst one: it restores coverage by making every routine guard edit demand a
ruleset transaction, which is the ergonomics cost G10A was right to reject. The
open question is a control that is cheap in the common case and still refuses
self-attestation. Candidate directions, none yet evaluated: pinning guard-script
and guard-test digests in a single protected manifest, so one protected path
covers many files; or a machine-checkable non-vacuity proof in `PR Gate` on the
F23 planted-defect pattern, so a weakened guard fails by demonstration rather
than by review. The second needs no second human at all, which suits this
repository.

**Reopen trigger for this entry:** any change to the protected-path set, the
`justfile` digest pin, or `required_approving_review_count`; or the adoption of
a replacement control, which should supersede this entry rather than amend it.

## D035. The replacement for the G10A protected set is a non-vacuity check plus three protected paths, not a digest manifest

**Status:** adjudicated. Supersedes D034, which recorded the reversal and
deferred the choice of control. D034's corrected counts are carried forward
here; its reopen trigger is replaced by the one at the end of this entry.

**Correction to D034's arithmetic.** The `protected=(...)` array in
`.github/workflows/governance-root.yml` either side of `621f4d44d` holds **32
entries before and 5 after: 27 dropped, 0 added.** D034 reports "27 entries to
5" and "22 dropped"; the 27 is the count of dropped paths mislabelled as the
pre-state, and the 22 is back-computed from that error. Verified three times by
set arithmetic on the array bounds, and independently by piglet. Separately,
`scripts/required-checks.json` carried **31** entries in `protected_paths.required`
before G10A against the workflow's 32: the two lists disagreed by the
one test path under `tests/` that the workflow protected and the JSON did not. The drift predates G10A and is not caused by it.

**Why not Option A, a protected digest manifest.** Three findings, in
increasing order of severity.

*It would pin bytes that nothing executes.* Of the guards this repository
describes as load-bearing, five gate a pull request. Three budget guards exit 1
against a clean, green `main`, and the budget checker's test suite, now
`tests/test_critical_path_budget.py`, was invoked by no workflow, recipe, or
script in the tree; it sat in `scripts/` at the time, where the wiring rule does
not reach. A manifest over the
pre-G10A set would spend the ceremony of a ruleset transaction on files whose
behaviour no gate observes.

*Its ergonomic cost is the one G10A was right to reject, and it is now
quantified.* Over the three months to 2026-08-17, commits on `main` touching the
27 dropped paths number **78**, which is **41** first-parent merges. A window is
opened per pull request, not per commit, so the ergonomic cost is 41 windows a
quarter for routine guard work. Protecting instead the three paths this entry
proposes costs **4** commits, **2** windows, over the same period. G10A traded
coverage for ergonomics without substituting a control; reverting would trade it
back at roughly twenty times the price.

*A digest of guard sources is defeatable without touching the pinned bytes.*
Measured on this tree: adding a `hashlib` module under `scripts/` whose `sha256` returns a fixed
value freezes `scope_digest()`, so `check_critical_path_budget.py
--expect-digest 5ed12e31...` still passes and exits 0 while the guard's
lifecycle and swallowed-error ceilings are raised from 441 to 9999. Python puts
a script's own directory at the front of `sys.path`, so *any* `python3
scripts/...` invocation is exposed. The attack is a **file addition**, and a
manifest enumerating existing guard files does not cover files that do not yet
exist. Option A is therefore not merely expensive; against this class it does not
work. G10A also dropped `scripts/rust_production_filter.py`, the unpinned
dependency of the only live PR-blocking guard, which is what makes the
count-preserving weakening of its line classifier reachable at all (piglet).

**What is adopted: Option B, `scripts/check_guard_nonvacuity.py`.** A registry
of ten claims, each binding a guard to a defect it must reject. Each claim
plants the defect, runs the guard, and fails if the guard accepts it; a plant
that no longer applies is reported as *unproven* rather than passing. Every
claim also pins the clean control value, so a plant whose success value could
coincide with the honest value cannot read as proof. Dormant guards carry a
verified reason and a wiring check, so a guard that silently stops gating is
reported rather than assumed. Cost: **0.6s** for the harness and **3.1s** for its
42 tests, against a `PR Gate` of roughly 55 minutes. It requires no second
human, which suits a repository whose `required_approving_review_count` is `0`
with an empty `bypass_actors`.

**What Option B does not cover, stated so the gap is inherited rather than
rediscovered.** A defect no claim names passes. The registry pins behaviour, not
bytes, so a rewrite that preserves the planted behaviour and loses everything
else is invisible. Dependencies are covered only where a claim names them. And
the harness was itself vulnerable to the shadow class above until
`c7051251a`: a single `ast` module dropped into `scripts/`, returning empty parse trees, made every
source look import-free and printed "10 claim(s) hold". It is fixed by scrubbing
`scripts/` from `sys.path` through the unshadowable builtin `sys` before the
first shadowable import, with the attack kept as a red test. That episode is
evidence for the approach rather than against it -- the hole was found by
adversarial review and closed with a control, not with an assertion -- but it is
also the reason no one should read this entry as claiming the control is
complete.

**Why a hybrid is still required: Option B cannot protect itself.** Exactly one
line in the tree invokes the harness, `justfile:9`, and at the time this entry
was written `justfile` was not a protected path. A pull request that deletes that line removes the control, and
nothing notices. No self-check can close this, because a control that is not
invoked cannot report its own absence. The minimum protected surface that makes
Option B durable is therefore three paths added to the surviving five:

```
justfile
scripts/check_guard_nonvacuity.py
tests/test_guard_nonvacuity.py
```

This is the boundary the harness was designed around: routine guard work -- new
ceilings, new logic, new tests -- needs no window, while changing *the claim
about what must fail* needs one. Measured cost, three months: 4 commits, 2 windows.

**Decision.** Adopt Option B with that three-path hybrid. Do not restore the
27-path list. Do not adopt a digest manifest, which is both more expensive and,
against file-addition attacks, ineffective.

**Residual gap, not closed here.** Every control in this entry runs inside the
gate it is checking. None of them proves the wired pipeline actually goes red
end to end -- the failure mode that gave `Fork Health` nineteen days of false
success from `... | tee` without `pipefail`. The control that would close it is a
scheduled mutation canary that plants a defect, opens a real pull request, and
asserts `PR Gate` fails. It costs a full gate run per canary and is not proposed
here; it is recorded so the next reader knows the difference between "the guard
rejects the defect in process" and "the gate rejects the pull request".

**Reopen trigger for this entry:** any change to the protected-path set, to
`justfile:9`, or to `required_approving_review_count`; a claim in
`scripts/check_guard_nonvacuity.py` that becomes permanently unproven; the
registry disagreeing with the set of guards `PR Gate` actually runs; or adoption
of the end-to-end canary, which would supersede the residual-gap paragraph.

**Editorial note on porting.** This entry was adjudicated on
`docs/fork/ideal-base/DECISIONS.md`, a path PR #155 deletes, and is reproduced
here at the surviving path. Three changes were made in the move, each because the
original would otherwise describe a property this repository does not have --
the D034 defect this entry exists to correct. First, the ergonomic figures are
restated in windows as well as commits: a window is opened per pull request, so
78 and 4 commits are 41 and 2 windows, reproducible with `git log --since
'3 months ago' --first-parent main -- <paths>`. Second, the sentence stating that
`justfile` is unprotected is put in the past tense, because the three paths this
entry prescribes are protected as of the pull request carrying this port: the
decision is implemented, not merely adopted. Third, the shadowing class described
above is no longer confined to the harness -- every `python3 scripts/...` line in
`justfile` now runs under `-I`, and the three guards that legitimately import a
sibling re-add their own directory explicitly. See D036.

## D036. Four controls were vacuous, and each failed by being absent rather than by breaking

**Status:** recorded, with the repairs implemented in the pull request carrying
this entry. Follows D035, which adjudicated the protected-path question; this
entry records what a sweep of the surrounding machinery found while that work was
in flight, and the residual risks left open.

**The common shape.** Every defect below reads as success from the outside. A
green check that never ran the assertion, a fatal rule no workflow invokes, a
guard that verifies a digest it can be told to compute, an error message pointing
at a file about to be deleted. None of them is a mechanism that broke; each is a
mechanism that was never wired, or was wired through something that could be
substituted. The corrective bias this entry argues for is to test for the red,
not for the green: a control nobody has watched fail is a control nobody has
evidence for.

**1. `Fork Health` was green for nineteen days while exiting 2.** The workflow
piped its checker into `| tee`. Under `bash -e` without `pipefail` the exit status
of a pipeline is the status of its last command, so `tee` exiting 0 masked the
checker exiting 2. Repaired by setting `pipefail`. The repair was verified by
demonstration rather than by re-reading the code: the same pipeline in isolation
exits 0 without `pipefail` and 2 with it, and the first run on the repaired
workflow was dispatched and inspected rather than assumed.

**2. Any `python3 scripts/...` invocation could be shadowed.** Python puts a
script's own directory at the front of `sys.path`, ahead of the standard library.
`justfile` invoked the critical-path digest guard as `python3
scripts/check_critical_path_budget.py --expect-digest <sha>`, and `scripts/` is
not a protected path. A six-line module dropped into that directory, named for
the standard library's `hashlib` and returning a fixed sentinel, made the guard
exit 0 against a digest that is not this repository's. The
sentinel was chosen so it could not coincide with the honest value. This is the
same class D035 records against `scripts/check_guard_nonvacuity.py`; what is new
here is that it also reached the guard `just check` runs on every local
iteration, and that a digest manifest -- Option A in D035 -- would not have
covered it, because the attack is a file *addition*.

Repaired by running every `python3 scripts/...` line in `justfile` under `-I`,
which drops the script directory from `sys.path`, and by having the three guards
that legitimately import a sibling module re-add their own directory explicitly.
`-I` alone breaks those three, so the two halves are one change and were tested
together. `PYTHONSAFEPATH` was rejected: it is a no-op on Python 3.9, which is
old enough to be present on a contributor machine.

**3. A fatal documentation rule was invoked by nothing.**
`scripts/check_docs_references.py` fails the build on any machine-local path in a
document, and its budget file records a ratchet of zero, so the rule is fatal by
design. No workflow, recipe, or script called it: `grep` over `justfile` and
`.github/workflows` returns no invocation. It is now wired into `lint-docs`, the
recipe `Checks / Docs lint` already runs, which makes it cost nothing new; the
lint step's `nix shell` had no interpreter, so `nixpkgs#python3` was added
alongside `just` and `vale`. A contract test asserts the recipe still contains
the invocation, so removing it fails a test rather than passing silently.

**4. The governance gate's error message pointed at a file being deleted.**
`.github/workflows/governance-root.yml` told a blocked contributor to "use the
recorded ruleset maintenance procedure (design.md section 4)". PR #155 deletes
that design document along with 1020 other files. Workflow YAML is invisible to
the documentation checker in item 3, so nothing in the repository would have
caught the dangling pointer. The message now points at the *Ruleset maintenance
window* section below, and that section exists because this entry adds it.

**The decision log is now append-only, which is not the same as protected.** The
instruction behind this work was to add `docs/architecture/GOVERNANCE_DECISIONS.md`
to the protected set. Measured first: the log took 34 commits, **33 first-parent
merges**, in the three months to 2026-08-17. Listing it in `protected=(...)`
would therefore cost 33 maintenance windows a quarter -- more than the 41 that
D035 rejects as too expensive for twenty-seven guard files -- and every one of
those windows would be the price of *recording a decision*. A control that taxes
its own upkeep will be routed around, and the resulting silence looks exactly
like agreement.

What is enforced instead, inside `governance-root.yml`, which is itself
protected: the log may gain lines freely and may not lose them. Deleting the
file, truncating it, or renaming it all fail the gate; a rename is caught because
it reads as a deletion, which is correct, since moving the log is a governance
event. Appending an entry needs no window. The stanza was tested against six
scenarios before merge -- no change, append, single-line deletion, removal,
rename, and an unrelated protected path -- and behaves in all six. The cost of
this choice is that a typo fix in the log needs a window; that is accepted, and
is the ordinary meaning of an append-only record.

This is not an original design. The 2026-08-17 reconciliation dissent recorded
both options and chose full protection on the argument that appends are rare
governance acts, while naming the append-only checker as the designed fallback
"if the owner finds window-per-append too heavy in practice." The measurement
above decides between them: appends are not rare, so the fallback is the primary.
The dissent's reasoning was sound and its premise was wrong, which is the useful
thing about writing the losing side down -- it stayed available to be corrected
by a number rather than re-argued from scratch. The residual it identified stands:
this protects the norm, not the content. Nothing here stops an appended entry from
being false; it only stops a true one from vanishing quietly.

**Residual risks, accepted rather than closed.**

*The governance contract is materially smaller than the log used to claim.* D034
records the reversal and D035 adjudicates the replacement. Eight paths are
protected where thirty-two once were, and the argument for that is ergonomic, not
security-theoretic. It is written down so a later reader inherits the trade
rather than rediscovering it.

*Tag protection is detected, not prevented.* A tag pushed outside the rails is
surfaced by the scheduled fork-health run, which means detection can lag by up to
roughly a day. No control here shortens that window.

*Nothing proves the pipeline goes red end to end.* This is D035's residual gap and
it survives this entry. Every control described above was verified in process --
the defect was planted, the guard rejected it -- but none of them proves that a
pull request carrying the defect is actually blocked by `PR Gate`. The control
that would close it is a scheduled mutation canary that opens a real pull request
and asserts the gate fails. It costs a full gate run per canary and is not
adopted here. Item 1 of this entry is precisely what that gap looks like when it
bites.

*The stale-path class was checked, not assumed clear.* Wiring the documentation
checker into `lint-docs` made it run for the first time against this entry as it
was being written, and it rejected the draft: the text cited two scoreboard
scripts as surviving hazards when the reorganisation removes the scripts
themselves. The claim was wrong and is gone. A control that catches its author's
error in its own first execution is the only kind worth adding.

**Reopen trigger for this entry:** any change to the `| tee` handling or
`pipefail` in a workflow that gates merges; a new `python3 scripts/...`
invocation added without `-I`; `check_docs_references.py` losing its caller in
`lint-docs`; the append-only stanza being weakened or the decision log moving
again; or adoption of the end-to-end canary, which would supersede the third
residual risk.

## Ruleset maintenance window

This section is the procedure `governance-root.yml` names when it blocks a pull
request. It replaces section 4 of the design document PR #155 deletes.

**When it applies.** A pull request that changes a protected path fails the
`Governance Root` required check by design. The check is not advisory and is not
to be bypassed with `--admin`; the window is the sanctioned way through, and it
exists so the change is deliberate, brief, and reversible.

**The invariant.** The ruleset is `18509013 protect-fork-rails` on
`jerudnik/jcode`. What must round-trip is the *writable* contract, not the raw
API response: `updated_at` and `_links` are server-managed and change on every
write, so hashing the whole document cannot round-trip by construction. Canonical
form is the writable keys only, and its baseline digest is:

```
99823fdb7ab60b4b4ab9592f414dc1cdbb494beb1cc4bf4464b1a26650aef374
```

**Superseded 2026-08-22; the current baseline is:**

```
71b7f6bab2265b6c4f490aee8901d49418165ed15a5ef2395eff6dc97ee0baa9
```

The digest above it is the pre-rollout value, left in place because this log
does not rewrite history. GitHub added
`require_extra_approval_for_unattributed_changes` to `pull_request` rules
between the 08-20 and 08-21 fork-health runs and set it true on the live
ruleset; `2006984dd` declared it in `scripts/required-checks.json` and recorded
both hashes, leaving the constant here for the operator to rotate. The canonical
diff between the two is exactly that one parameter line. Compare against the
lower value; a mismatch with it is the incident this step is for, and
`governance_compare.py --manifest scripts/required-checks.json --live` is the
second opinion that says whether the contract itself still holds.

Compute it with:

```sh
gh api repos/jerudnik/jcode/rulesets/18509013 > /tmp/rs.json
python3 -I -c 'import json,sys,hashlib
W=("name","target","enforcement","conditions","rules","bypass_actors")
d=json.load(open("/tmp/rs.json"))
print(hashlib.sha256(json.dumps({k:d[k] for k in W if k in d},
      sort_keys=True,separators=(",",":")).encode()).hexdigest())'
```

At rest the ruleset is `active`, has empty `bypass_actors`, carries the
`deletion`, `non_fast_forward` and `pull_request` rules, and requires exactly the
`Governance Root` and `PR Gate` contexts.

**The procedure.**

1. Bring the pull request green on every check except `Governance Root`, which
   will stay red for as long as the change is what it is. Confirm the branch is
   `main` or `automation/**` and that the diff is only what the window is for;
   an unrelated file merged under a window is the failure this ceremony exists
   to prevent.
2. Capture the ruleset to a file and assert the digest equals the baseline
   before touching anything. If it does not, stop: something changed the
   contract outside a window, and that is the incident, not the pull request.
3. Open the window by removing the `Governance Root` context from the
   `required_status_checks` rule and `PUT`ting the ruleset back. Change nothing
   else.
4. Merge through the REST endpoint,
   `PUT /repos/jerudnik/jcode/pulls/<n>/merge` with `merge_method=merge`. The
   GraphQL mutation is 503-flaky on this repository and merge method must be
   merge-commit.
5. Close the window by `PUT`ting the captured writable contract back verbatim.
6. Verify independently. Re-`GET` the ruleset, recompute the digest, and compare
   it to the baseline yourself rather than trusting the transcript of the script
   that just ran. Also confirm the merge landed as exactly one first-parent
   commit and that the merged diff matches the file and line counts expected.
7. Append an entry to this log describing what changed and why. Appending needs
   no window.

**Keep the two lists in lockstep.** The protected set is declared twice, in the
`protected=(...)` array in `.github/workflows/governance-root.yml` and in
`protected_paths.required` in `scripts/required-checks.json`. The comparator
checks set equality in both directions, so a path added to one and not the other
turns `Governance Root` red on every subsequent pull request. Verify before
merging with:

```sh
python3 -I scripts/governance_compare.py \
  --manifest scripts/required-checks.json --live --workflows-dir .github/workflows
```

which reports the number of paths it believes are enforced and exits non-zero on
disagreement.

## D037

**Docs lint is blind to the change it exists to catch.**

Date: 2026-08-17. Status: recorded, not fixed.

`Docs lint` is gated on `inputs.docs_only`, so it runs only for pull requests that
touch nothing but documentation. The job it runs includes the `stale-code-path`
check, whose purpose is to catch a document that still points at a code path
after that path has moved. A pull request that moves a code path is, by
definition, not docs-only. The check is therefore structurally absent from every
pull request that can trip it, and present only on the ones that cannot.

This was found by watching the final pull request's own checks: `Docs lint`
reported `skipped`, on a pull request whose description claimed to have repaired
that very job.

Not fixed here, deliberately, but the cost was measured rather than guessed.
Running the whole recipe against a pristine `main` shows two separate problems.
The style checker crashes before it lints anything: one skill file carries a
frontmatter description containing an unquoted colon, which is not valid YAML,
and the parser stops there. With that single line quoted, the checker completes
and reports 27 errors and 4 warnings across 120 files, all of them from two
vocabulary rules rather than anything structural. The document checker itself
passes at 122 active documents.

So the blast radius is one malformed line and 27 wording fixes, not an unknown.
That is a tractable pull request, and it is a different pull request from this
one: this is a window change, and a window is the worst place to be editing
prose across 120 files. Recording the number here is the point. It converts
"nobody knows what happens if we turn it on" into a task with a size.

The generalisation is the recurring one in this log: a control that cannot fire
in the situation it was written for reads exactly like a control that has nothing
to report.

## D038

**A hardening fix that broke the harness that proved the hardening.**

Date: 2026-08-17. Status: closed.

The guard-bypass fix (D036) needs the three budget guards to keep importing a
sibling module while running under `-I`, which removes the script directory from
`sys.path`. The first version appended that directory back and left it there.
Every claim in the guards still held, the shadowing attack was still blocked, and
`tests/test_guard_nonvacuity.py` went red anyway: it asserts that `sys.path` does
not contain `scripts/` after the harness runs, and the guards were now putting it
back in-process.

Two things are worth recording.

The assertion was stricter than the threat. Appending to the *end* of `sys.path`
leaves the standard library ahead of `scripts/`, so the planted-module attack
fails either way. The harness was still right to reject it: an entry left on the
path is a hazard for every later import, not just the one it was added for. The
fix borrows the entry for the duration of a single import and removes it in a
`finally`.

A second version resolved the sibling by explicit file path and avoided
`sys.path` entirely. It was cleaner and it was wrong twice: the harness copies a
guard into a temporary directory and relies on the sibling resolving through the
path, and its static import-closure walk follows a literal `import` statement
that `importlib` no longer provides. Both failures were mechanical and immediate.
Neither would have been predicted from reading the guard alone.

The same leak already existed in `tests/test_ci_workflow_commands.py`, which
inserted `scripts/` at the front of `sys.path` and never removed it. That made
the scrub assertion order-dependent on `main`: running the two suites in one
command with the CI-contract suite last fails, and has failed since the harness
landed. Both suites now borrow and return, and both orders pass under Python 3.9
and 3.14.

A harness earns its keep when it rejects its author's own work. This one did, on
consecutive attempts, for reasons that were correct each time.

## D039

**Every pull request paid for the full product route, including the ones that
could not change the product.**

Date: 2026-08-17. Status: closed.

A one-line edit to a governance workflow ran the same route as a change to the
compiler-facing code: the `full-test` recipe rebuilt the release binary, the Nix
package was built, and `just release-check` ran twice in parallel, once inside
the Nix validate job and once as the Smoke job. Measured on the run for #162:
`full-test` 13m58s, Nix package build 19m, `release-check` 728s in validate and
637s in smoke. About forty runner-minutes per pull request, spent to say nothing
about the edit that triggered it.

The duplication is worth naming on its own. `Smoke check` and the
`Run release-check recipe` step in `Validate Nix and workflow policy` are the
same command on the same runner image, and each was the critical path of its own
job. Nobody chose that; the two jobs were written at different times and neither
knows about the other.

The fix is a second route out of the existing classifier. A change set is
product-impacting unless every path in it matches an allowlist, so anything
unrecognised takes the expensive route by default. Only the legs that exercise
the built artifact are gated on it. actionlint, the reusable-call and permission
checkers, the workflow contract tests, `cargo check` and the test-graph compile
still run on every pull request, and those are what actually judge a workflow
edit. Replayed over the last twelve merges the classifier routes exactly one of
them, #161, to the cheap path.

Two details are the whole reason this is safe to have. The route is compared
against `'false'` rather than `'true'`, so a classifier that fails to write its
output runs the expensive legs instead of skipping them, and an empty change set
classifies as impacting. The gated Nix steps spell out
`github.event_name == 'push'`, because a tag push carries no inputs at all and an
undefined input evaluates false -- which would have skipped the release build on
exactly the event that most needs it.

The generalisation, again, is the one this log keeps recording: a skipped job and
a passing job are the same green tick. Gating work on a classifier is only
acceptable when the failure mode of the classifier is to do more work, not less.

Measured afterwards rather than predicted, on two pull requests against the same
base. #163 changed the route-defining workflows and so took the full route:
Rust checks 927s, Nix validate 787s, Nix package build 944s, Smoke 791s. #164
changed only `.vale.ini` and took the cheap one: Rust checks 317s, Nix validate
84s, package build and Smoke reported `skipped` by the jobs API rather than
inferred from a green tick. Critical path 15m44s to 5m17s; total job wall clock
3449s to 401s.

The symptom that prompted this was a governance-workflow edit paying for the full
product route, and that case is answered: a change to `governance-root.yml` or
`main.yml` alone now classifies as inert. The five workflows that define the
product legs -- `ci.yml`, `pr.yml`, `fork-ci.yml`, `nix.yml`, `freebsd-smoke.yml`
-- deliberately keep the full route, because they are the files that decide what
runs, and a route that could exempt its own definition would be no route at all.

## D040. D001 and D002 are released; `597598fb9` is the frozen pre-reorg state

**Decision:** release D001 and D002. Both are superseded by the owner-authorized
documentation reorganization that merged as #155 (`8299dd932`), which deleted
1020 tracked paths including the whole of `docs/fork/`. Their preservation
guarantees are honored in history, not in the working tree, and this entry
records where.

Cite `597598fb9` (= `8299dd932^1`) as the frozen final mainline state of the
pre-reorg tree. Each fact below was re-derived rather than carried forward:

- `597598fb9` is an ancestor of `github/main` (`git merge-base
  --is-ancestor`), so every deleted path stays permanently reachable from
  `main`.
- The predecessor register is `597598fb9:docs/fork/ideal-base/DECISIONS.md`,
  blob `db0b1ee0bd455585a696044dc22e5aff1b2f9d7b` -- 1027 lines, 37 entries,
  ending at D034. It is the last state of the old file, and the only place
  D034 appears there.
- `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` is identical at `794114a82` and
  `597598fb9`: blob `a0c92c0b5f02e958ad2f734c116b36fe65fe4fae`, content
  sha256 `ca3f19980b1e4fab0a734397d7c6f41ccd5c203a4fa209cfe9eef2f16beed5b6`.
  D002's byte-for-byte guarantee is intact in history.
- Ruleset 18509013 carries `deletion` and `non_fast_forward` with
  `enforcement: active` and `bypass_actors` **key present** and empty, so
  reachability cannot be quietly severed. Key presence was asserted before the
  value was read; an omitted key cannot be misread as an empty list.

**Reason:** the reopen triggers of both entries have fired. D001's was "an
explicitly authorized archive migration"; D002's was "explicit user
authorization to break the tracked-baseline preservation guarantee". The
reorganization was exactly that. Left unreleased, both entries keep asserting
in the present tense things that are false on `main` today: D001 says
`docs/fork/recovery/` and `docs/fork/normalization/` "remain at their existing
paths" when `git ls-tree github/main docs/fork` returns nothing, and D002
forbids editing a file that no longer exists. That is the stale-claim class
D034 already names as the more damaging half -- an entry that is wrong outlasts
an entry that is absent, because a reader still treats it as binding.

The second defect this closes is discoverability, not reachability. Nothing in
the merged tree cited either revision: the log named the old path once, in
D035's editorial note, and never named the commit. Reachability was never in
question; findability was.

**On D027:** it stays. The argument for re-homing it assumed a pruned contract
register accumulating roadmap items, and no pruned register exists -- the whole
953-line journal was relocated verbatim, so D027 sits inside preserved history
rather than on a curated contract surface. Moving it would require deleting log
lines, which the append-only control in `governance-root.yml` correctly
refuses. Its own closing line, "filed as a post-epic direction so it survives",
already describes its status accurately: filed, not binding.

**Method note, on a figure that aged.** D036 and the comment at
`governance-root.yml:49` both cite the log's churn as "34 commits, 33
first-parent merges" in the three months to 2026-08-17. Re-measured today the
same query returns 41 and 37. The pair is not wrong; two things happened to it.
The labels are transposed -- as of `c04dc0932`, the commit that introduced the
sentence, the measurement was 33 all-commits and 34 first-parent, not the other
way round. And the quantity is monotonically growing, so a number correct when
taken drifts every time the log is appended to, which is the routine act the
sentence exists to describe. Neither is load-bearing: the argument only needs
"appending is frequent", and every value in the range clears that bar by an
order of magnitude. It is recorded because a growing count written as a
constant is the same defect the log keeps cataloguing, and the next reader
should meet it labelled rather than discover it.

The transposition is also a reminder that `--first-parent` can exceed the
unfiltered count, because a merge commit carries its whole side branch and so
survives a path filter that the individual side commits do not. A reader who
assumes the filtered number must be smaller will read the swap as an error in
the tooling rather than in the label.

**Reopen trigger:** an authorized history rewrite, an organization migration or
repository re-creation, or the adoption of shallow mirroring on any backup or
distribution path. Any of those makes reachability-by-history insufficient, at
which point the belt is one command --
`git push github 597598fb9:refs/tags/pre-docs-reorg-2026-08` -- anchoring
`597598fb9`, not `794114a82`, which predates #154 and misses D034.

## D041. A skipped required context merges under a ruleset, verified rather than cited

The audit of check deciders (#187 through #190) closed three defects and filed
four open items. All four are now closed, and the issue file is deleted rather
than archived, per the rule that solved issues do not linger.

**The platform question is settled by observation, not by documentation.**
GitHub's troubleshooting page states that `success`, `skipped` and `neutral` all
satisfy a required check, but says nothing about whether rulesets differ from
classic branch protection, and this repository uses a ruleset. That gap was
recorded as unresolvable from inside the fork, because settling it appeared to
need a pull request that deliberately disables a gate here.

It did not. A throwaway private repository carried the same shape -- one active
ruleset, no bypass actors, one required context -- and three arms:

| arm | required context | `mergeable_state` | merge attempt |
| --- | --- | --- | --- |
| gate runs | `success` | `clean` | not attempted |
| gate fails | `failure` | `blocked` | refused, 405 "Required status check is failing" |
| gate skipped by `if: ${{ false }}` | `skipped` | `clean` | **merged** |

The third arm merged. So under a ruleset, as under classic protection, a
required context skipped by a conditional satisfies the requirement. The failing
arm proves the ruleset had teeth, which is what makes the third arm readable at
all. The probe repository was archived afterwards; it holds no fork content.

This is why the plants added in #188 matter more than the protection added in
#189: the platform will not refuse a gate that has been switched off, so the
refusal has to come from a check that runs.

**Failure capability was mis-measured, and the correction is the interesting
part.** The audit reported that only `Governance Root` had ever been observed
failing. That was derived from the check-runs of *merged* pull request heads,
which are green by construction -- the sample could not have produced any other
answer. Querying failed workflow runs instead shows `PR Gate` failing 11 times,
`Checks / Fork CI / Rust checks` 8, and every routed leg at least once. Eight of
the ten audited checks have demonstrated they can fail; the two that have not
are the two that gate nothing.

**Two mechanisms replace the two remaining items.** The route contract
(`_ROUTE_CONTRACT` plus `plant_route_contract`) pins which ci.yml jobs each of
the three classifier routes runs, so widening a condition reddens rather than
quietly skipping a leg. `CONTRACT_PLANTS` plus
`_check_required_contexts_have_plants` requires every required context named in
the manifest to have a plant proving its `if:` contract can fail, so adding a
third required context without a detector is now a failure rather than a
reproduction of the defect this workstream started from.

**Reopen trigger:** GitHub changing how a skipped check is scored, which would
show up as the merge in arm three being refused; or a required context being
added by a path that does not read `scripts/required-checks.json`, which
`_check_required_contexts_have_plants` would not see.

## D042. Deleting dead code shrank a critical domain, which the budget gate refuses by design

Date: 2026-08-27. Status: closed.

PR #209 removed two TUI production files that nothing referenced,
info_widget_timeline.rs and swarm_plan_graph.rs, both directly under
`crates/jcode-tui/src/tui/`. Neither had a `mod` declaration anywhere in the
workspace. `Checks / Fork CI / Rust checks` then failed on
`scripts/check_critical_path_budget.py`: `tui lost in-scope production files:
197 -> 196`.

That is the gate working. `scope_shrink_regressions` exists because a count-only
ceiling has a shrinking denominator, so moving a file out of a critical
directory would read as cleanup. The check cannot distinguish debt that left the
scope from debt that was fixed, so it refuses both and asks for a recorded
decision. Here the debt was removed with the dead code: no ceiling moved, and
the domain's measured counts fell (tui swallowed_error 596 -> 594, oversize 33
-> 32) rather than relocating.

**What changed.** `EXPECTED_FILE_COUNTS["tui"]` 197 -> 196, and the `just check`
digest pin `5ed12e31...` -> `249e7ab1...`. The pin lives in `justfile`, a
protected path, so this merged through the ruleset maintenance window above.

**Observed while diagnosing, not fixed here.** The budget checker's test suite,
now `tests/test_critical_path_budget.py` and at the time still under `scripts/`,
is not wired into any recipe: `test-python` iterates `tests/test_*.py`, and this
file sits in `scripts/`. It has four failing tests on `main` at `45b96548d`,
unchanged by this PR. Two of them,
`test_expected_counts_match_the_current_tree` and
`test_expected_counts_sum_to_the_scanned_total`, assert the expected counts equal
the measured tree exactly; the tree has drifted upward (lifecycle 69 vs 66,
provider_infrastructure 21 vs 20) because growth is permitted and never
re-recorded. A third, `test_pr_gate_runs_the_pinned_check_recipe`, still expects
`nix shell nixpkgs#just -c just check` after fork-ci.yml added `nixpkgs#python3`
to that command. An unwired test that would have caught the drift it was written
for is the same shape as D037.

**Reopen trigger:** a further decrease in any domain's file count, which will
fail the same way and should be recorded the same way; or wiring
that suite into `test-python`, which requires
resolving the four pre-existing failures first.

**Resolved 2026-08-27.** The second trigger fired. The suite moved to
`tests/test_critical_path_budget.py`, which the `test-python` glob already
covers and which `check_test_wiring.py` polices, so it cannot be orphaned
again. Of the four failures, the two count assertions were the drift itself and
were fixed by re-recording `EXPECTED_FILE_COUNTS` (lifecycle 66 -> 69,
provider_infrastructure 20 -> 21, digest `249e7ab1...` -> `aae1ad95...`); the
other two were stale assertions about `fork-ci.yml` and about panic headroom,
and were corrected without weakening what they test. All 33 cases pass.

## The decision log rule now measures loss instead of forbidding edits

**2026-08-27.** `governance-root.yml` failed any pull request that removed a
single line from this file. The intent, stated in the comment above the rule, is
that losing the log is the failure PR #155 nearly caused. The implementation was
stricter than that intent, and the gap was not free.

Moving the critical-path budget test suite from `scripts/` to
`tests/test_critical_path_budget.py` left three citations in
this file pointing at a path that no longer exists.
`scripts/check_docs_references.py` holds this file at a zero baseline for
stale code paths and says to update the citation to where the code moved, and
refuses to have its baseline raised. This rule said the edit that would do so was
forbidden. Both controls are individually sound and they gave opposite orders, so
no honest version of that change could pass. Both were run to confirm it rather
than argued from: updating the citations lost four lines and failed here, and
leaving them and appending a correction instead reported three findings there.

The rule now checks three things. The file must exist, so deleting or moving it
still fails and moving the log stays a governance event. Its `##` entry count must
not fall, so no decision can be dropped. Its total length must not fall, so an
entry cannot be gutted while its heading is left in place to satisfy the count.
Correcting a citation inside an entry passes, because it removes lines without
losing anything.

This does not license rewriting history. The log's convention is unchanged and
the digest supersession above is still the pattern to follow when a recorded fact
goes stale: append the correction, leave the original. The rule simply no longer
treats every in-place edit as an attempt to destroy the record.

**Reopen trigger:** a change that drops a decision while adding enough lines and
headings elsewhere to keep both counts up. The guard would not see it. If that
happens, compare entry headings by name rather than by count.

## The standalone ratchet scripts are retired; the wired gate measures the tree

**2026-08-27.** The four per-dimension ratchet scripts and their baselines are
deleted: the panic, swallowed-error, code-size, and test-size checkers with
`panic_budget.json`, `swallowed_error_budget.json`, `code_size_budget.json`,
and `test_size_budget.json`. They ran only from `scripts/preflight.sh`, which
no workflow and no justfile recipe executes, and the guard non-vacuity harness
had already recorded that three of them exited 1 against clean main. A checker
that fails on the code it is supposed to certify, and that nothing runs, gates
nothing; keeping it invites the C7 failure mode this log records above, where
machinery survives on the strength of what it used to do.

What they nominally provided is now measured directly.
`scripts/check_critical_path_budget.py`, which `just check` and both CI entry
workflows already run digest-pinned, previously read the repository totals for
its trend section out of those baseline files. It now scans the tree itself
with the same shared classifier (`scripts/rust_production_filter.py`) and
holds the measured totals under its pinned `REPOSITORY_CEILINGS`: panic-prone
lines, swallowed errors, oversize production files and their total LOC, and
oversize test files. At the switch the measured tree sat at or below every
mark, verified by running both the old baseline read and the new scan; no
ceiling moved. The scope digest is unchanged because the marks and scope are
unchanged.

What is lost, deliberately: the per-file ratchets. The old scripts pinned debt
to individual files, so debt could not migrate between files without an edit
to a baseline. The marks bound only the totals. That trade was accepted
because the per-file mechanism was unwired and red, so its precision was
theoretical, and because the critical domains keep per-domain ceilings with
per-file reporting in the wired gate. `warning_budget.txt` stays: counting
warnings needs a full compile, so the recorded number remains the input, and
`scripts/check_warning_budget.sh` remains its maintainer.

**Reopen trigger:** a sustained rise in a repository total toward its mark
with the debt concentrating in files outside the critical domains. That is the
migration the per-file ratchets would have caught, and it would justify wiring
a successor rather than resurrecting the dead scripts.
