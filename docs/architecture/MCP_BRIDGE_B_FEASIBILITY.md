# Option B feasibility: standalone MCP shim for native swarm

## Verdict

**Feasible as a fallback, low divergence, moderate polish cost.** A small stdio MCP server can expose a narrow `swarm_*` MCP surface and forward each call to an existing live jcode daemon session via `jcode debug --quiet -S <session_id> 'tool:swarm {json}'`. The primitive is already implemented, live-tested, and returns structured JSON. The shim does not need to link against jcode internals, so it can live outside the tree or as one small helper plus a `~/.jcode/mcp.json` entry.

This is best treated as Option B fallback, not the ideal native integration: process-per-call shelling is simple and robust but adds CLI latency, quoting/error-handling friction, and requires a coordinator session to exist or be created.

## Evidence

### 1. Forwarding primitive and exact contract

The debug command executor has an explicit `tool:` path:

- `crates/jcode-app-core/src/server/debug_command_exec.rs:178-190` parses commands beginning with `tool:`, splits the tool name from the trailing JSON, and parses the JSON input with `serde_json::from_str`.
- `crates/jcode-app-core/src/server/debug_command_exec.rs:191-198` calls `agent.execute_tool(name, input).await?` and returns pretty JSON with exactly `output`, `title`, and `metadata` fields.

Relevant source:

```text
178 if trimmed.starts_with("tool:") {
183     let mut parts = raw.splitn(2, |c: char| c.is_whitespace());
184     let name = parts.next().unwrap_or("").trim();
185     let input_raw = parts.next().unwrap_or("").trim();
189     serde_json::from_str::<serde_json::Value>(input_raw)?
192     let output = agent.execute_tool(name, input).await?;
193     let payload = serde_json::json!({
194         "output": output.output,
195         "title": output.title,
196         "metadata": output.metadata,
197     });
198     return Ok(serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string()));
```

Live probe:

```sh
$ jcode debug --quiet create_session
{"friendly_name":"sheep","is_canary":false,"session_id":"session_sheep_1783526223097_95c3414bd067289e","swarm_id":null,"working_dir":null}

$ jcode debug --quiet -S session_sheep_1783526223097_95c3414bd067289e 'tool:swarm {"action":"list_swarms"}'
{
  "metadata": null,
  "output": "Live swarms:\n\n- `/Users/jrudnik`: 2 member(s), coordinator `giraffe` (ready)\n  ...\n- `/Users/jrudnik/labs/jcode/.git`: 12 member(s), coordinator `humpback` (ready)\n  ...\n",
  "title": null
}
```

This confirms the exact user-provided contract: `jcode debug --quiet -S <session_id> 'tool:swarm {"action":"list_swarms"}'` returns JSON `{output,title,metadata}`.

### 2. `~/.jcode/mcp.json` format and MCP tool discovery

MCP server config shape is defined in `crates/jcode-base/src/mcp/protocol.rs:169-200`:

- `command: String` for stdio servers.
- `args: Vec<String>`.
- `env: HashMap<String, String>`.
- `shared: bool`, default true, with comments explaining stateless servers should be shared and stateful servers should not.
- optional `type`/`transport`, `url`, `enabled`, and `disabled` compatibility fields.

`McpConfig` accepts both jcode's historical `servers` key and Claude Code's `mcpServers` alias at `crates/jcode-base/src/mcp/protocol.rs:233-239`.

The management tool prints the canonical example at `crates/jcode-app-core/src/tool/mcp.rs:377-382`:

```json
{
  "servers": {
    "server-name": {
      "command": "/path/to/server",
      "args": [],
      "env": {},
      "shared": true
    }
  }
}
```

Load behavior:

- Global `~/.jcode/mcp.json` is loaded at `crates/jcode-base/src/mcp/protocol.rs:474-480`.
- Project-local `.jcode/mcp.json`, `.mcp.json`, and `.claude/mcp.json` are loaded and override earlier same-named servers at `crates/jcode-base/src/mcp/protocol.rs:433-447`.
- Generated `.apm/mcp.json` and `.agents/mcp.json` are also loaded before local override paths at `crates/jcode-base/src/mcp/protocol.rs:502-517`.
- Non-stdio servers are skipped today at `crates/jcode-base/src/mcp/protocol.rs:519-532`.

Discovery/call schema:

