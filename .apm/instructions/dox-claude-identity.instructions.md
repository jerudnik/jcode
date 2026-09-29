---
description: Ownership and validation of the shared Claude Code OAuth identity.
applyTo: "crates/jcode-provider-core/src/claude_cli_identity*.rs"
---

# Claude Code OAuth identity

- This module owns installed-version detection and the reviewed fallback. Derive user-agent, billing attribution, and preflight app version from the same cached identity. Do not restore separate version literals in consumers.
- Query the executable on PATH without a shell, bound time and stdout, and cache failures as well as successes. Never install, update, or authenticate Claude Code during detection.
- API-key requests do not need this identity. Keep request formatting pure by passing the selected identity from the runtime.
- Verify detection, malformed output, command failure, timeout, fallback, and request-field propagation. Keep fallback and restart guidance in `OAUTH.md` current.
