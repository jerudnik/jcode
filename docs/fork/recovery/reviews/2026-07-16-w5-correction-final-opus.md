# W5 Onboarding-Consent Correction Final Review: **PASS**

Date: 2026-07-16
Repo: `/Users/jrudnik/labs/jcode-w5-consent`
Branch: `recovery/fix-w5-onboarding-consent-2026-07-16`
Frozen HEAD: `f42f79bfcd3c0ec27f839b0ccef54f4755d9d056` (verified, clean tree)
Base: `566d79306` | Rejected prior head: `dfe5d1ec4` | Correction test commit: `95861f4f5` | Correction evidence head: `f42f79bfc`

Read-only audit. No edits, commits, Nix, dev_cargo, network, accounts, providers, credentials, onboarding, import, daemon, or reload actions were performed.

**Zero IMPORTANT/CRITICAL findings.**

---

## Adjudication posture (contradictory prior PASS)

The preserved prior Opus PASS (`2026-07-16-w5-final-opus-skunk.md`) is treated as **non-authoritative and contradictory**: it self-reports that at `dfe5d1ec4` the timeout regression *also passed* when the timeout branch was restored to the buggy `onboarding_finish_import_review()` call. A mutation that survives is an evidence failure, correctly adjudicated as IMPORTANT in `2026-07-16-w5-adjudication-evidence-failure.md`. This review re-verifies the corrected slice independently and treats the contradictory PASS as FAIL.

---

## Source correctness

- **Timeout branch directly fails closed.** `onboarding_flow_control.rs:1579-1582`: on `decision_timed_out` in `Login { import: Some(_) }`, the code calls `self.onboarding_handle_login_failed(None)` (comment: "Silence is not consent to import credentials."). It no longer calls `onboarding_finish_import_review()`. Confirmed by direct read, not just diff.
- **Production net LOC = zero.** `git diff --numstat 566d79306 f42f79bfc` on `onboarding_flow_control.rs` = `2 2` (a 2-line swap). Production file does not appear in the R09 code-size growth list, confirming net-zero impact.
- **Path scope clean.** Only two code files changed across the range (`onboarding_flow_control.rs`, `tests/onboarding_flow.rs`); everything else is under `docs/fork/recovery/`. No files outside scope.
- **R02 / external_auth / protocol / baselines / prompt unaffected.** `external_auth.rs` unchanged (`crates/jcode-app-core/src/external_auth.rs` not in the diff). No protocol/baseline/prompt files touched.

## Test correctness and discrimination

- **Exactly one new focused timeout test.** `import_review_timeout_fails_closed_without_import_task_transition` added as pure addition (+40/-0); exactly one `#[test]` added; no modifications to existing test lines, no rustfmt churn survives (correction commit `95861f4f5` only strengthened two assertions within the new test).
- **Assertions genuinely discriminate the buggy no-runtime path:**
  - `assert!(app.onboarding_import_failed_provider.is_none())` — the fixture is `CodexLegacy` (`ExternalAuthReviewCandidate::fixture("OpenAI/Codex", ...)`), whose `telemetry_auth_labels()` returns `[("openai","import")]`. The buggy `onboarding_finish_import_review()` sets `onboarding_import_failed_provider = Some("openai")` (`:767`) and `onboarding_handle_login_failed` never clears it, so the buggy path leaves `Some("openai")`. The fixed direct `handle_login_failed(None)` path never sets it, leaving `None`.
  - `assert_eq!(app.onboarding_import_error.as_deref(), Some("We couldn't import those logins."))` — the fixed path (`reason: None`) yields the exact generic fallback. The buggy no-runtime path calls `handle_login_failed(Some("The import could not start (no async runtime)."))`, producing a different summarized string. Both assertions therefore fail on the buggy path.
  - Follow-through: `handle_onboarding_continue_prompt_key(Enter)` opens the interactive login picker, proving the fail-closed recovery hatch.

## Mutation proof (adversarial)

