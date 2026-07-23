# F17 failure-injection plan

## Purpose and scope

`WORK_GRAPH.json` defines F17's first acceptance gate as **"Injected failures in
each rail fail CI semantics"** and requests a failure-injection proof. This plan
uses disposable commits only. No production branch, workflow, or test is to retain
an injected failure.

The workflow has exactly one `continue-on-error`: the advisory
`latest-stable-canary` job at [fork-ci.yml:213-216](../../../../../.github/workflows/fork-ci.yml#L213-L216).
It is not a F17 rail. There is no `continue-on-error` in any step or job in the
three blocking job ranges below. Therefore, an executable step that returns
nonzero fails its job, and a failed blocking job makes the Fork CI workflow run
red. For the test commands, `cargo test` returns nonzero when an injected test
fails; the timeout wrapper is part of the command and propagates that failure.

The table names every executable gate step in the blocking jobs. “No COE” means
there is no `continue-on-error` declaration in the cited enclosing job range;
line 216 is the workflow's sole declaration and belongs to the excluded canary.
“Fails run” follows from the preceding job semantics, not from a test continuing
to a later step.

## 1. Static semantics proof

### Quality Guardrails (`quality`, job lines 48-212)

| Rail step | Workflow line(s) | No COE | Conclusion on a nonzero command |
| --- | --- | --- | --- |
| Verify pinned Rust toolchain coherence | 55-106 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |
| Check formatting (blocking for fork-touched files) | 123-145 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |
| Check all targets and all features | 149-150 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |
| Clippy (blocking for fork-touched files) | 159-188 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |
| Enforce warning budget | 190-191 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |
| Enforce oversized-file ratchet | 193-194 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |
| Enforce oversized-test ratchet | 196-197 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |
| Run Rust production filter tests | 199-200 | Yes, job range 48-212 | A failing Python test fails `Quality Guardrails`, therefore Fork CI is red. |
| Enforce panic-prone usage ratchet | 202-203 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |
| Enforce swallowed-error usage ratchet | 205-206 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |
| Enforce no unused dependencies | 208-211 | Yes, job range 48-212 | Fails `Quality Guardrails`, therefore Fork CI is red. |

### Build & Test (macOS) (`macos`, job lines 240-305)

| Rail step | Workflow line(s) | No COE | Conclusion on a nonzero command |
| --- | --- | --- | --- |
| Build release binary | 259-260 | Yes, job range 240-305 | Fails `Build & Test (macOS)`, therefore Fork CI is red. |
| Verify built binary launches | 262-263 | Yes, job range 240-305 | Fails `Build & Test (macOS)`, therefore Fork CI is red. |
| Compile library and binary tests | 265-268 | Yes, job range 240-305 | A compilation failure fails the macOS job and Fork CI. |
| Run workspace library tests | 272-276 | Yes, job range 240-305 | A failed workspace test fails the macOS job and Fork CI. |
| Run jcode-tui library tests | 278-281 | Yes, job range 240-305 | A failed TUI test fails the macOS job and Fork CI. |
| Run jcode-app-core library tests | 283-286 | Yes, job range 240-305 | A failed app-core test fails the macOS job and Fork CI. |
| Compile integration test binaries | 288-292 | Yes, job range 240-305 | A compilation failure fails the macOS job and Fork CI. |
| Run provider matrix tests | 294-297 | Yes, job range 240-305 | A failed provider-matrix test fails the macOS job and Fork CI. |
| Run e2e tests | 299-305 | Yes, job range 240-305 | A failed e2e test fails the macOS job and Fork CI. |

### Linux Tests (`linux-tests`, job lines 307-380)

| Rail step | Workflow line(s) | No COE | Conclusion on a nonzero command |
| --- | --- | --- | --- |
| Install mold linker | 323-326 | Yes, job range 307-380 | Fails `Linux Tests`, therefore Fork CI is red. |
| Configure mold linker | 328-335 | Yes, job range 307-380 | Fails `Linux Tests`, therefore Fork CI is red. |
| Compile library and binary tests | 337-340 | Yes, job range 307-380 | A compilation failure fails the Linux job and Fork CI. |
| Run workspace library tests | 344-348 | Yes, job range 307-380 | A failed workspace test fails the Linux job and Fork CI. |
| Run jcode-tui library tests | 350-353 | Yes, job range 307-380 | A failed TUI test fails the Linux job and Fork CI. |
| Run jcode-app-core library tests | 355-358 | Yes, job range 307-380 | A failed app-core test fails the Linux job and Fork CI. |
| Compile integration test binaries | 360-364 | Yes, job range 307-380 | A compilation failure fails the Linux job and Fork CI. |
| Run provider matrix tests | 366-369 | Yes, job range 307-380 | A failed provider-matrix test fails the Linux job and Fork CI. |
| Build jcode binary for e2e | 371-372 | Yes, job range 307-380 | Fails `Linux Tests`, therefore Fork CI is red. |
| Run e2e tests | 374-380 | Yes, job range 307-380 | A failed e2e test fails the Linux job and Fork CI. |

The test commands are the same logical commands on both platforms, with only the
target triple changed. One injected test in a selected crate/test target therefore
makes both the macOS and Linux instances of that logical rail red in one workflow
run. The preflight compile steps are blocking too, but are not separate runtime
test rails and need no additional injection.

## 2. Rail-to-injection map

### Already credited, do not reinject

Fork CI run **29931842057** (`pull_request`, head SHA `5cfe70580`, using this same
workflow file) is the organic-red proof for two rails:

| Logical rail | Evidence already captured | Disposition |
| --- | --- | --- |
| workspace-lib (`--workspace --lib --bins --exclude jcode-tui --exclude jcode-app-core`) | Seven macOS `SUN_LEN` failures in the workspace-library step | Credit the macOS workspace-lib rail as blocking. No injection. |
| `jcode-tui --lib` | Eight Linux `jcode-tui` failures in the TUI-library step | Credit the TUI rail as blocking. No injection. |

The remaining distinct rail commands need the four injections below. The first
branch intentionally combines Quality and app-core. It is safe and evidence-rich:
`quality`, `macos`, and `linux-tests` have no `needs:` dependency and run in
parallel, so the independent Quality failure cannot prevent the two app-core
steps from being scheduled or reported.

| Remaining rail | Disposable branch | Exact injection and file | Expected red job and step |
| --- | --- | --- | --- |
| Quality Guardrails, exercised through its blocking format guard | `ci-proof/f17-inject-quality-app-core` | Append this deliberately un-rustfmt-formatted but valid test line to `crates/jcode-app-core/src/lib.rs`: `#[test] fn f17_inject_fail_app_core(){assert!(false,"F17 rail-block proof");}`. The branch changes this file relative to `origin/vendor/upstream`, so the formatting step's fork-touched-file intersection is nonempty and exits 1. | `Quality Guardrails` → `Check formatting (blocking for fork-touched files)` (lines 123-145). |
| `jcode-app-core --lib` | `ci-proof/f17-inject-quality-app-core` | The same one-line test above is compiled by the app-core library test target and fails at runtime. | `Build & Test (macOS)` → `Run jcode-app-core library tests` (283-286), and `Linux Tests` → same-named step (355-358). |
| provider-matrix (`--test provider_matrix`) | `ci-proof/f17-inject-provider-matrix` | Append a normally formatted test to `tests/provider_matrix.rs`: `#[test] fn f17_inject_fail_provider_matrix() { assert!(false, "F17 rail-block proof"); }` (expand to rustfmt's normal three-line layout before committing). | `Build & Test (macOS)` → `Run provider matrix tests` (294-297), and `Linux Tests` → same-named step (366-369). |
| e2e (`--test e2e`) | `ci-proof/f17-inject-e2e` | Append a normally formatted test to the e2e integration-target root, `tests/e2e/main.rs`: `#[test] fn f17_inject_fail_e2e() { assert!(false, "F17 rail-block proof"); }` (expand to rustfmt's normal three-line layout before committing). | `Build & Test (macOS)` → `Run e2e tests` (299-305), and `Linux Tests` → same-named step (374-380). |

For the two normally formatted snippets, the **only intentional breakage is the
single `assert!(false, ...)` statement**. The compact app-core test is deliberately
not formatted because it doubles as the Quality format-gate injection. Do not run
`cargo fmt` on that first branch.

## 3. Minimal run schedule

**Required injected commits/branches: 3. Required new Fork CI workflow runs: 3.**
This is the smallest sufficient schedule after crediting run 29931842057.

1. Create and push `ci-proof/f17-inject-quality-app-core`, containing only the
   compact app-core test shown above. Dispatch Fork CI against that ref (or open a
   disposable PR against `main`; a normal non-`main` push alone does not trigger
   this workflow because `push` is restricted to `main` at lines 16-24). Record
   one red `Quality Guardrails`/format step plus red macOS and Linux app-core test
   steps. The same run also demonstrates that its preceding workspace and TUI
   steps reached green before app-core fails.
2. Create and push `ci-proof/f17-inject-provider-matrix`, containing only the
   formatted provider-matrix failing test. Dispatch Fork CI and record the red
   provider-matrix step in both platform jobs. Its earlier workspace, TUI, and
   app-core steps must be green, which confirms ordinary progression to the
   targeted rail.
3. Create and push `ci-proof/f17-inject-e2e`, containing only the formatted e2e
   failing test. Dispatch Fork CI and record the red e2e step in both platform
   jobs. Its earlier workspace, TUI, app-core, and provider-matrix steps must be
   green.

Do not combine the provider-matrix and e2e injections: they are sequential steps
in each platform job, so a provider-matrix failure stops the job before e2e and
would not prove the e2e step's failure semantics. Do not combine app-core with
either downstream injection for the same reason. Quality is the sole combinable
case because it is in an independent, concurrently scheduled job. Thus four
logical remaining rails collapse to three dispatched runs, while the two organic
rails require none.

Each dispatch run also enables the advisory canary due to its condition at lines
213-216, but its `continue-on-error: true` status means it is not evidence for,
or a substitute for, any blocking rail.

## 4. Cleanup and evidence retention

1. For every run, save the workflow-run ID, ref, injected commit SHA, completion
   timestamp, and the failed job/step URLs in the F17 evidence record before
   cleanup. Preserve the test-output lines containing the `F17 rail-block proof`
   message and the job conclusion.
2. Confirm the three disposable commits contain no file other than their one
   injection-file edit. Do not merge, cherry-pick, or rebase them onto `main`.
3. After the run IDs and logs are captured, delete the three remote throwaway
   branches and their local counterparts:
   `ci-proof/f17-inject-quality-app-core`,
   `ci-proof/f17-inject-provider-matrix`, and
   `ci-proof/f17-inject-e2e`.
4. No revert commit is needed on protected history because no injection branch is
   merged. If an injection was accidentally merged, immediately revert that
   specific commit with a new revert commit, then rerun the normal green F17
   workflow before considering the evidence complete.
