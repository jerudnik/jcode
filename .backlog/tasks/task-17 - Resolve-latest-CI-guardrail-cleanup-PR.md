---
id: TASK-17
title: Resolve latest CI guardrail cleanup PR
status: Done
assignee:
  - '@jcode-agent'
created_date: '2026-05-18 04:41'
updated_date: '2026-05-28 02:44'
labels:
  - upstream
  - owner-interest
dependencies: []
references:
  - 'https://github.com/1jehuang/jcode/pull/212'
priority: medium
ordinal: 17000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Upstream #212 is owner-authored and currently open. Reconcile the CI guardrail cleanup with current master, determine whether it is still needed, and either land equivalent fixes or close as obsolete.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Current CI guardrail failures are identified and either fixed or marked obsolete
- [x] #2 Backlog notes record whether PR #212 was superseded, merged, or replaced
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Self-improvement loop:
1. Inspect upstream PR #212 and current CI guardrail files to identify intended cleanup.
2. Compare current dev/master state for whether those guardrail failures are already addressed or obsolete.
3. Apply any still-needed equivalent fixes with focused validation.
4. Record superseded/merged/replaced conclusion in final summary, preserve PR #212 reference, commit, push, and rebuild if code changed.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
PR #212 is still open upstream but stale: its listed guardrail edits are already present or superseded in current dev, while the branch lacks newer compaction/provider work.

Current live guardrail failures found during reconciliation: cargo fmt --check failed on jcode-storage formatting and full cargo clippy --all-targets --all-features -- -D warnings failed on newer clippy lints across current code. Applied minimal mechanical fixes.

Validation passed: cargo fmt --check; cargo check --all-targets --all-features; cargo clippy --all-targets --all-features -- -D warnings; python3 scripts/validate_agent_content.py; selfdev build target=tui.

Ratchet scripts still fail on broad pre-existing baseline drift unrelated to PR #212: code-size, test-size, panic, and swallowed-error budgets require separate baseline/cleanup reconciliation.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Reconciled upstream PR #212 (Satisfy latest CI guardrails) against current dev.

Conclusion:
- PR #212 is effectively superseded/replaced. It remains open upstream, but its original browser setup purpose was already landed on master, and the guardrail style changes it carried are already present or stale relative to current dev.
- Current dev had newer CI guardrail failures not covered by #212. This change fixes the active rustfmt/clippy failures instead of replaying the stale PR branch.

Changes:
- Applied rustfmt-required formatting in jcode-storage.
- Fixed current stable-clippy findings across terminal launch, context pruning, auth/cursor, ACP init, live coverage helpers, setup hints, open-tool helpers, copy selection, process title, and related tests.
- Converted now-unfulfilled clippy expectations to current allow/removal where appropriate.

Validation:
- cargo fmt --check
- cargo check --all-targets --all-features
- cargo clippy --all-targets --all-features -- -D warnings
- python3 scripts/validate_agent_content.py
- selfdev build target=tui

Notes:
- Ratchet budget scripts still fail due broad pre-existing baseline drift unrelated to PR #212 and should be handled in a dedicated cleanup/baseline task.
- Upstream PR reference preserved: https://github.com/1jehuang/jcode/pull/212
<!-- SECTION:FINAL_SUMMARY:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Regression or validation added where applicable
- [x] #2 Upstream issue/PR reference preserved in final notes
<!-- DOD:END -->
