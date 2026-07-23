# F17 evidence index (POINTER)

Node F17: make Linux, macOS, app-core, and deterministic TUI test rails
blocking (CI semantics) on push/PR, and run jcode-tui rather than compile-only.

Head of record: `86acc7ae1` (branch `ci-validation`, PR #16 -> `main`,
repo `jerudnik/jcode`). This is the commit of the fully-green Fork CI run
(30023191393, 22m50s) with the serial jcode-tui rails and the mermaid
deferred-worker synchronization fix. The earlier "green" run at `081c61300`
was a non-deterministic pass: the same suite flaked RED on `55757322a`
(`test_side_panel_visibility_change_resets_diagram_fit_context`), exposing an
async pollution race that `--test-threads=1` alone did not close. `86acc7ae1`
fixes the race at its source (synchronous render mode in tests) and caps the
jcode-tui rails at one thread; validated 0/80 serial rounds on a Linux repro
box vs the clean tree reproducing the exact flake (1/40).

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
- Provenance (green, of record): `30023191393` Fork CI pull_request @ 86acc7ae1.
- Prior green (non-deterministic, superseded): `29998586620` @ 081c61300.
- Flake recurrence (RED, motivated the real fix): jcode-tui @ 55757322a.
- Prior provenance (RED, root-caused): `29985424892` @ a5f3bf6a8.
- Injection RED: `29981845808`, `29981846809`, `29981847785`.
- Organic RED credit: `29931842057`.
- Superseded provenance: `29981592316` (flaky test, fixed in a5f3bf6a8).

Related commits:
- `86acc7ae1` fix(F17): synchronous render mode + serial jcode-tui rails (green).
- `081c61300` test(jcode-base): fix two env-lock pollution races.
- `be45954fc` test(auth): isolate JCODE_HOME in unranked-provider test.
- `a5f3bf6a8` test(jcode-tui): close two parallel-suite hermeticity races (F17).
- Workflow rails: see `workflow_diff.patch` for the blocking promotion.
