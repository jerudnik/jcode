# Grant rename allowlist

This file records the W4-A1 classification boundary for the assignment-authority rename. Assignment-derived authority uses **grant**. The word **tier** remains valid for ranked concepts such as ambient action risk, subscription products, performance classes, and evaluation score bands.

`scripts/check_grant_vocabulary.py` enforces the legacy assignment-authority pattern `capability[_ ]?tier|CapabilityTier|SESSION_CAPABILITIES`. The checker scans tracked text files and permits the historical occurrences listed below. This file and the checker source are excluded because they must spell the legacy pattern to define the rule.

## Assignment-authority inventory

All hits in this table were classified as `authority` and renamed to grant vocabulary.

| File at classification time | Classified hits | Decision and action |
|---|---|---|
| `crates/jcode-app-core/src/tool/grant.rs` | Module name and docs; `CapabilityTier`; `CapabilityTierError`; `SessionCapability`; `SESSION_CAPABILITIES`; install, clear, rename, binding, authorization, and unit-test identifiers; refusal text | `authority`: file renamed to `tool/grant.rs`; types, storage, functions, fields, variables, tests, and model-visible refusal text renamed to grant vocabulary. |
| `crates/jcode-app-core/src/tool/mod.rs` | Module declaration, authorization call, assignment-authority comments, `tier_blocked` lifecycle label | `authority`: renamed to the grant module and `grant_blocked`. The adjacent ambient action tier call remains unchanged and now states the boundary. |
| `crates/jcode-app-core/src/server/comm_control.rs` | Assignment install and scoped-clear calls, derived tier variables, stale-tier event labels | `authority`: renamed to grant calls, variables, comments, and `SWARM_GRANT` / `stale_grant_replaced` log vocabulary. |
| `crates/jcode-app-core/src/server/client_disconnect_cleanup.rs` | Session capability cleanup | `authority`: renamed to session grant cleanup. |
| `crates/jcode-app-core/src/server/client_session.rs` | Two session capability rename calls | `authority`: renamed to session grant rename calls. This current-HEAD hit was absent from the expected plan table. |
| `crates/jcode-app-core/src/server/comm_graph.rs` | Terminal task capability cleanup | `authority`: renamed to assignment grant cleanup. |
| `crates/jcode-app-core/src/server/comm_session.rs` | Stopped-session capability cleanup | `authority`: renamed to session grant cleanup. |
| `crates/jcode-app-core/src/server/comm_control_tests/client_attached_dispatch.rs` | Test name, binding lookup, type, authorization and cleanup calls, fixtures, assertion text | `authority`: renamed mechanically to grant vocabulary. This current-HEAD test file was absent from the expected plan table. |
| `crates/jcode-app-core/src/tool/tests.rs` | Read-only and verify capability-tier registry tests, fixtures, calls, assertion text | `authority`: renamed mechanically. Ambient action tier tests in the preceding section remain unchanged. |

The renamed `Grant` type has no serialization derives or serde rename/tag attributes. `SESSION_GRANTS` remains an in-memory map, so W4-A1 does not change a wire or disk name.

## Ambient action tiers

These hits are `ambient-tier`: they rank unattended action risk and keep **tier**.

| File or family | Classified hits | Action |
|---|---|---|
| `crates/jcode-app-core/src/tool/ambient/ambient_gate.rs` | `ActionTier`, `TIER_GATE_EXEMPT`, `tier_refusal`, `check_ambient_action_tier`, tier-1/tier-2 docs | Keep. The module comment now distinguishes risk tiers from assignment grants. |
| `crates/jcode-app-core/src/tool/mod.rs` | `check_ambient_action_tier` dispatch boundary and safety-tier comment | Keep. The neighboring assignment-grant comment names the distinction. |
| Ambient runner/tool tests and `docs/SAFETY_SYSTEM.md` | Ambient tier gate behavior and current safety documentation | Keep. These describe ranked action risk rather than worker authority. |

## Required TUI classification

The four plan-named TUI files contain no legacy assignment-authority pattern at current HEAD. Their plain `tier` hits are `other-concept` and stay out of this chunk.

| Current file | Observed concept | Decision and reason |
|---|---|---|
| `crates/jcode-tui/src/tui/app/tests/onboarding_eval.rs` | Tier 0 through Tier 10 evaluator score bands for coverage, flow, screen quality, efficiency, accessibility, and related rubrics | `other-concept`: ranked evaluation dimensions, not assignment authority. Keep unchanged. |
| `crates/jcode-tui/src/tui/app/auth.rs` | Jcode subscription tiers and model minimum subscription tier | `other-concept`: commercial subscription/access catalog, not assignment authority or OAuth grant flow. Keep unchanged. |
| `crates/jcode-tui/src/tui/app/turn.rs` | “low-resource tiers” controlling redraw cadence | `other-concept`: runtime performance/resource class. Keep unchanged. |
| `crates/jcode-tui/src/tui/ui_viewport.rs` | `PerformanceTier` policy gates for decorative and prompt-entry animation | `other-concept`: runtime performance/animation policy. Keep unchanged. |

## Historical occurrences allowed by the checker

These documents are historical issue narratives and are not rewritten by W4-A1. The checker pins their exact matching lines, so adding or changing a legacy occurrence requires an explicit allowlist update.

- `docs/issues/capability-tier-deferred-gaps.md`: four matching lines, covering the historical title, heading, and old module path references.
- `docs/issues/swarm-runaway-growth.md`: three matching lines describing the historical capability-tier proposal and enforcement gap.
