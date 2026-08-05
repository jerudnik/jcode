# D01-FIX-4: the ambient tier gate keys on a session id nothing registers

The ambient tier gate refuses tier-2 tools (write, bash, edit) when a session is
unattended. Its first line is the whole defect:

```rust
if !is_ambient_session_registered(session_id) {
    return Ok(());          // ungated
}
```

The gate is keyed on a *registered session id*. Two paths run with no human
present but on a session id nothing ever registers, so both fall through this
early return and are ungated.

## The two ungated paths, re-derived

Both were confirmed by reading the code, then by a failing test that drives the
real production path rather than the gate function.

| path | session id it runs on | registered before this change |
|---|---|---|
| a subagent worker spawned by an ambient parent | fresh id from `Session::create` in `run_subagent_worker` | no |
| the overnight coordinator | `child.id`, created by `create_coordinator_session` | no |

An overnight run is unattended *by construction*: it is scheduled to run while
the user is asleep. A subagent worker is unattended only when its parent is.

## Why inheritance rather than unconditional registration

Registering every spawned child would gate an interactive user's subagent and
break ordinary use. Registering none leaves the escalation open. So the subagent
seam inherits:

```rust
AmbientSessionGuard::inherit(&parent_session_id, worker_session_id)
```

which registers the child *only if the parent is itself registered*, and returns
`Option<Self>` so an interactive spawn allocates no guard at all. The overnight
path registers unconditionally, because there is no interactive case.

The guard registers for the **whole supervisor loop**, not per turn, so the `?`
returns inside the loop still unregister on drop.

## Tests, and the controls that make them mean something

Six tests. Three drive the real production entry points (`run_subagent_worker`,
`run_supervisor`), so they fail if the guard exists but is never wired in, which
is the defect they exist to prevent.

| test | asserts |
|---|---|
| `guard_inherit_registers_child_when_parent_is_ambient` | an inherited worker is registered and refused a tier-2 write |
| `guard_inherit_leaves_child_ungated_when_parent_is_interactive` | an interactive parent's worker is **not** gated |
| `inherited_and_registered_sessions_still_run_tier_one_tools` | read/grep/glob/ls/todo still run unattended |
| `subagent_worker_inherits_the_gate_from_an_ambient_parent` | e2e: the worker cannot write |
| `overnight_supervisor_gates_a_tier_two_tool` | e2e: the coordinator cannot write |
| `overnight_supervisor_unregisters_its_session_on_exit` | the registration does not leak past the run |

Five controls, each planted from a backup, **confirmed on disk before its exit
code was read**, and restored byte-identical (`diff -q`). Each fails on a
*different* assertion, so no two controls prove the same thing:

| control | mutation | failed at | on |
|---|---|---|---|
| A | delete the `inherit` call from `subagent.rs` | `runner_tests.rs:485` | worker actually wrote the file |
| B | make `inherit` register unconditionally | `tests.rs:546` | the *interactive* assertion, while the gating half still passed |
| C | delete the guard from `run_supervisor` | `runner_tests.rs:523` | coordinator actually wrote the file |
| D | register without the RAII guard (leak it) | `runner_tests.rs:558` | registration outlived the run |
| E | make `AutoAllowed` refuse too | `tests.rs:576` | tier-1 `read` was refused |

Control B is the load-bearing one: the plausible wrong fix (register every
child) looks correct without a counter-check for the interactive case. Control E
is the acceptance side: every other test asserts something is *refused*, so a
"fix" that refused everything would pass them all while making an ambient agent
useless.

Controls A and C were re-run against the final tree after the file split below,
since the code moved after they were first verified.

## An overclaim corrected

The probes-extraction approach in the first attempt was described as free. It
was not: it moved 11 swallowed-error-like patterns into a **new production
file**, which the swallowed-error ratchet rejects on path novelty alone even
though the combined count was provably unchanged (30 at HEAD, 30 after). The
counts were relocated, not added, but the gate is about production surface, not
totals. Corrected by splitting the **test module** instead.

## The file split, and why it moved tests rather than code

`overnight.rs` was already over the 1200-LOC cap (1275). Adding the guard grew
it, tripping the code-size ratchet. Re-baselining with `--update` was not an
option, so something had to move.

The size ratchet counts **raw lines including the test module**, while the
swallowed-error ratchet **skips test files**. So moving the 228-line test module
to `overnight/tests.rs` satisfies the size gate while moving **zero production
lines**:

| | before | after |
|---|---|---|
| `overnight.rs` | 1275 | 1101 |
| `overnight/tests.rs` | n/a | 234 |
| production lines relocated | | **0** |
| swallowed-error count | 30 | 30 |

All 8 pre-existing overnight tests were verified preserved by name across the
move (none lost, none added).

Checked before the split, because a previous field design died on it:
`EXPECTED_FILE_COUNTS` in `check_critical_path_budget.py` only fails on a
*decrease*, and `overnight.rs` is outside all five `CRITICAL_PATHS` domains, so
no digest-pin cascade.

## Validation

- `cargo test -p jcode-app-core`: 1199 passed, 1 failed, 23 ignored.
- The single failure is `debug_tool_selfdev_reload_returns_promptly_for_direct_execution`
  ("Could not find jcode repository directory"). **Pre-existing, not mine**:
  proven by stashing all changes and reproducing it on an unmodified tree.
- `scripts/preflight.sh`: all 17 gates pass, including the swallowed-error,
  code-size and test-size ratchets, rustfmt, and clippy.
