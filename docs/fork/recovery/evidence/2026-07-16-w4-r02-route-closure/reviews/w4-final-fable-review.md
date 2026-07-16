# W4 R02 evidence-only closure: final independent review (Fable)

- Reviewer scope: read-only review of `/Users/jrudnik/labs/jcode-w4-r02` at HEAD `db1e48d26c25a9379feab7a7dd9207ed521517fe`, branch `recovery/close-w4-r02-compose-2026-07-16`.
- Declared anchors verified: base `566d79306` (parent of declaration), declaration `b14c34157`, interruption amendment `e17fe0cee`, evidence head `db1e48d26` = HEAD. Commit chain is exactly these three commits on top of the base.
- Verdict: **PASS**. Zero CRITICAL findings, zero IMPORTANT findings, four MINOR observations.

## 1. Product truth: existing `catalog_routes` behavior matches the closure claim

- `crates/jcode-base/src/provider/catalog_routes.rs:710` and `:727` guard OpenRouter fallback route creation with `openrouter::standard_catalog_lists_model(&or_model) != Some(false)` for Anthropic and OpenAI alternative routes. The same composed predicate appears on the provider-key-aware path at `:928-956`, keyed off `resolve_current_model_spec(model).provider_key` (`Some("claude")` / `Some("openai")`), so fork provider/profile identity and upstream definitive-absence suppression are composed in current code, exactly as the declaration states.
- `standard_catalog_lists_model` (`crates/jcode-base/src/provider/openrouter.rs:34-40`) returns `None` when no cache entry loads or when the model list is empty, and `Some(bool)` otherwise. Freshness is enforced upstream of it: `fresh_disk_cache` (`crates/jcode-provider-openrouter/src/lib.rs:370-378`) returns `None` for entries older than `CACHE_TTL_SECS`. Therefore suppression (`Some(false)`) can only arise from a fresh catalog that definitively omits the model; missing, stale, or empty catalogs yield `None` and stay optimistic. This matches "optimistic fallback for missing/unknown catalog; suppression only on definitive fresh absence."
- The exact targeted test `provider::catalog_routes::tests::openrouter_alternative_routes_skip_models_absent_from_catalog` (`catalog_routes.rs:1345-1378`) proves both sides: (a) no catalog cache, `gpt-5.3-codex-spark` gets an optimistic OpenRouter fallback route; (b) a fresh catalog listing `openai/gpt-5.3-codex` and `openai/gpt-5.5` but not spark suppresses the spark fallback while keeping the listed model's route. The test uses `EnvGuard` (temp `JCODE_HOME`, global test-env lock, `catalog_routes.rs:1168-1229`) and a directly written cache file with a current `cached_at`, so it is deterministic and offline.
- Product-truth non-adoption is anchored where the closure says it is: `docs/fork/recovery/RECOVERY_PLAN.md` §14 ("W4 product-decision amendment", upstream five-tier schema/prices/budgets/floors are not fork product truth) and `docs/fork/recovery/PROGRESS.md` 2026-07-16T04:31:30Z amendment. The W4 closure declares evidence-only confirmation and does not reopen tier governance; no subscription catalog/API/governance file changed.

## 2. Zero Rust changes are correct

- `git diff 566d79306..HEAD --name-only` lists exactly `docs/fork/recovery/seams/R02-config-provider-routing/ledger.md` plus files under `docs/fork/recovery/evidence/2026-07-16-w4-r02-route-closure/`. No `crates/` or `src/` file changed. `git diff --stat -- crates/` across the range is empty.
- The accepted run's `pre_status`/`final_status` logs show only `?? docs/fork/recovery/evidence/2026-07-16-w4-r02-route-closure/` (untracked evidence dir), i.e. no staged or unstaged tracked modification at run time.
- Run timing places the accepted run at source head `e17fe0cee`: `/tmp/...-075303Z-accepted/manifest.tsv` mtime 2026-07-16T03:53:39-0400, between the `e17fe0cee` commit (03:42:22) and the evidence commit `db1e48d26` (03:54:51), matching the ledger's "closed at source head e17fe0cee plus this documentation commit."

## 3. Accepted attempt `383680ygrb`: internally consistent

- `accepted-run-validation.sh` matches the ledger/README description exactly: `PATH` pinned to the combined nix-store toolchain `/nix/store/iywn852j...rust-default-1.96.0/bin` plus system dirs, `CARGO_NET_OFFLINE=true`, disposable `JCODE_HOME`/`JCODE_RUNTIME_DIR`, telemetry/nudge/hints guards, no Nix command and no `scripts/dev_cargo.sh` anywhere in the driver or manifest.
- All 29 manifest rows have `expected == actual`, and raw logs corroborate each row I spot-checked: exact test `EXIT: 0` with `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1171 filtered out` (run with `--exact`); one-passed grep exit 0; `cargo check -p jcode-base` exit 0; Rust-diff guards 0/0; non-allowed-path filter exit 1 (grep found nothing outside the allowed two paths); R09 ratchets red as expected (classifier 0, dependency 0, panic 1, swallowed 1, code-size 1, test-size 1, wildcard 0, warning 0, shell syntax 0, diff check 0); `rustc --print sysroot` resolves to the combined toolchain store path.
- The accepted `no_update_invocation_guard` scans only `manifest.tsv` and `run-validation.sh` (executed command metadata and driver content), constructs the forbidden token without embedding it, and masks only its own construction line. Result `NO_UPDATE_INVOCATION_HITS 0`, exit 0. This is a sound narrowing relative to the superseded attempt's guard, since the manifest records every executed command in the run. `final_status` ran and exited 0. The README's claim about the guard scope is accurate.
- Fidelity to originals: every `raw/accepted-*.txt.gz` decompresses byte-identical to its `/tmp/jcode-w4-r02-route-closure-20260716T075303Z-accepted/raw/*.txt` original; `accepted-validation-manifest.tsv` and `accepted-run-validation.sh` are byte-identical to the attempt-dir copies.

