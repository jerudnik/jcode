# Provider confusion: unknown model names silently fall back to the default provider

Status: implemented, 2026-08-28

## Implemented Path A

Swarm spawn selection now enforces the first provider-router invariant: a
concrete model request must resolve to a provider or explicit route, match the
coordinator's exact model, or fail before a visible or headless session is
created. The error names the requested model and directs the coordinator to
`swarm list_models` or an explicit prefix such as `openai-oauth:`.

Successful spawn responses and the corresponding server log now include the
resolved `model`, `provider_key`, and `route`. The response fields are optional
for wire and persisted-mutation compatibility. Tests cover unknown per-spawn
and configured models, `inherit`, exact coordinator-model inheritance, and the
explicit OpenAI and Claude API/OAuth prefixes. The earlier test that accepted
an unknown prefix with `provider_key=None` was deliberately replaced.

The 2026-08-28 follow-up now resolves bare spawn names against the same live
`ModelRoute` catalog returned by `swarm list_models`. It carries the selected
route's API method into session creation and rejects listed-but-unavailable and
uncataloged names before a worker starts.

## Symptom

An operator spawned swarm workers with `model: "gpt-5.6-sol-xhigh-fast"` and
`model: "kimi-k3-high"` — names taken directly from `swarm list_models` output,
where they appear as live Cursor routes. Every one of those workers actually ran
on **Claude/claude-opus-5** (the coordinator/default identity). The operator
noticed only because the Anthropic usage meter moved; nothing in the spawn
result, worker output, or completion report said the requested model had been
ignored.

Observed in `swarm list` after the fact:

```
macaque  Task: remediation plan synthesis   Model: Claude/claude-opus-5   (requested gpt-5.6-sol-xhigh-fast)
tigress  Task: kimi plan review             Model: Claude/claude-opus-5   (requested kimi-k3-high)
puppy    Task: plan update per review       Model: Claude/claude-opus-5   (requested gpt-5.6-sol-xhigh-fast)
```

Probes run during the incident:

- `spawn model="cursor:gpt-5.6-sol-high-fast"` → session fails with
  `Cursor agent stream error: {"code":"not_found", ...}`. So the prefixed form
  routes to Cursor, but Cursor itself rejects the name.
- `spawn model="openai-oauth:gpt-5.6-sol"` → verified `Provider: OpenAI / gpt-5.6-sol`.
  Explicit auth-route prefixes work.
- Before Path A, provider identity was only visible *after* spawn, via
  `swarm status` / `swarm list` (`Provider:` line), and only once the session
  had begun running.

## Root cause

Two catalogs disagree, and resolution fails open.

1. `swarm list_models` prints the **live** Cursor catalog (dozens of names like
   `gpt-5.6-sol-xhigh-fast`, `kimi-k3-high`, `cursor-grok-4.5-high-fast`).
2. Bare-name routing uses the **static** compatibility shim
   `crates/jcode-base/src/provider/cursor.rs::is_known_model`, whose
   `AVAILABLE_MODELS` is 11 entries (`composer-2.5`, `gpt-5.4-high`,
   `sonnet-4.6`, ...). None of the live-catalog names above are in it.
3. `resolve_model_spec` (`crates/jcode-base/src/provider/models.rs`) does not use
   the live Cursor catalog. Some names therefore return `provider_key: None`;
   broad built-in heuristics can classify others as a different provider, such
   as OpenAI for a `gpt-*` name. Before Path A, a `None` selection proceeded and
   downstream session creation fell back to the server default identity.

So the failure chain is: `list_models` advertises a name → static bare-name
resolution returns no provider or a different provider → spawn proceeds without
checking the live route identity → the operator's provider choice is silently
discarded. In the unresolved incident path, downstream session creation fell
back to the Anthropic coordinator/default identity. The cost is real: this
burned a constrained Anthropic usage budget while the operator believed work
was running on OpenAI/Cursor capacity.

The same fail-open shape existed for any typo or arbitrary name that resolved
to no provider. Names captured by a broad family heuristic are a separate live
route disambiguation problem deferred below.

## Why it is hard to catch

- Before Path A, the spawn result contained only the session id, not the
  resolved model/provider.
- Before the 2026-08-28 follow-up, headless workers did not announce their
  identity in reports.
