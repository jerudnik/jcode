---
status: open
priority: medium
owner: maintainers
opened: 2026-08-01
---

# jcode-desktop hot-reload tests poison a shared mutex on parallel/env-sensitive runs

Surfaced 2026-08-01 by `scripts/ci_local.sh` (the new fleet-offloaded pre-push
CI mirror) running the macOS `Build & Test` command list against a clean fleet
builder environment. Two tests failed where hosted CI had been passing:

```
---- desktop_hot_reload_persists_workspace_focus_before_spawn ----
panicked at crates/jcode-desktop/src/main_tests.rs:457:52:
  workspace preferences saved            # load_preferences() returned Ok(None)

---- desktop_hot_reload_restarts_default_launched_workspace_as_workspace ----
Error: desktop hot reload env lock poisoned   # cascade from the panic above
```

This is the same class DECISIONS.md already records for jcode-tui: "order-
dependent flakes that pass singly but poison on process-global pollution in
full-suite runs." It is filed, not fixed in-program, to avoid widening scope
during W4/W5 signoff. No product runtime path is implicated.

## Mechanism

`crates/jcode-desktop/src/main_tests.rs` serializes the hot-reload tests with a
process-global `static DESKTOP_PREFS_ENV_LOCK: Mutex<()>` because they mutate
the process-global `JCODE_DESKTOP_STATE` env var (read by
`desktop_prefs::preferences_path`). The lock makes the tests mutually exclusive,
but two things still break under a clean/parallel environment:

1. **The panic, not the mutex, is the primary fault.** `DesktopRelaunch::for_app`
   (`desktop_reload.rs:673`) calls `save_preferences` and *swallows any error*
   (logs and continues). The test then asserts `load_preferences()?.expect(
   "workspace preferences saved")`. When the save silently fails, the load
   returns `Ok(None)` and the `.expect` panics. A swallowed write error becoming
   a downstream `None` is a real durability smell even though these two callers
   are test-only today.

2. **The panic poisons the mutex, so the fault cascades.** A panic while holding
   `DESKTOP_PREFS_ENV_LOCK` leaves it poisoned; the sibling test's
   `lock()` then hits the `Err(_) => bail!("desktop hot reload env lock
   poisoned")` arm and fails for a reason that has nothing to do with its own
   assertion. One real failure is thus reported as two, obscuring the cause.

Why the save fails on the fleet builder specifically is still open: the most
likely trigger is that `JCODE_DESKTOP_STATE` set/removed across the two tests
plus any third env-sensitive test races despite the mutex (the mutex guards the
desktop-prefs tests against each other, not against the rest of the parallel
suite that also touches process env), so `preferences_path` occasionally
resolves somewhere the subsequent load does not read.

## Suggested fix (deferred to the human-noticed-issues backlog)

Smallest correct change, in order of value:

1. Make `for_app` **not** silently swallow the persist error in a context that
   later asserts the write landed: either propagate it, or have the tests assert
   on the `save_preferences` result directly rather than round-tripping through
   a global-env load.
2. Recover the poisoned lock (`lock().unwrap_or_else(|e| e.into_inner())`) so one
   real failure is reported once, not twice.
3. Stop threading desktop-prefs location through a **process-global** env var in
   tests; pass the path explicitly (a `preferences_path_override` arg or a
   thread-local) so the tests need no cross-suite env lock at all. This is the
   durable fix; the mutex is a workaround for a process-global that should not
   be process-global in a parallel test binary.

## Repro

```bash
scripts/ci_local.sh   # macOS job on the fleet builder; fails at the workspace step
# or, directly:
scripts/dev_cargo.sh test -p jcode-desktop --bin jcode-desktop
```