## 4. Hashes and manifest integrity

- `shasum -a 256 -c SHA256SUMS` passes for all 57 entries. The 57 hashed paths plus `SHA256SUMS` itself equal the 58 tracked files in the evidence directory; nothing tracked is unhashed and nothing hashed is missing.
- All gzip raw logs have zeroed mtime fields (deterministic gzip), as claimed.
- `reviews/product-truth-audit-crocodile.md` hashes to the declared `c6581850...4d54b` and is byte-identical (`cmp`) to `/tmp/crocodile_1784183153541_240726e5ac746626-final.md`.

## 5. Invalid/interrupted/superseded attempts: preserved and excluded

- Interrupted `642651535l`: untracked `commands.log` exists, is ignored by the user-global `*.log` rule (verified via `git check-ignore -v`), and its content confirms the ledger narrative: command `env CARGO_NET_OFFLINE=true scripts/dev_cargo.sh test -p jcode-base --lib provider::catalog_routes::tests -- --nocapture`, dev-shell re-entry banner, SIGTERM mid-compile, `EXIT: 143`, no test result. `raw/invalid-interrupted-commands-log.txt.gz` decompresses byte-identical to `commands.log`. `interruption.txt` records `result_counted=false`.
- Failed toolchain `7967177mwt`: manifest and raw logs preserved byte-identical to `/tmp/...-074316Z`; split cargo/rustc store paths visible in `PATH`; exact test failed with `E0463: can't find crate for std`, `EXIT: 101`, before any test execution. Excluded (`result_counted=false`).
- Superseded `848872a5nf`: manifest and all raw logs preserved byte-identical to `/tmp/...-074408Z-combined`. Its broader `no_update_guard` scanned checker stdout and hit 4 files (`panic.txt`, `code_size.txt`, `swallowed.txt`, `test_size.txt`), each of which indeed contains the inherited ratchet advisory "Run scripts/check_*.py --update only after intentional cleanup." Manifest ends at `no_update_guard 0 1`; `final_status` is absent (driver `run_expect` aborts on mismatch), matching `final_status_ran=false` and `result_counted=false`. Notably its exact test and all other checks matched expectations, so superseding it in favor of a clean rerun is conservative, not result-shopping.
- No other attempt directories exist in `/tmp` beyond the three plus two pointer entries; the attempt inventory is complete.

## 6. Declared paths and claim limits

- Path boundary is exact: the declaration (`b14c34157`) restricts W4 to the R02 ledger plus the new evidence directory; the actual three commits touch exactly those paths and nothing else. `RECOVERY_PLAN.md`, `PROGRESS.md`, prompts, baselines, protocol, subscription catalog/API, and all Rust files are untouched, matching the "Claim limits" section.
- Claims are correctly bounded: W4 claims only evidence-only route-composition confirmation of already-present behavior plus documented tier non-adoption. No new behavior, adoption, or governance claim is made.

## Minor observations (non-blocking)

1. The `no_rust_diff_before/after` guards use pathspec `crates/**/*.rs crates/*.rs`, which does not cover root-level Rust (`src/cli/*.rs`, `src/bin/*.rs`). The gap is closed by `pre_status`/`final_status` (clean tracked tree) and by the commit-range diff, so the "No Rust source changed" claim still holds, but the guard name overstates its own coverage.
2. `changed_paths_allowed` runs `git diff --name-only` (unstaged tracked changes only); it would miss staged changes on its own. Again closed by the status checks.
3. The targeted test exercises the missing-cache flavor of "unknown" (`None`); the empty-model-list and stale-TTL flavors of `None` are code-verified (`openrouter.rs:36-38`, `lib.rs:370-378`) but not directly exercised by this test. The declaration's "proves both sides" claim is still fair for the optimistic-vs-definitive-absence contract.
4. The accepted manifests do not record the git commit SHA the run executed against; it is established only indirectly (file mtimes vs reflog plus clean status). Recording `git rev-parse HEAD` in future evidence drivers would remove this inference step.

## Verdict

**PASS.** Product truth, evidence validity, zero-Rust-change correctness, hash/manifest integrity, preservation/exclusion of all three non-authoritative attempts, and path/claim boundaries are all internally consistent with no contradictions or material evidence gaps.
