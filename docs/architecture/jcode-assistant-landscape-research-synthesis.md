# Jcode Assistant Landscape Research Synthesis

Date: 2026-06-23

Seven focused research agents surveyed the current landscape for the remaining assistant architecture gaps: TUI UX, config, security/privacy, evidence schema, routing/placement, validation/evals, and implementation backlog split. This synthesis keeps the architecture grounded in Jcode-owned modules and the current crate layout.

## Executive synthesis

The clear opportunity is not to copy another agent stack. It is to make Jcode the harness that shows, records, and validates what other tools hide:

1. **Decision visibility**: users should see why a provider, model, tool, context source, memory, or node was used.
2. **Evidence as substrate**: session/event logs should power search, replay, evals, memory provenance, self-improvement, and future homelab distribution.
3. **Policy before power**: routing, context capture, memory writes, computer use, and telemetry need explicit safety and privacy rules before breadth.
4. **Local-first by default**: external tools, gateways, observability stacks, and eval frameworks are references or optional exports, not first-cut runtime dependencies.
5. **Background work as dashboard**: long-running work needs a first-class status view, not scrollback archaeology.
6. **Config should explain effective behavior**: layered config is normal, but users need to inspect the resolved route/tool/context/privacy policy.

## 1. Interaction UX

### Landscape patterns

- Claude Code, Codex, Continue, OpenCode, and Cursor-like tools are converging on multi-surface agent cores: terminal, IDE, web/server, headless, and background sessions.
- Background work is becoming dashboard-like: Claude Code agent view, Continue `/jobs`, OpenCode attach/session/server flows.
- Session history is valuable enough that third-party viewers exist for Claude Code, Codex, OpenCode, and Grok history, showing chat history, diffs, tokens, costs, and resume links.
- Routing and delegation are usually configurable, but rarely shown as a clear decision trace.

### Pain points

- Users cannot tell why an agent selected a model, tool, subagent, memory, or context source.
- Scrollback is a poor interface for background work, evidence, and session replay.
- Tool traces, costs, diffs, and decisions are often fragmented across logs, chat transcript, and external dashboards.

### Clear wins for Jcode

- Add a **routing/evidence side panel** showing the route decision, fallback path, selected provider/model, enabled tools, injected context, memory reads/writes, and validation status.
- Treat background tasks, swarm workers, scheduled tasks, and future homelab workers as rows in a **work dashboard** with status, node, task class, owner session, and required user action.
- Make session search and resume evidence-native: show not just matching transcript lines, but matching tool/evidence events.

### Implementation implication

- Use `jcode-protocol::ServerEvent` for bounded routing/evidence summaries.
- Render in `jcode-tui` without lower-layer TUI dependencies.
- Source the data from the Session Evidence Spine, not ad hoc UI state.

## 2. Config model

### Landscape patterns

- Layered config is table stakes: global, project, local/private, CLI/env override, and sometimes managed/admin scopes.
- Continue-style capability config is clearer than provider-only config: roles such as chat, edit, embed, rerank, summarize, plus capabilities like tool use and image input.
- LiteLLM-style aliases, fallback chains, budgets, and routing rules are useful patterns, but Jcode should not import a gateway as its first routing substrate.
- MCP/tool config is converging around named servers with transport, env, auth, scopes, enable flags, and trust boundaries.

### Pain points

- Config sprawl across settings files, MCP files, rules, memories, env vars, and plugins.
- Users struggle to see the effective config and precedence.
- Provider, route, deployment, model, and capability are often encoded in a single string.
- Context sources are mixed with tools, causing prompt bloat and unexpected data flow.

### Clear wins for Jcode

- Introduce a versioned assistant capability config that groups:
  - routing policies,
  - provider/model capabilities,
  - tool/MCP scopes,
  - context providers,
  - memory write/review rules,
  - privacy/telemetry policy,
  - local and future homelab workers.
- Provide an effective-config view in the TUI and a tool/debug command.
- Keep simple local defaults ergonomic, with advanced routing only when configured.

### Implementation implication

- Extend `jcode-config-types` only after the first app-core-only routing MVP proves the shape.
- Route through existing `jcode-provider-core::RouteSelection` and `Provider::set_route_selection`.
- Avoid using `jcode-gateway-types` for this, since it currently models paired-device gateway concepts.

