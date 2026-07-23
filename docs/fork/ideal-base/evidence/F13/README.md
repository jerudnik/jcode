# F13 Evidence: Cap verification under concurrency, failure, cancellation, restart

Date: 2026-07-20. Verifier: independent opus-class worker. Target: F12 caps
(accepted a1c9075af): pooled MCP child cap (`crates/jcode-base/src/mcp/pool.rs`
ensure_connected leader path, ~707-786) and background live-task cap
(`crates/jcode-base/src/background.rs` SpawnSlot + spawn_with_notify ~714-761,
insert/drop ~953-958).

## Approach taken

1. Fresh full runs of both suites (`nix develop -c cargo test -p jcode-base --lib ...`;
   `scripts/dev_cargo.sh` failed outside the nix shell in this session's PATH,
   nix at /nix/var/nix/profiles/default/bin worked).
2. Dynamic concurrency probe: standalone cargo project under
   `evidence/F13/probe/` (path-dep on `crates/jcode-base`, no files in
   `crates/` touched) driving the PUBLIC background API with a 16-way
   concurrent spawn burst at cap=2, cancellation + capacity-release check,
   and 50 mid-setup future-drop races. Source: `concurrency_probe.rs`
   (mirrored at `probe/src/main.rs`), runner: `run_probe.sh`.
   The MCP pool cap could not be probed dynamically (constructor and
   `set_pooled_cap_for_tests` are `pub(crate)`, and connects need real child
   processes), so gate 1 for the pool rests on the interleaving proof below
   plus the existing serial fixture `pooled_child_cap_refuses_then_releases_after_disconnect`.
3. Code-inspection interleaving proof for both caps (below).
4. Restart reconciliation read-through (gate 2, below).

## Fresh test runs

- `cargo test -p jcode-base --lib mcp`: **ok. 48 passed; 0 failed** (5.39s)
- `cargo test -p jcode-base --lib background`: **ok. 44 passed; 0 failed** (3.79s)

## Probe results (background cap, dynamic)

See `probe_output.txt`:

```
burst: accepted=2 refused=14
release-after-cancel: ok
cancel-race: full cap (2) still admittable, no leak
ALL INVARIANTS HELD
```

Exactly cap (2/2) admitted in the 16-way burst (no over- OR under-admission),
all 14 refusals carried the explicit cap-refusal reason, capacity fully
released after cancel, and full cap admittable again after 50
dropped-mid-setup spawn futures (no reservation leak). Loop run: **10/10
consecutive executions passed with identical results** (flake hunt, no
failures observed).

## Interleaving proof (gate 1)

Notation: for the pool, C = `clients.len()`, F = `connecting.len()` (leader's
own entry included, then `saturating_sub(1)`); admission iff
`C + (F-1) < cap`. For background, M = live map len, R = `in_flight_spawns`
(own reservation excluded); admission iff `M + (R-1) < cap`.

