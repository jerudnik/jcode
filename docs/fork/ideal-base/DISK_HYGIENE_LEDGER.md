# Disk-hygiene ledger (node-keyed cleanup triggers)

Recorded: 2026-07-23. Owner: coordinator (durable-state hygiene, thematically
F25 "centralize socket/metadata cleanup" + S03 "final disposition").

## Why this file exists

The F17 CI-validation campaign spun up many throwaway `CARGO_TARGET_DIR`,
`CARGO_HOME`, scratch-`HOME`, and worktree build caches. On 2026-07-23 the
data volume hit **99% (4.4 GiB free)**; an emergency sweep of cold standalone
build/scratch dirs recovered **~55 GiB** (see "Already reclaimed" below).
The remaining large targets are **still load-bearing** for in-flight
verification (F17 reconciliation, F21 clean-state gate) and must NOT be
deleted on sight. This ledger records **what to delete and the exact node/event
that makes each deletion safe**, so the cleanup happens deliberately instead of
as another emergency.

Rule of engagement: never delete a live git worktree (`git worktree list`)
or an evidence bundle referenced by a node's `evidence[]`. Build `target/`
and cargo caches are always reversible (a rebuild rebuilds them); worktrees
and evidence are not.

## Delete-when table

| # | Target | Size | Kind | Owner node | Delete WHEN | Reversible? |
|---|--------|------|------|-----------|-------------|-------------|
| 1 | `/private/tmp/jcode-f17-final/target` | ~20 GiB | build cache inside accepted-F17 worktree | F17 | F17 checkpointed `accepted` into STATE.json AND reconciliation (step 1 of report) confirms `68a5ecbf5` is canonical. Delete the `target/` only; keep the worktree until #6. | yes (rebuild) |
| 2 | `/Users/jrudnik/labs/jcode/target/debug` | ~43 GiB | primary debug build cache | F21 | Only when NOT mid-build. F21 requires a clean-state build twice anyway, so time this with the first F21 clean run. `cargo clean -p <crate>` or drop `debug/` wholesale. | yes (rebuild, ~10-20 min) |
| 3 | `/Users/jrudnik/labs/jcode/target/selfdev` | ~12 GiB | selfdev harness build cache | F21 | When no selfdev build/reload is queued. Rebuilt on next `selfdev build`. | yes (rebuild) |
| 4 | `/private/tmp/jcode-f17-inject-{tui,app-core,final-4f12}`, `jcode-f17-base-smooth`, `jcode-f17-synth-check` | ~227 MiB each (~1.1 GiB) | ci-proof git worktrees | F17 | After F17 accepted AND its CI proof is no longer needed. Remove with `git worktree remove <path>` (NOT `rm -rf`, to keep worktree metadata consistent). `f17-inject-*` map to branches `ci-proof/f17-tui`, `ci-proof/f17-app-core`; drop those branches too if the PR is closed. | worktrees: re-addable from commits; branches: pushed to origin |
| 5 | `~/.jcode/builds/versions/*` (stale) | ~4.5 GiB | old selfdev binary versions | F26 | F26 ("sweep dead PID markers / liveness-aware state") is the natural home for a builds-version GC. Keep `current`, `shared-server`, `stable`; prune versions with no live PID and older than the retained set. | yes (rebuilt on demand) |
| 6 | `/private/tmp/jcode-f17-final` (whole worktree) | ~20 GiB (mostly #1) | accepted-F17 evidence source | S03 | Final disposition only. The frozen evidence bundle `/tmp/f17-node-75f08991e4ec-evidence` (checksummed) is the durable record; once F17 evidence is cited by path+checksum in `evidence/F17/`, the worktree itself is redundant. `git worktree remove`. | source re-checkoutable from `68a5ecbf5` |
| 7 | `~/.jcode/logs/*` (rotated) | ~2.0 GiB | session/server logs | F25 | Bound retention per F25 "bound terminal control-log retention". Safe to trim logs older than the current investigation window now; formalize a cap under F25. | yes (regenerated) |
| 8 | `~/.cargo/registry/{cache,src}` | ~1.8 GiB | downloaded crate sources | none (opportunistic) | Any time under real pressure: `cargo cache -a` or delete `registry/src` (re-downloaded on next build). Lowest priority; shared across all repos. | yes (re-download) |
| 9 | `/private/tmp/jcode-desktop-*.sock` + malformed swarm state | ~0 B (churns) | leaked test sockets | F25 | Continuously. F25 owns "centralize socket/metadata cleanup"; a startup/periodic sweep should unlink dead sockets. Swept manually 2026-07-23; recurs on every desktop test run until F25 lands. | n/a |

## Sequencing notes

- **Do nothing that forces a rebuild right now** (explicit user constraint
  2026-07-23). Rows #2/#3 are the biggest wins (~55 GiB) but are deferred to
  F21's clean-state runs, which rebuild anyway, so the cost is absorbed for
  free there.
- **Row #1 (20 GiB) is the best near-term reclaim** that does not disturb the
  live repo build: it only rebuilds the *frozen* worktree, which is idle.
  Unblocked the moment F17 reconciliation (report step 1) picks `68a5ecbf5`
  as canonical.
- **Worktree removals (#4, #6) must use `git worktree remove`**, never
  `rm -rf`, or `git worktree list` and `prune` drift. If a dir was already
  `rm`'d, run `git worktree prune` to reconcile.
- Emergency floor: if free space drops below ~10 GiB again before these nodes
  land, rows #1, #7, #8 are all individually safe to take immediately (idle
  worktree cache, rotated logs, re-downloadable registry) without touching the
  live repo build or any evidence.

## Already reclaimed (2026-07-23 emergency sweep, ~55 GiB)

Cold standalone dirs (not worktrees, no live process, mtime Jul 21-22),
deleted by `rm -rf`:
- `/private/tmp/jcode-f17-target` (27 GiB), `jcode-f17-ci-repro-target`
  (8.8 GiB), `jcode-f17-proof-target` (5.1 GiB), `jcode-f17-final-target`
  (4.4 GiB), `jcode-f17-final-injected-red-target` (2.7 GiB),
  `jcode-f17-base-target` (1.4 GiB), `jcode-target-f17-macos` (1.0 GiB),
  `jcode-f17-cargo-home` (673 MiB), `jcode-f17-final-cargo-home` (595 MiB).
- `/tmp/f17-*-home.*`, `f17-pid-marker-targeted.*`, `f17-deadpid-repro.*`,
  `f17-workspace-preflight*` scratch HOMEs (~3.5 GiB).
- Dead `/private/tmp/jcode-desktop-*.sock` (0 B).

Preserved: all live `git worktree list` entries, and the checksummed evidence
bundles under `/tmp/f17-*-evidence*` (~462 MiB) referenced by the F17 handoff.

Result: data volume 4.4 GiB free (99%) -> 59 GiB free (81%).
