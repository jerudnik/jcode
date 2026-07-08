# Swarm Fleet Actions Plan

Status: Proposed. Execution vehicle is an inline swarm (all workers
`spawn_mode: "inline"`). This plan seeds the swarm task graph, assigns
instances, and defines per-instance verification so the DAG is hill-climbable.

## Why a swarm

The three workstreams below are mostly independent and each has a crisp,
machine-checkable done-state, so they map cleanly onto a swarm DAG: three
root instances that can run in parallel, each gated by its own `cargo`/script
check rather than human review. We dogfood the fleet surface (`/swarm fleet`,
`swarm list_swarms`) we are extending in Workstream B while building it.

Vocabulary (repo convention): a plan node is an **instance**; its `kind`
displays as **phase**; member type resolves assigned-instance phase → Agent
preset `subagent_type` → free-form swarm tag → untyped.

## Workstreams

### A. Fork-CI touched-file clippy as a local script (confidence after push)

Problem: `.github/workflows/fork-ci.yml` already runs the "clippy blocking only
for fork-touched files" gate (whole-tree clippy JSON, primary-span file names
diffed against `origin/vendor/upstream..HEAD`). There is no local one-command
equivalent, so "run it after push for extra confidence" is a manual re-derive
of the CI jq pipeline every time.

Deliverable: `scripts/fork-touched-clippy.sh`, a local port of the fork-ci
`quality` clippy step:
- Computes fork-touched `.rs` files: `git diff --name-only --diff-filter=d
  <vendor-ref> HEAD -- '*.rs'` (vendor ref resolves `github/vendor/upstream`
  then `origin/vendor/upstream`, override with `--vendor-ref`).
- Runs `cargo clippy --all-targets --all-features --message-format=json` via
  `scripts/dev_cargo.sh` when present (Nix-aware; see Workstream C), else bare
  `cargo`.
- Extracts primary-span file names of `warning`/`error` messages, `comm -12`
  against the touched set.
- Exit non-zero listing fork-touched lints; report vendor-only drift as a
  warning (byte-parallel to CI so local == CI verdict).
- `--fmt` companion flag mirrors the fmt gate; `--staged`/`--range` optional
  niceties only if free.

Verification (instance done-state):
- `bash scripts/fork-touched-clippy.sh` exits 0 on the current clean tree.
- Introduce a deliberate lint in a fork-touched file → script exits non-zero
  and names that file; revert → exits 0. (Worker proves both transitions,
  reverts the probe, leaves tree clean.)
- `shellcheck scripts/fork-touched-clippy.sh` clean.
- Wire into `scripts/fork-health.sh` docs or `--help` cross-reference only if
  it does not grow the fork's workflow diff (health check forbids `main`
  touching `.github/workflows`).

### B. Actionable fleet rows in the TUI (`/swarm fleet` → drill-in)

Problem: `render_swarm_fleet()` (crates/jcode-tui/src/tui/app/commands_swarm.rs)
emits a static text block. Operators cannot act on a listed swarm/member/
instance without retyping ids into `/swarm status|plan|start|stop`.

Deliverable: make fleet output actionable via the existing picker substrate
(`PickerKind` in crates/jcode-tui/src/tui/mod.rs; pattern in
inline_interactive.rs / auth_account_picker.rs), not a bespoke overlay.

Staged so each stage is independently shippable:
- B1 (parse + model, no UI): keep `render_swarm_fleet` pure; add a sibling
  that returns a structured `FleetSelection` list (swarm_id, coordinator
  session, member/instance ids, needs-attention flag). Unit-tested against the
  same `SwarmFleetEntry` fixtures already in `commands_swarm.rs::tests`.