| # | Scenario | Interleaving | Why safe |
|---|----------|--------------|----------|
| P1 | N leaders, distinct servers, cap boundary | Each leader inserts into `connecting` (begin_connect, under one mutex) BEFORE its cap check. Leader k reads F >= k. | Each in-flight connect reserves a slot via its `connecting` entry, so at most `cap - C` leaders pass. Mutex serializes insertions; no two leaders can both see the same free slot. |
| P2 | Leader transitioning connecting->clients while another checks | Checker reads F first, C second (pool.rs 725-743 comment). Transition order is: insert `clients` (finish_connect 624-626) THEN remove `connecting` (guard drop 645). | Transitioner is visible in at least one map at any moment. Worst case double-counted (in both) -> conservative refusal, never over-admission. |
| P3 | Connect failure mid-flight | finish_connect Err arm (632-641) records `last_errors`, then guard Drop removes `connecting` entry and wakes waiters. No client inserted. | Slot (connecting entry) is released exactly once via RAII; no capacity consumed, no leak. Cooldown suppresses hot retry. |
| P4 | Cancellation mid-connect (future dropped across the `connect_with_tracker` await, pool.rs 768-777) | `guard` (ConnectingEntryGuard) is a local held across the await; drop of the future drops the guard, removing the entry and waking waiters. | RAII: reserved slot released, waiters not stranded. No client exists yet, so nothing leaks. |
| P5 | Cap refusal path | Refusal (745-766) drops guard explicitly, writes back-dated `last_errors` (no 30s cooldown penalty). | Slot released; immediate retry after capacity frees. |
| B1 | Burst of spawns at cap boundary | Each spawner does `SpawnSlot::reserve` (fetch_add SeqCst) BEFORE reading R-then-M (721-733). Spawner k sees R >= k. | At most `cap - M` spawners pass. Atomic counter gives total order on reservations. Verified dynamically by probe part A. |
| B2 | Spawner transitioning slot->map while another checks | Checker reads R first, M second. Transition is: insert map (953-956) THEN drop slot (958). | Seen in at least one place; double-count refuses conservatively. Same ordering argument as P2, comment at 722-728. |
| B3 | Refusal / persistence-failure paths | Cap refusal (735-760) and initial-persist failure (819-833) both return before/while `spawn_slot` is live; RAII Drop (99-104) decrements. | No leak on any early return. |
| B4 | Spawn future dropped mid-setup (caller cancels `spawn_with_notify` before insert) | `spawn_slot` is a local of the async fn; dropping the future drops it, decrementing the counter. Known residue (F12 round-3 non-blocking): if dropped after `tokio::spawn` (858) but before map insert (953), the spawned task still runs unregistered, and `registered_rx.await` (884) gets a channel-drop error, which `let _ =` swallows, so terminal persistence still proceeds. | Capacity is NOT leaked (counter released, task self-terminates and was never in the map, prune is a no-op). Verified dynamically by probe part C (50 drop races, full cap admittable after). Matches the review's non-blocking classification. |
| B5 | Instantly-finishing task vs. insert race | Task awaits `registered_rx` before terminal prune (884). | Prune cannot precede insert, so no permanent phantom map entry (which would leak capacity the other way). |

Conclusion gate 1: no interleaving admits more than `cap` and every early
exit releases its reservation via RAII. **PASS** (dynamic for background,
analytic + serial fixture for pool).

## Restart reconciliation (gate 2)

Both caps are purely in-memory: pool cap counts `clients` map + `connecting`
map; background cap counts `tasks` live map + `in_flight_spawns` atomic. All
four start empty on process start. **No cap ever reads on-disk state**, so
stale `Running` status files from a dead owner cannot count against the cap.

Sweep direction (disk hygiene, not capacity): `reconcile_orphaned_tasks`
(background.rs 501-534, called at daemon startup, server.rs 1262) plus lazy
self-heal in `list`/`status` (1203, 1223) finalize non-detached `Running`
files whose `owner_pid`/`owner_instance` is dead or a different instance as
`Failed` (`status_is_reconcilable_orphan`, 406-421). Detached tasks
(owner_pid=None, pid=Some) are exempt, correct since they outlive the daemon,
and they never enter the live map so they never consume cap. The live map is
populated ONLY by `spawn_with_notify` / `adopt` inserts in the current
process, never rebuilt from disk. Covered by passing tests:
`reconcile_marks_orphan_from_reloaded_process_failed`,
`reconcile_marks_orphan_from_dead_process_failed`,
`reconcile_leaves_non_orphans_alone`, `status_read_self_heals_orphaned_task`.
**PASS: restart cannot double-count stale owners.**

## Verdict

**PASS.** Gate 1 pass (probe + proof), gate 2 pass (analytic + existing
tests). No findings contradicting F12; the B4 residue matches the review's
known non-blocking follow-up (an unregistered-but-running task briefly evades
`bg list` live view, capacity itself is safe).

## Not checked

- MCP pool cap under REAL concurrent child spawns (needs pub(crate) access
  or real MCP servers; analytic proof only).
- Loop/flake hunting of the FIXTURE suites under load (probe itself was looped 10x clean; the cargo test suites ran once each).
- Cross-process contention (two daemons sharing an output dir).
- `adopt` path cap interaction (adopt bypasses the SpawnSlot check by design;
  not re-reviewed here).
