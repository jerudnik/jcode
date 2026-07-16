# W5 Onboarding-Consent Correction: Final Independent Review (fable)

Date: 2026-07-16. Reviewer: fable (verify). Read-only.
Repo: `/Users/jrudnik/labs/jcode-w5-consent`, frozen HEAD `f42f79bfcd3c0ec27f839b0ccef54f4755d9d056` (confirmed, worktree clean, 0 dirty paths).
Base `566d79306`, rejected evidence head `dfe5d1ec4`, corrected test `95861f4f5`, evidence head `f42f79bfc`. All ancestry confirmed append-only (`566d79306` -> `dfe5d1ec4` -> `28df534b7` -> `95861f4f5` -> `f42f79bfc`).

## Verdict: PASS

Zero IMPORTANT or CRITICAL findings.

## Behavioral verification

1. **Timeout silence fails closed via the direct generic helper.** At HEAD, the `Login { import: Some(_) }` decision-timeout branch (`onboarding_flow_control.rs:1580-1582`) calls `self.onboarding_handle_login_failed(None)` and returns. It does not call `onboarding_finish_import_review()` and cannot reach the import spawn. `onboarding_handle_login_failed(None)` clears the import sub-state to `Login { import: None }`, clears `onboarding_import_in_progress`, and sets the exact generic error `"We couldn't import those logins."` (`:1445-1449`).
2. **Explicit affirmative remains the only import transition.** `onboarding_finish_import_review()` has exactly one call site (`:600`), reachable only from the review key handler on Enter/Space/`y` with `finished = true`. Grep confirms no other caller in the crate.
3. **Escape remains fail-closed.** The universal Esc hatch (`:383-406`) clears `onboarding_import_in_progress`/`onboarding_import_error` and exits onboarding without importing; covered by passing fixture `liveness_esc_always_exits_onboarding_from_every_guided_phase`.
4. **Decline-all remains fail-closed.** `approved.is_empty()` returns before any spawn (`:753-758`); covered by passing fixture `liveness_import_review_decline_all_then_enter_escapes`.

## Discrimination of the corrected test

The single corrected test `import_review_timeout_fails_closed_without_import_task_transition` (test-only commit `95861f4f5`, +5/-1 over the rejected version) now asserts state that provably differs under the buggy path:

- Buggy path (`onboarding_finish_import_review()` on timeout): the fixture candidate is `CodexLegacy`, whose `telemetry_auth_labels()` deterministically yields `("openai", "import")` (`jcode-app-core/src/external_auth.rs:120`), so `onboarding_import_failed_provider` is set to `Some("openai")` (`:767-774`) before the no-runtime fallback fires. `onboarding_handle_login_failed` never clears that field, so `assert!(app.onboarding_import_failed_provider.is_none())` fails. Additionally, the no-runtime fallback passes a specific reason, so the error string would be `"The import could not start (no async runtime)."`, failing the exact-string assertion too. Double discrimination.
- Fixed path (`onboarding_handle_login_failed(None)`): the provider field is never touched (stays `None`) and the error is exactly the generic string. Both assertions hold.

This directly satisfies the adjudication contract in `docs/fork/recovery/reviews/2026-07-16-w5-adjudication-evidence-failure.md` (minimum: `failed_provider.is_none()` plus exact generic error).

## Mutation proof

- `correction-run/mutation-proof/buggy-timeout-only.diff` restores only the buggy timeout call (single 2-line swap plus comment, inverse of the fix). `buggy-timeout-only.paths` contains only `crates/jcode-tui/src/tui/app/onboarding_flow_control.rs`.
- `worktree.meta` records a disposable detached worktree at `95861f4f5` (`/tmp/jcode-w5-buggy-worktree.xDbT9u`), consistent with compile paths inside the log.
- `buggy-timeout-test.exit` = `101`. The decompressed log shows the exact corrected test failing at `onboarding_flow.rs:317:9` with `assertion failed: app.onboarding_import_failed_provider.is_none()`, matching line 317 of the committed test file. This is the exact test, exact assertion, and exact predicted failure mode.

## Evidence validity

