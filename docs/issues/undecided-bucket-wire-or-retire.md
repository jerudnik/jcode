---
title: "Audit: 88 unwired scripts, 16 dormant guards, and 21 undischarged commitments have no decision attached"
status: open
priority: high
owner: maintainers
opened: 2026-08-28
related:
  - scripts/preflight.sh
  - scripts/check_guard_nonvacuity.py
  - docs/issues/fork-health-false-green.md
---

# The undecided bucket: machinery that is neither wired nor retired

A four-way read-only audit (2026-08-28, four independent agents: script
wiring, guard registry, execution surfaces, docs commitments) measured how
much machinery exists in the repository without anything executing it and
without a recorded decision to keep or kill it. The ratchet retirement merged
in PR #213 removed four instances of this pattern; this issue holds the rest,
so each item gets an explicit wire-or-retire decision instead of ambient rot.

## 1. Script wiring (145 scripts audited)

25 WIRED, 32 INDIRECT, 88 UNWIRED. The load-bearing findings:

- `scripts/preflight.sh`: **WIRE** through the required `just pre-pr` local
  gate. The recipe uses the PR path classifier to run docs-only checks alone
  or add preflight plus the broader local checks for non-docs changes.
- Red dormant checks (fail today, nothing notices):
  `check_ambient_roots.sh` (seven moved call sites plus stale allowlist),
  `check_web_mobile.sh` (machine-local path baked in),
  `check_branch_handoff.py`, `count_blank_user_turns.py`,
  `test_oauth_usage.py`, and `check_warning_budget.sh` outside a Nix shell.
- Green orphan guards: **WIRE** all four through `scripts/preflight.sh`, which
  is executed by `just pre-pr`: `check_env_lease_drop_order.py` (1197 files
  clean), `check_tui_render_lock.py` (31 locked, 0 unlocked),
  `check_wildcard_reexport_budget.py` (13 vs baseline 16), and
  `check_config_env_lease.py` (148 keys clean).
- Passing test modules stranded under `scripts/` where the wired
  `tests/test_*.py` glob cannot collect them, including
  `test_docs_references.py`, `test_warning_budget.py`,
  `pipeline_tests.py`, and `test_benchmark_discovery.py`.

## 2. Guard registry

`check_guard_nonvacuity.py` carries 33 entries: 17 gating, 16 dormant.
`security_preflight` is gating but explicitly lacks a plant, so its
non-vacuity is asserted, not demonstrated. Mechanisms recorded in the
decision log as intended but never implemented: the security plant, the
scheduled end-to-end mutation canary, disabled-model policy enforcement
(docs/issues/swarm-model-policy-enforcement.md), swarm capability tiers,
hard cwd enforcement (only a soft signal exists), and the role-aware
liveness watchdog.

## 3. Execution surfaces

- `.github/workflows/build-matrix.json:3-9` claims to be authoritative and
  lists `aarch64-darwin` as built, but `nix.yml:101-104` builds only
  `x86_64-linux`, and `tests/test_nix_distribution_policy.py:454-460` trusts
  the JSON instead of parsing the workflow. This is a live false-green: a
  test certifying a build that does not happen.
- `security.yml:39-41` has a non-strict branch no in-repo caller can reach
  (both callers pass `strict: true`). Dead or intentionally external, but
  undocumented either way.
- `pr.yml:37` cites the deleted `docs/issues/classifier-shadow-precedes-its-guard.md`.
- `tests/e2e/expand_badge_headed_wtype.py` and
  `tests/e2e/expand_badge_headless.py` sit outside the CI collection glob;
  the headed one has no caller at all.
- Clean: all 7 justfile recipes wired, all 10 flake checks have live
  subjects.

## 4. Documented commitments never discharged (21 found)

Recorded follow-ups with no implementation or closure evidence, by doc:

- `docs/AGENT_NATIVE_VCS_CORE_BEHAVIOR.md:336`: concrete schemas for the
  drafted VCS concepts. Nothing beyond the doc exists.
- `docs/MEMORY_ARCHITECTURE.md:807`: sleep-like memory consolidation still
  "TODO - Design pending"; runtime only backfills embeddings.
- `docs/SERVER_LIFECYCLE_INVARIANTS.md:55`: spawner heartbeat for abandoned
  daemon cleanup.
- `docs/ASSISTANT_PROFILE_PERSONAS.md:119`: the deferred live stance
  comparison was never run.
- `docs/agent-workflows.md:171-172`: self-hosted macOS and Linux runners
  named nowhere but in that paragraph.
- `docs/architecture/W1_STORE_SCHEMA_COMPARISON.md:216`: retire the separate
  coordinators map; it remains a live HashMap with hundreds of references.
- `docs/architecture/GOVERNANCE_DECISIONS.md:379`: cross-process
  last-writer-wins protection for background-task fields; only process-wide
  mutexes exist (`background/store.rs`).
- `docs/architecture/GOVERNANCE_DECISIONS.md:409-410`: mcp-serve owner
  liveness vs PID reuse; single-owner ECHILD reaping. Neither exists.
- `docs/architecture/provider-confusion.md:103,109,119,123`: live-catalog
  spawn resolution, resolved identity in completion reports, narrowing
  `set_model`'s fail-open branch, and the subagent tool still inheriting
  provider key blindly (`tool/subagent.rs:64-70`).
- `docs/issues/swarm-model-policy-enforcement.md:89`: disabled models are
  still pickable, spawnable, and inheritable.

The remaining six commitments live inside already-open issue files
(`swarm-runaway-growth.md:129-131` capability tiers, cwd enforcement at
tool-call time, role-aware watchdog; `batch-same-file-edit-race.md:35`;
`fork-health-false-green.md:47` RULESET_AUDIT_TOKEN lifecycle;
`cross-session-content-leakage.md:153`). Those are tracked where they are
and are listed here only for the count.

## What resolution looks like

Not a mass fix. Each item above gets one of three verdicts, recorded where
the item lives: WIRE (name the gate that will run it), RETIRE (delete it and
note the loss, the PR #213 pattern), or KEEP-MANUAL (record that it is an
operator tool and who runs it). The false-green items in section 3 and the
red dormant checks in section 1 should be decided first, because they
actively misinform. An unwired green guard is latent waste; a wired false
claim is active harm.