- B2 (picker): add `PickerKind::SwarmFleet`; `/swarm fleet` opens a picker of
  swarm rows (attention-sorted). Selecting a swarm expands to its members/
  instances (reuse roster's phase-first type resolution). Escape closes; no
  live swarms → keep the existing "No live swarms found." text path.
- B3 (verbs): a selected member/instance offers status / plan / start / stop.
  These dispatch the existing `comm_*` verbs (do not add new wire verbs) by
  synthesizing the same command `parse_swarm_verb` already produces, so the
  drill-in is a thin front-end over Workstream-shipped logic. `stop` is the
  only destructive verb: require an explicit confirm row, never auto-fire.

Constraints:
- Selection type resolution uses assigned-instance **phase** first, exactly
  like `render_swarm_roster`; do not regress the legacy tag fallback.
- Pure parse/model logic stays unit-testable without an `App`; dispatch wired
  in remote/key_handling.rs; render in remote/server_events.rs, matching the
  existing `/swarm` split.

Verification (instance done-state):
- New pure tests for B1 selection extraction pass (`cargo test -p jcode-tui
  --lib fleet` scoped to exact names).
- B2/B3: a `debug_socket` tester frame shows `/swarm fleet` opening the picker,
  arrow/enter drilling swarm→member, and a verb firing; capture one frame per
  stage as the visual proof. Add a headless key-handling unit test for the
  picker state machine where feasible (mirror remote_model_picker_hotkeys.rs).
- fmt/check green via `scripts/dev_cargo.sh`.

### C. Fix the coordinated selfdev build outside Nix (`cargo: not found`)

Problem: `selfdev build` failed outside the Nix dev shell because
`selfdev_build_command_for_target` (crates/jcode-build-support/src/paths.rs)
emits `bash -lc "scripts/dev_cargo.sh build ..."`, and `dev_cargo.sh` assumes
`cargo` is already on `PATH`. The Nix fallback worked only because the shell
was entered manually. The coordinated build path should self-heal.

Deliverable (smallest correct fix, decided by the worker after reading
dev_cargo.sh's PATH assumptions):
- Preferred: have `dev_cargo.sh` (or the emitted command) locate a toolchain
  when `cargo` is absent: source the repo Nix dev env non-interactively
  (`nix develop --command ...`) when a `flake.nix` and `nix` exist, else fall
  back to `~/.cargo/bin` / rustup shims, else emit a precise, actionable error
  naming both remedies. Must stay idempotent inside an already-provisioned Nix
  shell (no double-wrap).
- Keep the change in the build-command builder + wrapper only; do not change
  the selfdev queue/state machine.

Verification (instance done-state):
- Reproduce: run the emitted command with `cargo` stripped from `PATH`
  (`env -i PATH=/usr/bin:/bin bash -lc '<emitted>'` or equivalent) → before
  fix fails with `cargo: not found`, after fix builds (or errors with the new
  actionable message when no toolchain is discoverable at all).
- Inside the Nix shell the command still builds with no behavior change and no
  redundant re-entry.
- Unit test for the builder's new branch (paths.rs already has command-shape
  tests near line 900; extend them) asserting the PATH-recovery wrapper is
  present only when expected.
- fmt/check green.

## Swarm task graph (seed)

Three independent root instances, one per workstream, each its own phase.
No cross-instance dependencies; the coordinator integrates on green.

```
A: fork-touched-clippy   (phase: tooling)     ready
B1: fleet selection model (phase: implement)  ready
  └─ B2: fleet picker      (phase: implement)  blocked_by B1
       └─ B3: fleet verbs   (phase: implement) blocked_by B2
C: selfdev PATH recovery  (phase: implement)   ready
```

Assignment: spawn workers inline. Suggested tags/presets so member typing is
legible in `/swarm status`: A → tooling, B* → implement, C → implement. The
coordinator holds verify/integration and lands commits per workstream (three
focused commits, not one) after each instance's done-state check passes.

Execution order: A, B1, C can start immediately in parallel. B2 waits on B1,
B3 on B2. Integrate each workstream independently so a slip in B does not hold
A or C.

## Guardrails (repo conventions)

- All workers `spawn_mode: "inline"`.
- cargo via `nix develop --command sh -lc 'scripts/dev_cargo.sh ...'`.
- Scope test filters to exact names / `--lib` to avoid wrapper exit 97 on
  zero-match binaries.
- No casual reloads; a deliberate reload happens only to live-smoke Workstream
  B after its verify frames pass.
- `main` must not add `.github/workflows` diffs (fork-health invariant).
- Commit as we go: one focused commit per workstream on green.
