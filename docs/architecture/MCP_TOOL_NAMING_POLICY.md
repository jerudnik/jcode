# MCP tool naming policy

Status: decided 2026-09-25 from probe evidence (per-transport boundary probes
on Anthropic, the Codex Responses backend, Kimi, Z.AI and xAI; inert
call-and-dispatch checks through `jcode run` with a stub MCP server; evidence
kept in the operator's recovery notes for that date). Supersedes
the 64-character assumption and the hyphen-normalization idea from the
recovered design. Binding decisions from the recovery stay in force: no
umbrella `mcp` permission, worker MCP grants are an explicit coordinator
decision, Auto exposure is re-estimated only at the #206 latch.

## 1. Composition is the identity carrier, not the identity

Jcode advertises MCP tools as `mcp__{server}__{tool}` and dispatches through
the registry map to an `McpTool` that holds `(server, tool)` separately
(`Tool::mcp_identity`). The composed name is only a key. Every decision below
follows from keeping that key a bijection onto registered `(server, tool)`
pairs.

Rules:

- Never alter a valid `server` or `tool` string when composing. No hyphen
  replacement, no case folding, no truncation. Names that use only
  `[A-Za-z0-9_-]` are valid on every transport Jcode has evidence for.
- Never parse a composed name to recover identity where the registry is
  reachable. Use `mcp_identity()`. The `split_once("__")` sites in
  `tool/mod.rs` (server counts), `server/client_state.rs` (MCP server list) and
  `server/debug_command_exec.rs` must move to identity lookups. The
  telemetry classifier in `jcode-usage-types`, which only sees a name string,
  stays a display heuristic and is documented as such.
- Never prefix-match `mcp__{server}__` for membership. The merged
  `unregister_mcp_tools` already does this by identity; the same rule applies to
  reconnect filtering and any future permission or grant logic.

## 2. Ambiguous `__` combinations

Two distinct pairs `(S1, T1)` and `(S2, T2)` collide only when the longer
server name equals the shorter one plus a prefix of `__T`, so at least one
server name contains `__` or ends with `_` (example of the second form:
`a_`/`_c` and `a`/`__c` both give `mcp__a____c`). Tool names alone cannot
cause a collision, so tool names are unrestricted. Registration-time
detection below is the authoritative guard; the config warning is advisory.

Handling, all explicit:

- Config load: a server name containing `__` or ending with `_` is accepted
  but logged as a warning naming the risk. Existing configs are not rejected.
- Registration: `Registry` keeps a persistent `ambiguous_keys` map from
  composed key to the set of distinct `mcp_identity` values that have claimed
  it. On a `mcp__*` registration whose key already holds a different identity,
  or whose key is already in `ambiguous_keys`, the incoming identity is added
  to that set, any live entry under the key is removed, and the registration
  is refused. The key stays refused for every identity in the set until
  identity-based unregister (server disconnect, reload, config removal) drops
  identities from the set; when one identity remains, the set entry is
  cleared and that identity may register on its next connect. A warning is
  logged and the MCP status output lists the conflict with the fix (rename
  one server). Same-identity re-registration (schema refresh after connect)
  is unchanged.
- No alias, encoding or history system. The user resolves collisions by
  renaming a server; nothing is guessed.

Rationale: the inert checks showed the current map insert silently hides one
tool and the winner depends on registration iteration order. Refusing every
claimant, and remembering the refusal until the claimant set shrinks to one,
is the only outcome that is deterministic across connect order, reconnects
and a third claimant, and that cannot dispatch to the wrong server.

## 3. Per-transport name limits

Each provider declares a `ToolNameLimit { max_len, charset }` through
`ProviderCapabilities`. Values come from evidence, not a universal constant:

| transport | max_len | charset | evidence |
|---|---|---|---|
| Anthropic Messages, OAuth | 128 | `[A-Za-z0-9_-]` | 129 rejected, 128 accepted (probe) |
| Anthropic Messages, API key | 128 | `[A-Za-z0-9_-]` | same endpoint and error path as OAuth; not probed on key auth |
| OpenAI Responses via ChatGPT Codex backend | 128 | `[A-Za-z0-9_-]` | 129 rejected, 128 accepted (probe); Codex PR #39594 |
| OpenAI platform key route (`api.openai.com`) | 64 | `[A-Za-z0-9_-]` | documented `^[a-zA-Z0-9_-]{1,64}$`; raise only after a probe on that route |
| OpenAI-compatible profile `kimi` | 128 | `[A-Za-z0-9_-]` | 129 rejected, 128 accepted (probe) |
| OpenAI-compatible profiles `zai`, `grok-direct` | 128 | `[A-Za-z0-9_-]` | accepted every sampled name including invalid ones, so no rejection evidence exists; 128 chosen to match validating peers, not because a higher limit was shown |
| OpenAI-compatible, unspecified profile | 64 | `[A-Za-z0-9_-]` | inherits the documented OpenAI limit; profiles override with evidence |
| Bedrock Converse | 64 | `[a-zA-Z0-9_-]+` | documented; not configured, not probed |
| Gemini API | 64 | letters, digits, `_`, `.`, `:`, `-` | documented; Gemini CLI truncates at 63; not configured, not probed |
| Copilot, Cursor, Antigravity, OpenRouter, ACP, Claude CLI, others | 64 | `[A-Za-z0-9_-]` | no evidence; conservative default until probed |

Enforcement happens in Jcode, before the request, because two configured
routes (Z.AI, xAI) accepted every sampled invalid name and Anthropic rejects
the entire request when one name is over. A tool whose composed name breaks
the active transport's limit is:

- kept in the registry (it stays callable on transports that allow it, and
  identity-based unregister still finds it),
- omitted from the tool definitions sent to that provider,
- refused at execution for that session while the exclusion is in force: the
  agent records the excluded names when it locks its tool snapshot and
  `Agent::execute_tool` rejects them, so a replayed, hand-built or
  `batch`-nested call cannot reach the server through a name the provider
  never saw,
- logged once per session as `MCP_NAME_UNSUPPORTED` with server, tool,
  length, transport and limit,
- shown in MCP status as "not advertised on <transport>: name too long".

No encoded fallback name is generated. Today the longest configured composed
name is 60 characters, so this exclusion affects nothing live; it exists so a
future server cannot take a session down.

## 4. Permissions stay on exact identity

Every gate in `Registry::execute` (session tool policy allow and disable sets,
ambient action tier, swarm assignment grant) and every `--tools` allow-list
keys on the registered composed name. Section 2 makes that name a bijection
onto `(server, tool)`, so a name-keyed permission is an exact-identity
permission. Consequences:

- No umbrella `mcp` permission, and no `mcp__{server}__*` wildcard grants.
- A permission never survives a collision: the ambiguous key is not
  registered, so it cannot be granted.
- Per-transport exclusion (section 3) does not change permissions. An
  excluded tool is refused by the agent's exclusion gate on that transport,
  not newly permitted or denied.
- Worker MCP grants remain an explicit coordinator decision per tool identity.
- Prompt caching: exclusion changes the advertised tool array only when an
  excluded tool exists. No configured server has one today (longest composed
  name is 60), so cache keys for current users are unchanged.

## 5. Where this lives in code

- `jcode_provider_core::ToolNameLimit` and `ProviderCapabilities::tool_name_limit`;
  per-runtime values in the Anthropic, OpenAI and OpenAI-compatible runtimes.
- `Registry::register` collision refusal and `Registry::mcp_ambiguous_keys`
  in `crates/jcode-app-core/src/tool/mod.rs`; `unregister_mcp_tools` releases
  claims by identity.
- `Agent::apply_tool_name_limit` and the exclusion check in
  `validate_tool_allowed` (`crates/jcode-app-core/src/agent/turn_execution.rs`).
- `McpConfig::warn_ambiguous_server_names` in `crates/jcode-base/src/mcp/protocol.rs`.
- The `mcp` tool's `list` action reports refused ambiguous names.

## 6. Out of scope

- Whether a model reliably emits a 128-character name in a call is a model
  quality question, not a naming rule.
- Non-ASCII server or tool names: bytes and characters diverge; the limits
  above are in characters and were only probed in ASCII. Treat a non-ASCII
  name as outside the charset and therefore excluded per section 3.
- Headless `jcode run` now resolves project-local MCP config against the
  process working directory (`register_run_command_mcp_tools`), matching
  interactive sessions.
