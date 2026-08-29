---
title: "Audit: unwired scripts, dormant guards, and false execution claims need decisions"
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
without a recorded decision to keep or kill it. This issue now holds only the
remaining script, guard, and execution-surface findings.

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
- WIRE: `tests/test_docs_references.py`, `tests/test_warning_budget.py`,
  `tests/test_pipeline.py`, and `tests/test_benchmark_discovery.py` now live
  under `tests/`, where `just test-python` runs them through the existing
  `tests/test_*.py` glob. No test coverage was retired.

## 2. Guard registry

`check_guard_nonvacuity.py` carries 33 entries: 17 gating, 16 dormant.
`security_preflight` is gating but explicitly lacks a plant, so its
non-vacuity is asserted, not demonstrated. Mechanisms recorded in the
decision log as intended but never implemented: the security plant, the
scheduled end-to-end mutation canary, swarm capability tiers, hard cwd
enforcement (only a soft signal exists), and the role-aware liveness
watchdog. Disabled-model policy enforcement left this list when spawn-time
rejection landed in PR #229.

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

## What resolution looks like

Not a mass fix. Each item above gets one of three verdicts, recorded where
the item lives: WIRE (name the gate that will run it), RETIRE (delete it and
note the loss, the PR #213 pattern), or KEEP-MANUAL (record that it is an
operator tool and who runs it). The false-green items in section 3 and the
red dormant checks in section 1 should be decided first, because they
actively misinform. An unwired green guard is latent waste; a wired false
claim is active harm.