- `mutation-proof/buggy-timeout-only.diff` restores **only** the buggy call (`handle_login_failed(None)` -> `onboarding_finish_import_review()`); `buggy-timeout-only.paths` lists exactly `crates/jcode-tui/src/tui/app/onboarding_flow_control.rs`.
- `worktree.meta`: disposable detached worktree at `95861f4f5` (`/tmp/jcode-w5-buggy-worktree...`), separate target dir.
- `buggy-timeout-test.exit` = **101**; log shows the exact test `... FAILED` with `test result: FAILED. 0 passed; 1 failed`, panicking at `onboarding_flow.rs:317`: `assertion failed: app.onboarding_import_failed_provider.is_none()`. The restored buggy mutation is now genuinely killed, resolving the prior evidence failure.

## Accepted no-Nix evidence (correction-run)

Driver (`correction-run/driver.sh`) uses only cached store-path `cargo`/`rustfmt` (`rust-default-1.96.0`) and CLT `python3`, `CARGO_NET_OFFLINE=true`, disposable `JCODE_HOME`/`JCODE_RUNTIME_DIR`, `nix_invocations=none`. No `dev_cargo.sh`, no live/import/provider/daemon/reload action.

| Check | Expected | Actual |
|---|---:|---:|
| Corrected timeout fixture (exact new test) | 0 | 0 (1 passed) |
| Escape liveness fixture | 0 | 0 (1 passed) |
| Decline-all liveness fixture | 0 | 0 (1 passed) |
| Explicit affirmative fixture | 0 | 0 (1 passed) |
| Affected `cargo check -p jcode-tui` | 0 | 0 |
| Source-only rustfmt check | 0 | 0 |
| R09 classifier / dependency / wildcard / warning / shell-syntax / diff-check | 0 | 0 |
| R09 panic / swallowed / code-size / test-size (inherited fork-wide debt) | 1 | 1 |
| Detached buggy-timeout mutation | nonzero | 101 |

Each fixture log runs its correctly-named single test, 1 passed / 0 failed, `1874 filtered out`. R09 red gates enumerate growth across many unrelated crates (server, base, desktop, etc.), confirming pre-existing fork-wide debt, not W5 regressions; no `--update` used.

## Integrity

- **Process snapshots:** `process_before`/`process_after` identical except the timestamp line; the only listed process is the same pre-existing ssh mux (PID `70674`, `/tmp/nix-shell.../clan-ssh...[mux]`). No new nix/daemon/reload processes.
- **Manifests verify.** correction-run: `SHA256SUMS` 43/43 OK, `RAW_SHA256SUMS` 19/19 decompressed logs match (including the mutation-proof log). Top-level: `SHA256SUMS` 38/38 OK, `RAW_SHA256SUMS` 18/18 match.
- **History append-only.** Rejected head `dfe5d1ec4`, adjudication `28df534b7`, and the contradictory prior PASS reports are preserved unchanged; the correction is appended (`95861f4f5`, `f42f79bfc`). `invalid-unsafe-driver/` retains pre-protocol unsafe attempts as audit-only. `git diff --check` clean.

## Validation performed

git range/diff/numstat audit; direct source read of timeout branch, `handle_login_failed`, `finish_import_review`, and fixture provider mapping; test-diff pure-addition and single-`#[test]` check; mutation diff/paths/exit/log inspection; all four fixture logs; R09 expected-exit reconciliation; process before/after diff; full compressed + raw-decompressed SHA verification of both evidence packages; driver safety scan (cached bins, offline, no Nix/live actions); scope and external_auth/R02/protocol/baseline invariance.

## Untested surfaces (non-blocking)

- Live async import (`run_external_auth_auto_import_candidates`) under a real tokio runtime is not exercised by design (no-credential constraint). Discrimination now rests on two state-shape assertions that provably diverge between the buggy no-runtime import path (`failed_provider = Some("openai")`, runtime-specific error) and the fixed direct fail-closed path (`None`, exact generic error), and the mutation proof confirms the buggy path is killed.

## Verdict: **PASS** (High confidence)

The source is the intended net-zero fail-closed timeout branch, the single new regression test now genuinely discriminates the buggy path (mutation exits 101 on the `failed_provider.is_none()` assertion), all fixtures and affected checks pass with expected exits, no Nix/network/live action occurred, process state is undisturbed, both SHA manifests verify, and the contradictory prior history remains append-only. Zero IMPORTANT/CRITICAL findings.
