# D01-FIX-1: wiring the safety tier classifier into tool dispatch

Node: `D01-FIX-1` (parent `W6`), owning finding `D01-F13`.
Branch: `automation/d01-fix-1-tier-gate`. Base: `95dbff895`.

## What was wrong

`SafetySystem::classify` (`crates/jcode-base/src/safety.rs:177`) and the
`AUTO_ALLOWED` table (`safety.rs:132`) were complete and had no production
caller. Tool dispatch gated only on `SessionToolPolicy`. The documented tier
gate therefore protected nothing, and an ambient (unattended) session received
the full tool registry with no policy set (`ambient/runner.rs:915`), so an
unattended agent could run `bash` with no human in the loop.

## Gate 2: before / after grep

Before, at `95dbff895`:

```
$ grep -rn "ActionTier" crates/ --include=*.rs -l
crates/jcode-base/src/safety.rs

$ grep -rn "\.classify(" crates/ --include=*.rs
crates/jcode-base/src/safety.rs:551 ... :590      (26 lines, all inside #[cfg(test)])
```

One file, and every `.classify(` caller is a test in that same file.

After:

```
$ grep -rn "ActionTier" crates/ --include=*.rs -l
crates/jcode-app-core/src/tool/ambient.rs
crates/jcode-base/src/safety.rs

$ grep -rn "\.classify(" crates/ --include=*.rs | grep -v "^crates/jcode-base/src/safety.rs"
crates/jcode-app-core/src/tool/ambient.rs:144:    match get_safety_system().classify(tool) {
```

## The wire

`Registry::execute` (`crates/jcode-app-core/src/tool/mod.rs:581`) calls
`check_ambient_action_tier` (`tool/ambient.rs:137`) after alias resolution and
`SessionToolPolicy`, before the tool is looked up. The check is a no-op unless
`is_ambient_session_registered(session_id)` is true, and only two production
call sites register a session (`ambient/runner.rs:922` and the TUI's
`set_ambient_mode`, `construction.rs:856`), so interactive sessions are never
gated.

### Why some tools must bypass the gate

`end_ambient_cycle`, `request_permission`, `schedule_ambient`, `send_message`
and `bash`/`edit`/`write` all classify as `RequiresPermission`, because
`AUTO_ALLOWED` lists only read-only tools. A naive "refuse everything not in
`AUTO_ALLOWED`" therefore deadlocks in two distinct ways:

- `end_ambient_cycle` is how a cycle terminates (`ambient/runner.rs` reads the
  result via `take_cycle_result`). Gating it leaves the cycle unable to finish.
- `request_permission` is the tool used to ask. Gating it means the only escape
  from the gate is itself gated.

`TIER_GATE_EXEMPT` names that control-plane set. `batch` is included because it
re-enters `Registry::execute` for each inner call (`tool/batch.rs:237`), so
inner tools are still classified individually; gating the wrapper would have
blocked batched tier-1 reads.

### Refusal, not suspension

Nothing resumes a tool call after approval. `record_decision`
(`safety.rs:243`) removes the request from the queue and appends a `Decision`
to history; no caller re-runs the original action, and nothing in production
reads `PermissionRequest::wait` (`request_permission` always returns
`Queued`). So the gate refuses and tells the agent to call
`request_permission` and retry the work itself. Describing this as "queued for
review, will resume" would have been false.

## Controls

Both controls were planted with `cp` backups, run, then restored and verified
byte-identical (`diff -q` against the backup).

### Control 1: remove the dispatch call

Replaced `ambient::check_ambient_action_tier(...)?;` in `Registry::execute`
with a comment, confirmed on disk by grep before running.

```
test tool::tests::registry_execute_refuses_tier2_tool_for_ambient_session ... FAILED
  unattended agent must not run a tier-2 tool unasked:
  ToolOutput { output: "Command completed successfully (no output)", title: Some("true"), ... }
```

That failure output *is* the defect: an ambient session ran `bash` and it
succeeded.

### Control 2: make the gate over-broad

Replaced the `match classify(...)` arm with an unconditional `Err`, so the gate
refuses tier 1 as well.

```
test tool::tests::registry_execute_allows_tier1_and_control_plane_for_ambient_session ... FAILED
  tier-1 tools must not be newly blocked:
  Some(Tool 'ls' requires user permission in an unattended session. ...)
```

This control exists because control 1 alone is not sufficient: a gate that
refused *everything* would still pass the refusal test. The two controls fail
on different tests, which is what distinguishes a correct gate from a gate that
merely blocks.

## Tests

Four tests, in `crates/jcode-app-core/src/tool/tests.rs`. Three of them run
through `Registry::execute` rather than calling the helper directly, so they
fail if the gate is defined but never wired into dispatch, which is exactly the
defect class this node exists to close.

| Test | Asserts |
|---|---|
| `registry_execute_refuses_tier2_tool_for_ambient_session` | ambient `bash` is refused, and the message names `request_permission` |
| `registry_execute_allows_tier2_tool_for_interactive_session` | the same tool and input succeed when the session is not ambient |
| `registry_execute_allows_tier1_and_control_plane_for_ambient_session` | ambient `ls` still runs |
| `tier_gate_exempts_the_tools_an_ambient_cycle_needs_to_finish_and_ask` | `end_ambient_cycle` and `request_permission` bypass the gate |

Full `jcode-app-core` lib suite: **1186 passed, 1 failed, 23 ignored**. The one
failure is `server::debug_command_exec::tests::debug_tool_selfdev_reload_returns_promptly_for_direct_execution`
("Could not find jcode repository directory"), reconfirmed as pre-existing by
stashing this change and rerunning it on the clean baseline, where it fails
identically. `cargo clippy --all-targets --all-features` is clean.

## Scope not covered, stated rather than implied

The gate keys on the ambient session registry, so these unattended paths are
**not** gated:

- **Subagent / swarm workers.** `run_subagent` creates a fresh session with a
  new id (`tool/subagent.rs:62`), which is never ambient-registered, so a
  subagent spawned by an ambient agent escapes the gate. Those workers are
  policy-scoped (an `allowed` set is passed at `subagent.rs:82`) but not
  tier-gated. Ambient sessions do receive `subagent` and `swarm` in the default
  registry, so this is a reachable bypass, not a theoretical one.
- **The overnight supervisor** (`overnight.rs:260`) builds an agent with no
  ambient registration.
- **Scheduled resume/spawn** targets, same reason.

Also not closed: approval cannot resume a refused call, and interactive
sessions remain gated only by `SessionToolPolicy`.

These are recorded here and in `docs/SAFETY_SYSTEM.md` rather than left for a
later reader to discover. A follow-up node should decide whether the registry
should be ancestry-aware (a child session inheriting its parent's ambient
status) or whether unattended-ness belongs on `ToolContext` instead.

## Terminology

The node text and the older doc headings said "tier 1/2/3". The implementation
has exactly two tiers (`ActionTier::AutoAllowed`, `ActionTier::RequiresPermission`),
and `docs/SAFETY_SYSTEM.md:17` already stated there is no third "always denied"
tier. The doc row was renamed to "tier 1/2" to match the code rather than
inventing a third tier to match the node's phrasing.
