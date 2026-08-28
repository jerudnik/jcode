---
title: "Swarm spawn silently lands on an unrequested model, and every surface reports a different identity"
status: open
priority: high
owner: unassigned
opened: 2026-08-27
related:
  - docs/issues/swarm-model-policy-enforcement.md
  - docs/architecture/provider-confusion.md
---

# Swarm spawn silently lands on an unrequested model, and every surface reports a different identity

Observed 2026-08-27 in the session titled `agent-config-audit` (coordinator
`session_peacock_1787861778353_a0d0e76b48e5f975`, `gpt-5.6-sol` via
`openai-oauth`, working dir: the operator's home directory). A spawned swarm
worker ended up making real provider requests as `anthropic/claude-sonnet-4`, a model the
coordinator never asked for and one far below the routing policy floor. Four
different surfaces reported four different identities for the same worker.

Screenshot evidence: operator screenshot of the coordinator TUI, 2026-08-27
16:22 local (worker chip reads `claude-sonnet-4 · Kimi Code`).

## Timeline (from the coordinator journal)

All timestamps 2026-08-27 UTC.

1. `20:19:45` — coordinator runs `swarm list_models`. Output confirms no
   `agents.swarm_model` pin and lists available routes.
2. `20:20:06` — spawn with `model: "glm-5.2"` fails: *"could not be resolved
   to a provider or route"*.
3. `20:20:20` — spawn with `model: "deepseek-v4-pro"` fails the same way.
   Both names are explicitly recommended in `.jcode/swarm-prompt.md` for
   load-spreading, so the routing prompt steered the coordinator into two
   dead ends before it started guessing.
4. `20:20:30` — spawn with `model: "bridge/gemini-3-flash-agent"`
   **succeeds**, creating `session_shark_1787862031120_2330b8c39a007acb`.
   The tool reports: `Resolved identity: model=bridge/gemini-3-flash-agent
   provider_key=openrouter route=none`.

## The four conflicting identities

For the single worker session `session_shark_1787862031120_2330b8c39a007acb`:

| Surface | Provider | Model |
| --- | --- | --- |
| Spawn tool result | `openrouter` (`route=none`) | `bridge/gemini-3-flash-agent` |
| Persisted session meta (`.json` and every journal entry) | `openrouter` | `k3` |
| Evidence log `provider_request` events (actual API traffic) | `OpenRouter` | `anthropic/claude-sonnet-4` |
| TUI swarm chip | `Kimi Code` | `claude-sonnet-4` |

Supporting detail:

- The evidence log shows every `provider_request` for the worker going out as
  `provider: "OpenRouter", model: "anthropic/claude-sonnet-4"` from the first
  request at `20:20:32` onward. This is the ground truth of what was billed
  and what generated the worker's output.
- `~/.jcode/provider_activity.json` records `openai-compatible:kimi` as last
  used at the exact timestamps the worker was active, which matches the
  `Kimi Code` label in the UI but matches neither the evidence log's
  `OpenRouter` attribution nor the persisted `openrouter` provider key.
- `~/.jcode/config.toml` sets `default_provider = "kimi"`, a plausible source
  for the runtime fallback.

## What this implies

1. **The resolver accepts unlistable names.** `glm-5.2` and `deepseek-v4-pro`
   were refused, but `bridge/gemini-3-flash-agent`, equally absent from
   `list_models`, passed, apparently because a slash-form name is assumed to
   be a valid OpenRouter model id (`route=none`). The fail-closed behavior
   from `provider-confusion.md` Path A does not cover this shape.
2. **Whatever the resolver decided, the worker ran something else.** The
   requested `bridge/gemini-3-flash-agent` never appears in the worker's
   traffic. Something between spawn and first request substituted
   `anthropic/claude-sonnet-4`, silently, with no error surfaced to the
   coordinator, and persisted a third name (`k3`) to session metadata.
3. **Policy violation with no refusal path.** The routing policy forbids
   models of this class as swarm workers, yet the worker ran
   `claude-sonnet-4` for its whole life. This is a concrete instance of the
   advisory-policy gap already tracked in
   `swarm-model-policy-enforcement.md`.
4. **The routing prompt is stale against the live catalog.** Two of the three
   models it recommends for rotation do not resolve, which is what pushed the
   coordinator into guessing an off-catalog name in the first place.
5. **Identity display is untrustworthy.** With spawn result, session
   metadata, evidence log, and UI all disagreeing, an operator cannot audit
   which model produced which output. The UI's `Kimi Code` label and
   `provider_activity.json` attribution suggest display/attribution code
   resolves identity through a different path (likely
   `default_provider = "kimi"`) than the request path.

## Suggested first steps (not yet designed)

- Make spawn resolution fail closed for names not present in `list_models`,
  including slash-form OpenRouter-shaped names, or validate them against the
  provider before reporting success.
- Derive session metadata, UI chip, and activity attribution from the same
  source the request path uses, and assert they agree in tests.
- Reconcile `.jcode/swarm-prompt.md` model recommendations with the live
  route catalog, or make the prompt reference `list_models` output instead of
  naming models.

## Reproduction pointers

- Coordinator journal: `~/.jcode/sessions/session_peacock_1787861778353_a0d0e76b48e5f975.journal.jsonl`
- Worker session + journal + evidence: `~/.jcode/sessions/session_shark_1787862031120_2330b8c39a007acb.{json,journal.jsonl,evidence.jsonl}`
- Provider activity: `~/.jcode/provider_activity.json` (`openai-compatible:kimi` entry)

## Addendum (2026-08-27/28): live reproductions and root cause confirmed

Fresh reproductions while setting up the follow-up audit:

- `spawn model=k3`, `grok-4.5`, `kimi-for-coding`, `grok-4.6` all failed
  resolution despite appearing in `list_models` (the resolver never consulted
  the catalog); `bridge/gemini-3-flash-agent` — equally absent from the
  resolver's vocabulary — passed via the '/'-implies-OpenRouter heuristic.
- `spawn model=openai-compatible:kimi:k3` spawned but sent wire requests as
  model `kimi:k3` (first-colon misparse) and died with a transport error.
- Evidence logs labeled every openai-compatible request `OpenRouter`
  regardless of endpoint.

Root cause chain (full trace in the provider audit): spawn stamps session meta
before applying the model override; the override failed warn-only after
clearing the active kimi profile; execution fell to the OpenRouter slot's
hardcoded `DEFAULT_MODEL = "anthropic/claude-sonnet-4"` pointed at Kimi's
endpoint.

Fixes on this branch:

1. Catalog-based spawn resolution, fail closed (`fca321c34`).
2. Spawn fails on model-override failure; profile cleared only after a
   successful rebind (`4a9a21e89`).
3. Phantom default model and OpenRouter shape-passthrough removed
   (`95fdfff46`).

Remaining (tracked separately): single identity source for session meta /
evidence / UI chip / provider_activity, and de-multiplexing the OpenRouter
slot — the stage-3 refactor. Observability defects seen during the audit are
docs/issues/swarm-observability-status-and-wake-gaps.md.