- `swarm list_models` presents one flat namespace with no marker for "this
  name is spawnable as-is" vs "this name needs a route prefix".

## Stepwise remediation plan

1. **Implemented in Path A: echo resolved identity at spawn time.** The direct
   `spawn` result includes the resolved model, provider key, and route beside
   the session id. Every successful internal swarm spawn writes the same
   identity to the server log.
2. **Implemented in Path A: fail closed on unresolved concrete models.** In
   `resolve_swarm_spawn_selection`, when a concrete requested model resolves
   with `provider_key: None` and does not match the coordinator's model, reject
   the spawn with an error directing the coordinator to `swarm list_models` or
   an explicit route prefix, instead of falling back to inheritance. An
   explicit `inherit` sentinel already exists for the inherit case, so nothing
   legitimate is lost.
3. **Implemented on 2026-08-28: reconcile the catalogs.**
   `catalog_selection_for_model` in `server/comm_session.rs` resolves bare names
   against the live route list, selects an available route, preserves its API
   method, and fails closed when the name is absent or unavailable. Tests
   `resolve_swarm_spawn_model_accepts_any_listed_catalog_model`,
   `resolve_swarm_spawn_model_reports_unavailable_catalog_routes`, and
   `resolve_swarm_spawn_model_rejects_uncataloged_slash_names` cover the
   list-and-spawn contract. This supersedes the earlier proposal to require a
   route-prefixed catalog entry or a separate unique-match lookup.
4. **Implemented on 2026-08-28: surface identity in completion reports.**
   `completion_report_with_identity` in `server/swarm.rs` adds the resolved
   provider and model to every non-empty report before the report is stored and
   sent to the owner. The identity therefore survives cleanup in the durable
   completion report. Test
   `update_member_status_includes_completion_report_in_owner_notification`
   covers stored and delivered text.
5. **Implemented on 2026-08-28: apply the resolver contract to `subagent`.**
   `resolve_subagent_selection` in `tool/subagent.rs` resolves overrides against
   the provider's live route catalog, preserves explicit auth routes, enforces
   `agents.swarm_denied_models`, inherits only for an omitted/sentinel/same-model
   request, and rejects unresolved concrete names before creating the child
   session. Tests
   `model_override_resolves_against_live_catalog_instead_of_inheriting_parent_route`,
   `unresolved_model_override_fails_closed_instead_of_inheriting_parent_route`,
   and `denied_model_override_fails_before_inheriting_the_same_parent_model`
   cover the former blind provider-key inheritance and policy bypass.

## 2026-08-28 supersession verdict

The planned narrowing of the unknown-model branch in
`MultiProvider::set_model` is superseded by the resolver fixes that now guard
the affected path. Swarm and standalone subagent concrete model requests
resolve or fail before session creation, and headless model-switch failures are
fatal and roll the new session back. The remaining fallback at
`crates/jcode-base/src/provider/mod.rs:720-722` stays deliberately
provider-local: it asks the already active provider to accept a custom model
ID and cannot select a default or different provider. Removing it would break
forced providers and configured OpenAI-compatible endpoints that use private
model names without preventing the provider-confusion incident fixed here.

## References

- `crates/jcode-base/src/provider/models.rs` — `resolve_model_spec`,
  `base_builtin_provider_for_model`.
- `crates/jcode-base/src/provider/cursor.rs` — static `AVAILABLE_MODELS` shim;
  header comment explains the runtime moved to
  `jcode-provider-cursor-runtime`.
- `crates/jcode-app-core/src/server/comm_session.rs` —
  `catalog_selection_for_model`, `selection_for_concrete_model`,
  `resolve_swarm_spawn_selection`,
  `explicit_route_for_configured_model` (doc comment documents the intended
  `openai-api:gpt-5.5` prefix behavior).
- `crates/jcode-app-core/src/server/swarm.rs` —
  `completion_report_with_identity`, which stores and reports the resolved
  runtime identity.
- `crates/jcode-app-core/src/tool/subagent.rs` —
  `resolve_subagent_selection`, the standalone subagent resolver.
- Related proposal records: `swarm-lifecycle-remediation` (trusting signals
  that do not hold) and `swarm-session-identity` (name reuse and attachment
  ambiguity from the same incident).
