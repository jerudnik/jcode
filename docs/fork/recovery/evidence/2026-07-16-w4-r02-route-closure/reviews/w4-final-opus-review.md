# W4 R02 Evidence-Only Closure: Independent Review

**Verdict: PASS**

Scope: read-only independent review of W4 R02 route closure in `/Users/jrudnik/labs/jcode-w4-r02` at HEAD `db1e48d26`. Base before W4 `566d79306`; declaration `b14c34157`; interruption amendment `e17fe0cee`. No edits, commits, Nix/dev_cargo, network, accounts, providers, or daemons were used.

## Verification results

| Check | Result | Evidence |
|---|---|---|
| Only declared docs/evidence paths changed | PASS | `git diff --name-only 566d79306 HEAD` = 59 paths, all under `docs/fork/recovery/evidence/2026-07-16-w4-r02-route-closure/` and the R02 `ledger.md`. Range `b14c34157^..HEAD` identical scope. |
| Zero Rust diff | PASS | `git diff --stat 566d79306 HEAD -- '*.rs' '*.toml' '*.lock' 'Cargo.*'` is empty. |
| Existing test supports product truth | PASS | `catalog_routes.rs:1346` `openrouter_alternative_routes_skip_models_absent_from_catalog` genuinely asserts both sides: optimistic fallback with no cache (`:1353-1358`), suppression of catalog-definitively-absent model (`:1365-1370`), retention of catalog-listed model (`:1371-1376`). Composed-route claim is faithful. |
| Accepted manifest: 1/1 targeted fixture | PASS | `exact_catalog_route_test` exit 0; raw log shows `test result: ok. 1 passed; 0 failed`. `exact_catalog_route_test_one_passed` nonzero-filter exit 0. |
| Affected `jcode-base` check | PASS | `cargo check -p jcode-base` exit 0 in manifest and raw log. |
| All R09 expected exits | PASS | Raw exits match manifest: panic 1, swallowed 1, code_size 1, test_size 1 (red-but-visible); classifier 0, dependency 0, wildcard 0, warning 0, shell_syntax 0, diff_check 0. Consistent with ledger W4 statement. |
| Final status | PASS | `final_status` (git status --short) exit 0, shows only the untracked evidence dir. |
| No-update invocation guard | PASS | Guard scans only executed command metadata and driver content (`manifest.tsv`, `run-validation.sh`), constructs `--update` token via concatenation, exits 0. No literal `--update` invocation present in accepted manifest/script (`grep -c` = 0/0). |
| SHA256SUMS verifies from evidence dir | PASS | `shasum -a 256 -c SHA256SUMS` = 57/57 OK, zero failures. Self and ignored `commands.log` correctly excluded; all 57 tracked non-SHA files covered. |
| Invalid exit-143 attempt preserved, not counted | PASS | `interruption.txt`: task `642651535l`, exit 143, `result_counted=false`. Raw interrupted log shows SIGTERM (signal 15). `commands.log` untracked and git-ignored (`*.log`). |
| Failed-toolchain attempt preserved, not counted | PASS | `failed-toolchain-attempt.txt`: task `7967177mwt`, exit 101, `result_counted=false`. Raw shows `E0463` missing std sysroot from split cargo/rustc PATH. |
| Superseded false-positive preserved, not counted | PASS | `superseded-no-update-guard-attempt.txt`: task `848872a5nf`, guard scanned R09 checker stdout containing inherited `--update` advisory text, `final_status_ran=false`, `result_counted=false`. All four attempt task ids are distinct. |
| No baseline/protocol/governance/prompt changes | PASS | Range grep confirms none of RECOVERY_PLAN/PROGRESS/baseline/protocol/prompt/governance/rust/subscription changed in `b14c34157^..HEAD`. |
| Claim limits accurate | PASS | README/ledger claim only evidence-only route-composition confirmation, no Rust/catalog/API/governance/network. Crocodile audit SHA-256 `c658185...` matches; its RECOVERY_PLAN:286-290 and PROGRESS:181-192 non-adoption citations verified accurate (those product-truth sources predate W4 and are unchanged). |
| No contradictory PASS | PASS | No log presents a PASS conflicting with its recorded exit. Non-authoritative attempts are consistently marked `result_counted=false`. |

## Notes

- The accepted run (`383680ygrb`) used the cached combined toolchain `rust-default-1.96.0` directly with `CARGO_NET_OFFLINE=true`, disposable `JCODE_HOME`/`JCODE_RUNTIME_DIR`, and no Nix/dev_cargo/network/provider/daemon. Confirmed via raw log headers.
- The path-boundary guard (`changed_paths_allowed`) correctly exits 1 (no disallowed paths), preserved in raw and `post-guards.txt`.
- The interruption amendment (`e17fe0cee`) accurately records the procedural freeze and the invalid first attempt before any counted result, satisfying the append-only preservation requirement.

## Findings

No IMPORTANT or CRITICAL findings. No contradictions between claims and evidence.

**PASS.**
