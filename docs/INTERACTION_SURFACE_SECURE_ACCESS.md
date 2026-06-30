# Interaction Surface Secure Access Path

## Decision

The operational path for the interaction surfaces is local or private mesh access first. Public exposure is blocked unless a separate documented security review concludes it is low risk and the operator explicitly sets `gateway.access_mode = "public_reviewed"` and `gateway.public_exposure_reviewed = true`.

Kanidm OIDC with Authorization Code + PKCE and WebAuthn/passkeys remains the intended future browser auth path, but it is disabled by default in code and config until the review approves issuer, audience, JWKS validation, group mapping, ticket/cookie handling, TLS, and redirect URI ownership.

## Supported modes

### Local loopback

Use this on the workstation only or when a reverse tunnel terminates locally:

```toml
[gateway]
enabled = true
bind_addr = "127.0.0.1"
access_mode = "local"
public_exposure_reviewed = false
oidc_enabled = false
```

The gateway refuses `access_mode = "local"` with a non-loopback bind address.

### Private mesh or LAN

Use this for ZeroTier, Tailscale, or trusted LAN reachability:

```toml
[gateway]
enabled = true
bind_addr = "0.0.0.0"
access_mode = "mesh"
public_exposure_reviewed = false
oidc_enabled = false
```

This is the default operational mode. It preserves the existing pairing-token flow and WebSocket bearer/query token validation.

### Public reviewed path

Public mode is not enabled by default and is not recommended for this program. If a future review approves public exposure, operators must set both:

```toml
[gateway]
access_mode = "public_reviewed"
public_exposure_reviewed = true
```

Without the explicit review flag, the gateway fails closed.

## Disabled OIDC/Kanidm integration gate

The config accepts placeholders for the future Kanidm path:

```toml
[gateway]
oidc_enabled = false
# oidc_issuer = "https://idm.example.test/oauth2/openid/jcode"
# oidc_client_id = "jcode"
# oidc_audience = "jcode"
# oidc_required_group = "jcode-users"
```

If `oidc_enabled = true` is set today, the gateway refuses to start. This provides a testable disabled-by-default integration gate without pretending that token validation, cookie security, or WebAuthn ceremony is complete.

## Review checklist before enabling public exposure or OIDC

- TLS terminates at a trusted endpoint with a stable hostname and no mixed-content downgrade for browser WebSockets.
- Kanidm issuer, audience, client id, redirect URI, and JWKS are pinned to expected values.
- Authorization Code + PKCE is used for browser login.
- WebAuthn/passkeys or YubiKey policy is enforced by Kanidm for the relevant users.
- Required group/claim mapping is validated server-side.
- WebSocket access uses short-lived tickets or secure same-site cookies after OIDC login.
- Pairing tokens are not accepted on public unauthenticated endpoints unless explicitly reviewed.
- Re-auth and auth failure paths are visible in the web client and tested.
- Public DNS, firewall, reverse proxy, and certificate changes are reviewed before deployment.

## Validation

Implemented tests:

- `test_gateway_access_policy_allows_local_and_mesh_paths`
- `test_gateway_access_policy_blocks_public_and_oidc_without_review`
- `test_gateway_access_policy_local_rejects_non_loopback_bind`

Run:

```bash
nix develop --command cargo test -p jcode-base gateway_tests --lib
```
