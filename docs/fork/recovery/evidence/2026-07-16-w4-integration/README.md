# W4 post-integration validation evidence

W4/R02 was serially integrated at merge commit `bce68e09852ac4bcc64131f318c7042d5d099143` from reviewed closure `6cc72ef780af5c3cdc5a8ac04622a6950b733705`.

## Accepted run

The accepted run used direct cached tools only, with `CARGO_NET_OFFLINE=true`, disposable `JCODE_HOME` and `JCODE_RUNTIME_DIR`, disabled telemetry/nudges/setup hints, and the warm W4 target directory. It invoked no Nix command, `scripts/dev_cargo.sh`, network, provider, credential, daemon, reload, publication, release, installer, updater, or external action.

Key results in `accepted-manifest.tsv`:

- exact catalog-route fixture: exit `0`, exactly `1 passed; 0 failed`
- `cargo check -p jcode-base`: exit `0`
- W4 evidence `SHA256SUMS`: exit `0`
- integrated Rust diff from the pre-W4 main head: empty
- integrated path boundary: expected/actual exit `1`
- R09 green gates: classifier, dependency, wildcard, warning, shell syntax, and diff check exit `0`
- inherited R09 debt: panic, swallowed-error, production-size, and test-size exit `1` as expected
- prompt diff SHA-256 remained `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`
- stash count remained exactly `4`
- before/after process snapshots were identical and contained only the same pre-existing SSH mux; no remote builder appeared
- no-update invocation guard and final status exited `0`

## Preserved invalid attempt

The first coordinator post-integration attempt is preserved under `invalid-process-guard-*` and `raw/invalid-process-guard/`. Its process-boundary command was malformed by shell expansion of an awk `$0` expression and exited `2` before any Cargo test or other validation ran. It is non-authoritative and not counted.

Raw transcripts are deterministic gzip files. `RAW_SHA256SUMS` records their uncompressed bytes; `SHA256SUMS` covers all tracked evidence files except itself.
