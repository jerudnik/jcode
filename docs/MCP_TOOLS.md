# MCP tool exposure

Jcode can advertise MCP schemas eagerly or let the model look them up when
needed. Configure this under `[tools]` in `config.toml`:

```toml
[tools]
mcp_tools = "auto"
mcp_tools_token_threshold = 8000
```

| Setting | Default | Behavior |
| --- | --- | --- |
| `tools.mcp_tools` | `"auto"` | `eager`, `deferred`, or `auto` |
| `tools.mcp_tools_token_threshold` | `8000` | Non-negative estimated-token threshold for Auto |

- **Eager** advertises individual `mcp__{server}__{tool}` schemas and omits the
  fixed pair unless the allow-list explicitly requests either fixed tool.
- **Deferred** advertises `mcp_search` and `mcp_call` instead of individual MCP
  schemas. Search returns each matching tool's composed name, server, raw tool
  name, description, and input schema. Call accepts `server`, `tool`, and an
  arbitrary `arguments` object. Omitted or null arguments become `{}`.
- **Auto** selects deferred exposure when the filtered eager tool definitions
  exceed the threshold. The estimate includes built-in and MCP definitions,
  but excludes the fixed pair. It uses Jcode's approximate serialized-size
  token estimate, not a provider tokenizer. Equality stays eager. A threshold
  of `0` defers every non-empty visible MCP catalog.

A session with no visible MCP catalog omits the fixed pair in every mode.
Allow/deny filtering and transport name limits run before the estimate. If a
transport excludes every proxy name, it omits the fixed pair too. A
session explicitly allowing `mcp_call` can select deferred exposure from its
permitted catalog without listing every proxy in its allow-list. This does
not expose additional eager proxies. To request discovery and dispatch
explicitly, allow both `mcp_search` and `mcp_call`. Explicit fixed-tool entries
remain visible in Eager and below Auto's threshold when a catalog exists.

**Upgrade behavior:** Auto is the default. Existing sessions with large MCP
catalogs can therefore switch from individual schemas to the fixed pair when
they start a new agent, without a config edit. Choose `eager` to retain the
previous prompt surface.

## Overrides and session lifetime

Environment variables override the file:

- `JCODE_MCP_TOOLS=auto|eager|deferred`
- `JCODE_MCP_TOOLS_TOKEN_THRESHOLD=8000`

The corresponding global CLI flags are `--mcp-tools MODE` and
`--mcp-tools-token-threshold TOKENS`. CLI values override environment values.
An invalid environment mode falls back to Auto and an invalid environment
threshold falls back to 8000, with a warning emitted once per invalid value.
Invalid TOML mode values are configuration errors.

The agent captures mode and threshold at construction. A later config edit
does not change that agent's locked tool surface. Auto initially resolves
before the first lock. If new MCP tools arrive after an eager lock, the
existing one-shot late-registration rebuild may re-estimate and switch to
deferred exposure. Further arrivals do not rebuild it. Explicit tool reload
unlocks the surface and resolves it again with the captured settings.

An already deferred lock ignores later per-tool registrations without
consuming the latch or resetting the provider prompt cache. Route changes
that accept the fixed names preserve the lock, while refreshing transport
exclusions for the MCP names considered when it was built. Deferred search and
dispatch also check the current limit for identities that arrive later. Debug tool
introspection applies the same mode and transport filters to the current
catalog rather than the locked snapshot.
Deferred dispatch retains the composed-name limit so changing exposure mode
cannot bypass a transport exclusion enforced on eager and nested calls.
History and `mcp:servers` report registered MCP identities permitted by the
session policy and transport limit, regardless of exposure mode.

## Permissions, discovery, and failures

There is no umbrella `mcp` allow entry and no server-prefix wildcard grant.
The management tool named `mcp` grants no permission to invoke other MCP tools.
An exact MCP allow entry permits the fixed pair to be used, but deferred calls
still check the dispatched identity. Explicit `mcp_call` permission grants
broad dispatch. Exact dispatch denies and `mcp_call` denies always win.
See the [MCP naming policy](architecture/MCP_TOOL_NAMING_POLICY.md#4-permissions-stay-on-exact-identity)
for collision handling, transport exclusions, and worker grants.

Search combines fingerprint-matched cached schemas from enabled configured
servers with live definitions. Live definitions win for the same identity.
A cached result does not promise availability: its first call can fail while
connecting. The existing reconnect, cooldown, and timeout policies are
unchanged, and a failed call does not prevent later use of the session.
Disabled servers are neither listed nor callable through `mcp_call`.

`mcp_call.arguments` deliberately allows additional properties. It is not
eligible for OpenAI strict mode, so provider schema normalization must not
strip the target tool's payload. Mode and threshold do not affect the MCP
schema-cache fingerprint or cache version.
