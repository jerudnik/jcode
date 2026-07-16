# R08A Operator input and command semantics: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light`, escalated at the dangerous-consent boundary |
| Research budget | `6 decisive checkpoints, exhausted without expansion` |
| Recommended disposition | `defer` |
| Confidence | `high` for the consent escalation and cancel-handoff boundary; `medium` for the unexecuted interactive fixture |

R08A owns operator input parsing, keymap interpretation, cancel/interrupt intent, slash-command-to-server intent, and the local selection/abort semantics of dangerous confirmations. It excludes R08B rendering and operator feedback, R01 identity/reload target truth, R02 credential/provider truth, R03A compatibility policy, R04/R12 terminal and cancellation outcome truth, R07A tool-consent enforcement, and all backend mutation authority. The source tree was read-only. No live daemon, credential, external application network call, stash/ref operation, destructive action, or publication was used.

## Six decisive checkpoints

| # | Finding | Evidence and deterministic reproduction | Consequence |
|---:|---|---|---|
| 1 | The fixed comparison inputs resolve and their merge base is the recorded base. | `git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b` returned `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`. | R00 bounds every conclusion to reproducible refs and forbids treating upstream provenance as authority. |
| 2 | Operator-input code is materially two-sided, requiring a semantic decision rather than a file-level adoption. | From the merge base, upstream changes `input.rs` `276+/40-`, `commands.rs` `115+/114-`, and remote key handling `23+/9-`; fork changes `274+/42-`, `181+/117-`, and `172+/14-`. Direct fork/upstream comparison is `239+/36-` across those three files. | No wholesale upstream or fork file adoption is justified. The light ledger must retain only bounded intent semantics. |
| 3 | The ordinary cancel gesture has one operator meaning in local and remote paths, while outcome remains backend truth. | Local `input.rs:2101-2116,2214-2255` maps Ctrl-C/Ctrl-D or Escape while processing to `cancel_requested`, clears local interleave/soft-interrupt state, and never declares a terminal result. Remote `remote/key_handling.rs:601-608,2545-2570` maps the same gestures to `cancel_with_reason("keyboard_ctrl_c_or_d"|"keyboard_escape")`. `git diff --unified=0 802f69098 7ff4fc6be` over both handlers has no cancel/interrupt/Escape hunk. | Retain the shared intent mapping. R08A can promise a request, not that work stopped or evidence completed. |
| 4 | The login-import confirmation auto-selects a dangerous action after 60 seconds, so neither fixed ref is an acceptable authority for that consent decision. | Base, upstream, and fork each contain `onboarding_flow_control.rs` timeout logic: `decision_timed_out` calls `onboarding_finish_import_review()` with the comment “import every currently-checked login” (`fork:1567-1585`; same semantic markers at base/upstream). `onboarding_flow.rs:32-36` sets `DECISION_TIMEOUT = 60s`; `ImportReview::approved_indices` returns checked candidates (`:212-219`), and `onboarding_finish_import_review` passes them to `external_auth::run_external_auth_auto_import_candidates` (`onboarding_flow_control.rs:738-795`). The test source explicitly says all candidates are pre-checked and default action imports all (`app/tests/onboarding_flow.rs:88-113`). | **Escalate to full review before any onboarding/external-auth import is exercised.** Matching fork/upstream behavior is evidence of shared behavior, not a safety verdict. |
| 5 | The wire and server own cancellation execution, ordering, and terminal truth, not the key handler. | `jcode-protocol/src/wire.rs:141-143` defines `Request::Cancel`; remote `backend.rs:822-836` sends it. Server `client_lifecycle.rs:937-944` deliberately signals cancellation before its Ack, then dispatches `cancel_processing_message` at `:1101-1114`; server tests include `cancel_without_local_task_still_signals_session_control` and detached-stream cancellation. | R08A must not render “Interrupted” as success or add another cancellation policy. R04/R12 own liveness/terminal outcome and R06A owns durable evidence. |
| 6 | The bounded pilot can avoid R08A, while R09 debt and incident review do not clear the consent defect. | `RESPONSIBILITIES.md:72-88` makes R08 non-prerequisite unless the chosen stack exercises it and stops the pilot before real credentials or UI/platform behavior. Keyword review of the maintenance incidents found no operator-input incident that supersedes source evidence; `audit-orthogonality-2026-07-14.md` (SHA-256 `a472b43ecaed612514bdb7d5fcb1547222f41fc4f58118a05df519b8f5637747`) concerns debug-control authorization, not this input flow. No current R09 ledger/quality-gate entry was located for `input.rs`, `commands.rs`, remote key handling, or keybinds. | This is not a blocker for the no-UI, no-credential pilot, but it is a blocker for any pilot or release path that opens onboarding or imports external credentials. Any future R08A source debt remains visible and attributed under R09 with no `--update`. |

