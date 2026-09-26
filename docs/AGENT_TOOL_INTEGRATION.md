# Agent Tool Integration Surfaces

This branch keeps Jcode's agent-tool integration model centered on existing runtime surfaces instead of adding a parallel package manager inside Jcode.

## APM-managed skills

Jcode loads skills from these project-local directories:

1. `.jcode/skills/` for Jcode-native project skills.
2. `.agents/skills/` as the canonical APM/shared projection.
3. `.claude/skills/` only as a compatibility fallback when `.agents/skills/`
   has no loadable skills.

`.apm/skills/` is authoring source. APM projects it into client discovery
directories during install; Jcode does not load the source tree directly or
read two generated projections of the same bundle.

Every declared skill name must be unique across all loaded sources. When two
`SKILL.md` files declare the same name, Jcode removes that name from the active
registry and reports every conflicting path. Discovery order is not an override
mechanism; consolidate or rename the sources.

Global skills load from installed Claude plugins, `~/.jcode/skills/`, and the
shared `~/.agents/skills/` projection. Duplicate declared names across those
sources fail under the same rule.

## APM-managed MCP servers

Jcode loads MCP config from these project-local files, in order, resolved
against the session's working directory (interactive sessions) or the process
working directory after `-C` (headless `jcode run`):

1. `.apm/mcp.json`
2. `.agents/mcp.json`
3. `.mcp.json`
4. `.jcode/mcp.json`
5. `.claude/mcp.json`

Jcode accepts both native `servers` and APM/Claude-style `mcpServers` top-level keys. Only stdio MCP servers are currently supported by Jcode's MCP client. HTTP/SSE declarations are skipped rather than failing the whole config.

APM-generated stdio declarations such as this are supported:

```json
{
  "mcpServers": {
    "plane": {
      "type": "stdio",
      "command": "sh",
      "args": [
        "-lc",
        "exec phase run --app phase --env development --path mcp/ 'PLANE_WORKSPACE_SLUG=4nix uvx plane-mcp-server stdio'"
      ]
    }
  }
}
```

Exact environment placeholders in MCP `env` values are expanded at load time:

```json
{
  "env": {
    "HETZNER_API_TOKEN": "${HETZNER_API_TOKEN}"
  }
}
```

Partial interpolation is intentionally not supported. Use a shell wrapper when a value needs composition.

### MCP request and health deadlines

Each server can set two optional timing fields:

| Field | Meaning | When absent or zero |
| --- | --- | --- |
| `timeout_secs` | Reply budget in seconds for each request, including initialization, tool discovery, and tool calls. | 30 seconds. |
| `health_deadline_ms` | Milliseconds without a reply before Jcode sends a liveness ping. | Positive `JCODE_MCP_HEALTH_DEADLINE_MS` from Jcode's environment, otherwise 15,000 milliseconds. |

For example, a server entry can include `"timeout_secs": 120` and
`"health_deadline_ms": 90000`. Jcode silently clamps the effective health
deadline to the request budget. A smaller `timeout_secs` therefore cannot
disable hung-server detection. These fields do not invalidate cached tool schemas.
A connect triggered by a tool call waits at least 30 seconds, or `timeout_secs`
if that is larger, for the handshake. For a `shared` server the pool's own
config applies; a project-local entry with the same name does not change the
shared connection's timing.

If the server answers the ping, Jcode waits for the rest of the reply budget.
If the ping fails, Jcode declares the server hung and disconnects it. The ping
probe can add up to two seconds to the wait. A delivered call is not retried,
because it may already have produced side effects.

A single-threaded server that cannot answer a ping during a long tool call
will still be declared hung at its health deadline, after the failed probe.
Raising only `timeout_secs` does not help it. Also raise that server's
`health_deadline_ms`, or use `JCODE_MCP_HEALTH_DEADLINE_MS` as the global default,
to cover the expected call duration. The request budget must be at least as
large as the intended health deadline.

## Phase runtime secrets

Jcode does not store Phase credentials. Phase integration should stay at runtime boundaries:

- Wrap MCP stdio commands with `phase run` when the server needs secrets.
- Export `PHASE_HOST=https://phase.4nix.cc` and `PHASE_SERVICE_TOKEN` from the host secret store or a wrapper outside the repository.
- Keep committed MCP files free of raw tokens. Use `${ENV_NAME}` placeholders or `phase run` wrappers.

This matches the 4nix pattern where host-owned secrets bootstrap Phase, and Phase injects per-tool runtime secrets.

## Herdr coordination lane

Jcode auto-reports pane lifecycle to Herdr when Herdr injects:

- `HERDR_ENV=1`
- `HERDR_SOCKET_PATH`
- `HERDR_PANE_ID`

For manual or wrapper-based launches, Jcode also accepts an explicit opt-in:

```sh
export JCODE_HERDR=enabled
export JCODE_HERDR_SOCKET_PATH="$HOME/.config/herdr/herdr.sock"
export JCODE_HERDR_PANE_ID="w1:p1"
```

Optional aliases `JCODE_HERDR_WORKSPACE_ID` and `JCODE_HERDR_TAB_ID` populate the matching Herdr metadata fields.

If Herdr coordination appears inactive, check:

```sh
herdr status server
herdr pane list
printf '%s\n' "$HERDR_ENV" "$HERDR_SOCKET_PATH" "$HERDR_PANE_ID"
```

A Jcode process launched outside a Herdr-managed pane will not report unless the explicit `JCODE_HERDR=enabled` alias set is present.
