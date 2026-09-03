---
title: "Remote test sync fingerprint verifies commit identity, not tree content"
status: open
priority: high
owner: maintainers
opened: 2026-09-03
related:
  - scripts/remote_build.sh
  - scripts/test_remote.sh
  - docs/issues/selfdev-auto-reload-policy.md
---

# Remote test sync fingerprint verifies commit identity, not tree content

## Problem

`scripts/remote_build.sh` guards the remote source tree with a sync fingerprint of the
form `<git-commit-hash>:<diff-blob-hash>`. On 2026-09-03 a remote build on
`serious-callers-only` failed with E0432 (`jcode_storage::SessionInboxId`,
`durable_path`, `tag` not found) while the fingerprint line claimed a verified
`3c6c77814...:e69de29b...` match — `e69de29b` is the git blob hash of the **empty
file**, and the local commit `3c6c77814` compiles cleanly (built locally, CI green on
the PRs that compose it).

So the guard can pass while the remote tree is stale or partially synced: the tree had
C5's `crates/jcode-base/src/inbox/store.rs` but a `crates/jcode-storage/src/lib.rs`
predating the exports it imports. A commit-identity match does not imply tree-content
match, and rsync partial transfers / concurrent syncs from multiple sessions into one
per-host worktree dir can leave mixed trees behind.

## Desired behavior

The retained fingerprint must reflect **content**, not just commit identity. Options to
evaluate in a small planning pass:

1. `git write-tree` (requires committing the index) or `git stash create` style
   content-tree hash included in the fingerprint line, computed pre-sync locally and
   re-verified remotely post-sync.
2. Cheap per-file digest manifest (e.g. `git ls-files -z | xargs -0 shasum` sorted) —
   slower on huge trees but exact; jcode is ~hundreds of tracked files so this is fine.
3. Minimum fix: make the empty-diff sentinel (`e69de29b`) impossible to equal a real
   diff hash and rsync with `--checksum` instead of mtime+size (mtime alone missed the
   stale `lib.rs`).

Also consider: `--delete` on rsync so removed files don't linger in the remote tree,
and a per-worktree lock to serialize concurrent syncs into one remote dir.

## Root cause (verified 2026-09-03)

Two independent defects compound:

1. **Fingerprint is not content-aware** (as described above): a mixed remote tree
   claimed a verified match because the diff-hash component was the empty-blob
   sentinel. A forced full re-sync did NOT fix the build — because the content was
   already identical, rsync transferred nothing.
2. **Stale-rmeta poisoning survives re-sync**: during the mixed-tree window, cargo
   built `libjcode_storage*.rmeta` from a stale `lib.rs` (missing the root re-exports).
   rsync `-a` preserves source mtimes, so the synced `lib.rs` stayed *older* than the
   poisoned rmeta; cargo's mtime comparison considered jcode-storage fresh forever and
   kept reusing the bad rmeta. Every unresolved name was exactly a root re-export.

The fix that worked: `touch` the synced source file on the remote so its mtime exceeds
the rmeta's. Cargo invalidated jcode-storage, rebuilt it, cascaded to jcode-base, and
E0432 disappeared (verified: `Finished selfdev profile in 1m 33s`).

## Fix direction

- After rsync, `touch` every file rsync actually transferred (rsync `-i`/itemize
  output lists them) — precise invalidation even when the fingerprint lies.
- Make the fingerprint content-aware (`git write-tree`-style tree hash) so a mixed
  tree can never claim a match; on fingerprint change/full re-sync, touch the synced
  tree or `cargo clean -p` the affected workspace members.
- Add `--delete` to rsync and a per-host sync lock so concurrent sessions cannot
  interleave two syncs into one remote dir.

## Related defect: sync-back ignores --target (2026-09-03)

`scripts/remote_build.sh` sync-back looks for artifacts under `target/<profile>/`
only. A build invoked with `--target <triple>` (as `scripts/ci_local.sh` does for
the release leg) writes to `target/<triple>/<profile>/`, so sync-back reports
"Skipping sync-back: target/release/jcode not found on remote" and the local
recipe then fails with `./target/<triple>/release/jcode: No such file or
directory`. Observed twice on 2026-09-03: session tiger fetched the artifact
manually, and a `just pre-pr` run on the apm-hygiene worktree failed its final
leg the same way after an otherwise green remote build. Sync-back must honor
the target triple when composing both the remote and local artifact paths.

## Reproduction

1. Poison state: sync a tree, then remotely revert one `.rs` file to older content
   and run a build (produces artifacts from mixed content), then restore the file
   with `rsync -a` (mtime stays old).
2. Run `scripts/test_remote.sh -p <crate>`: fingerprint verifies, E0432-style
   unresolved-import errors about re-exports persist across any number of re-syncs.
3. `ssh <host> touch <remote-tree>/<that file>` and rerun: green.
