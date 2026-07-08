# MCP Server Registration Guardrails

See also:

- [`MCP_SERVE_FORKBOMB_INCIDENT.md`](./MCP_SERVE_FORKBOMB_INCIDENT.md)
- [`../SERVER_LIFECYCLE_INVARIANTS.md`](../SERVER_LIFECYCLE_INVARIANTS.md)
- [`../fork/SYNC_MODEL.md`](../fork/SYNC_MODEL.md)

This is the safety contract for registering MCP servers in `~/.jcode/mcp.json`
and project-local variants.

## The self-reference rule

The daemon MUST NOT load an MCP server entry that is a `jcode mcp-serve` shim.
That shim re-enters the daemon's own tool registry over MCP and can recursively
create sessions and child processes until the daemon becomes a fork bomb. See
[`MCP_SERVE_FORKBOMB_INCIDENT.md`](./MCP_SERVE_FORKBOMB_INCIDENT.md) for the
full chain.

The rule is enforced by:

- `McpConfig::drop_self_referential_servers`
- `McpServerConfig::is_jcode_mcp_serve_shim`
- `crates/jcode-base/src/mcp/protocol.rs`

Those functions drop self-referential entries during config load and log a
warning. The guard applies to daemon config loading only.

**Detection scope.** The guard matches the *direct* form — a jcode-ish command
whose first argument is the `mcp-serve` subcommand — which is the realistic
accidental / LLM-generated case (e.g. a dogfooding entry left pointing at the
selfdev binary). It intentionally does not try to defeat deliberate obfuscation
such as a shell wrapper (`{"command":"sh","args":["-c","jcode mcp-serve"]}`).
The guard is not a security boundary; it is a footgun-removal. The real backstop
against *any* runaway spawn — obfuscated self-reference included — is the cap
layer below (owned-MCP children, sessions, testers), which bounds the blast
radius even when the self-reference guard is bypassed.

External MCP clients are the intended users of `jcode mcp-serve`. Editors, SDK
sessions, and other external clients may launch `jcode mcp-serve`; they do not
go through the daemon's MCP config load path and are unaffected by this guard.

## Safe example vs the footgun

This is safe daemon MCP configuration because the command is an external MCP
server, not a `jcode mcp-serve` shim:

```json
{
  "servers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
      "shared": true
    }
  }
}
```

This is also safe when placed in an external MCP client's config, because the
external client is the process launching the shim:

```json
{
  "servers": {
    "jcode": {
      "command": "jcode",
      "args": ["mcp-serve"]
    }
  }
}
```

This is unsafe inside `~/.jcode/mcp.json` or a project-local daemon MCP config:

```json
{
  "servers": {
    "jcode": {
      "command": ".../jcode",
      "args": ["mcp-serve"],
      "shared": false
    }
  }
}
```

The daemon reads this entry while registering MCP tools for a session. Because
the command is `jcode` and the first non-flag argument is `mcp-serve`, the entry
would make the daemon spawn its own MCP shim. The shim's `tools/list` path
ensures a daemon session exists, which registers MCP tools again and reloads the
same entry.

`shared:false` makes the failure worse by putting the server on the per-session
owned path. Each recursive session gets another child process instead of sharing
one pooled process.

## The caps that backstop it

The self-reference rule is the primary guard. These caps are independent
backstops for runaway process or session growth:

| Cap | Scope | File |
|---|---|---|
| `MAX_OWNED_MCP_CHILDREN = 64` | Owned, non-shared MCP children process-wide | `crates/jcode-base/src/mcp/client.rs` |
| `MAX_TESTERS = 8` | Live tester daemons spawned by one daemon | `crates/jcode-app-core/src/server/debug_testers.rs` |
| `MAX_TESTER_DEPTH = 1` / `JCODE_TESTER_DEPTH` | Tester daemons must not spawn further testers | `crates/jcode-app-core/src/server/debug_testers.rs` |
| `MAX_TOTAL_SESSIONS = 1500` | Live sessions in one daemon | `crates/jcode-app-core/src/server/headless.rs` |

The lifecycle side of this contract is in
[`../SERVER_LIFECYCLE_INVARIANTS.md`](../SERVER_LIFECYCLE_INVARIANTS.md).

## `shared` semantics

`shared:true` is the default. Shared servers are pooled and deduped, so one
server process is reused per configured name.

`shared:false` means a per-session owned process. Use it only for state-carrying
servers that must not be shared across sessions.

`shared:false` on a high-fanout server multiplies processes by session count. It
is appropriate for isolated state, not for common stateless tools or bridge
processes.
