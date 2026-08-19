---
status: open
priority: medium
owner: maintainers
opened: 2026-08-19
related:
  - crates/jcode-app-core/src/tool/communicate/run_plan.rs
  - crates/jcode-core/src/activity.rs
  - crates/jcode-base/src/background.rs
---

# `run_plan` may hold no activity lease, so daemon idle shutdown can end a live plan

Carried from a bug register during proposal triage. **Stub: the mechanism is
grounded in code, but the failure has not been reproduced live.** Reproduce
before designing the fix.

## Claim

A `run_plan` driver can be running real work while the daemon believes nothing
is happening, so idle-shutdown accounting is free to terminate it.

## What is grounded

`ActivityClass` (`crates/jcode-core/src/activity.rs:20-38`) has eight carriers.
Two matter here:

- `SwarmWaiter` — "C8: live swarm await watcher tasks". Acquired at
  `crates/jcode-app-core/src/server/comm_await.rs:398`, so a backgrounded
  `await_members` does hold the daemon open.
- `BackgroundTask` — "C5: non-detached background tasks. **C6 (detached) never
  leases.**" Acquired in `crates/jcode-base/src/background.rs:161` via
  `acquire_task_lease`.

`run_plan` defaults to `background: true`. Its own `RunPlanClaimGuard`
(`run_plan.rs:315`) is a driver mutual-exclusion claim, not an activity lease:
it prevents two drivers for one session and is resolved against
`BackgroundTaskManager::is_live_task`. It does not participate in idle
accounting. No `ActivityClass` acquisition appears in `run_plan.rs`.

So the asymmetry is real on its face: the waiter has a carrier, the driver
appears not to.

## What is NOT established

- Whether `run_plan`'s driver actually takes the detached (C6) path, or is
  spawned as a leased C5 task. **This is the load-bearing question** and it
  decides whether the issue exists at all.
- Whether some other lease incidentally covers the window: a live provider turn
  in a worker holds `ProviderTurn`, and a connected client holds
  `ClientConnection`, so a fully headless plan with all workers between turns
  may be the only exposed shape.
- Whether idle shutdown has ever actually fired during a plan. No incident is
  on record; this was reasoned from the enum comment, not observed.

## How to settle it

Start a `run_plan` with `background: true`, no attached client, and workers
idle between turns. Read the activity table (the shutdown authority exposes
per-class counts, see `server/shutdown.rs:274-290`) and check whether any
carrier is held. If the table is empty while the plan is live, the issue is
confirmed and the fix is a `run_plan` lease held for the driver's lifetime,
released on terminal state.

The falsifier is a non-empty table naming a class attributable to the plan.

## Why it matters

A plan that dies to idle shutdown looks identical to a plan that finished, and
the coordinator is woken only at terminal state. The failure would be silent.
