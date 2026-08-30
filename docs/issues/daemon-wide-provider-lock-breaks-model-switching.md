---
title: "Auth-change activation locks the whole daemon to one provider, breaking model switching for every session"
status: open
priority: high
owner: maintainers
opened: 2026-08-30
related:
  - crates/jcode-base/src/provider/activation.rs
  - crates/jcode-base/src/auth/lifecycle.rs
  - crates/jcode-base/src/provider/selection.rs
---

# Daemon-wide provider lock breaks model switching

## Symptom

Users cannot switch models to another provider from the TUI model picker.
The picker stages the switch, the server rejects it, and from the user's
perspective nothing happens. Observed 2026-08-30 12:38 in
`~/.jcode/logs/jcode-2026-08-28.log` (the long-running daemon writes to the
file named for its start date):

```
server_set_route_request id=5 current_provider=Claude requested_model=zai/glm-5.3 requested_provider=OpenAI-compatible
[ERROR] server_model_change_failed error="Model 'openai-compatible:zai/glm-5.3' targets an OpenAI-compatible provider but --provider is locked to Anthropic. ..."
```

The user never passed `--provider`. The daemon locked itself.

## Root cause

1. A client Claude login/auth change sends `notify_auth_changed` to the
   shared daemon.
2. The handler calls `direct_provider_activation("claude")`
   (`crates/jcode-base/src/auth/lifecycle.rs:1005`), which builds
   `ProviderActivation::locked(Claude, Claude)`.
3. `apply_env()` (`crates/jcode-base/src/provider/activation.rs:192`) sets
   **process-wide** env vars in the daemon: `JCODE_FORCE_PROVIDER=1`,
   `JCODE_ACTIVE_PROVIDER=claude`, `JCODE_RUNTIME_PROVIDER=claude`.
   Nothing ever unlocks them.
4. Every session Agent constructed afterwards reads
   `forced_provider_from_env` (`crates/jcode-base/src/provider/selection.rs:54`,
   used in `startup.rs:311`), so every new session's `MultiProvider` is
   locked to Anthropic.
5. `model_switching.rs:64/:83` then rejects any switch to a non-Anthropic
   provider.

Observed lock event in the daemon log:

```
[2026-08-28 23:24:48] request_kind=notify_auth_changed ...
[2026-08-28 23:24:48] AUTH event=runtime_activation provider=claude label=Anthropic/Claude selection=locked active_provider=claude
```

The daemon (pid 2848, up since 2026-08-28) has been locked ever since.

The design assumed a single-user CLI process where `--provider claude`
legitimately locks that process. In the shared daemon, per-login activation
leaks into global state affecting all sessions.

## Blast radius

- All sessions created after any locked activation cannot switch providers.
- Child processes inherit the polluted env. Confirmed:
  `test_azure_login_completion_switches_local_model_without_completion`
  fails on `main` when run from a shell that inherited the daemon's env,
  and passes with `env -u JCODE_FORCE_PROVIDER -u JCODE_ACTIVE_PROVIDER
  -u JCODE_RUNTIME_PROVIDER -u JCODE_OPENROUTER_TRANSPORT_STATE`.
- The failure is silent in the TUI. `server_model_change_failed` is logged
  server-side but the user sees no error (check
  `model_picker_select_failed` handling in
  `crates/jcode-tui/src/tui/app/inline_interactive.rs:3007`).

## Fix direction

- Stop using process-wide env as the carrier for per-session provider
  selection in the daemon. Activation from `notify_auth_changed` should
  update per-session or per-agent state, not `set_var`.
- At minimum: auth-changed activation in the server should use an unlocked
  selection (`RuntimeSelection::Unlocked`) or explicitly unlock after the
  login flow completes.
- Surface `server_model_change_failed` to the requesting client so a
  rejected switch is visible.

## Workaround

Restart the daemon (clears the env) and avoid Claude login/auth-change
events afterwards.
