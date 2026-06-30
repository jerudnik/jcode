# Interaction Surface Implementation Prompt

Status: Next-session execution prompt, 2026-06-30

Use this prompt to start the next implementation session.

```text
You are Jcode working on the jcode repository. Implement the next concrete slice of the interaction-surface project.

Primary docs:
- docs/INTERACTION_SURFACES.md
- docs/INTERACTION_SURFACE_REQUIREMENTS.md
- docs/PERSONAL_INTERACTION_SURFACES.md
- docs/SURFACE_WORKSPACE_SUBSTRATE_PLAN.md

Related infrastructure repo:
- ~/infrastructure/nix-config

Mission:
Turn the documented surface plan into working, tested code. Start with the smallest useful slice that improves the current browser/mobile client while preserving the TUI as the primary coding cockpit.

Non-negotiable product constraints:
- Keep the first web path zero-build where possible. Prefer plain JS/CSS and the existing web/jcode-mobile structure before adding dependencies.
- Treat mobile browser background WebSocket disconnects as normal. Do not rely on a background socket for correctness.
- Persist drafts, captured intents, pending commands, active session, and local surface state before or while editing.
- On foreground/network return, reconnect with capped backoff, resubscribe, call get_history, and reconcile pending local commands.
- Skip a bespoke device-scoped refresh/revocation auth layer. If Kanidm works reliably, use Kanidm OIDC with Authorization Code + PKCE and Kanidm WebAuthn/passkeys for YubiKey.
- Public exposure is allowed only if security risk is very low after review. Otherwise use ZeroTier mesh access and DNS jcode.mesh.rudnik.online.
- You may edit ~/infrastructure/nix-config to set up Kanidm, mesh DNS, routing, TLS, or service exposure needed for jcode.
- Do not build Backlog.md sync, GitHub Issues sync, Milkdown, tldraw, a heavy drag/drop framework, or a heavy rich-text editor in this slice.
- Preserve text/command fallbacks for rich actions.

Required workflow:

1. Research current implementation patterns.
   - Inspect web/jcode-mobile, gateway pairing/WebSocket code, session history handling, server protocol types, and existing storage helpers.
   - Inspect existing tests around pairing, server client lifecycle, get_history, reconnect, and protocol events.
   - Inspect ~/infrastructure/nix-config for Kanidm, oauth2-proxy or OIDC patterns, ZeroTier, DNS, and existing service exposure conventions.
   - If implementation details are unclear, use web resources to verify browser lifecycle behavior, WebSocket reconnect best practices, OIDC PKCE, Kanidm OIDC integration, or WebAuthn requirements.

2. Produce a brief implementation plan before editing.
   - Name the exact requirement IDs being targeted, likely SURF-G-001 to SURF-G-003 and SURF-G-008 first, plus auth/network scaffolding only if needed.
   - List files to change.
   - Define test fixtures and acceptance checks.
   - Decide whether this slice stays local-pairing only, adds OIDC scaffolding, or touches ~/infrastructure/nix-config. Prefer the smallest working slice.

3. Implement the code.
   - Add mobile lifecycle handling: visibilitychange, pageshow, pagehide, online, offline.
   - Add reconnect state machine with capped exponential backoff and jitter.
   - Persist drafts and pending local actions before send or on each edit.
   - On reconnect, send subscribe and get_history, then mark the UI resynced.
   - Make unsent commands visible and recoverable.
   - Add status text that distinguishes offline, reconnecting, resyncing, live, idle session, and auth failure.
   - If touching auth, prefer a minimal, testable boundary: Kanidm OIDC design or scaffold, short-lived WebSocket tickets/cookies, and no long-lived provider secrets in browser storage.
   - If touching infrastructure, keep it declarative, inspect existing nix-config conventions, run the repo's checks, and commit infra changes separately in that repo if needed.

4. Write tests and fixtures.
   - Unit-test reconnect/backoff state transitions where possible.
   - Add browser-client fixtures for close while backgrounded, foreground resync, offline/online, expired/bad token, and get_history after reconnect.
   - Add persistence tests for draft and pending command recovery.
   - Add protocol tolerance tests for unknown events and reconnect ordering.
   - If OIDC or infrastructure is touched, test bad callback, expired session, missing group/claim, and mesh DNS failure where practical.

5. Run validation.
   - Run targeted JS/Rust tests first.
   - Run cargo check or the relevant cargo test packages.
   - Use selfdev build for TUI changes when done.
   - For browser UI behavior, use browser automation or local manual fixtures. Prefer scripted validation over asking the user.
   - If tests fail and the cause is not obvious, troubleshoot with logs, code search, runtime debug socket, and web research.

6. Review and refine.
   - Review your diff for simplicity, security, and maintainability.
   - Remove dead code, debug noise, and stale TODOs.
   - Check that the UI does not imply a command was sent when it is only pending locally.
   - Confirm public exposure is not enabled unless security review criteria are explicitly satisfied.
   - Update docs only where the implementation changed the plan.

7. Commit and report.
   - Run git diff --check.
   - Commit focused jcode changes with a clear message.
   - If ~/infrastructure/nix-config was changed, validate and commit there separately.
   - Push when done.
   - Report concise summary, tests run, files changed, and any remaining risks or follow-up work.
```
