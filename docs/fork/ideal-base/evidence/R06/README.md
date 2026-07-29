# R06 evidence — sticky-server process-group signaling and unchecked detachment

Node: `R06` (implement, parent `W4`). Branch `automation/w4-r06`, based on
`eee5ccc71`. Source issue:
`docs/fork/ideal-base/human-noticed-issues/STICKY_SERVER.md`.

## 1. Revalidated line citations

The issue note predates the distribution merge. Every citation was re-resolved
against this tree before any edit:

| Issue note claim | Current location | Verdict |
|---|---|---|
| `src/cli/commands.rs:2305` liveness check | `src/cli/commands.rs:2292` (pre-fix) | moved, claim holds |
| `src/cli/commands.rs:2310` SIGTERM to group | `src/cli/commands.rs:2297` (pre-fix) | moved, claim holds |
| `src/cli/commands.rs:2369` SIGKILL escalation | `src/cli/commands.rs:2356` (pre-fix) | moved, claim holds |
| `crates/jcode-base/src/platform.rs:260` `kill(-pid, ...)` | `crates/jcode-base/src/platform.rs:260` | unchanged, claim holds |
| `crates/jcode-app-core/src/server/socket.rs:262` `libc::setsid();` ignored | `crates/jcode-app-core/src/server/socket.rs:262` | unchanged, claim holds |
| `platform_tests.rs` already has `signal_detached_process_group_terminates_descendant_tree` | exists only under `#[cfg(windows)]` | **claim wrong**: there was no Unix descendant-tree test; this node adds one |

Kernel semantics were re-verified live on this machine before designing the
fixtures (macOS 26, aarch64, uid 502):

- A plainly spawned child inherits the parent PGID, so `pid != pgid`, and
  `kill(-pid, SIGTERM)` returns `ESRCH` while `kill(pid, 0)` shows it alive.
  This is exactly the reported "No such process (os error 3)" on a live pid.
- 180 live `uid=0` process-group leaders exist; `kill(-pid, 0)` on them returns
  `EPERM`, not `ESRCH`, which is why `EPERM` must never be laundered into a
  fallback.

## 2. Changes

`crates/jcode-app-core/src/server/socket.rs:222-235` — new
`detach_into_new_session()`. Turns a failed `setsid()` into an `Err` returned
from `pre_exec`, which aborts `Command::spawn`. `spawn_server_notify` calls it at
`socket.rs:277` instead of the previous discarded `libc::setsid();`. **(a)**

`crates/jcode-base/src/platform.rs:315-385` — new `SignalScope`,
`group_signal_may_fall_back()`, and `signal_detached_process_tree()`. The group
signal is attempted first, so a correctly detached leader still takes its helper
descendants down **(b)**. `ESRCH` plus a live PID falls back to the individual
process and returns `SignalScope::IndividualProcess` so callers can report the
narrower reach **(c)**. Every other errno, including `EPERM`, is returned
unchanged **(d)**. `pid <= 1` is refused up front: `kill(-1, ...)` broadcasts to
every signalable process and `kill(0, ...)` targets our own group, so neither can
be reachable through a daemon PID.

A fallback whose direct `kill` also fails returns a wrapped error naming both
attempts. That is honest reporting and it is also what makes gate 4 testable:
on POSIX, a group `EPERM` implies an individual `EPERM`, so errno alone cannot
distinguish "surfaced" from "fell back and failed identically".

`src/cli/commands.rs:2239-2256, 2317-2319, 2367-2370` — `server stop --force`
routes both stages through `signal_detached_process_tree` and reports the actual
scope via `signal_stage_detail`. The SIGKILL escalation previously discarded its
result with `let _ =`; it now appends its outcome (or its error) to the reported
detail.

`src/cli/commands.rs` is already 2.7x over the code-size threshold, and
`scripts/check_code_size_budget.py` requires tracked oversized files to stay
flat or shrink. The final shape therefore folds both call sites into one
`Result`-taking reporter and leaves the file at exactly its 3259-line baseline
(`check_code_size_budget.py` exits 0).

## 3. Fixtures

Both required process shapes are real processes, not mocks.

- **Group leader with a descendant** —
  `crates/jcode-base/src/platform_tests.rs:79-149`. `spawn_detached` (setsid) a
  `/bin/sh` that backgrounds a second `sh` which sleeps 3s and then writes a
  survival marker. The test asserts `getpgid(leader) == leader`, signals, then
  asserts the descendant PID dies and the marker never appears.
- **Live non-group-leader** —
  `crates/jcode-base/src/platform_tests.rs:151-213`. A plain `Command::spawn` of
  `/bin/sleep 30` inherits the harness PGID. The test first asserts
  `getpgid(pid) != pid`, that the bare group signal returns `ESRCH`, and that the
  process is still alive — reproducing the reported symptom — then asserts the
  fallback reaches it and `wait()` reports death by the requested signal. Run for
  `SIGTERM` and for `SIGKILL`.
- **Non-ESRCH errors** — `platform_tests.rs:215-234` pins the policy function
  over `EPERM/EACCES/EINVAL/EFAULT/EAGAIN`; `platform_tests.rs:236-269` signals a
  real foreign (root-owned) group leader and requires a verbatim `EPERM` that did
  not pass through the fallback path. Skips with a printed reason when run as
  root or when no foreign leader exists.