- `crates/jcode-base/src/mcp/client.rs:241-250` initializes a stdio MCP server and immediately calls `refresh_tools()`.
- `crates/jcode-base/src/mcp/client.rs:101-110` sends JSON-RPC `tools/list` and deserializes a `ToolsListResult`.
- `crates/jcode-base/src/mcp/protocol.rs:113-126` defines tool definitions as `{ name, description?, inputSchema }`.
- `crates/jcode-base/src/mcp/client.rs:58-76` calls a tool using JSON-RPC `tools/call` with `{ name, arguments }`.
- `crates/jcode-base/src/mcp/tool.rs:98-107` registers discovered tools as `mcp__<server>__<tool>`, while `crates/jcode-base/src/mcp/tool.rs:45-47` uses the server-supplied `inputSchema` as the jcode tool parameter schema.

### 3. Minimal shim surface

Expose a small set of first-class MCP tools rather than the entire native `swarm` action enum. The goal is enough for external MCP clients to coordinate real work without having to understand every internal action.

Recommended minimal tools:

1. `swarm_list_swarms`
   - For fleet discovery and smoke testing.
   - Forwards `{"action":"list_swarms"}`.
2. `swarm_status`
   - Inspect current coordinator or target session.
   - Forwards `{"action":"status", "target_session"?: ...}`.
3. `swarm_spawn`
   - Create agents.
   - Arguments: `prompt` or `initial_message`, `label`, `subagent_type`, `model`, `effort`, `spawn_mode`, `working_dir`.
   - Forwards `{"action":"spawn", ...}`.
4. `swarm_assign_task`
   - Assign a task to an existing or spawned worker.
   - Arguments: `task_id`, `prompt`/`message` or task payload fields used by native `assign_task`, `target_session`, `spawn_if_needed`, `prefer_spawn`.
   - Forwards `{"action":"assign_task", ...}`.
5. `swarm_run_plan`
   - Run a task graph with concurrency.
   - Arguments: `concurrency_limit`, `background`, `notify`, `wake`, `retain_agents`.
   - Forwards `{"action":"run_plan", ...}`.
6. `swarm_report`
   - Let an external client report completion into the swarm control plane.
   - Arguments: `status`, `message`, `validation`, `follow_up`, `tldr`.
   - Forwards `{"action":"report", ...}`.

Optional but useful next tools:

- `swarm_task_graph`, `swarm_complete_node`, and `swarm_plan_status` if external clients need DAG-level structured handoffs.
- `swarm_list_models` for model routing.
- `swarm_dm` only if external clients need point-to-point chat. Keep it out of the minimal surface to avoid encouraging chatty orchestration.

Avoid one generic `swarm_call` as the only public tool. It is easy to implement but weak for external clients because MCP discovery would advertise no precise schemas. A hidden or advanced `swarm_call` escape hatch is acceptable.

### 4. Session handling

The shim needs a coordinator `session_id` for `-S`.

Create/get evidence:

- `crates/jcode-app-core/src/server/debug_session_admin.rs:15-49` parses `create_session`, `create_session:<path>`, and `create_session:selfdev:<path>`.
- `crates/jcode-app-core/src/server/debug_session_admin.rs:70-105` creates a headless session via `create_headless_session(...)` and returns the created JSON.
- Debug help lists `sessions`, `create_session`, `create_session:<path>`, and `create_session:selfdev:<path>` at `crates/jcode-app-core/src/server/debug_command_exec.rs:503-506`.
- A live `jcode debug --quiet create_session` returned `{ friendly_name, is_canary, session_id, swarm_id, working_dir }`.

Workable session strategies:

1. **Environment-configured session, simplest**
   - `JCODE_SWARM_SESSION_ID=session_...` in `~/.jcode/mcp.json` `env`.
   - Every tool call shells to `jcode debug --quiet -S $JCODE_SWARM_SESSION_ID ...`.
   - Lowest code, but brittle if the session is destroyed.

2. **Lazy create and cache, recommended fallback default**
   - Shim starts with optional `JCODE_SWARM_SESSION_ID`.
   - If missing, run `jcode debug --quiet create_session:<working_dir>` or `create_session`.
   - Cache the returned `session_id` in process memory and optionally in `~/.jcode/swarm-mcp-session.json`.
   - If forwarding returns unknown session, create a fresh session and retry once.

3. **Per-call targeting**
   - Add optional `session_id` to every MCP tool schema.
   - If present, use it for `-S`; otherwise use the cached coordinator session.
   - This is workable and important for advanced clients, but should be optional to keep the common path simple.

Working directory:

- If the MCP config is global, use an env var such as `JCODE_SWARM_WORKING_DIR` or a tool argument on `swarm_spawn`/`swarm_run_plan`.
- `create_session:<path>` is the clean way to bind the coordinator to a project.

### 5. Implementation sketch

Recommended language: **Python 3, bare JSON-RPC over stdio**.

