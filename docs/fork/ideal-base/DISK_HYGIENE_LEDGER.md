# Disk-hygiene ledger (node-keyed cleanup triggers)

Recorded: 2026-07-23. Owner: coordinator (durable-state hygiene, thematically
F25 "centralize socket/metadata cleanup" + S03 "final disposition").

## Status: F17 campaign debt retired

The F17 CI-validation campaign spun up many throwaway `CARGO_TARGET_DIR`,
`CARGO_HOME`, scratch-`HOME`, and worktree build caches. Those are now gone:

- The ~55 GiB of cold standalone build/scratch dirs were swept in the
  2026-07-23 emergency reclaim.
- The F17 ci-proof git worktrees (`jcode-f17-final`, `jcode-f17-inject-*`,
  etc.) have been removed; `git worktree list` shows only the live repo.
- The `/tmp/f17-*` and `/private/tmp/jcode-f17-*` session scratch (~1.3 GiB,
  including the handoff evidence bundles, now superseded by the committed,
  self-contained `evidence/F17/` tree) was deleted on F17 acceptance.
- Leaked `/private/tmp/jcode-desktop-*.sock`: currently 0.

Data volume as of this update: ~67 GiB free (79%). No emergency pressure.

Rule of engagement (unchanged): never delete a live git worktree
(`git worktree list`) or an evidence bundle referenced by a node's
`evidence[]`. Build `target/` and cargo caches are always reversible (a rebuild
rebuilds them); worktrees and evidence are not.

## Remaining live triggers

| # | Target | Size | Kind | Owner node | Delete WHEN | Reversible? |
|---|--------|------|------|-----------|-------------|-------------|
| 1 | `/Users/jrudnik/labs/jcode/target/debug` | ~40+ GiB | primary debug build cache | F21 | Only when NOT mid-build. F21 requires a clean-state build twice anyway, so time this with the first F21 clean run. `cargo clean -p <crate>` or drop `debug/` wholesale. | yes (rebuild, ~10-20 min) |
| 2 | `/Users/jrudnik/labs/jcode/target/selfdev` | ~12 GiB | selfdev harness build cache | F21 | When no selfdev build/reload is queued. Rebuilt on next `selfdev build`. | yes (rebuild) |
| 3 | `~/.jcode/builds/versions/*` (stale) | ~4.5 GiB | old selfdev binary versions | F26 | F26 ("sweep dead PID markers / liveness-aware state") is the natural home for a builds-version GC. Keep `current`, `shared-server`, `stable`; prune versions with no live PID and older than the retained set. | yes (rebuilt on demand) |
| 4 | `~/.jcode/logs/*` (rotated) | ~2.0 GiB | session/server logs | F25 | Bound retention per F25 "bound terminal control-log retention". Safe to trim logs older than the current investigation window now; formalize a cap under F25. | yes (regenerated) |
| 5 | `/private/tmp/jcode-desktop-*.sock` + malformed swarm state | ~0 B (churns) | leaked test sockets | F25 | Continuously. F25 owns "centralize socket/metadata cleanup"; a startup/periodic sweep should unlink dead sockets. Recurs on every desktop test run until F25 lands. | n/a |
| 6 | `~/.cargo/registry/{cache,src}` | ~1.8 GiB | downloaded crate sources | none (opportunistic) | Any time under real pressure: `cargo cache -a` or delete `registry/src` (re-downloaded on next build). Lowest priority; shared across all repos. | yes (re-download) |

## Sequencing notes

- **Do nothing that forces a rebuild without cause.** Rows #1/#2 are the
  biggest wins but are deferred to F21's clean-state runs, which rebuild
  anyway, so the cost is absorbed for free there.
- **Worktree removals must use `git worktree remove`**, never `rm -rf`, or
  `git worktree list` and `prune` drift. If a dir was already `rm`'d, run
  `git worktree prune` to reconcile.
- Emergency floor: if free space drops below ~10 GiB, rows #4 and #6 (rotated
  logs, re-downloadable registry) are safe to take immediately without touching
  the live repo build or any evidence.
