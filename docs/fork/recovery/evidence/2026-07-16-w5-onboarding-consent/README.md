# W5 onboarding consent evidence

Source HEAD validated: `b3b0103160883a5f5e6894d071e816bff92cccd1`.
Branch: `recovery/fix-w5-onboarding-consent-2026-07-16`.
Base: `566d7930606f96add92aed65564c95b539a03df0`.

## Accepted run

Accepted evidence is the top-level compressed log set in this directory, produced by `driver.sh` at `2026-07-16T07:14:10Z`. Raw `.log` process-snapshot files were deterministically gzip-compressed after validation to make `git diff --check` green without altering their byte-exact decompressed content. `RAW_SHA256SUMS` records the SHA-256 of each uncompressed accepted raw file; `SHA256SUMS` records the tracked compressed files and metadata.

Safety constraints:

- No live onboarding, credential, provider, network, daemon/reload, import, or external action was used.
- No Nix command was invoked by the accepted driver. It used cached direct tools only:
  - `/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin/cargo`
  - `/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin/rustfmt`
  - `/Library/Developer/CommandLineTools/usr/bin/python3`
- `JCODE_HOME` and `JCODE_RUNTIME_DIR` were disposable temp directories and cleaned by the driver.
- `CARGO_NET_OFFLINE=true`, `JCODE_NO_TELEMETRY=1`, `FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`.
- `process_before.log` and `process_after.log` record the same pre-existing `/tmp/nix-shell` ssh mux and no driver-created Nix/remote-builder process.

## Result counts

| Section | Expected | Actual |
|---|---:|---:|
| Timeout fixture | 0 | 0 |
| Existing Escape fixture | 0 | 0 |
| Existing decline-all fixture | 0 | 0 |
| Existing explicit affirmative fixture | 0 | 0 |
| Affected `cargo check -p jcode-tui` | 0 | 0 |
| Source-only rustfmt check | 0 | 0 |
| R09 classifier | 0 | 0 |
| R09 dependency | 0 | 0 |
| R09 panic budget | 1 | 1 |
| R09 swallowed-error budget | 1 | 1 |
| R09 production-size budget | 1 | 1 |
| R09 test-size budget | 1 | 1 |
| R09 wildcard re-export budget | 0 | 0 |
| R09 warning budget | 0 | 0 |
| R09 shell syntax | 0 | 0 |
| R09 diff check | 0 | 0 |

No command used `--update`. The unchanged R09 expected-red gates remain attributed inherited debt, not W5 regressions.

## Invalid/preaccepted attempts

`invalid-unsafe-driver/` preserves byte-exact artifacts from earlier preaccepted runs that used `scripts/dev_cargo.sh`, plain or safe-wrapper Nix, or were started before the accepted top-level cleanup. Raw `.log` files and `cargo_path.stderr` there were deterministically gzip-compressed after preservation; `invalid-unsafe-driver/RAW_SHA256SUMS` records their uncompressed SHA-256 values. They are retained for audit only and are not accepted validation evidence. The accepted run above supersedes them.

## 2026-07-16 correction-run

`correction-run/` supersedes the top-level accepted run for integration purposes after coordinator adjudication rejected `dfe5d1ec4b359ea68d956eab9feaa62399e29618` as internally contradictory evidence. The prior source fix was preserved, but the timeout regression was strengthened at `95861f4f5f354dbb3123c19754ac1ca1d13083ac` to assert state that differs between the buggy no-runtime import path and direct `onboarding_handle_login_failed(None)`:

- `onboarding_import_failed_provider.is_none()`.
- `onboarding_import_error.as_deref() == Some("We couldn't import those logins.")`.

The correction run preserved the same safety boundary: no live onboarding, credentials, provider, network, daemon/reload, import, Nix command, or `scripts/dev_cargo.sh`. It used only cached direct Cargo/rustfmt and system Python. Full result table and mutation proof are in `correction-run/README.md`.

Correction-run highlights:

- Four fixtures: timeout, Escape, decline-all, explicit affirmative all exit `0`.
- Affected `cargo check -p jcode-tui`, source-only rustfmt, R09 classifier/dependency/wildcard/warning/shell-syntax/diff-check all exit `0`.
- R09 panic, swallowed-error, production-size, and test-size expected-red gates all exit `1` and remain inherited fork-wide debt, not W5 regressions.
- Detached disposable worktree mutation proof restored only the buggy timeout call and the exact corrected test failed with exit `101` on `assertion failed: app.onboarding_import_failed_provider.is_none()`.
- `correction-run/RAW_SHA256SUMS` records uncompressed log SHA-256 values; `correction-run/SHA256SUMS` records the tracked compressed logs and metadata.

## Final independent correction reviews

Frozen W5 correction head `f42f79bfcd3c0ec27f839b0ccef54f4755d9d056` received two fresh independent read-only final reviews after the correction-run evidence was committed. Both returned **PASS** with zero IMPORTANT/CRITICAL findings and are preserved append-only under `../../reviews/`:

- Opus final correction review: `../../reviews/2026-07-16-w5-correction-final-opus.md`, SHA-256 `582b3d122a36e85ef60dfc76ad9f2c4d848d3c62791975ae2f82fae41c8806f5`, verdict **PASS**, zero IMPORTANT/CRITICAL findings.
- Fable final correction review: `../../reviews/2026-07-16-w5-correction-final-fable.md`, SHA-256 `d701676fd28fd82db90a285cab4c69810dce9977920dc5431d5230fbcde8f6bf`, verdict **PASS**, zero IMPORTANT/CRITICAL findings.

The reviews independently confirm the append-only correction posture, the discriminating timeout regression, the detached mutation proof exit `101`, the no-Nix/no-live-action evidence boundary, and the verified SHA manifests.
