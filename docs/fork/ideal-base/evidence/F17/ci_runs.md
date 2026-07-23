# F17 CI evidence: run ledger and rail-coverage proof

Node: **F17** — "Make intended Linux, macOS, app-core, and deterministic TUI
test rails blocking on push/PR."

Acceptance gates (from `WORK_GRAPH.json`):
1. Injected failures in each rail fail CI semantics.
2. jcode-tui deterministic tests run rather than compile-only.

Repository: `jerudnik/jcode`. Workflow: `.github/workflows/fork-ci.yml`.
Head of record: **`a5f3bf6a8`** (branch `ci-validation`, PR #16 → `main`).

## 1. What "blocking" means here (honest scope)

F17's contract defines blocking as **CI semantics**: an injected regression in
a rail causes that rail's job to conclude `failure`, surfacing a red check on
the push/PR. This is proven below.

`main` is **not** branch-protected on this fork (`GET
/branches/main/protection` → 404 "Branch not protected"), so there is no
server-side merge block wired to these checks. That is out of F17's scope
(F17 owns the workflow rails, not repo settings) and is recorded here so the
claim is not overstated: the rails **fail the run**, they do not **mechanically
prevent merge**.

Only the `Latest stable canary (advisory)` job carries
`continue-on-error: true`. Every rail asserted below lives in a job with no
job-level `continue-on-error` and no step-level `continue-on-error`.

## 2. Failure-injection proof (gate 1)

Three throwaway branches each injected one regression class, dispatched via
`workflow_dispatch`. Every injected rail concluded `failure`:

| Injected regression | Job | Failing step | Run |
|---|---|---|---|
| bad formatting + app-core test | Quality Guardrails | Check formatting (blocking for fork-touched files) | 29981845808 |
| bad formatting + app-core test | Build & Test (macOS) | Run jcode-app-core library tests | 29981845808 |
| bad formatting + app-core test | Linux Tests | Run jcode-tui library tests | 29981845808 |
| provider-matrix / targets break | Quality Guardrails | Check all targets and all features | 29981846809 |
| provider-matrix / targets break | Build & Test (macOS) | Compile integration test binaries | 29981846809 |
| provider-matrix / targets break | Linux Tests | Run jcode-tui library tests | 29981846809 |
| e2e break | Quality Guardrails | Clippy (blocking for fork-touched files) | 29981847785 |
| e2e break | Build & Test (macOS) | Run e2e tests | 29981847785 |
| e2e break | Linux Tests | Run e2e tests | 29981847785 |

Aggregate rail coverage demonstrated RED:
- Quality: `Check formatting`, `Check all targets and all features`, `Clippy`.
- macOS: `Run jcode-app-core library tests`, `Compile integration test
  binaries`, `Run e2e tests`.
- Linux: `Run jcode-tui library tests`, `Run e2e tests`.

Organic RED credit (pre-injection, real regression on an earlier head):
- `29931842057` — Fork CI `pull_request` on `5cfe70580` failed
  `Run workspace library tests` and `Run jcode-tui library tests`, independently
  confirming those two rails execute and can fail the run.

Injection branches to delete after this ledger is committed:
`ci-proof/f17-inject-quality-app-core`,
`ci-proof/f17-inject-provider-matrix`,
`ci-proof/f17-inject-e2e`.

## 3. jcode-tui runs, not compile-only (gate 2)

The workflow diff (`workflow_diff.patch`, `origin/main..a5f3bf6a8`) adds, on
**both** macOS and Linux jobs:

```
- name: Run jcode-tui library tests
  run: |
    python3 .github/scripts/run_with_timeout.py 1800 \
      cargo test --target <triple> -p jcode-tui --lib
```

Previously jcode-tui was compile-only (`--no-run`) on both rails. The
injection runs above show this step reaching real test *execution* and failing
on a seeded regression, which a compile-only step could not do.

## 4. Green provenance run (gate: rails pass on the real head)

- **Superseded:** `29981592316` — Fork CI `pull_request` on `68a5ecbf5` →
  `failure`. Root cause: a single flaky macOS `jcode-tui` test
  (`render_system_message_uses_scheduled_task_card`) racing on
  `TERM_PROGRAM`/`TERM`. Not a product defect; a test-hermeticity gap, which is
  itself inside F17's charter.
- **Fix:** commit `a5f3bf6a8` closes two parallel-suite hermeticity races
  (env-lock guards on two scheduled-task reader tests; Full-tier policy pin on
  the remote-startup redraw test). Verified locally: 5 back-to-back full
  parallel `jcode-tui` rounds, `1867 passed; 0 failed`, under load avg ~5.
- **Provenance run of record:** `29985424892` — Fork CI `pull_request` on
  `a5f3bf6a8`. Result recorded in section 5 once complete.

## 5. Provenance result (a5f3bf6a8)

_To be filled from run 29985424892 on completion._

## 6. actionlint

`actionlint 1.7.7` on `.github/workflows/fork-ci.yml`: **0 findings, exit 0**.
(Output in `actionlint.md`.)
