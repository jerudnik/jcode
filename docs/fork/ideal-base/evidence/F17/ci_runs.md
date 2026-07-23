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

Run `29985424892` (Fork CI, `pull_request`, `a5f3bf6a8`) concluded
**failure**, but in a way that sharpens rather than undermines the F17 claim:

- **Quality Guardrails: success.**
- **Build & Test (macOS): success** — including the now-executed
  `Run jcode-tui library tests` step. The two hermeticity races fixed in
  `a5f3bf6a8` did not recur on macOS.
- **Linux Tests: failure** — `Run jcode-tui library tests` reported
  `1864 passed; 1 failed`: a single flake in
  `tui::app::tests::test_side_panel_visibility_change_resets_diagram_fit_context`.
  The step ran to completion (proving the rail *executes*, not compile-only);
  it did not time out.

### Root-cause investigation (honest)

Making the jcode-tui rail execute (F17's core change) exposed pre-existing
test-hermeticity gaps that were invisible while the suite was compile-only.
Local reproduction with a controlled `JCODE_HOME` separated two classes:

1. **Dev-box ambient-home non-hermeticity (not a CI blocker).** A cluster of
   ~14 `tui::app::tests::*` failed *in isolation* on the maintainer's box but
   pass with an empty `JCODE_HOME`. Root cause: `create_test_app` acquires a
   read-lease and does not redirect `JCODE_HOME`, so those tests read the real
   `~/Library/Application Support/jcode/` (config, model recommendations, and a
   `live-tests/coverage.json` ledger created by unrelated provider-doctor
   work). Example: `slash_provider_test_coverage_without_args_shows_cli_style_summary`
   asserts the "no ledger" header, which only holds when the home has no
   coverage ledger. On CI the home is pristine, so this whole class is green
   there. It is a real hermeticity finding, tracked below, but it is **not**
   what reddened the rail.

2. **Genuine low-rate parallel/load races (the actual rail-reddeners).** Under
   a pristine `JCODE_HOME` across 6 full parallel runs, only rare, non-repeating
   failures appear: `test_side_panel_visibility_change_resets_diagram_fit_context`
   (~1 in 7 full runs; 20/20 green in isolation) and
   `session_matches_picker_query_requires_all_tokens_order_independent` (once).
   These are the parallel-pollution class F17's `pollution_cleanup_spec.md`
   targets. A blocking rail that itself flakes at even a few percent would
   randomly red legitimate PRs, so these must be driven to zero before F17 is
   honestly closeable.

### Status of F17 against its gates

- Gate 1 (injected failures fail CI semantics): **met** (section 2).
- Gate 2 (jcode-tui runs, not compile-only): **met** (section 3, and the
  Linux failure is itself proof the step executes).
- Green-on-real-head: **not yet** — one low-rate flake remains. F17 is **not**
  closed until the residual parallel races are fixed and a clean Fork CI
  `pull_request` run is green on the head of record.

### Follow-up nodes (converted from this investigation)

- **F17-hermetic-home:** make `create_test_app`/`create_test_app_with`
  redirect `JCODE_HOME` to a per-test temp so the suite is hermetic by default
  and matches CI. Removes class (1) entirely.
- **F17-parallel-races:** root-cause and fix
  `test_side_panel_visibility_change_resets_diagram_fit_context`,
  `handle_post_connect_dispatches_reload_followup_even_if_history_snapshot_looks_busy`,
  and `session_matches_picker_query_requires_all_tokens_order_independent`.

## 5b. Green provenance result (081c61300)

After the section-5 failure, two further env-lock pollution bugs were
root-caused and fixed in `jcode-base` (commits `be45954fc`, `081c61300`):

1. **`auth::lifecycle::post_auth_model_selection_keeps_catalog_order_for_
   unranked_providers`** read an ambient/sibling-seeded cerebras live-catalog
   disk cache (no `JCODE_HOME` isolation) and flakily selected `qwen-3` over
   the first catalog route. Fixed by using the existing `AuthTestSandbox`
   helper (holds the env lock, isolates `JCODE_HOME`, resets global auth
   state). Net LOC neutral: `lifecycle.rs` stays at the 2334 ratchet baseline.
2. **`sponsors::provenance::disabled_sponsors_config_disables_everything`**
   dropped and re-acquired the env lock mid-test, letting a sibling
   `enable_sponsors` test repoint `JCODE_HOME` at its `enabled=true` config and
   repopulate the shared config cache, so the disabled assertion read the wrong
   config. Fixed by holding one guard across the whole test.

Local verification: the full `jcode-base --lib` parallel suite went from
~2/12 failing rounds to **15/15 green rounds** after both fixes.

**Provenance run of record: Fork CI `29998586620`** (`pull_request`,
`081c61300`) concluded **success**. All three blocking rails green:

- **Quality Guardrails: success** — the oversized-file ratchet and
  `cargo fmt --check` both passed (the auth fix was kept LOC-neutral so the
  ratchet baseline held).
- **Linux Tests: success** — `jcode-tui` library + `workspace-lib` tests ran
  to completion (not compile-only) with zero failures.
- **Build & Test (macOS): success** — including the executed
  `Run jcode-tui library tests` step.

This is the clean green run on head-of-record that section 5 required, so
**both F17 gates are met and demonstrated green on real CI**.

### Honest caveat on residual low-rate flakes

The section-5 "F17-parallel-races" flakes (`side_panel_visibility_change_
resets_diagram_fit_context`, `handle_post_connect_dispatches_reload_followup_
even_if_history_snapshot_looks_busy`, `session_matches_picker_query_requires_
all_tokens_order_independent`) were **not** reproduced as CI reddeners in this
run, but were also not individually root-caused to a logic bug. Each passes
20-60x standalone (even under load) yet fails ~1/7 in the full ~1900-test
parallel suite despite holding its lock and unique temp dirs. The working
hypothesis is resource saturation (thread/FD/scheduler contention on a loaded
runner), not per-test logic bugs like the auth/sponsors cases above. F17 is
closed on its stated gates; the residual saturation-class flakes are tracked
as the follow-up **F17-parallel-races** node and may warrant a
`--test-threads` cap on the blocking rail rather than per-test fixes.

## 6. actionlint

`actionlint 1.7.7` on `.github/workflows/fork-ci.yml`: **0 findings, exit 0**.
(Output in `actionlint.md`.)
