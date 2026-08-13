# Remote build staleness: remote runs can test a tree that is not your tree

Status: root cause pinned; remediation in progress

## Symptom

An independent verifier ran the standard gate through `scripts/dev_cargo.sh`
(which delegates to `scripts/remote_build.sh` when a remote host is
configured). The remote run executed `cargo test -p jcode-tui --lib` on host
`serious-callers-only` against a tree that was **nine commits behind local
HEAD** — the entire test-remediation series was missing. Nothing in the
run's output indicated the divergence; results would have been reported as
if they described the local tree.

## Evidence

Checked at 2026-08-13 ~04:46 UTC, local HEAD `d85916421` (remediation series
`db3e2db37..f15a5b94f` all committed hours earlier):

- Remote checkout `~/.cache/remote-builds/jcode/jcode-base-check` lacked
  `with_center_code_blocks_override` in
  `crates/jcode-tui/src/tui/ui_messages/tests.rs` (added by `db3e2db37`) and
  still contained `fn test_remote_scroll_cmd_j_k_fallback` (deleted by
  `e868b48c8`). That content identifies the tree as pre-remediation base
  `dbe279c87`.
- The in-flight SSH command (captured from `ps`) carried
  `JCODE_BUILD_GIT_HASH=dbe279c87 JCODE_BUILD_GIT_DIRTY=0` — stamped hours
  stale, at a moment when the local repo was at `d85916421`.

The stale checkout was not an rsync failure. A detached local worktree existed
at `/private/tmp/jcode-base-check` on the exact stale commit
`dbe279c872bf924c2192b2b120884bc6e355d6f5`. Both wrappers resolve their source
from the script being executed, not from the caller's current checkout:

- `dev_cargo.sh` sets `repo_root` from `${BASH_SOURCE[0]}` and executes that
  checkout's `scripts/remote_build.sh`.
- `remote_build.sh` sets `LOCAL_DIR` from `$0`, then derives the default remote
  directory from `basename "$LOCAL_DIR"`.

Running the wrapper from that detached worktree therefore performed a fresh,
successful sync of the stale worktree into
`~/.cache/remote-builds/jcode/jcode-base-check` and stamped it with
`dbe279c87`. The remote output was internally consistent, but it did not name
the source fingerprint prominently enough for an operator comparing it with a
different checkout to notice.

Code reading also found a separate unsafe path: `remote_build.sh --no-sync`
sets `SYNC_SOURCE=0` and proceeds directly to the remote Cargo command without
checking that the retained remote directory matches the current local source.
`dev_cargo.sh` does not add `--no-sync`, there is no concurrent remote-run
guard that reuses an invocation, and per-purpose remote directories are chosen
only by `JCODE_REMOTE_DIR`, `--remote-dir`, or the source checkout's basename.
The fingerprint remediation below closes that independent stale-reuse path and
makes the source identity explicit on every run.

## Related, same wrapper, same shape

During the same session, implementers found `scripts/dev_cargo.sh fmt` runs
rustfmt **on the remote host** and does not sync results back: it reports
success while changing nothing locally (journaled in the remediation
IMPL_NOTES; workaround `JCODE_REMOTE_CARGO=0`). Both defects are the same
disease: the wrapper reports outcomes about a tree that is not the tree the
operator is working on, with no divergence check.

## Why this is dangerous

- A verification gate that silently tests the wrong tree converts "verified"
  into noise: false green on stale code, false red on already-fixed code.
- The failure is invisible precisely when it matters — the operator sees the
  familiar command succeed.
- Independent reviewers are the most likely victims: they run gates without
  having built local context about which tree artifacts should reflect.

## Stepwise remediation plan

1. **Reproduce and pin the root cause.** Instrument `remote_build.sh` to log
   `SYNC_SOURCE`, `REMOTE_DIR`, `LOCAL_DIR`, and the computed git hash per
   invocation; identify which path produced a no-sync run against
   `jcode-base-check`.
2. **Verify, don't trust: hash check on the remote.** The script already
   computes `local_git_hash`. After sync (or when skipping sync), compare an
   equivalent content fingerprint on the remote (e.g. hash of the rsync file
   list, or a `.jcode-sync-fingerprint` written during sync) and **fail
   loudly on mismatch** instead of running the command.
3. **Make sync-skipping explicit and visible.** Any branch that runs without
   a fresh sync must print the fingerprint it is reusing and its age.
4. **Fix fmt sync-back** (or refuse to run fmt remotely): mutating commands
   whose value is their local side effect must either sync results back or
   force local execution.
5. **Tests/guards.** A CI-side or script self-test: create a throwaway commit
   locally, invoke the wrapper, assert the remote fingerprint matches; assert
   fmt either changes local files or exits nonzero with guidance.

## References

- `scripts/remote_build.sh` — rsync at `[1/3] Syncing source files...`
  (`--delete`, excludes `.git`), metadata stamping from `$LOCAL_DIR`
  (`local_git_hash=...`), sync-back handling (`--sync-back` flags).
- `scripts/dev_cargo.sh` — remote delegation and down-cache logic
  (`remote_down_cache_*`).
- `/tmp/jcode-test-audit/remediation/IMPL_NOTES.md` (session artifact) — fmt
  no-sync-back journal entries and the `JCODE_REMOTE_CARGO=0` workaround.
- Sibling proposals from the same session:
  `docs/proposals/provider-confusion.md`,
  `docs/proposals/swarm-session-identity.md` — different subsystems, same
  theme: the system reports state the operator reasonably believes is about
  X while it is actually about Y.
