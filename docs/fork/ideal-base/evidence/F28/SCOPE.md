# F28 scope: restore jcode-tui test parallelism (hermeticity hardening)

Follow-up to F17. F17 made the jcode-tui rails blocking and green by
(a) fixing the mermaid deferred-worker `ACTIVE_DIAGRAMS` pollution race at its
source (runtime synchronous-render mode in tests) and (b) capping the two
fork-ci jcode-tui rail steps at `--test-threads=1` as belt-and-suspenders for
the remaining shared-global/timing hermeticity debt. F28 pays down that debt so
the thread cap can be lifted.

## Not an architecture change

The runtime is **multi-process**: each swarm agent spawns as its own OS process
(`current_exe`/spawn hook, `JCODE_SPAWN_KIND=swarm-agent`, one window/tab per
agent), so no two live `App` contexts share process-global TUI view state in
production. The flakes are a **test-harness** artifact (many tests share one
process and touch process-global caches), not a product race. F28 is test
discipline only; it must not change runtime behavior.

## Work items

1. **Render-lock discipline.** A static scan found ~28 tests that render or
   mutate render/frame caches **without** taking the render lock
   (`lock_test_render_state`): ~17 `.draw(` sites, ~6 `side_panel_render_cache`,
   ~3 `record_layout_snapshot`, etc. Make each take the render lock (or an
   equivalent scoped guard that runs `reset_tui_test_globals` on acquire), so a
   test cannot observe or leak another test's process-global render state.

2. **video_export multi-App leak.** `crates/jcode-tui/src/tui/video_export.rs`
   is the only place that renders multiple `App`s in one process. It is
   sequential and guarded by `set_video_export_mode`, so the leak is cosmetic,
   but F28 should remove it or prove it inert under the test lock.

3. **Lift the thread cap.** After 1-2, run the full jcode-tui suite over >=3
   parallel rounds without `--test-threads=1`; if green, drop the cap from both
   fork-ci jcode-tui rail steps (macOS + Linux) and re-verify on CI.

## Acceptance gates (from WORK_GRAPH F28)

- Render/cache-touching tests hold the render lock; static scan finds zero
  unlocked render-cache mutators.
- video_export multi-App view-state leak removed or proven inert.
- Full jcode-tui suite green over >=3 parallel rounds without `--test-threads=1`.
- fork-ci jcode-tui rails no longer need `--test-threads=1`.

## Sibling themes

- **R05** (W4): multi-client session contention (shared live state across two
  clients) is the runtime analogue of this test-harness contention.
- **F25** (W4): socket/durable-swarm-state hygiene.

## Pointers

- Render lock: `crates/jcode-tui/src/tui/app/test_support.rs`
  (`lock_test_render_state`, `reset_tui_test_globals`).
- The F17 fix that this builds on: synchronous render mode
  (`crates/jcode-tui-mermaid`: `SYNCHRONOUS_RENDER_MODE`,
  `set_synchronous_render_mode`/`is_synchronous_render_mode`; gated at the top
  of `render_mermaid_deferred_inner` in `mermaid_cache_render.rs`).
- F17 evidence: `docs/fork/ideal-base/evidence/F17/` (see `ci_runs.md` 5c).
