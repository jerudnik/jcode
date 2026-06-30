# Next Session Prep: Kanidm Auth for jcode Gateway

## Goal

Set up a reviewed Kanidm OIDC Authorization Code + PKCE auth path for the jcode web/mobile gateway without weakening the existing local/mesh default.

## Current state

- Gateway supports safe access modes:
  - `local`: requires loopback bind.
  - `mesh`: default private LAN/mesh path.
  - `public_reviewed`: requires `public_exposure_reviewed = true`.
- OIDC fields exist in config, but `oidc_enabled = true` intentionally fails closed pending implementation and security review.
- Existing pairing-token and WebSocket bearer/query auth remains tested.
- Reference doc: `docs/INTERACTION_SURFACE_SECURE_ACCESS.md`.

## Recommended next-session outcome

Implement Kanidm OIDC as a disabled-by-default, testable auth provider that can be enabled for reviewed local/mesh deployments first.

Do not make public DNS, firewall, reverse proxy, or production Kanidm changes without explicit confirmation.

## Information to gather before coding

Ask/confirm:

1. Kanidm issuer URL, for example `https://idm.example.com/oauth2/openid/jcode`.
2. Client ID, likely `jcode`.
3. Redirect URI to register, likely `http://127.0.0.1:<gateway_port>/auth/oidc/callback` for local testing and a reviewed HTTPS origin for mesh/public later.
4. Required group/claim for access, for example `jcode-users`.
5. Whether the first implementation should be local-only, mesh-only, or public-reviewed.
6. Cookie/session storage preference, default should be secure HTTP-only same-site cookies for browser login plus short-lived WebSocket tickets.

## Implementation plan

### 1. Config and validation

- Keep existing fields:
  - `oidc_enabled`
  - `oidc_issuer`
  - `oidc_client_id`
  - `oidc_audience`
  - `oidc_required_group`
- Add any needed fields only if necessary:
  - `oidc_redirect_url`
  - `oidc_scopes`
  - `oidc_insecure_allow_http_loopback` for local dev only, if needed.
- Change policy so `oidc_enabled = true` is allowed only when all required values are set and `access_mode` is `local` or reviewed `mesh` first.

### 2. OIDC provider module

Add a small auth module, likely in `crates/jcode-base/src/gateway_auth.rs` or `gateway/oidc.rs`:

- Discovery document fetch and issuer validation.
- JWKS fetch/cache.
- ID token verification:
  - issuer
  - audience/client id
  - expiry/not-before
  - nonce
  - signature key id and alg
- Group/claim check.
- Auth errors map to explicit gateway auth failure events.

Prefer well-maintained crates if already acceptable in the repo, such as `openidconnect`, `jsonwebtoken`, or lower-level `oauth2` plus JWK validation. Avoid hand-rolling crypto.

### 3. Browser login flow

Add gateway HTTP routes:

- `GET /auth/oidc/login`
  - creates PKCE verifier/challenge, CSRF state, nonce.
  - stores pending auth transaction server-side with short TTL.
  - redirects to Kanidm authorize URL.
- `GET /auth/oidc/callback`
  - validates state.
  - exchanges code with PKCE verifier.
  - verifies ID token and group.
  - creates secure local gateway session.
- `POST /auth/logout`
  - clears session.
- `POST /auth/ws-ticket`
  - authenticated browser session gets short-lived one-time WebSocket ticket.

### 4. WebSocket auth integration

- Preserve current pairing-token auth for non-OIDC local/mesh mode.
- When OIDC is enabled, accept:
  - short-lived one-time WS tickets from authenticated browser sessions.
  - optionally existing device pairing only if explicitly allowed by config.
- Reject expired, reused, or wrong-origin tickets.

### 5. Web/mobile UI

- Add auth lifecycle states:
  - unauthenticated
  - login required
  - login in progress
  - authenticated
  - auth expired
  - auth failed
- Add login/logout buttons.
- On auth expiry, stop sending commands, persist pending commands, prompt re-auth, then reconnect/resubscribe/get_history/reconcile after auth restores.

### 6. Tests

Unit tests:

- access policy accepts complete local OIDC config.
- incomplete OIDC config fails closed.
- issuer/audience/group mismatches fail.
- expired token fails.
- valid mocked token passes.
- WS ticket is one-time and TTL-bound.

Integration tests:

- fake OIDC provider with discovery/JWKS/token endpoint.
- login callback creates session.
- websocket can connect with ticket.
- auth expiry/re-auth path preserves pending commands.

Rendered web tests:

- login required screen.
- auth failure screen.
- authenticated workspace surface still renders.

## Security checklist before enabling beyond local dev

- HTTPS only for non-loopback browser origins.
- Registered redirect URI exactly matches deployed origin.
- PKCE S256 required.
- State and nonce are unpredictable and short-lived.
- Cookies are HTTP-only, same-site, secure when HTTPS.
- WS tickets are one-time, short-lived, bound to server session, and never logged.
- JWKS cache handles key rotation and rejects unknown algorithms.
- Required Kanidm group/claim enforced server-side.
- Public exposure still requires `access_mode = "public_reviewed"` and `public_exposure_reviewed = true`.

## Suggested first command next session

```bash
git status --short
sed -n '1,220p' docs/INTERACTION_SURFACE_SECURE_ACCESS.md
rg "oidc_|GatewayAccessMode|validate_access_policy|extract_ws_auth|DeviceRegistry" crates/jcode-base crates/jcode-config-types crates/jcode-app-core web/jcode-mobile
```

Then create a small implementation task: `Add local-only Kanidm OIDC discovery + login callback skeleton with fake-provider tests`.
