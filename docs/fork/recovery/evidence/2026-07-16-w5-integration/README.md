# W5 post-integration validation evidence

W5/R08A onboarding-consent safety was serially integrated at merge commit `34743816cf7c668393d0fe407a19e917d4fa7e2b` from reviewed branch closure `52aed00e95887f8c694dd3249927fbaeed1a04ba`. The corrected discriminating test is commit `95861f4f5f354dbb3123c19754ac1ca1d13083ac`; the frozen correction-evidence head reviewed independently was `f42f79bfcd3c0ec27f839b0ccef54f4755d9d056`.

## Accepted run

The accepted run used direct cached tools only, with `CARGO_NET_OFFLINE=true`, disposable `JCODE_HOME` and `JCODE_RUNTIME_DIR`, disabled telemetry/nudges/setup hints, and the warm W5 correction target directory. It invoked no Nix command, `scripts/dev_cargo.sh`, network, provider, credential, daemon, reload, publication, release, installer, updater, or external action.

Key results in `accepted-manifest.tsv`:

- corrected timeout regression: exit `0`, exactly `1 passed; 0 failed`
- Escape, decline-all, and affirmative-import companion fixtures: each exit `0`, exactly `1 passed; 0 failed`
- `cargo check -p jcode-tui` and source `rustfmt --check`: exit `0`
- top-level and correction-run W5 evidence `SHA256SUMS`: exit `0`
- preserved mutation proof: recorded exit `101` and the discriminating `onboarding_import_failed_provider.is_none()` assertion failure
- production source diff: exactly `2` additions and `2` deletions, net-zero LOC
- test diff: exactly one pure `40`-line addition
- integrated path boundary: expected/actual exit `1`
- R09 green gates: classifier, dependency, wildcard, warning, shell syntax, and diff check exit `0`
- inherited R09 debt: panic, swallowed-error, production-size, and test-size exit `1` as expected
- prompt diff SHA-256 remained `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`
- stash count remained exactly `4`
- before/after process snapshots were identical and contained only the same pre-existing SSH mux; no remote builder appeared
- no-update invocation guard and final status exited `0`

Fresh independent review artifacts are byte-exact under `docs/fork/recovery/reviews/`:

- Opus PASS, zero IMPORTANT/CRITICAL findings, SHA-256 `582b3d122a36e85ef60dfc76ad9f2c4d848d3c62791975ae2f82fae41c8806f5`
- Fable PASS, zero IMPORTANT/CRITICAL findings, SHA-256 `d701676fd28fd82db90a285cab4c69810dce9977920dc5431d5230fbcde8f6bf`

The earlier contradictory mutation-surviving PASS and all unsafe/preaccepted attempts remain preserved append-only and are not counted.

Raw transcripts are deterministic gzip files. `RAW_SHA256SUMS` records their uncompressed bytes; `SHA256SUMS` covers all tracked evidence files except itself.
