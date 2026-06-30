# jcode Web Mobile MVP

This is a zero-build browser client for jcode's existing local gateway. It is aimed at two devices/use cases:

- **BlackBerry Key2 / small Android browser**: lightweight text-first chat, physical-keyboard friendly, one-column layout.
- **Lenovo Legion Y700 / 8.8 inch tablet**: richer two-column tablet layout with chat plus sessions/models side panel.

The app lives in `web/jcode-mobile/` and uses ArrowJS from a pinned CDN import. There is no npm install or bundler required.

See also:

- [`PERSONAL_INTERACTION_SURFACES.md`](./PERSONAL_INTERACTION_SURFACES.md) for the shared design language, device roles, typography, glyph, and cross-surface architecture direction.
- [`INTERACTION_SURFACE_REQUIREMENTS.md`](./INTERACTION_SURFACE_REQUIREMENTS.md) for implementation-ready requirements, command contracts, object schemas, and acceptance criteria.

## Why web first

A web app gets us a useful Android client quickly without committing to Android app scaffolding yet. It also exercises the exact protocol a later native Android app would use:

- `POST http://HOST:7643/pair`
- `WS ws://HOST:7643/ws?token=TOKEN`

The query-token WebSocket path is deliberate because browsers cannot set `Authorization` headers on WebSocket constructors. The gateway already accepts query-token auth for browser clients.

## Workstation setup

Enable the gateway in `~/.jcode/config.toml`:

```toml
[gateway]
enabled = true
port = 7643
bind_addr = "0.0.0.0"
```

Restart jcode, then run:

```bash
jcode pair
```

Use the printed host and pairing code in the web app.

If the displayed host is not reachable from Android, set this on the workstation before running `jcode pair`:

```bash
export JCODE_GATEWAY_HOST=your-machine.your-tailnet.ts.net
```

## Serving the web app

From the repo root:

```bash
python3 -m http.server 8787 --directory web/jcode-mobile
```

Then open this on Android over LAN or Tailscale:

```text
http://WORKSTATION_HOST:8787/
```

Pairing talks directly from the Android browser to the jcode gateway at `http://WORKSTATION_HOST:7643`.

## Current MVP features

- Pair via host, port, code, and device name.
- Store paired credentials in `localStorage`.
- Store drafts, selected server/session, filters, focus mode, and pending local commands in `localStorage` before or while editing.
- Connect to saved servers.
- Auto-reconnect a saved workstation with capped jittered backoff.
- Treat mobile background, page hide/show, and offline/online changes as normal: foreground/network return resubscribes and requests `get_history` before sending queued local messages.
- Send prompts.
- Queue prompts locally when the socket is unavailable, show them as pending, and allow retry, edit-back-to-draft, or discard. Commands that may have been sent before an ack are marked for review instead of auto-replayed.
- Cancel a running turn.
- Sync history.
- Render streamed assistant text, reasoning, tool calls, errors, notifications, token summaries, sessions, and model list events.
- Switch sessions and models when the server provides them.
- Responsive layout for small phones and tablets.
- 8.8 inch tablet cockpit shell with live link/session/stream/turn/tool telemetry.
- Focus mode that hides pairing and side panels for transcript-first supervision.
- Quick prompt deck for away-from-keyboard control patterns.
- Searchable session and model lists plus a compact pulse panel for status, model, token, and server readouts.
- Explicit link states for offline, reconnecting, resyncing, live, idle session, auth failure, and generic error.

## Design direction

The portal is intentionally not a heavy admin dashboard. It should feel like a jcode instrument panel:

- **Razor sharp hierarchy**: one primary transcript plane, one secondary control rail, and terse telemetry.
- **Lightning fast**: zero build step, pinned ArrowJS CDN import, no icon packs, no charting runtime, no unnecessary animation.
- **Featherweight**: plain HTML/CSS/JS, local state only, tolerant protocol handling, and no dependency expansion until installable PWA work requires it.
- **Tablet first**: optimized for 8.8 inch landscape use with thumb-safe controls, sticky side controls, horizontal quick chips, and high contrast dark mode.
- **Near future ready**: room for adaptive/agentic UI without committing to opaque generative UI. The next 6 to 8 months should add context-aware command chips, file/tree review, and multi-session watch panes while keeping the shell static and inspectable.

## Known limitations

- No QR scanner yet. Manual entry is intentional for first MVP.
- No vendored ArrowJS bundle yet. The app imports `@arrow-js/core@1.0.6` from `esm.sh`.
- No HTTPS/WSS yet. Use Tailscale or LAN. Some browsers may block `ws://` if the page itself is served over `https://`; serve the app over `http://` for this MVP.
- Credentials are in browser `localStorage`. This is acceptable for a local-first prototype, but native Android should move tokens to Android Keystore.
- Auth is still local pairing-token only. Kanidm OIDC + PKCE remains the planned P1 path; no public exposure is configured by this app.
- The UI is protocol-tolerant but not exhaustive. Unknown events are ignored with a status note.

## Validation

Run:

```bash
./scripts/check_web_mobile.sh
./scripts/check_web_mobile_rendered.mjs
```

The rendered smoke script launches Chrome through the DevTools Protocol, serves `web/jcode-mobile/` locally, and checks Key2, Y700, and laptop viewports. It verifies the app renders without runtime errors or horizontal overflow, queues a local command, reloads, and confirms the pending command remains visible and persisted. Screenshots and `report.json` are written under ignored `.tmp/web-mobile-rendered/`.

`check_web_mobile.sh` checks JavaScript syntax, including the rendered harness, and verifies the static app contains the required gateway protocol pieces.
It also runs `web/jcode-mobile/surface_state.test.mjs`, which covers reconnect backoff, foreground resync request ordering, offline/background close handling, auth-failure close classification, and draft/pending-command recovery.

## Next slices

1. **Key2 polish**
   - Add a true "lite mode" toggle that hides sessions/models and maximizes transcript height.
   - Add keyboard shortcuts: Enter send, Ctrl+Enter newline, Esc cancel.
   - Add larger touch targets and smaller memory footprint checks.

2. **Y700 tablet mode**
   - Add split transcript/session inspector.
   - Add model picker search.
   - Add local transcript cache and export.

3. **Installable PWA**
   - Vendor ArrowJS locally.
   - Add app manifest and service worker for offline shell caching when served from a secure origin or localhost.

4. **Native Android**
   - Reuse this protocol layer.
   - Add Android Keystore, share intents, notifications, background reconnect, and better IME/keyboard integration.
