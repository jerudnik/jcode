# MCP Bridge — Feasibility Verdict (DAG swarm, Option C preferred)

Date: 2026-07-08. Driven via the Option-A bridge (`jcode debug tool:swarm ...`)
against coordinator session `humpback`, workers `crocodile` (C-native) and `ox`
(B-shim). The native swarm was reachable and functional throughout (proven live:
`list_swarms` returned real fleet state). Findings below are the corroborated
ground truth; the coordinator (this session) finalized the verdict directly after
the workers stalled mid-investigation on the same reload/idle flakiness noted in
FORK_HARDENING_FINDINGS.

## Verdict: OPTION C is FEASIBLE and PREFERRED. Implement C. B not needed.

Every primitive Option C requires already exists in-tree, so C is a pure additive
seam (new file + one clap arm + one dispatch arm), not a new subsystem. Because C
subsumes B (it exposes the whole tool registry, swarm included, to any MCP client),
the B shim is unnecessary once C lands.

## Why C is feasible (all evidence file:line)

1. **MCP protocol types already exist** (client side, reusable for the server side):
   `crates/jcode-base/src/mcp/protocol.rs` defines `InitializeResult`,
   `ServerCapabilities`, `ToolsCapability`, `McpToolDef {name, description,
   inputSchema}` (:115), `ToolsListResult` (:125), `ToolCallParams` (:131),
   `ToolCallResult {content, isError}` (:138). We serialize the same shapes back out.

2. **A stdio JSON-RPC server loop already exists to mirror:** `src/cli/acp.rs`
   runs exactly this pattern — `BufReader` over stdin, `read_line`, dispatch by
   `method`, write framed JSON to stdout, with the standard JSON-RPC error codes
   (:17-22). `mcp-serve` is a second, smaller instance of the same loop.

3. **The tool registry enumerates name + schema generically:**
   `ToolRegistry::definitions()` (`crates/jcode-app-core/src/tool/mod.rs:327`)
   returns `Vec<ToolDefinition {name, description, input_schema}>`
   (`jcode-message-types/src/lib.rs:19`) — a 1:1 map to MCP `McpToolDef`. That is
   `tools/list` for free, swarm included.

4. **Generic execute-by-(name, json) already works over the daemon:** the debug
   socket path `tool:<name> <json>` → `agent.execute_tool(name, input)`
   (`server/debug_command_exec.rs:178-199`) is exactly `tools/call`. The client
   side is `src/cli/debug.rs:53-101` (connect debug socket, send
   `{"type":"debug_command","command":"tool:NAME {json}", "session_id":...}`, read
   `debug_response.output`). `mcp-serve` reuses this verbatim — no new protocol.

5. **Subcommand cost is one enum arm + one dispatch arm:** commands live in
   `src/cli/args.rs` (clap enum; `Serve`/`Acp`/`Connect`/`Doctor` are siblings) and
   dispatch in `src/cli/dispatch.rs` (`Command::Acp => acp::run_acp_command(...)`,
   :115). Add `Command::McpServe { session, socket }` and a matching dispatch arm
   calling a new `src/cli/mcp_serve.rs`.

## Additive-seam / upstreamability assessment

- **Additive:** one new file (`src/cli/mcp_serve.rs`), one clap variant, one
  dispatch arm, one module registration. Zero upstream-line deletions. This is the
  exact "new file + one registration line" the fork model prizes.
- **Upstreamable:** re-publishing a coding agent's tool registry over MCP is a
  generic capability upstream would plausibly want; it touches no fork-specific
  code and reuses upstream's own MCP types and ACP loop.

## Risks / caveats

- **Session binding:** `tools/call` needs a target `session_id` for tools that run
  in a session context (swarm coordinator). `mcp-serve` takes `--session` (or
  auto-creates one via the `create_session` debug command) and threads it into
  every `debug_command`. Mirrors how `debug.rs` already passes `session_id`.
- **Debug socket must be enabled** (`[display] debug_socket = true`) and a daemon
  running — same precondition as `jcode debug`. `mcp-serve` should emit the same
  actionable error `debug.rs:31-40` does.
- **Result mapping:** `debug_response.output` is a string; wrap it as a single MCP
  text `ContentBlock`. `ok=false` → `isError=true`. Trivial.

## Why B is now unnecessary

Option B (external stdio shim shelling out to `jcode debug`) would work and is ~40
lines, but it (a) only exposes swarm, (b) adds an out-of-tree artifact to maintain,
and (c) duplicates the connect/framing logic C gets from reusing `debug.rs`. Since
C is a small additive seam that exposes *all* tools to *any* MCP client (including
this SDK harness via `~/.jcode/mcp.json`), C strictly dominates. B remains the
fallback only if C's in-tree wiring proves harder than measured (it did not).

## Decision: implement C (`jcode mcp-serve`), register it in `~/.jcode/mcp.json`.
