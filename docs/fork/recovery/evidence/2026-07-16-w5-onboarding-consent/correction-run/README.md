# W5 onboarding consent correction-run evidence

Correction run HEAD: `95861f4f5f354dbb3123c19754ac1ca1d13083ac`.
Corrects rejected evidence head: `dfe5d1ec4b359ea68d956eab9feaa62399e29618`.
Branch: `recovery/fix-w5-onboarding-consent-2026-07-16`.
Base: `566d7930606f96add92aed65564c95b539a03df0`.

## Why this append-only run exists

Coordinator adjudication rejected integration at `dfe5d1ec4` because the prior timeout regression also passed when the timeout branch was locally restored to the buggy `onboarding_finish_import_review()` call. The source fix remained the intended net-zero fail-closed branch, but the test evidence was insufficiently discriminating.

This correction-run records the strengthened single timeout regression and proves it fails against the restored buggy timeout call.

## Safety constraints

- No live onboarding, credential, provider, network, daemon/reload, import, or external action was used.
- No Nix command and no `scripts/dev_cargo.sh` were invoked by accepted correction evidence.
- Direct cached tools only:
  - `/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin/cargo`
  - `/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin/rustfmt`
  - `/Library/Developer/CommandLineTools/usr/bin/python3`
- `CARGO_NET_OFFLINE=true`, `JCODE_NO_TELEMETRY=1`, `FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`.
- `JCODE_HOME` and `JCODE_RUNTIME_DIR` were disposable temp directories and cleaned by the driver.

## Result counts

| Section | Expected | Actual |
|---|---:|---:|
| Corrected timeout fixture | 0 | 0 |
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
| Detached buggy-timeout mutation proof | nonzero | 101 |

No command used `--update`. The unchanged R09 expected-red gates remain inherited fork-wide debt, not W5 regressions.

## Mutation proof

`mutation-proof/` is a disposable detached-worktree proof. The only source mutation was restoring the timeout branch to the buggy call:

```diff
-                    // Silence is not consent to import credentials.
-                    self.onboarding_handle_login_failed(None);
+                    // Timeout default: import every currently-checked login.
+                    self.onboarding_finish_import_review();
```

`mutation-proof/buggy-timeout-only.paths` contains only `crates/jcode-tui/src/tui/app/onboarding_flow_control.rs`. The exact corrected test returned `101`, failing on `assertion failed: app.onboarding_import_failed_provider.is_none()`. That proves the strengthened regression discriminates the buggy no-runtime import path from the fixed direct `onboarding_handle_login_failed(None)` path.

## Manifests

Raw `.log` files were deterministically gzip-compressed after validation to keep `git diff --check` green. `RAW_SHA256SUMS` records SHA-256 of each uncompressed log, including the mutation proof log. `SHA256SUMS` records tracked compressed logs, exits, metadata, driver, README, and mutation-proof artifacts.
