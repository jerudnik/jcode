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

## Reproduction

1. On the remote host, revert any synced file to an older commit's content
   (`git -C ~/.cache/remote-builds/jcode/jcode checkout <old-ref> -- <path>`).
2. Run `scripts/test_remote.sh -p <crate>` (or any dev_cargo build) from the repo.
3. Observe: fingerprint verification passes, cargo fails with errors about missing
   symbols from the newer commit.

## Acceptance

- A tampered/partially-synced remote tree is detected and either re-synced or refused,
  never passed to cargo.
- Test: plant a stale file remotely, run the wrapper, assert it either fixes or exits
  with a fingerprint-mismatch error.