## 3. Security and privacy policy

### Landscape patterns

- Claude Code, Codex, and OpenInterpreter emphasize action classification: read-only by default, explicit approval for writes/destructive commands/network/system actions, and opt-in unattended modes.
- Screenpipe’s strongest pattern is local-first capture with explicit consent for third-party integrations.
- MCP security guidance treats servers as delegated principals with scopes, tokens, prompt surfaces, and exfiltration risks.
- OpenTelemetry guidance emphasizes avoiding sensitive telemetry by design, then redacting before export.

### Pain points

- Users cannot easily answer: what left my machine, what was stored, and what was injected into the model?
- MCP tools can quietly expand trust and exfiltration surface.
- Memory writes can fossilize bad or sensitive information.
- Raw context capture can become surveillance without budgets, redaction, and inspect-before-inject.

### Clear wins for Jcode

- Add a central **Policy Engine** before expanding power:
  - classify tool actions by impact,
  - classify context/memory/log data by sensitivity,
  - require review for risky memory writes and raw context injection,
  - keep telemetry/export opt-in and redacted,
  - expose policy decisions as evidence events.
- Make “what leaves the machine” inspectable in TUI.

### Implementation implication

- Reuse existing seams: `Registry::execute`, pre-tool hooks, `ToolContext`, memory tool, telemetry content-sharing controls, and existing secret redaction helpers.
- Emit policy decisions into evidence logs before adding broad context capture or remote workers.

## 4. Evidence/session schema

### Landscape patterns

- OTel GenAI, OpenInference, Phoenix, LangSmith, and Langfuse show the value of spans/traces with provider/model/tool/retrieval/eval metadata.
- Local tools like Entire/Lore/Jolli-like systems show interest in commit-pinned session capture and searchable local stores.
- Products often separate operational traces, session transcripts, and replay artifacts. Users then have to stitch them together manually.

### Pain points

- Session logs are not durable contracts.
- Tool inputs/outputs can be too large or too sensitive for raw logs.
- Search over transcripts misses tool/evidence events.
- Replay and evals become flaky without event schema stability.

### Clear wins for Jcode

- Make `SessionLogEvent` a stable local-first event envelope with:
  - schema version,
  - monotonic sequence,
  - session/parent/child ids,
  - node/runtime identity,
  - git state,
  - provider/model/route,
  - tool call ids,
  - hashes/byte counts instead of large raw payloads,
  - correlation ids,
  - validation/task outcome records.
- Use JSONL first, with optional derived SQLite/FTS cache later.
- Search evidence alongside transcript messages.

### Implementation implication

- Types in `jcode-session-types`.
- Writer/reader in `jcode-base/src/session/evidence.rs` using `jcode-storage::append_json_line_fast`.
- Index/search in `jcode-app-core/src/tool/session_search.rs`.
- Instrument `Agent` turns, provider route changes, and `Registry::execute`.

## 5. Routing and homelab placement

### Landscape patterns

- Capability-first scheduling beats gateway-first routing: Ray, Nomad, Temporal, NATS queue groups, Dask, and Nix remote builds all separate capability, health, cost, latency, and placement.
- Common shape: filter by hard requirements, exclude unhealthy targets, score by latency/load/locality/privacy/cost, then apply fallback.
- Workload classes matter: interactive turns, tool calls, background tasks, swarm agents, builds/tests, summarization, evals, and indexing need different policies.
- Sticky placement matters for stateful sessions and actors.
- No-responder behavior avoids hanging when a remote worker is offline.

### Pain points

- Gateways centralize traffic but can add latency, coupling, and operational overhead.
- Remote workers can silently become unavailable.
- Batch tasks and interactive turns need different placement, but users rarely get visibility into that decision.

### Clear wins for Jcode

- Build the **Interaction Routing Engine** locally beside the TUI/interface.
- Route by task class and capability, not by “everything through gateway”.
- Keep direct native provider/tool paths as the default.
- Add future homelab workers through explicit worker protocols and queue-like semantics after local routing/evidence are stable.

### Implementation implication

- Start in `jcode-app-core/src/routing.rs` with app-core-only types.
- Use existing `RouteSelection`, background tasks, swarm members, and tool registry.
- Add node inventory later with local node plus static config entries first.

## 6. Validation/evals/self-improvement

### Landscape patterns