## Authority and supported disposition

`defer` is the sole supported disposition. The fork retains local/remote cancel intent and extends command routing, but its dangerous-consent timer is shared with upstream. There is no authority basis to declare expiry an affirmative consent. A full review must decide a fail-closed interaction contract, including explicit confirmation, Escape/decline behavior, timeout behavior, and the boundary between R08A selection semantics and R02's credential import enforcement.

This is an escalation, not a request to change source in this lane. The ledger does not infer whether an import succeeds, which credential route is selected, whether a tool is allowed, or whether a cancellation has reached a terminal state.

## Pilot relevance, fixture boundary, and cross-seam contract

- **Pilot relevance:** R08A is `smoke only` and R08 is not a prerequisite unless the selected stack exercises it. The bounded pilot must remain no-UI, no-onboarding, and no-credential. If it emits a TUI key, uses a slash command that changes session work, or enters onboarding/import, it must first satisfy the escalation below.
- **Deterministic no-network fixture:** a future isolated harness must construct the input state directly and assert: Ctrl-C/Ctrl-D and Escape emit exactly one cancel intent when processing; the same gestures only request quit/clear input when idle; slash input is parsed before an outbound intent; and import review never calls `run_external_auth_auto_import_candidates` without an explicit current-turn affirmative confirmation. Candidate test surfaces are `jcode-config-types/src/keybindings.rs` and `jcode-tui/src/tui/app/tests/onboarding_flow.rs` (`import_review_collects_checked_logins`, `import_review_decision_timer_counts_down_and_times_out`, and decline-all tests).

  ```bash
  JCODE_HOME=$(mktemp -d) bash scripts/dev_cargo.sh test -p jcode-config-types --lib -- --test-threads=1
  JCODE_HOME=$(mktemp -d) bash scripts/dev_cargo.sh test -p jcode-tui-core --lib keybind -- --test-threads=1
  # Full-review fixture required before onboarding/import use:
  JCODE_HOME=$(mktemp -d) bash scripts/dev_cargo.sh test -p jcode-tui --lib onboarding_flow -- --test-threads=1
  ```

  The first two compact commands were attempted with a disposable `JCODE_HOME` and timed out at 300 seconds while waiting for the shared Cargo target-directory lock, before test execution. This is an explicit execution defer, not a test pass or semantic failure. The full TUI onboarding fixture is intentionally deferred to the required full review. None requires a live daemon, provider credential, or external network when a local build is available.
- **R02:** R08A may collect a selected/declined candidate set only. R02/external-auth owns credential discovery, trust persistence, validation, provider selection, and any credentials that could be read. The full review must prove selection expiry cannot authorize a read.
- **R07A/R07C:** tool/network consent and telemetry consent are not key-handler truth. R08A must not treat a displayed prompt or a command string as permission for an external side effect.
- **R04/R12/R06A:** cancel is an intent. R04/R12 decide interruption and terminal state; R06A records evidence. R08A cannot manufacture a `Done`, `Interrupted`, or persisted result.
- **R08B and R03B:** rendering/status is R08B and transport delivery is R03B. R08A’s contract is stable mapping from an operator gesture to intent, including a local decline/abort.
- **R09:** no existing scoped red-debt entry was located. Before a source change, rerun applicable trusted gates without `--update`, publish changed counts, and assign any red delta to R08A.

## Negative findings and gaps

- No direct fork/upstream diff hunk changes Ctrl-C/Ctrl-D, Escape, or cancel/interrupt behavior in the two reviewed local/remote key handlers. This supports the narrow cancel mapping only, not broader input equivalence.
- No live daemon, terminal, remote client, credential, onboarding import, provider call, tool, or network activity was exercised.
- The attempted no-network keybinding tests did not begin because of a Cargo target-directory lock. No test result is claimed.
- The no-UI pilot avoids this consent path by policy. That avoidance does not retire the consent defect for onboarding, releases, or any credential-import experience.
- The review did not enumerate every slash command or render state. R08B owns rendering; command business effects remain with their owning backend seams.

