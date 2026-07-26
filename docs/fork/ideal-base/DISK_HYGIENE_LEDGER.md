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

## Update 2026-07-26 (F20c close-out)

`nix store gc` reclaimed **112 GiB** (45G -> 157G free; volume now ~68% used).
No disk pressure remains. `target/` sits at ~90 GiB and stays disposable per
rows #1/#2 (defer to F21's clean-state rebuilds). New post-F20c-merge facts
for row #3: `~/.jcode/builds` is 4.6 GiB across 24 versions, and the live
launcher chain is stranded on the *old* path
(`~/.local/bin/jcode -> ~/.jcode/builds/current/jcode -> versions/59521d509-dirty`,
a dirty Jul 20 build that wins PATH over the nix-managed v0.46.0; the
post-F20c fixed path `~/.jcode/current/jcode` does not exist yet). Republish
and relink **after** the F20c PR merges, then reclaim `builds/`. Six git
stashes also remain; triage before dropping.

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

---

## Provenance of "where did this come from?" state (2026-07-26)

Three pieces of local/remote state had no obvious owner when noticed. All three
are now traced. Recorded here because the cost of re-deriving provenance is
what makes people leave unexplained state alone indefinitely.

### `~/.jcode/ambient/queue.json` — was a defect, fixed

Held six scheduled items despite ambient mode being disabled
(`state.json` = `Disabled`, `total_cycles: 0`). Not user-created: every item
carried `created_by_session: "ambient"` and a uniform five-minute offset, the
shape the TUI ambient-widget test constructed before `4c2d66c21` made it
filesystem-free. A test suite was writing into live user state.

Undeliverable by construction: with ambient disabled the runner drains only
direct-delivery targets, and nothing surfaces the queue, so the items
accumulated silently. Last leaked entry (07-25 10:08Z) predates the fix
(10:29Z) by 21 minutes; nothing leaked after. Cleared, backup at
`~/.jcode/ambient/queue.testleak-backup.json`. The residual fragile mechanism
(a hand-rolled `JCODE_HOME` restore that a panic would skip) was replaced with
a shared RAII guard in `09faa4627`.

### `jerudnik/jcode-recovery-archive` — deliberate, keep

Created 2026-07-17 during fork normalization; documented in
`docs/fork/normalization/STATUS.md` and the `2026-07-17-post-promotion-checkpoint`
evidence package. Verified against GitHub rather than trusting the doc: still
**private**, still **42 branches**, `pushed_at` unchanged at 2026-07-17T20:57Z.
No script or workflow pushes to it. The 19 gitleaks hits were test/example
patterns already reachable from public `main`, so it introduced no new exposure.

| Target | Size | Kind | Delete WHEN | Reversible? |
|--------|------|------|-------------|-------------|
| `jerudnik/jcode-recovery-archive` (remote) | ~364 MB | one-shot private ref archive | **Not while the hard fork is in flight.** This is insurance against exactly the branch-history loss the fork risks. Revisit only after the fork lands and `main` is stable. | **no** — 41 local branches + a detached worktree tip; some tips may exist nowhere else |

### The six stashes — five are dead, one is live

Each has a deliberate descriptive message, so none are mystery state. Status
determined by reverse-applying each stash hunk against `HEAD`:

| Stash | Subject | Verdict |
|-------|---------|---------|
| `{0}` | `f17-local-variant-full` (76 files) | **Redundant** — byte-identical to branch `f17-local-dirty-backup`. Drop the stash, keep the branch. |
| `{1}` | F02 aborted after Anthropic 429 (7 files) | **Superseded** — F02 was accepted at `2b5607882` after 3 review rounds. This is the attempt that failed review; the shipped activity-lease system is live in 8 files. |
| `{2}` | config-hotpath part 3 | **Landed** — the exact `Config::load()` → `config()` change is in the tree. |
| `{3}` | config-hotpath part 2 (7 files) | **Landed** — all reads converted. The one remaining `Config::load()` in `inline_interactive/helpers.rs` is `save_agent_model_override`, a *write* that needs an owned mutable copy; correctly untouched. |
| `{4}` | config warn-once + sidecar dedup (4 files) | **Superseded by better** — both halves shipped in stronger form: `warn_unknown_config_keys_once_with` (injectable for testing) and a sidecar diagnostic that reports a `suppressed` count plus a recovery notice, neither of which the stash's plain `HashSet` did. |
| `{5}` | `latent-outcomes-agent-mesh.md` condensation | **LIVE** — the only one with unmerged content. The doc is still byte-identical to the stash's base (unchanged since `4920ba714`, 2026-07-02), so this 288→219 line condensation was never applied and never superseded. Apply or discard on the merits; it is a judgement call about the proposal, not a mechanical one. |

Dropping `{0}`–`{4}` reclaims negligible disk (stashes are cheap objects); the
value is removing five decoys so the one live stash is visible.
