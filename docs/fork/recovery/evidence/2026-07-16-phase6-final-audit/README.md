# Phase 6 final coordinator audit evidence

## Fixed state

- Accepted source head: `51168d16e9c708ae4afff09a6fc6402642d17782` on `recovery/2026-07-15`.
- Merge base and `vendor/upstream`: `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`.
- Sole dirty path: the user-controlled `docs/fork/recovery/ORCHESTRATOR_PROMPT.md` edit, diff SHA-256 `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`.
- Exactly four stashes remained untouched.
- Direct cached Cargo/rustc from `/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin` were used with `CARGO_NET_OFFLINE=true`. No Nix command, `scripts/dev_cargo.sh`, network, provider, credential, daemon/reload, release, installer/updater, profile mutation, or remote builder was invoked.

## Accepted cross-seam floor

The accepted driver ran 62 distinct expected-exit checks. Its TSV manifest has
76 physical lines because 14 embedded multi-line Python commands each add one
continuation line. All 62 checks matched their expected exit.

- `jcode-build-support --lib`: 48 passed.
- `jcode-protocol --lib`: 81 passed; `PROTOCOL_VERSION = 1`.
- R02 `subscription`: 38 passed.
- R02 provider filter family: 4 passed.
- R04 authoritative exact matrix: 14 commands, each exactly 1 passed and 0 failed, with a fresh disposable `JCODE_HOME` and `JCODE_RUNTIME_DIR` per fixture.
- R12 `r12_`: 11 passed.
- `cargo check`: `jcode-build-support`, `jcode-protocol`, `jcode-base`, `jcode-app-core`, `jcode-storage`, and `jcode-tui` passed.
- R09 trusted greens passed: 17 classifier tests, dependency boundaries, wildcard count 16, warning count 0, shell syntax, and diff check.
- R09 expected-red debt stayed visible: panic `31 -> 48`, swallowed-error `2987 -> 3074`, production-size red, and test-size red. Every expected red exited 1. No baseline update was used.
- Fixed refs, branch, prompt hash, sole-dirty-path, stash count, no-active-build, before/after process equality, and final status checks passed. The same pre-existing SSH mux appeared before and after; no remote builder appeared.

## Independent static reconciliation inputs

- All 17 non-deferred seam ledgers exist.
- All 17 tracked recovery evidence `SHA256SUMS` manifests verify from their owning directories.
- Phase 5 introduced no `crates/jcode-protocol` diff relative to approved source head `6c6a4f2c8`, no new `provider_session_id` assignment, and no production identity writer outside R01. The two new runtime-identity assignment tokens are test-only in `recovery_pilot_tests.rs`; terminal token additions are test-only or test-name-only.
- Deferred risks remain enumerated in `RECOVERY_PLAN.md` section 4 with owner, reason, evidence gap, and escalation trigger.

## Preserved invalid attempt

`invalid/historical-r02-count-guard/` preserves the first final-audit attempt. Its R02 suite passed 38 tests, but the historical count guard expected 35 and stopped the attempt. It is not accepted evidence.

## Reproduction

```bash
cd docs/fork/recovery/evidence/2026-07-16-phase6-final-audit
shasum -a 256 -c SHA256SUMS
accepted/verify_raw.sh
invalid/historical-r02-count-guard/verify_raw.sh
```
