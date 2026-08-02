# F30-FIX-4: retire orphaned installer residue

Reviewed commit `7185a5ece`, published in merge `b88250783` (PR #90).

## Partially implemented, and the rejection is the substance

The node names three items. Two are stale; only one is a real defect. Recording
this honestly matters more than closing the node cleanly: acting on all three as
written would have deleted working, reachable production code because a
ten-day-old node description said so.

### Real: `scripts/lib/configure_path.sh`

Orphaned. Nothing sources it. Its own header describes keeping it in sync with
the inline copy in `install.sh` "because it is run via `curl ... | bash`". Both
that script and that distribution channel are retired. Removed, and added to
`RETIRED_PATHS` so it cannot return.

### Rejected: the `uninstall.sh` reference

`scripts/uninstall.sh` does not exist, and `configure_path.sh` contained no
reference to it. The stated linkage is simply not present in the tree.

### Rejected: `crates/jcode-build-support/src/paths.rs:1076-1080`

This is not an installer. It is `retired_layout_dir()` and
`retired_layout_residue()`, which enumerate pre-F20c residue so it can be
reported and cleaned. Production call sites:

```text
src/cli/commands/doctor.rs:249   retired_layout_dir
src/cli/commands/doctor.rs:167   retired_layout_report
src/cli/commands/doctor.rs:204   run_clean_retired_layout_command
```

reached from `run_doctor_command` via `jcode doctor --clean-retired-layout`.
Deleting this would remove **the mechanism that clears the retired layout**, not
the layout itself. The code even documents why removal is opt-in: those
directories can hold the only copy of the binary a user is currently running.

## Verification

Control, observed failing: recreating `scripts/lib/configure_path.sh` FAILS with

```text
retired path returned: scripts/lib/configure_path.sh
```

Suite 12 tests green at the time; hermetic derivation green. Re-verified on
published `main`: 13/13 OK.
