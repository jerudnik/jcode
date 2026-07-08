# GLM Fork Survey — improvement proposals (ponytail-revised)

> Originated as a glm-5.2 (z.ai) survey of the fork's swarm/DAG/comm/mcp/lifecycle
> surfaces. **Revised by a ponytail (lazy-senior) review**: every claim was checked
> against the actual code. GLM tends to over-value big refactors and occasionally
> misreads code — several of its items are wrong or invented. Still advisory: verify
> before acting, but this triage is trustworthy.
>
> 13 original proposals → **3 KEEP**, **2 DEFER (speculative)**, **8 REJECT**.

---

## Do these (ranked by real impact / effort)

| # | Item | Real impact / effort | Verdict |
|---|------|----------------------|---------|
| 1 | [Delete redundant `set_working_dir` call in headless](#1-delete-the-redundant-set_working_dir-call) | Low / ~zero | **KEEP** |
| 2 | [Fold remaining swarm-state Arcs into the existing `SwarmState` struct](#2-extend-the-existing-swarmstate-struct-instead-of-threading-arcs) | Medium / Medium | **KEEP (downranked)** |
| 3 | [`HashSet` for `addressable_session_ids` lookup](#3-hashset-for-the-addressable-session-lookup) | Low / Low | **KEEP (downranked)** |

## Maybe later (real features, no measured need yet)

| # | Item | Real impact / effort | Verdict |
|---|------|----------------------|---------|
| 4 | [MCP `tools/call` streaming/progress](#4-defer-mcp-toolscall-streaming) | Speculative / High | **DEFER** |
| 5 | [Purge terminal swarm members from the cap](#5-defer-purge-of-terminal-members) | Low / Medium | **DEFER** |

---

## 1. Delete the redundant `set_working_dir` call
**File:** `crates/jcode-app-core/src/server/headless.rs` (~L142–156)
`create_headless_session` sets `working_dir` twice; the first call (`if let ... && let Some(path) = dir.to_str()`) is a strict subset of the second, which already handles the non-UTF8 fallback. Delete the first block. Four lines gone, behaviour identical, one less "which one wins?" question at 3am. Pure win because the effort is ~zero.

## 2. Extend the existing `SwarmState` struct instead of threading Arcs
**File:** `crates/jcode-app-core/src/server/comm_plan.rs`, `client_comm_message.rs`, `state.rs`
Verified: `handle_comm_propose_plan` takes 15 params, `handle_comm_message` takes 16, and ~15 handlers repeat the `swarm_members / swarms_by_id / plans / coordinators / queues / event_history / event_counter` shape. GLM's core observation is real.

But GLM missed that a `SwarmState { members, swarms_by_id, plans, coordinators }` struct **already exists** (`state.rs:108`). The lazy move is not a new `SwarmContext` — it's widening the struct that's already there (or a thin wrapper adding `sessions / queues / event_history / event_counter / txs`) and passing `&SwarmState` instead of the loose Arcs. Kills the threading, reduces "mismatched-lock" foot-guns, easier to construct in tests.

Downranked from GLM's **High/Low** to **Medium/Medium**: it's a wide, mechanical edit across ~15 signatures — real, but not "Low effort," and it buys maintainability, not runtime. Do it when you're already touching these handlers, not as a standalone crusade.

## 3. `HashSet` for the addressable-session lookup
**File:** `crates/jcode-app-core/src/server/client_comm_message.rs`
Verified: `addressable_session_ids: Vec<String>` is scanned with `.contains()` inside the `for session_id in &target_sessions` loop — genuinely O(N·M), both bounded by `MAX_SWARM_MEMBERS = 1000`. Build it as a `HashSet` once; the loop becomes O(1) lookups. A few lines.

Downranked from GLM's **High** to **Low**: worst case is ~1M short-string compares on one broadcast — microseconds, not the "DoS vector" GLM claims. Worth doing because the fix is trivial and obviously correct, not because anything is measurably slow.

## 4. (DEFER) MCP `tools/call` streaming
**File:** `src/cli/mcp_serve.rs` — *merges GLM's items 6 and 10, which are the same proposal listed twice.*
Verified: `handle_tools_call` runs one blocking `debug_command` and returns the whole payload as a single text block; no progress notifications, no `stream:true`, and there is no `ServerEvent::ToolProgress` variant anywhere.

But the tools exposed here are single tool-registry calls (`tool:<name>` → bash/read/edit/etc.), **not** long-running agent conversation turns, so GLM's "editor times out after 30s" rationale barely applies today. This is a legitimate feature, but speculative until a real MCP client actually hits a timeout. High effort (debug-socket protocol change + MCP `notifications/progress` plumbing). Note that the socket already has a general `events:subscribe` streaming path to build on if/when needed. YAGNI until someone reports the timeout.

## 5. (DEFER) Purge of terminal members
**File:** `crates/jcode-app-core/src/server/swarm.rs`
Verified with a correction to GLM: `sweep_dead_pid_swarm_members` marks dead members `crashed` and keeps them — but the comment's motivation is the opposite of what GLM claims (it skips already-dead members to avoid *re-loading* them from disk, `swarm.rs:272`). Members are **capped** at `MAX_SWARM_MEMBERS`, so this is *not* "unbounded memory growth."

The real (smaller) risk: on a multi-day daemon the 1000-slot cap could fill with corpses and start refusing new spawns. That's worth a periodic purge of long-terminal members eventually. Downranked from **High** to **Low/Medium** — low-priority hardening, not a footgun. Do it if long-uptime cap exhaustion is ever observed.

---

## Rejected (and why)

- **#2 orig — "async awaits while holding swarm locks":** premise is FALSE. In both `salvage_assignments_of_dead_member` and `refresh_swarm_task_staleness` the write guard is scoped and dropped *before* the `persist_swarm_state_for().await`. No lock is held across disk I/O; the "daemon freezes under load" rationale is fabricated. The temp-`SwarmState` Arc-clone GLM dislikes is cheap and harmless.
- **#5 orig — strum/macro for enum string mappings:** ~120 lines of boilerplate is real, but `strum` is not a dependency and both enums have an `Other(String)` catch-all needing special handling. Adding a dependency (or a local macro to maintain) to replace working, tested boilerplate is a net complexity trade, not a win. Leave it.
- **#7 orig — unify the two lifecycle monitors:** overstated. They share a ~15-line interval skeleton but differ substantially (the temporary monitor adds owner-pid liveness + socket/metadata cleanup). Collapsing them into one `Option<OrphanPolicy>`-parameterised function trades a little duplication for a branchier abstraction. Not worth it.
- **#8 orig — memoize the spawn tree:** premature optimization. `swarm_ancestors` is O(N·depth) only in the addressing path (not per-event fan-out), and real spawn trees are shallow. A `parent_to_children` index is a new invariant to keep in sync on every join/death — a bug farm for an unmeasured gain.
- **#9 orig framing — "unbounded memory growth":** false as stated (members are capped). The genuine, smaller concern survives as DEFER item 5.
- **#11 orig — remove "dead" `event_txs`:** FALSE, GLM read it backwards. `event_txs` (plural) is the **live** multi-attachment routing map (insert on attach, broadcast to all attachments, detach, liveness checks). `event_tx` (singular) is the backward-compat/headless fallback. Removing `event_txs` breaks multi-view routing.
- **#12 orig — enforce DAG edge artifact typing:** the feature GLM wants to "validate" does not exist. `PlanItem` has no artifact input/output type fields — edges are bare `blocked_by: Vec<String>` ids. This isn't validation, it's designing and building a typed-dataflow system from scratch. Out of scope; no evidence it's needed.
- **#13 orig — remove `OwnedChildPermit`:** **REJECTED explicitly.** This is a deliberate, recently-added process-cap safety mechanism (fork-bomb guard): it caps concurrent owned MCP child processes process-wide at `MAX_OWNED_MCP_CHILDREN = 64` via a global atomic. The RAII `Drop` decrement is **load-bearing** — it releases the slot when the child dies. Removing the permit removes the ceiling. GLM mistook a safety cap for "legacy indirection." Do not touch.