- Promptfoo excels at local YAML suites, deterministic assertions, model-graded rubrics, latency/cost checks, red-team plugins, and CI.
- Inspect AI, DeepEval, LangSmith, Langfuse, Phoenix, OpenAI Evals, and SWE-bench all show variations of datasets, agents/solvers, scorers, traces, and experiments.
- The best systems connect live traces to offline regression datasets.

### Pain points

- Final-answer quality is evaluated more often than tool correctness.
- Agent evals are expensive and flaky without deterministic replay and tool stubbing.
- Real traces are useful but privacy-sensitive.
- LLM-as-judge is useful for triage, not sufficient for hard CI gates.

### Clear wins for Jcode

- Build Jcode-native eval records over the Session Evidence Spine.
- Add replay-backed regression tests for transcript shape, route/tool sequence, policy decisions, selfdev outcomes, and task outcome status.
- Use Promptfoo as an optional export/import runner later, not the canonical data model.
- Mine failed sessions into candidate eval cases with human approval before promotion.

### Implementation implication

- Types can start local to app-core/replay and move to a `jcode-eval-types` crate only when shared with protocol/TUI.
- Integrate with existing `replay`, `session_search`, `selfdev`, background tasks, and evidence logs.

## 7. Backlog split

### Landscape-informed implementation order

1. Planning tracker and small deterministic fixtures.
2. Evidence event types and JSON schema.
3. Evidence JSONL writer/reader.
4. Turn/provider/tool instrumentation.
5. Evidence-aware `session_search`.
6. Interaction router MVP for provider/model route choices.
7. Routing TUI summary and effective-config view.
8. Context provider MVP with budgets/redaction and session/git/task context only.
9. Memory provenance and review state.
10. Computer-use trace refactor behind current tool schema.
11. Native replay/eval runner and tool correctness suite.
12. Optional observability export.
13. Homelab worker protocol and placement after local contracts stabilize.

### Cross-cutting acceptance criteria

Every new module slice should specify:

- owning crate and file seam,
- shared type location,
- persistence location,
- protocol/TUI surface if any,
- evidence emitted,
- privacy/policy behavior,
- tests that run without homelab infrastructure.

## Recommended next move

Start with **Evidence event types and writer**. It unlocks all later work and is the narrowest durable contract:

- no UI redesign required,
- no remote workers required,
- no gateway dependency,
- validates quickly with serde and append/read tests,
- immediately improves search/replay/eval/memory/routing design.

Create Pad implementation tickets for Phase 1 evidence work, then move to the routing MVP only after route decisions can be recorded as evidence.

## Primary references surfaced by the research agents

- Claude Code overview, security, memory, settings, agent view: https://docs.anthropic.com/en/docs/claude-code/overview
- OpenAI Codex CLI, approvals/security, sandboxing, config: https://developers.openai.com/codex/cli
- Continue CLI/config/reference: https://docs.continue.dev/cli/tui-mode
- OpenCode docs/config/CLI: https://opencode.ai/docs
- MCP security best practices: https://modelcontextprotocol.io/specification/2025-06-18/basic/security_best_practices
- OWASP AI Agent Security Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/AI_Agent_Security_Cheat_Sheet.html
- OpenTelemetry sensitive data guidance: https://opentelemetry.io/docs/security/handling-sensitive-data/
- LiteLLM routing and reliability patterns: https://docs.litellm.ai/docs/routing
- Ray scheduling: https://docs.ray.io/en/latest/ray-core/scheduling/index.html
- Nomad scheduling: https://developer.hashicorp.com/nomad/docs/concepts/scheduling/schedulers
- Temporal task queues: https://docs.temporal.io/task-queue
- NATS queue groups: https://docs.nats.io/nats-concepts/core-nats/queue
- Nix distributed builds: https://nix.dev/manual/nix/2.18/advanced-topics/distributed-builds
- Promptfoo docs: https://www.promptfoo.dev/docs/intro/
- Inspect AI docs: https://inspect.aisi.org.uk/
- DeepEval docs: https://deepeval.com/docs/getting-started
- Langfuse eval docs: https://langfuse.com/docs/evaluation/overview
- Phoenix eval docs: https://arize.com/docs/phoenix/evaluation
- OpenAI Evals: https://github.com/openai/evals
- SWE-bench: https://www.swebench.com/
