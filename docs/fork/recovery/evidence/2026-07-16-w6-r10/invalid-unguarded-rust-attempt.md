# Invalid unguarded Rust validation attempt

This attempt is preserved append-only but is not accepted as passing evidence.

- Background task: `702684spm1`
- Command:

```bash
scripts/dev_cargo.sh test -p jcode-app-core update::tests::test_verify_asset_checksum_text_accepts_matching_digest -- --exact --nocapture
scripts/dev_cargo.sh test -p jcode-app-core update::tests::test_verify_asset_checksum_text_rejects_mismatch -- --exact --nocapture
scripts/dev_cargo.sh test -p jcode-app-core update::tests::test_verify_asset_checksum_text_requires_asset_entry -- --exact --nocapture
```

- Exit: `124`
- Duration: 600.0s
- Attribution: invalid for acceptance because it lacked the required guards `CARGO_NET_OFFLINE=true`, `FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`, `JCODE_NO_TELEMETRY=1`, and disposable `JCODE_HOME` / `JCODE_RUNTIME_DIR`.
- Observed result: timed out during cold Nix/Rust compilation, not a test assertion failure.