Reason: the shim only needs `initialize`, `tools/list`, and `tools/call`. jcode's MCP client ignores notifications and reads newline-delimited JSON responses, so a minimal line-oriented server is enough. A Python implementation can be dependency-free and easy to install.

Estimated size:

- 70-120 LOC for a robust dependency-free Python shim with:
  - JSON-RPC loop over stdin/stdout.
  - fixed tool definitions.
  - `subprocess.run([...], capture_output=True, text=True, timeout=...)`.
  - session lazy-create and retry-on-unknown-session.
  - result wrapping as MCP `content: [{"type":"text","text": ...}]`.
- 40-80 LOC only if omitting lazy create, retry, and detailed schemas.
- Rust would be ~150-250 LOC plus a crate target and build/install friction unless added inside the workspace.

Example `~/.jcode/mcp.json` entry:

```json
{
  "servers": {
    "jcode-swarm": {
      "command": "python3",
      "args": ["/Users/jrudnik/.jcode/bin/jcode_swarm_mcp.py"],
      "env": {
        "JCODE_BIN": "/Users/jrudnik/.local/bin/jcode",
        "JCODE_SWARM_WORKING_DIR": "/Users/jrudnik/labs/jcode"
      },
      "shared": true
    }
  }
}
```

A static tool definition example for `tools/list`:

```json
{
  "name": "swarm_list_swarms",
  "description": "List live jcode swarms via the native swarm tool.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": { "type": "string", "description": "Optional jcode session id to target." }
    }
  }
}
```

For `tools/call`, map MCP names to native payloads:

```python
NATIVE = {
  "swarm_list_swarms": lambda a: {"action": "list_swarms"},
  "swarm_status": lambda a: {"action": "status", **drop_session(a)},
  "swarm_spawn": lambda a: {"action": "spawn", **drop_session(a)},
  "swarm_assign_task": lambda a: {"action": "assign_task", **drop_session(a)},
  "swarm_run_plan": lambda a: {"action": "run_plan", **drop_session(a)},
  "swarm_report": lambda a: {"action": "report", **drop_session(a)},
}
```

Then run:

```python
subprocess.run([
  JCODE_BIN, "debug", "--quiet", "-S", session_id,
  "tool:swarm " + json.dumps(native_payload, separators=(",", ":")),
], ...)
```

Using an argv list avoids shell quoting issues. The `tool:swarm <json>` string is one argv element.

## Pros and cons vs Option C

Assuming Option C is a native/in-tree bridge that exposes the swarm tool through jcode's own MCP/server machinery rather than a standalone shelling shim:

Pros of Option B:

- **Zero fork divergence:** can live entirely outside the tree and be registered through existing `~/.jcode/mcp.json`.
- **Fast to ship:** no Rust workspace changes, no daemon protocol stabilization required.
- **Uses proven contract:** the debug `tool:` forwarding path already executes any registered tool and returns `{output,title,metadata}`.
- **Compatible with existing MCP client discovery:** just implement `initialize`, `tools/list`, and `tools/call`.
- **Easy rollback:** remove one MCP config entry.

Cons of Option B:

- **Shell-out overhead:** every tool call launches `jcode debug`, which is fine for coordination but not ideal for high-frequency operations.
- **Session lifecycle is externalized:** the shim must discover, cache, create, or be configured with a coordinator `session_id`.
- **Error contract is less native:** CLI stderr/exit codes must be translated into MCP `isError` content.
- **Weaker type fidelity:** unless the shim copies schemas from native `swarm`, it must maintain a small parallel schema surface.
- **Debug API coupling:** relies on `jcode debug` command semantics rather than a public stable MCP bridge API.

Option C likely wins for a polished product surface: no subprocess per call, native session selection, direct access to tool schemas, direct structured result handling, and fewer moving parts. Option B wins as the fallback because it is simple, deployable today, and does not require changing jcode internals.

## Maintenance cost

Low if intentionally narrow:

- Keep six stable external tools and a generic internal mapping to native actions.
- Avoid mirroring the full 40-action native enum.
- Treat `swarm_call` as an optional escape hatch for power users, not the primary interface.
- Pin behavior to the debug `tool:` contract and add one smoke test:
  `swarm_list_swarms` returns text containing `Live swarms` or a valid empty/error response.

Expected maintenance burden: small updates when native swarm action argument names change for the six exposed actions. Since forwarding uses native `swarm` unchanged, there is no forked coordination logic.

## Open questions

- Should the shim persist its auto-created coordinator session id on disk, or always create on MCP server start? Persisting avoids orphan sessions; recreating is simpler.
- Should external MCP clients be allowed to pass arbitrary `session_id`, or should this be gated behind an env var for safety?
- If Option C lands soon, Option B should probably remain an example/compatibility shim rather than a supported primary path.