## Acceptance, rollback, and escalation

- **Acceptance or retirement condition:** this ledger can be accepted only as a conditional no-UI pilot boundary. It retires into a full R08A/R02 joint review before onboarding/import is exercised, with a deterministic fixture proving timeout and Escape are fail-closed and selected credentials are never read without a contemporaneous explicit approval.
- **Rollback or stop condition:** do not modify source in this lane. Stop rather than broaden if validation needs a live daemon, credential, external import, network/tool action, a quality-baseline update, a backend policy decision, or more than this six-checkpoint budget.
- **Escalate to full review if:** **already triggered** by the 60-second timeout importing every pre-checked external login. It also triggers if a cancel gesture is shown as terminal before backend evidence, local/remote mappings diverge, a command causes a side effect without explicit owned consent, or the pilot adds any UI/onboarding/credential path.
- **Coordinator approval:** pending escalation disposition. The no-UI pilot boundary is recorded, but dangerous onboarding consent is not approved.
- **Fable review:** pending independent Phase 4 architecture review.

## 2026-07-15 W0 approval amendment

Coordinator approval: **PASS only as a `defer` light record and no-UI boundary**. The independent five-ledger review is [`../../reviews/2026-07-15-remaining-light-ledgers-opus-review.md`](../../reviews/2026-07-15-remaining-light-ledgers-opus-review.md), SHA-256 `b537bc5674fdb9385e60c2dd18a44db5e61ba4f57146cd57fbf91f7a58a8a55d`. Dangerous onboarding consent is not approved.

The stale Fable-pending line is discharged by corrected Phase 4 Fable plan SHA-256 `b0bae9803fa726a489e0560fdc423daefa20bd8478ede0aa2772f7684ea21eb9` and independent plan review SHA-256 `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92`. W5 begins with the mandated full R08A/R02 joint review before any source change or credential read. Timeout and Escape remain fail-closed acceptance requirements.

## 2026-07-16 W5 onboarding consent closure amendment

W5 closed the escalated onboarding import-consent defect in append-only history. The accepted source state is commit `b3b0103160883a5f5e6894d071e816bff92cccd1`, where the `Login { import: Some(_) }` decision-timeout branch no longer calls `onboarding_finish_import_review()` and instead fail-closes through `onboarding_handle_login_failed(None)`. This preserves R02/external-auth ownership: W5 did not change credential discovery, validation, persistence, provider selection, `external_auth.rs`, `auth.rs`, or any live import path.

Accepted deterministic evidence is `../../evidence/2026-07-16-w5-onboarding-consent/`. Its top-level accepted run used no Nix invocation, no `scripts/dev_cargo.sh`, no live onboarding, no credentials, no provider, no network, no daemon/reload, and no import. It used cached direct tools only: `/nix/store/iywn852j3pnz291ywvil7rxhibqn8953-rust-default-1.96.0/bin/{cargo,rustfmt}` and `/Library/Developer/CommandLineTools/usr/bin/python3`, with disposable `JCODE_HOME` and `JCODE_RUNTIME_DIR`. The before/after process snapshots show the same pre-existing `/tmp/nix-shell` ssh mux and no driver-created Nix/remote-builder process.

Fixture coverage:

- New timeout regression `import_review_timeout_fails_closed_without_import_task_transition`: exit `0`; proves timeout clears the import review to `Login { import: None }`, leaves `onboarding_import_in_progress == None`, records an import error through the shared failure helper, and then reaches the manual provider picker on Enter.
- Existing Escape regression `liveness_esc_always_exits_onboarding_from_every_guided_phase`: exit `0`.
- Existing decline-all regression `liveness_import_review_decline_all_then_enter_escapes`: exit `0`.
- Existing explicit affirmative positive control `import_summary_defaults_to_continue_and_enter_imports_all`: exit `0`.

Affected checks and R09:

- `cargo check -p jcode-tui`: exit `0`.
- Source-only rustfmt check for `onboarding_flow_control.rs`: exit `0`. The inherited-red test file was not whole-file rustfmted in accepted evidence.
- R09 classifier, dependency, wildcard, warning, shell syntax, and diff check: exit `0`.
- R09 panic, swallowed-error, production-size, and test-size gates: expected-red exit `1`, actual `1`; no `--update` was used and no W5 attribution change is claimed for inherited red debt.

