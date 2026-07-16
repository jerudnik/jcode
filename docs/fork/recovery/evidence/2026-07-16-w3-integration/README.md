# W3 post-integration evidence

Integration head: `566d7930606f96add92aed65564c95b539a03df0` on `recovery/2026-07-15`.

## Guarded exact fixtures

The post-integration ladder used `FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`, `CARGO_NET_OFFLINE=true`, `JCODE_NO_TELEMETRY=1`, and a fresh disposable `JCODE_HOME` plus `JCODE_RUNTIME_DIR` for every fixture.

- Exact sections: 14.
- Exact named passes: 14.
- Exit-zero sections: 14.
- Zero-filter passes counted: 0.
- Raw transcript SHA-256: `59a1ad41431683555e2bb41b7137cc81eb30507ea13b24abf8b4fad50f50bd2a`.
- `targeted-fixtures.log.gz` is a deterministic gzip of the byte-exact raw transcript.

## Affected checks and R09 expected exits

| Gate | Expected | Actual |
|---|---:|---:|
| affected package check | 0 | 0 |
| classifier | 0 | 0 |
| dependency boundaries | 0 | 0 |
| panic budget | 1 | 1 |
| swallowed-error budget | 1 | 1 |
| production-size budget | 1 | 1 |
| test-size budget | 1 | 1 |
| wildcard re-export budget | 0 | 0 |
| warning budget | 0 | 0 |
| shell syntax | 0 | 0 |
| diff check | 0 | 0 |

Every raw command, exit, log, guard file, and the source matrix manifest is preserved under deterministic gzip. No command used `--update`; the four expected-red ratchets remain visible and unchanged.

## Preservation

After validation, only the user-owned `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` edit was dirty. Its diff SHA-256 remained `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`; exactly four stashes remained untouched. The committed W3 evidence manifest still passed. No provider, credential, daemon, reload, network, release, installer, updater, publication, baseline, protocol, schema, or live-pilot action occurred.
