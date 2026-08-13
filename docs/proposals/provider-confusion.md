# Provider confusion: unknown model names silently fall back to the default provider

Status: proposal, evidence gathered 2026-08-12/13 (test-remediation pipeline session)

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
- Provider identity is only visible *after* spawn, via `swarm status` /
  `swarm list` (`Provider:` line), and only once the session has begun running.

## Root cause

Two catalogs disagree, and resolution fails open.

1. `swarm list_models` prints the **live** Cursor catalog (dozens of names like
   `gpt-5.6-sol-xhigh-fast`, `kimi-k3-high`, `cursor-grok-4.5-high-fast`).
2. Bare-name routing uses the **static** compatibility shim
   `crates/jcode-base/src/provider/cursor.rs::is_known_model`, whose
   `AVAILABLE_MODELS` is 11 entries (`composer-2.5`, `gpt-5.4-high`,
   `sonnet-4.6`, ...). None of the live-catalog names above are in it.
3. `resolve_model_spec` (`crates/jcode-base/src/provider/models.rs`) therefore
   returns `provider_key: None` for these names, and
   `selection_for_concrete_model` (`crates/jcode-app-core/src/server/comm_session.rs`)
   produces a selection with no provider key. Downstream, the spawned session
   falls back to the server default identity — in this deployment, the
   Anthropic coordinator model.

So the failure chain is: list_models advertises a name → bare-name resolution
does not recognize it → spawn proceeds anyway on the default provider → the
operator's model choice is silently discarded. The cost is real: this burned a
constrained Anthropic usage budget while the operator believed work was running
on OpenAI/Cursor capacity.

Note the same fail-open shape exists for typos: `model: "gpt-5.6-slo"` would
spawn happily on the default provider.

## Why it is hard to catch

- The spawn result contains only the session id, not the resolved
  model/provider.
- Headless workers do not announce their identity in reports.
- `swarm list_models` presents one flat namespace with no marker for "this
  name is spawnable as-is" vs "this name needs a route prefix".

## Stepwise remediation plan

1. **Echo resolved identity at spawn time.** The `spawn` (and `assign_task`,
   `run_plan`) result should include `model=<bare> provider=<key> route=<api
   method>` as resolved, so a mismatch is visible in the same tool result that
   returns the session id. Cheap, no behavior change.
2. **Fail closed on unresolved concrete models.** In
   `resolve_swarm_spawn_selection`, when a concrete requested model resolves
   with `provider_key: None` and does not match the coordinator's model, reject
   the spawn with an error listing near-miss route names, instead of falling
   back to inheritance. An explicit `inherit` sentinel already exists for the
   inherit case, so nothing legitimate is lost.
3. **Reconcile the catalogs.** Either (a) make `swarm list_models` print the
   route-prefixed spawnable form for every entry (e.g.
   `cursor:gpt-5.6-sol-high-fast`), or (b) feed the live Cursor catalog into
   bare-name resolution (`routes_memo` already caches live routes for other
   purposes). (a) is simpler and self-documenting; (b) fixes typo-adjacent
   confusion too. Do (a) first.
4. **Surface identity in completion reports.** Append the resolved
   model/provider to the worker's report header so post-hoc audits do not
   require `swarm list` before sessions are cleaned up.
5. **Tests.** Unit: `resolve_model_spec` returns `None` provider for a
   live-catalog-only Cursor name (pin the regression). Integration: spawn with
   an unknown model errors; spawn with `openai-oauth:` prefix resolves to
   OpenAI; `list_models` output round-trips through spawn.

## References

- `crates/jcode-base/src/provider/models.rs` — `resolve_model_spec`,
  `base_builtin_provider_for_model`.
- `crates/jcode-base/src/provider/cursor.rs` — static `AVAILABLE_MODELS` shim;
  header comment explains the runtime moved to
  `jcode-provider-cursor-runtime`.
- `crates/jcode-app-core/src/server/comm_session.rs` —
  `selection_for_concrete_model`, `resolve_swarm_spawn_selection`,
  `explicit_route_for_configured_model` (doc comment documents the intended
  `openai-api:gpt-5.5` prefix behavior).
- Related: `docs/proposals/swarm-lifecycle-remediation.md` (trusting signals
  that do not hold), `docs/proposals/swarm-session-identity.md` (name reuse and
  attachment ambiguity from the same incident).
