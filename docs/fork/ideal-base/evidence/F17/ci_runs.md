# F17 CI evidence: run ledger and rail-coverage proof

Node: **F17** — "Make intended Linux, macOS, app-core, and deterministic TUI
test rails blocking on push/PR."

Acceptance gates (from `WORK_GRAPH.json`):
1. Injected failures in each rail fail CI semantics.
2. jcode-tui deterministic tests run rather than compile-only.

Repository: `jerudnik/jcode`. Workflow: `.github/workflows/fork-ci.yml`.
Head of record: **`081c61300`** (branch `ci-validation`, PR #16 → `main`) —
the commit of the green provenance run (section 5b). `fork-ci.yml` is
byte-identical from `a5f3bf6a8` through `081c61300`; the historical run/diff
references below to `a5f3bf6a8` describe the same rails.

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
- Green-on-real-head: the residual flakes uncovered here are root-caused and
  driven to zero in section 5b; the head of record is green.

The flakes surfaced during this investigation are enumerated and closed by
class in section 5b (env-lock pollution, the mermaid deferred-worker race, and
ambient-`JCODE_HOME` reads). The remaining hermeticity hardening that lets the
rails drop `--test-threads=1` is scoped as follow-up node **F28**.

## 5b. Resolution and green run of record (cdb2ee303)

Closing F17 required driving three distinct flake classes to zero. They had
independent root causes, so they are listed and closed separately.

**Class 1 - env-lock pollution in `jcode-base` (fixed).** Two tests corrupted
shared global state under parallelism:

1. `auth::lifecycle::post_auth_model_selection_keeps_catalog_order_for_
   unranked_providers` read an ambient/sibling-seeded cerebras live-catalog
   disk cache (no `JCODE_HOME` isolation) and flakily selected `qwen-3` over
   the first catalog route. Fixed by using the existing `AuthTestSandbox`
   helper (holds the env lock, isolates `JCODE_HOME`, resets global auth
   state); LOC-neutral, `lifecycle.rs` holds the 2334 ratchet baseline.
2. `sponsors::provenance::disabled_sponsors_config_disables_everything`
   dropped and re-acquired the env lock mid-test, letting a sibling
   `enable_sponsors` test repoint `JCODE_HOME` and repopulate the shared config
   cache. Fixed by holding one guard across the whole test.

Commits `be45954fc`, `081c61300`. The full `jcode-base --lib` parallel suite
went from ~2/12 failing rounds to 15/15 green after both fixes.

**Class 2 - mermaid deferred-render worker (the real diagram flake, fixed).**
The deferred-render worker (`mermaid_cache_render.rs::deferred_render_worker`)
is a detached background thread that calls `register_active_diagram()`
**outside any test's lock scope**. Sequence: a render test triggers a deferred
render and finishes, `reset_tui_test_globals()` clears `ACTIVE_DIAGRAMS`, then
the worker wakes *later* and re-populates `ACTIVE_DIAGRAMS`; the next diagram
test reads the stale entry and asserts the wrong hash/splitter. Because the
pollution is asynchronous, `--test-threads=1` alone does **not** close it (it
reproduced at ~1/40 serial rounds), which is why it presented as a rare,
non-repeating parallel flake (e.g.
`test_side_panel_visibility_change_resets_diagram_fit_context`).

Fix (`86acc7ae1`, refined in `cdb2ee303`): a runtime `SYNCHRONOUS_RENDER_MODE`
`AtomicBool` (a runtime flag, **not** `cfg(test)`: `jcode-tui-mermaid` builds
as a non-test dependency when the `jcode-tui` test binary runs, so `cfg(test)`
is false there). The TUI test render lock and `create_test_app` enable it, and
the gate short-circuits at the top of `render_mermaid_deferred_inner` (before
any deferred-queue bookkeeping), so deferred renders run inline on the calling
thread and registration stays inside the test's lock scope. An independent
empty-context review returned ACCEPT-WITH-CAVEATS with zero production risk; its
one actionable note (a stale pending-queue entry when the gate sat lower) is
what `cdb2ee303` addressed by moving the gate to function entry.

**Class 3 - ambient-`JCODE_HOME` reads (green on CI, tracked).** A handful of
tests (`create_test_app` takes a read-lease without redirecting `JCODE_HOME`)
read the real `~/Library/Application Support/jcode/` and fail only on a
developer machine with ambient state. CI runs a pristine home, so this class is
green there. Making `create_test_app` hermetic-by-default is part of the F28
follow-up.

**Belt-and-suspenders serialization.** The two fork-ci jcode-tui rail steps run
`--test-threads=1`. This is a test-harness artifact, not a product race: the
runtime is multi-process (each swarm agent is its own OS process via
`current_exe`/spawn hook), so no two live `App` contexts share these globals in
production. Follow-up **F28** hardens the lock discipline so the cap can be
lifted.

**Validation.** On a Linux repro box (x86_64, 22c): 0/95 serial rounds flaked
with the fix (80 at `86acc7ae1` + 15 at `cdb2ee303`), vs the clean tree
reproducing the exact `side_panel_visibility_change_resets_diagram_fit_context`
flake (1/40). The `smoothness_benchmark_*` failures on that box are a
machine-load artifact (fail identically on clean and fixed binaries,
interleaved) and do not occur on GitHub CI.

**Provenance run of record: Fork CI `30026766050`** (`pull_request`,
`cdb2ee303`) concluded **success**. All three blocking rails green: Quality
Guardrails (ratchet held; the +14 LOC in `mermaid_cache_render.rs` was offset
with a `bump_debug_stats` helper), Linux Tests 26m24s (serial `jcode-tui`
executed, zero failures), and Build & Test macOS 45m14s (serial `jcode-tui`
executed). Both F17 gates are met and demonstrated green on the head of record.

## 6. actionlint

`actionlint 1.7.7` on `.github/workflows/fork-ci.yml`: **0 findings, exit 0**.
(Output in `actionlint.md`.)