Invalid/preaccepted evidence attempts are preserved under `../../evidence/2026-07-16-w5-onboarding-consent/invalid-unsafe-driver/` with hashes. They include earlier `scripts/dev_cargo.sh`, plain/safe-wrapper Nix, and pre-cleanup direct-tool attempts and are retained for audit only, not accepted validation.

Remaining review need: an independent Opus/spot-check review should verify the final base-relative source/test diff, the accepted no-Nix evidence package, and the invalid-attempt preservation. No R02 credential-validation semantics or external-auth internals were changed.

## 2026-07-16 W5 adjudication correction: IMPORTANT evidence failure

Coordinator adjudication after W5 evidence packaging found that W5 must **not** integrate at `dfe5d1ec4b359ea68d956eab9feaa62399e29618`. Two final reports are preserved exactly under reviews:

- Spot-check PASS report: `../../reviews/2026-07-16-w5-final-spot-check-hippo.md`, SHA-256 `fcf57921ac3c8d3d9669181340320aea3eef5423eefc6a2b4fffb45fd824eb10`.
- Opus PASS report: `../../reviews/2026-07-16-w5-final-opus-skunk.md`, SHA-256 `e219acf6202ee34d39d6ad0384beb7d0dd96bb4919036d6a6775a790f5125c39`.

Despite its PASS label, the Opus report states that restoring the buggy timeout call to `onboarding_finish_import_review()` still let the timeout regression pass. That mutation result contradicts the PASS conclusion for the evidence. W5 therefore records an **IMPORTANT evidence failure**: the source direction is still fail-closed, but the timeout test at `dfe5d1ec4` did not discriminate the buggy no-runtime path from the fixed direct failure-helper path.

Correction requirements are append-only: keep existing history, make a separate test-only correction to the single timeout regression, prove the corrected test fails when only the buggy timeout call is restored in a disposable detached worktree, and rerun accepted direct-tool no-Nix evidence in a new `correction-run/` package. No production source, R02 credential-validation semantics, `external_auth`, live import, credentials, provider, daemon, network, Nix, or `dev_cargo.sh` may be used.

## 2026-07-16 W5 adjudication correction closure

W5 correction is now append-only over the rejected evidence head. Source state still preserves the intended net-zero production fix: import-review timeout calls `onboarding_handle_login_failed(None)` rather than `onboarding_finish_import_review()`. The separate test-only correction commit `95861f4f5f354dbb3123c19754ac1ca1d13083ac` strengthens the single timeout regression to assert the discriminating provider/error state:

- `onboarding_import_failed_provider.is_none()`.
- `onboarding_import_error.as_deref() == Some("We couldn't import those logins.")`.

New accepted evidence is under `../../evidence/2026-07-16-w5-onboarding-consent/correction-run/`. It preserves the no-live/no-Nix/no-`dev_cargo` safety boundary and used only cached direct Cargo/rustfmt plus system Python. Four fixtures, affected TUI check, source-only rustfmt, and expected R09 matrix all matched expected exits. A detached disposable mutation proof restored only the buggy timeout call and the exact corrected test failed with exit `101`, on the new provider assertion, proving the regression now discriminates the previous unsafe default-import path.

## 2026-07-16 W5 final independent correction reviews

Frozen W5 correction head `f42f79bfcd3c0ec27f839b0ccef54f4755d9d056` received two fresh independent read-only final reviews after the correction-run evidence package:

- Opus final correction review: `../../reviews/2026-07-16-w5-correction-final-opus.md`, SHA-256 `582b3d122a36e85ef60dfc76ad9f2c4d848d3c62791975ae2f82fae41c8806f5`, verdict **PASS**, zero IMPORTANT/CRITICAL findings.
- Fable final correction review: `../../reviews/2026-07-16-w5-correction-final-fable.md`, SHA-256 `d701676fd28fd82db90a285cab4c69810dce9977920dc5431d5230fbcde8f6bf`, verdict **PASS**, zero IMPORTANT/CRITICAL findings.

Both reviews confirm the corrected W5 posture: timeout silence fails closed through `onboarding_handle_login_failed(None)`, the single strengthened timeout regression kills the restored buggy timeout call with mutation exit `101`, and the accepted correction evidence remains within the direct-tool no-Nix/no-live-action boundary.
