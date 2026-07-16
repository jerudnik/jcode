# Optional validation offline incident

This incident record preserves coordinator-provided invalid optional-validation logs. The attempts are not accepted as validation evidence.

## Summary

- Command class: optional `nix shell --offline` checks for `actionlint` / PowerShell parser availability.
- Incident: despite `--offline`, Nix unexpectedly contacted `cache.nixos.org`, the LAN cache, and an SSH builder.
- Duration before cancellation: 226 seconds.
- Outcome: tasks were cancelled.
- Mutation assessment: no process mutation, ref mutation, or repository mutation was accepted from the attempts.
- Evidence status: invalid and not accepted. The accepted W6 evidence remains the local/static checks recorded earlier.

## Preserved logs

| Log | SHA-256 | Status |
|---|---|---|
| `w6-actionlint-offline.log` | `65754c820ac6f21899c1770d87cab000886220957ca645755255d5563fa6f4ba` | invalid optional validation attempt |
| `w6-pwsh-offline.log` | `dcb6d4b033fc422430c1a6b3b33f2245d2abf2c74be8bdf9b28733dc2ad84fcd` | invalid optional validation attempt |

No further validation rerun was performed for this final docs-only append commit.
