# W5 adjudication declaration: contradictory PASS is an evidence failure

Date: 2026-07-16
Branch: `recovery/fix-w5-onboarding-consent-2026-07-16`
Current adjudicated head before this correction: `dfe5d1ec4b359ea68d956eab9feaa62399e29618`

## Preserved reports

- Spot-check PASS report: [`2026-07-16-w5-final-spot-check-hippo.md`](./2026-07-16-w5-final-spot-check-hippo.md), SHA-256 `fcf57921ac3c8d3d9669181340320aea3eef5423eefc6a2b4fffb45fd824eb10`.
- Opus PASS report: [`2026-07-16-w5-final-opus-skunk.md`](./2026-07-16-w5-final-opus-skunk.md), SHA-256 `e219acf6202ee34d39d6ad0384beb7d0dd96bb4919036d6a6775a790f5125c39`.

## Adjudication

Although both reports are labeled PASS, the Opus report contains a mutation result proving the timeout regression at `dfe5d1ec4` also passed when the timeout branch was locally restored to the buggy `onboarding_finish_import_review()` call. That is internally contradictory to a PASS conclusion for the test evidence. The source fix remains directionally correct, but the regression test was insufficiently discriminating.

This is therefore an **IMPORTANT evidence failure**, not an integration approval. W5 must keep the existing append-only history, correct the single timeout regression, prove the corrected regression fails against the restored buggy timeout branch in a disposable detached worktree, and rerun accepted direct-tool no-Nix evidence into a new append-only correction-run evidence package.

## Required correction contract

- No production source change beyond the already accepted net-zero timeout branch state.
- One focused timeout regression only, with no inherited test-file rustfmt churn.
- The test must assert state changed by the buggy no-runtime import path but not by direct `onboarding_handle_login_failed(None)`: at minimum `onboarding_import_failed_provider.is_none()` and exact generic helper error `Some("We couldn't import those logins.")`.
- The fixture candidate maps deterministically to provider `openai`; the buggy path sets the failed provider and reports the no-runtime-specific import-start failure.
- Mutation proof must use a disposable detached worktree, restore only the buggy timeout call, and show the exact test returns nonzero with cached direct Cargo only. No live import, credentials, provider, daemon, network, Nix, or `dev_cargo.sh`.
