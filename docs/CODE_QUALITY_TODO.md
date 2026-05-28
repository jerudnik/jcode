# Code Quality Program Todo List

This file used to track the execution backlog for the code-quality uplift program described in `docs/CODE_QUALITY_10_10_PLAN.md`. Active tracking has moved to Backlog.md. The historical execution log (with per-phase completion notes from the 2026-03 through 2026-05 cleanup waves) is preserved in git history at commit 0aea41ac and earlier.

For the durable design and phase definitions, see `docs/CODE_QUALITY_10_10_PLAN.md`. For the underlying audit, see `docs/CODE_QUALITY_AUDIT_2026-04-18.md`.

## Tracker pointers

Per-phase work, by phase from `docs/CODE_QUALITY_10_10_PLAN.md`:

- Phase 0: Prevent further decay (CI ratchets, file-size targets). Tracked in: TASK-40, TASK-52
- Phase 1: Warning and dead-code burn-down (`#![allow(dead_code)]` audit, stale unused fns). Tracked in: TASK-53, TASK-70
- Phase 2: Decompose the biggest files (`src/server.rs`, `src/agent.rs`, `src/provider/mod.rs`, `src/provider/openai.rs`, `src/tui/ui.rs`, `src/tui/info_widget.rs`). Tracked in: TASK-35, TASK-38, TASK-39, TASK-54
- Phase 3: Error-handling hardening (production `unwrap`/`expect` reduction, provider/reload/socket error context). Tracked in: TASK-32, TASK-33
- Phase 4: Test strategy improvements (e2e split helpers, reload/stream tests, snapshot/property tests). Tracked in: TASK-42, TASK-55
- Phase 5: Reliability and performance guardrails (reload/attach/detach reliability, memory budget, compile-perf roadmap). Tracked in: TASK-34

From the comprehensive audit backlog in `docs/CODE_QUALITY_AUDIT_2026-04-18.md`:

- Structural backlog: production files over 1200 LOC (server/agent/provider/tui mega-files). Tracked in: TASK-35
- Structural backlog: production files 801-1200 LOC. Tracked in: TASK-36
- Structural backlog: production functions >100 LOC (event handlers, render paths, turn loops). Tracked in: TASK-37
- `#[allow(clippy::too_many_arguments)]` retirement via request/context structs. Tracked in: TASK-40
- Inline tests embedded in production mega-files. Tracked in: TASK-41
- Production `unwrap`/`expect`/`panic!` hotspots in tool/auth/build/server/provider. Tracked in: TASK-32
- Remaining `allow(...)` suppressions audit. Tracked in: TASK-40
<!-- backlog-tracking-ignore: pointer text describing TASK-39's subject. -->
- TODO/FIXME/HACK marker burn-down in `src/` (and other doc cleanup). Tracked in: TASK-71
- Refresh the audit backlog after each major cleanup wave. Tracked in: TASK-72

Reliability/perf guardrails (Phase 5 detail):

- Compile-performance roadmap execution (see `docs/COMPILE_PERFORMANCE_PLAN.md`). Tracked in: TASK-51, TASK-76, TASK-77
- Repeated reload reliability coverage. Tracked in: TASK-34, TASK-55
- Memory regression budget tracking. Tracked in: TASK-34
- Observability around reload, swarm, and tool execution paths. Tracked in: TASK-34

## Status

Each tracked task in Backlog.md owns its own status, acceptance criteria, and notes via the `backlog task` CLI. Use `backlog task list --plain` for the live view and `backlog task <id> --plain` for details.