- **Broadcast PIDs** — `platform_tests.rs:288-297`.
- **setsid failure** —
  `crates/jcode-app-core/src/server/socket_tests.rs:561-608`. Real spawns. The
  happy path asserts the child leads its own group; the failure path calls
  `detach_into_new_session()` twice inside `pre_exec` (the second gets `EPERM`
  because the first made the child a leader) and requires `spawn()` to fail with
  that errno.
- **Truthful reporting** — `src/cli/commands_tests.rs:1042-1066`.

## 4. Non-vacuity

Each gate was re-run against a deliberately wrong implementation. All commands
below ran with `JCODE_REMOTE_CARGO=0` (the configured remote builder cannot see
this worktree).

| Mutation | Gate targeted | Result |
|---|---|---|
| A: group signal only, no fallback (the pre-fix behavior) | 3 | **FAILED** 2 tests: both SIGTERM and SIGKILL fallback tests panic with `Os { code: 3, ... "No such process" }` |
| B: fall back on *any* group error | 4 | **FAILED** 1 test: `permission_denied_group_signal_is_surfaced` reports `no process group led by pid 376; signalling the process directly failed: Operation not permitted` instead of a verbatim `EPERM` |
| C: naive "just signal the PID" fix | 2 | **FAILED** 1 test: `descendant should not have reached its survival marker` |
| D: `unsafe { libc::setsid() };` result ignored (the pre-fix behavior) | 1 | **FAILED** 1 test: `a failed setsid() must surface as a spawn error: Child { ... }` |

Mutation B is worth recording as a process finding: the **first** version of the
EPERM test passed under mutation B and was therefore vacuous. Signal-0 to a
foreign process yields `EPERM` at both scopes, so errno alone cannot discriminate.
The implementation was changed (wrapped fallback-failure error) so that the
distinction is observable, and only then did the test discriminate. Log:
`/tmp/r06-mutB.log` (vacuous pass) then `/tmp/r06-mutB2.log` (discriminating
failure).

## 5. Commands and counts

```text
JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh test -p jcode-base --lib platform
  -> ok. 12 passed; 0 failed; 1 ignored; 1210 filtered out
     (7 of the 12 are new: descendant-tree, SIGTERM fallback, SIGKILL fallback,
      errno policy, EPERM surfacing, broadcast refusal, plus the pre-existing
      spawn_detached_creates_new_session which still passes)

JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh test -p jcode-app-core --lib server::socket_tests
  -> ok. 21 passed; 0 failed; 0 ignored; 1148 filtered out
     (1 new: detach_into_new_session_failure_aborts_spawn)

JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh test -p jcode --lib cli::
  -> ok. 225 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     (2 new: signal_stage_detail_distinguishes_group_from_individual,
             signal_stage_detail_surfaces_signal_failure)
```

Mutation runs (each restored afterwards; `grep -r MUTATION crates src` is clean
in owned files at the committed head):

```text
mutation A: 10 passed; 2 failed   (/tmp/r06-mutA.log)
mutation B: 11 passed; 1 failed   (/tmp/r06-mutB2.log)
mutation C: 11 passed; 1 failed   (/tmp/r06-mutC2.log)
mutation D: 20 passed; 1 failed   (/tmp/r06-mutD.log)
```

## 6. Full command log at the committed head

```text
JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh fmt -- --check          -> clean
JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh clippy -p jcode -p jcode-base \
    -p jcode-app-core --lib --tests -- -D warnings                -> Finished, 0 warnings
JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh test -p jcode-base \
    --lib platform          -> ok. 12 passed; 0 failed; 1 ignored; 1210 filtered out
JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh test -p jcode-app-core \
    --lib server::socket_tests -> ok. 21 passed; 0 failed; 0 ignored; 1148 filtered out
JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh test -p jcode --lib cli::
                             -> ok. 225 passed; 0 failed; 0 ignored; 0 filtered out
JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh test -p jcode-app-core \
    --lib shutdown          -> ok. 42 passed; 0 failed; 0 ignored; 1127 filtered out
JCODE_REMOTE_CARGO=0 scripts/dev_cargo.sh test -p jcode-base \
    --lib background        -> ok. 44 passed; 0 failed; 0 ignored; 1179 filtered out
```

Repository budget scripts (run-only; these are protected paths and were never
edited):

```text
scripts/check_code_size_budget.py       -> exit 0 (commands.rs flat at 3259 LOC)
scripts/check_test_size_budget.py       -> exit 0 ("budget improved")
scripts/check_panic_budget.py           -> exit 0 (total=56 files=24)
scripts/check_swallowed_error_budget.py -> exit 0 ("budget improved";
    src/cli/commands.rs 24 -> 23, since the SIGKILL `let _ =` became a match)
```

The shutdown and background suites are regression cover: both are existing
callers of the process-group signal path that this node left on
`signal_detached_process_group` (unchanged semantics).

## 7. Residue

The fixtures reap every process they create: the descendant test waits for the
descendant PID to disappear and asserts its marker file never appeared, the
fallback tests `wait()` on the child and assert the terminating signal, and the
setsid tests kill and wait their children. Temp state lives in `tempfile`
directories that drop with the test.
