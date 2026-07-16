# W4 R02 route closure evidence

This directory preserves W4's resumed evidence-only closure for R02 provider route composition at the existing source head.

## Non-authoritative preserved attempts

- `642651535l`: invalid concurrency interruption. It used `scripts/dev_cargo.sh`, re-entered the Nix dev shell because `cargo` was not on `PATH`, and was coordinator-terminated with exit `143` before any test result completed. The original ignored `commands.log` remains append-only and untracked; `raw/invalid-interrupted-commands-log.txt.gz` is a deterministic gzip copy.
- `7967177mwt`: invalid toolchain attempt. It used direct cached tools but paired split Cargo/Rust paths without a visible standard-library sysroot, failing before test execution with `E0463` and exit `101`.
- `848872a5nf`: superseded false-positive no-update guard. The exact route test, cargo check, and R09 expected exits passed, but `no_update_guard` scanned arbitrary checker stdout and found inherited advisory text mentioning `--update`; `final_status` did not run, so no result is counted from this attempt.

## Product-truth audit artifact

`reviews/product-truth-audit-crocodile.md` is copied from `/tmp/crocodile_1784183153541_240726e5ac746626-final.md`.

Expected SHA-256: `c6581850509a0d7925b91175f50f48b24633d8fec9f81c44f7882ce48c44d54b`.

## Accepted validation

Accepted validation is background task `383680ygrb`, attempt directory `/tmp/jcode-w4-r02-route-closure-20260716T075303Z-accepted`. It used the cached combined Rust toolchain directly:

- `PATH=/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin:/usr/bin:/bin:/usr/sbin:/sbin`
- `CARGO_NET_OFFLINE=true`
- disposable `JCODE_HOME` and `JCODE_RUNTIME_DIR`
- telemetry/nudge guards in `accepted-run-validation.sh`
- no Nix command and no `scripts/dev_cargo.sh`

Key accepted results from `accepted-validation-manifest.tsv`:

| Check | Expected | Actual |
|---|---:|---:|
| exact catalog route test | 0 | 0 |
| one-passed nonzero filter | 0 | 0 |
| `cargo check -p jcode-base` | 0 | 0 |
| no Rust diff before/after | 0 | 0 |
| non-allowed path filter | 1 | 1 |
| classifier | 0 | 0 |
| dependency | 0 | 0 |
| panic ratchet | 1 | 1 |
| swallowed-error ratchet | 1 | 1 |
| code-size ratchet | 1 | 1 |
| test-size ratchet | 1 | 1 |
| wildcard re-export | 0 | 0 |
| warning budget | 0 | 0 |
| shell syntax | 0 | 0 |
| diff check | 0 | 0 |
| no-update invocation guard | 0 | 0 |
| final status | 0 | 0 |

The no-update invocation guard inspected only executed command metadata and the accepted driver content, not arbitrary checker stdout. Raw logs are deterministic gzip files under `raw/accepted-*`.

## Claim limits

No Rust source changed. No subscription catalog/API, governance, baseline, protocol, prompt, daemon, provider/account, network, external provider, Nix, or `dev_cargo` action is part of the accepted run. W4 closes as evidence-only route-composition confirmation.

## Final independent reviews

Both independent reviews of frozen W4 head `db1e48d26c25a9379feab7a7dd9207ed521517fe` returned PASS with zero IMPORTANT and zero CRITICAL findings.

| Review | Preserved artifact | SHA-256 | Verdict |
|---|---|---|---|
| Opus | `reviews/w4-final-opus-review.md` | `28984d90d1749945a1f19fad6e3ee5949640012a352d6e26e68fbfd70849aab9` | PASS; zero IMPORTANT/CRITICAL findings |
| Fable | `reviews/w4-final-fable-review.md` | `62e247e180774c6640facb284b6a696b64b2cbf9a6a3a99857ecc9b704561b56` | PASS; zero IMPORTANT/CRITICAL findings |

Both files were copied byte-for-byte from `/tmp/w4-final-opus-review.md` and `/tmp/w4-final-fable-review.md` respectively (verified with `cmp -s`).
