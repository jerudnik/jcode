# 2026-07-17 runtime promotion evidence

This bounded package records the post-N2 live cutover onto product/runtime commit
`8962bccb32eede3b6746c42bfe6d265df29e4471`.

## Result

- Exact release label: `8962bccb3-release`.
- Exact binary SHA-256:
  `6cf81221e8c0cee86ae714d2f1fc9fb55fe8715f45ee8082dc2ecf034a2515fc`.
- `current`, `stable`, and `shared-server` all resolve to the profile-qualified
  immutable release.
- Doctor reports client/server verdict `same` at `8962bccb3`.
- Forced reload reported `reloaded=true` and `handoff_ready=true` in the external
  transaction log. The committed no-op proof reports `already_current=true`.
- The active coordinator session survived the handoff.
- A direct main-socket smoke subscribed with an absolute working directory and
  received `ack`, MCP status events, and `pong`.
- Menubar and `com.jcode.hotkey` were deliberately restarted from older retained
  builds onto the exact release.

## Files

- `runtime-manifest.txt`: commit, binary, channel targets, markers, version, hash.
- `doctor.json`: final client/server identity and verdict.
- `reload-already-current.json`: stateful reload regression in the live runtime.
- `server-info.json`: debug-socket server identity.
- `sessions.json`: post-handoff session continuity evidence.
- `shared-server-cutover.{before,after}`: exact reversible channel transaction.
- `integrations.{before,after}`: retained menubar/hotkey executable identities.
- `SHA256SUMS`: package file hashes.

The larger pre-promotion and live rollback snapshots remain outside Git under
`/Users/jrudnik/labs/jcode-normalization-rollback/` and are intentionally retained
through the soak.