- **Fixtures exactly 1/1.** Correction-run timeout, Escape, decline-all, and explicit-affirmative logs each show `1 passed; 0 failed; ... 1874 filtered out`, all `.exit` = `0`. The top-level accepted run also shows 1/1 for all four.
- **Affected checks.** `affected_tui_check.exit` = 0 with log ending `Finished dev profile`; `rustfmt_source_control.exit` = 0 with byte-empty log (hash `e3b0c44...` = empty).
- **R09 matrix internally consistent.** Green gates (classifier, dependency, wildcard, warning, shell-syntax, diff-check) all exit 0; expected-red gates (panic, swallowed, code-size, test-size) all exit 1 with logs attributing debt to files W5 never touched (e.g. `ui_messages.rs`, `cli/dispatch.rs`). No `--update` appears in either driver.
- **Deterministic hashes consistent.** `shasum -c SHA256SUMS` passes 100% in the top-level, `correction-run/`, and `invalid-unsafe-driver/` directories. Every `RAW_SHA256SUMS` entry (top-level 18/18, correction-run 19/19 including the mutation-proof log, invalid 10/10) matches the SHA-256 of the decompressed `.gz` content.
- **Safety boundary respected.** Both drivers use only the cached store cargo/rustfmt and CLT python, `CARGO_NET_OFFLINE=true`, disposable `JCODE_HOME`/`JCODE_RUNTIME_DIR`, `nix_invocations=none`; unsafe-pattern scan of both drivers found no Nix/dev_cargo/network commands. Process before/after snapshots show only the same pre-existing ssh mux (pid 70674). Timestamps are ordered and consistent (correction run 07:52:21Z at head `95861f4f5` 07:43:11Z, snapshot-after 08:00:27Z, evidence commit 08:02:38Z).
- **Prior contradictory PASS preserved and excluded.** Both reports exist with SHA-256 exactly matching the adjudication record (`fcf57921...` hippo, `e219acf6...` skunk). The adjudication doc and the R08A ledger amendment explicitly classify `dfe5d1ec4` as an IMPORTANT evidence failure, not an approval; the top-level README states `correction-run/` supersedes the top-level run for integration.
- **Unsafe attempts preserved but excluded.** `invalid-unsafe-driver/` retains the preaccepted mixed attempts with verified hashes and a README declaring them audit-only.

## Scope cleanliness

`git diff 566d79306..f42f79bfc --name-only` outside `docs/` touches exactly two files: `onboarding_flow_control.rs` (net-zero 2-line swap) and `tests/onboarding_flow.rs` (one added test). No changes to `external_auth` (crate-wide diff empty), R02 semantics, protocol, baselines, prompts, or Cargo manifests. All docs changes are under `docs/fork/recovery/` (W5 evidence, reviews, R08A ledger). `git diff --check` over the full range is clean. `95861f4f5..f42f79bfc` contains zero crate changes (docs/evidence only).

## Minor observations (non-blocking, not IMPORTANT)

1. `invalid-unsafe-driver/preaccepted-mixed-attempts-20260716T071326Z/` exists on disk as an empty untracked directory (git does not track empty dirs); the README's "moved idempotently" note explains it. Cosmetic only.
2. The mutation proof is preserved as artifacts (meta, diff, paths, exit, log) but its command sequence is not scripted in `driver.sh`. The artifacts are internally consistent (worktree path matches compile lines, binary fingerprint `jcode_tui-f2edac99536d66c0` matches the correction run), so this is a reproducibility nicety, not an evidence gap.
3. `onboarding_handle_login_failed` does not clear `onboarding_import_failed_provider`, so a provider from a prior failed attempt could persist across a later timeout. Irrelevant to this test (field starts `None`) and to the fail-closed guarantee; noted for future hygiene.

## Validation performed

Git ancestry/range audits, full base-relative and per-commit diffs, call-site greps for the import transition, provider-label derivation trace through `jcode-app-core`, decompression and hash verification of all three manifests plus all RAW manifests, exit-code cross-checks against README expected tables, driver unsafe-pattern scans, mutation-proof log/diff/line-number cross-verification against the committed test, and preserved-report SHA-256 verification against the adjudication record.

## Untested surfaces

Per the read-only/no-build constraint I did not execute cargo tests myself; conclusions about runtime behavior rest on the recorded logs (hash-verified) plus static code analysis of both execution paths, which are unambiguous. The live async import path is untested by design (no-credential boundary), as documented.

**Result: PASS. Zero IMPORTANT/CRITICAL findings.**
