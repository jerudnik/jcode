# F17 evidence index (POINTER)

Node F17: make Linux, macOS, app-core, and deterministic TUI test rails
blocking (CI semantics) on push/PR, and run jcode-tui rather than compile-only.

Head of record: `a5f3bf6a8` (branch `ci-validation`, PR #16 -> `main`,
repo `jerudnik/jcode`).

| File | What it proves |
|---|---|
| `ci_runs.md` | Run ledger; failure-injection rail-coverage table; green provenance; honest branch-protection caveat. |
| `workflow_diff.md` | Human-readable summary of the advisory->blocking change. |
| `workflow_diff.patch` | Raw `origin/main..a5f3bf6a8` diff of `fork-ci.yml`. |
| `actionlint.md` | actionlint 1.7.7 clean (exit 0) on the workflow. |
| `failure_injection_plan.md` | The plan the injection branches implemented. |
| `pollution_cleanup_spec.md` | Parallel-test pollution spec (hermeticity work). |
| `assignment.md` | Original F17 assignment brief. |

Key run IDs:
- Provenance (of record): `29985424892` Fork CI pull_request @ a5f3bf6a8.
- Injection RED: `29981845808`, `29981846809`, `29981847785`.
- Organic RED credit: `29931842057`.
- Superseded provenance: `29981592316` (flaky test, fixed in a5f3bf6a8).

Related commits:
- `a5f3bf6a8` test(jcode-tui): close two parallel-suite hermeticity races (F17).
- Workflow rails: see `workflow_diff.patch` for the blocking promotion.
