# Fork Improvement Backlog

This backlog captures small-to-medium fork improvements verified against the current
codebase. The triage is intentionally conservative: keep the low-risk changes with
clear payoff, defer speculative features until there is measured need, and reject
changes whose premise is false or whose complexity outweighs the benefit.

## Do these

| # | Item | Real impact / effort | Verdict |
|---|------|----------------------|---------|
| 1 | [Delete redundant `set_working_dir` call in headless](#1-delete-the-redundant-set_working_dir-call) | Low / ~zero | **KEEP** |
| 2 | [Fold remaining swarm-state Arcs into the existing `SwarmState` struct](#2-extend-the-existing-swarmstate-struct-instead-of-threading-arcs) | Medium / Medium | **KEEP** |
| 3 | [`HashSet` for `addressable_session_ids` lookup](#3-hashset-for-the-addressable-session-lookup) | Low / Low | **KEEP** |

## Maybe later

| # | Item | Real impact / effort | Verdict |
|---|------|----------------------|---------|
| 4 | [MCP `tools/call` streaming/progress](#4-defer-mcp-toolscall-streaming) | Speculative / High | **DEFER** |
| 5 | [Purge terminal swarm members from the cap](#5-defer-purge-of-terminal-members) | Low / Medium | **DEFER** |

## 1. Delete the redundant `set_working_dir` call

**File:** `crates/jcode-app-core/src/server/headless.rs` (~L142-L156)

`create_headless_session` sets `working_dir` twice. The first call, guarded by
`if let ... && let Some(path) = dir.to_str()`, is a strict subset of the second,
which already handles the non-UTF8 fallback. Delete the first block. Four lines
go away, behavior stays identical, and there is one fewer ordering question in
session setup.

## 2. Extend the existing `SwarmState` struct instead of threading Arcs

**Files:** `crates/jcode-app-core/src/server/comm_plan.rs`,
`crates/jcode-app-core/src/server/client_comm_message.rs`,
`crates/jcode-app-core/src/server/state.rs`

`handle_comm_propose_plan` takes 15 params, `handle_comm_message` takes 16, and
roughly 15 handlers repeat the `swarm_members / swarms_by_id / plans /
coordinators / queues / event_history / event_counter` shape. The code already
has `SwarmState { members, swarms_by_id, plans, coordinators }`
(`state.rs:108`), so the next step is to widen that existing struct, or add a
thin wrapper for `sessions / queues / event_history / event_counter / txs`, and
pass `&SwarmState` instead of loose Arcs.

This is a mechanical edit across many signatures. It should improve
maintainability and reduce mismatched-lock footguns, but it is not urgent runtime
work. Do it when these handlers are already being touched, or when tests need a
cleaner state fixture.

## 3. `HashSet` for the addressable-session lookup

**File:** `crates/jcode-app-core/src/server/client_comm_message.rs`

`addressable_session_ids: Vec<String>` is scanned with `.contains()` inside the
`for session_id in &target_sessions` loop. Both sets are bounded by
`MAX_SWARM_MEMBERS = 1000`, so the current worst case is modest, but the fix is
simple and clearly correct: build a `HashSet` once and make the loop do O(1)
membership checks.

## 4. (DEFER) MCP `tools/call` streaming

**File:** `src/cli/mcp_serve.rs`

`handle_tools_call` runs one blocking `debug_command` and returns the whole
payload as a single text block. There are no progress notifications, no
`stream:true`, and no `ServerEvent::ToolProgress` variant.

This is a legitimate feature, but speculative for the current tool surface. The
tools exposed here are single tool-registry calls (`tool:<name>` ->
bash/read/edit/etc.), not long-running agent conversation turns. Implementing
streaming would require a debug-socket protocol change plus MCP
`notifications/progress` plumbing. The socket already has a general
`events:subscribe` streaming path to build on if a real client starts hitting
timeouts.

## 5. (DEFER) Purge of terminal members

**File:** `crates/jcode-app-core/src/server/swarm.rs`

`sweep_dead_pid_swarm_members` marks dead members `crashed` and keeps them. The
comment explains that already-dead members are skipped to avoid re-loading them
from disk (`swarm.rs:272`). Members are capped at `MAX_SWARM_MEMBERS`, so this
is not unbounded memory growth.

The real risk is smaller: on a multi-day daemon, the 1000-slot cap could fill
with terminal members and start refusing new spawns. Periodic purging of
long-terminal members is reasonable if long-uptime cap exhaustion is observed.

## Rejected

- **Async awaits while holding swarm locks:** false premise. In both
  `salvage_assignments_of_dead_member` and `refresh_swarm_task_staleness`, the
  write guard is scoped and dropped before `persist_swarm_state_for().await`. No
  lock is held across disk I/O; the temporary `SwarmState` Arc clone is cheap and
  harmless.
- **Strum or a macro for enum string mappings:** the boilerplate is real, but
  `strum` is not a dependency and both enums have an `Other(String)` catch-all
  needing special handling. Adding a dependency or a local macro to replace
  working, tested boilerplate is a net complexity trade.
- **Unify the two lifecycle monitors:** they share a short interval skeleton but
  differ substantially. The temporary monitor adds owner-pid liveness plus
  socket/metadata cleanup. Collapsing them into one policy-parameterized function
  trades a little duplication for a branchier abstraction.
- **Memoize the spawn tree:** premature optimization. `swarm_ancestors` is
  O(N*depth) only in the addressing path, and real spawn trees are shallow. A
  `parent_to_children` index would be another invariant to maintain on every
  join/death for an unmeasured gain.
- **Treat terminal members as unbounded memory growth:** false as stated because
  members are capped. The smaller cap-exhaustion concern is covered by the
  deferred purge item.
- **Remove "dead" `event_txs`:** false premise. `event_txs` is the live
  multi-attachment routing map: insert on attach, broadcast to all attachments,
  detach, and liveness checks. `event_tx` is the backward-compatible headless
  fallback. Removing `event_txs` breaks multi-view routing.
- **Enforce DAG edge artifact typing:** the feature to validate does not exist.
  `PlanItem` has no artifact input/output type fields; edges are bare
  `blocked_by: Vec<String>` ids. This would be a new typed-dataflow design, not
  a validation pass.
- **Remove `OwnedChildPermit`:** explicitly rejected. This is a deliberate
  process-cap safety mechanism for owned MCP child processes. It caps concurrent
  owned children process-wide at `MAX_OWNED_MCP_CHILDREN = 64` via a global
  atomic, and the RAII `Drop` decrement releases the slot when the child dies.
  Removing the permit removes the ceiling.
