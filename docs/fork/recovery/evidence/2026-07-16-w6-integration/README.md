# W6/R10 post-integration evidence

W6 was serially integrated into `recovery/2026-07-15` by merge commit `e228fdb0183c5aae01b46b17ad364fbd7dfa1ef3`, with parents `1976cf1cd75ed3eb359416c7d2ac9ac891bad2b4` and reviewed W6 branch head `19d90af988a52ad31294beceb89c8ffe51920e2c`.

The authoritative source/test review head was `c07654e259ef8bd016df1085437fd26e0e6c7e0d`. The later W6 commits are docs-only preservation of the exact Grok FAIL, corrected Grok PASS, Fable PASS, coordinator optional-validation incident, and evidence-manifest corrections. Source/test/workflow paths did not change after `c07654e25`.

## Accepted post-integration run

The accepted run used no Nix invocation, no network, no remote builder, no `gh`, no tag or release, no live updater or installer, no profile mutation, no credential, and no daemon/reload action. It used the cached Cargo/Rust tools directly with `CARGO_NET_OFFLINE=true`, telemetry/nudge guards, a warm local target directory, and disposable `JCODE_HOME` and `JCODE_RUNTIME_DIR`.

Expected exits all matched:

- Hermetic Python release/acquisition suite: `0`, 6/6 tests.
- Guarded Rust checksum tests: `0`, 3/3 tests.
- Affected `cargo check -p jcode-app-core`: `0`.
- Touched shell syntax: `0`.
- R09 classifier, dependency, wildcard, warning, and diff gates: `0`.
- R09 panic, swallowed-error, production-size, and test-size gates: expected-red `1`, actual `1`.
- Integrated W6 evidence manifest verification: `0`.

No command used `--update`. `process_before` and `process_after` are byte-identical and contain no active remote-builder process. Raw command/log SHA-256 values are in `RAW_SHA256SUMS`; deterministic gzip artifacts and metadata are covered by `RUN_SHA256SUMS` and the directory-level `SHA256SUMS`.

## Preserved invalid attempts

The W6 branch retains the original unguarded Rust timeout as invalid evidence and the coordinator's cancelled optional `nix shell --offline` actionlint/PowerShell attempts. The latter unexpectedly contacted configured caches and an SSH builder, were cancelled after approximately 226 seconds, and were not counted as validation. No remaining build process, release, ref movement, or repository mutation was found.
