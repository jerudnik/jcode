# W5 onboarding consent full review, pre-fix

Date: 2026-07-16
Branch: `recovery/fix-w5-onboarding-consent-2026-07-16`
Base: `566d7930606f96add92aed65564c95b539a03df0`
Scope: W5 onboarding consent fail-closed review for `Login { import: Some(_) }` decision timeout.

## Verdict

**CRITICAL DEFECT REPRODUCED.** The current source treats silence on the import-review decision screen as consent to import credentials.

## Evidence

- Source location: `crates/jcode-tui/src/tui/app/onboarding_flow_control.rs:1575-1583`.
- The `onboarding_tick` phase match handles `Some(OnboardingPhase::Login { import: Some(_), .. })`.
- When `decision_timed_out` is true, the code calls `self.onboarding_finish_import_review()`.
- `onboarding_finish_import_review()` is the import-commit boundary. It collects approved candidates and can reach `crate::external_auth::run_external_auth_auto_import_candidates(...)` at `onboarding_flow_control.rs:790`.
- The import list defaults to every detected login checked, so the timeout path can import all candidates without an explicit affirmative action.

## Contrast with existing fail-closed behavior

Escape is already fail-closed for guided onboarding phases. The liveness test `liveness_esc_always_exits_onboarding_from_every_guided_phase` covers `Login{import:Some}` and asserts Escape reaches an escapable state while clearing any stale import progress flag. The reproduced defect is specific to the decision timeout branch, not Escape.

Decline-all is also intended to be synchronous and no-import. Existing tests cover `import_review_decline_all_falls_back_to_manual_login` and `liveness_import_review_decline_all_then_enter_escapes`, but they do not exercise the timed-out `onboarding_tick` branch.

## Required remediation

The timeout branch for `Login { import: Some(_) }` must fail closed. The minimal acceptable behavior is to decline/abandon the import review and fall back to the manual login prompt, preserving onboarding liveness without crossing the import task/spawn/credential-read boundary.

The fix must not change R02 credential-validation semantics, `external_auth` internals, or provider/import behavior. It should remain in `onboarding_flow_control.rs` and avoid growing unrelated code.

## Required deterministic tests

Add no-credential tests proving:

1. Timeout declines or abandons import review and does not call `onboarding_finish_import_review` or reach the external auto-import boundary.
2. Explicit Escape remains fail-closed and reaches no import boundary.
3. Decline-all reaches no import boundary.
4. Explicit affirmative still reaches the import boundary exactly once.

A direct observable is required. The preferred observable is an in-test counter/hook around the import boundary that does not read credentials or spawn import work.
