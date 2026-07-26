# F28 — jcode-tui test hermeticity, parallelism restored

## Outcome

All four `--test-threads=1` rails removed from `fork-ci.yml` (macOS and Linux,
`jcode-tui` and `jcode-app-core`). Verified over three consecutive full parallel
rounds per crate before the caps came off.

| crate | tests | rounds | failures |
|---|---|---|---|
| `jcode-tui` | 1867 | 3/3 green | 0 |
| `jcode-app-core` | 1136 | 3/3 green | 0 |
| `jcode-base` (regression check) | 1204 | 1 | 0 |

Raw output: `parallel-rounds.txt`, `render-lock-scan-after.txt`.

## What the scoping assumption got right, and what it missed

The node predicted "~28 render/cache-touching tests that currently skip the
render lock". The measured number is **zero**, and it is worth recording why the
estimate was so far off, because a naive scan reproduces it.

A first-pass scan reported **51** unlocked tests. Both large corrections came
from reading the backing stores rather than trusting call-site names:

1. **Lock acquisition is almost always indirect.** Only four call sites mention
   `lock_test_render_state` at all; the rest reach it through helpers such as
   `with_serialized_mermaid_state` or a `_lock()` fixture. Resolving the call
   graph transitively moved 47 tests from "unlocked" to "locked".

2. **Half the remaining hits were not shared state.** The side-panel
   markdown/render/debug caches are swapped to `thread_local!` under
   `#[cfg(test)]` in `ui_pinned.rs`, so tests touching only those cannot
   interfere across threads. Flagging them would be a false positive, and a
   scan that cries wolf gets ignored.

The mermaid registries do count, because `ACTIVE_DIAGRAMS`,
`STREAMING_PREVIEW_DIAGRAM`, and `IMAGE_STATE` live in the separate
`jcode-tui-mermaid` crate, where `jcode-tui`'s `cfg(test)` does not reach.
`WIDGETS_STATE` and `SLOW_FRAME_HISTORY` are plain statics with no test variant.

So the lock discipline was already in place. What was missing was anything
*enforcing* it, which is why `scripts/check_tui_render_lock.py` now runs in the
Quality Guardrails rail: the property is easy to regress and the symptom is an
intermittent failure that surfaces long after the commit that caused it.

The gate was proven able to fail before being trusted: injecting a test that
calls `clear_active_diagrams()` without the lock trips it, and removing the
injection returns it to zero.

## The two real bugs, both the same root cause

Neither failure that parallelism exposed was a hermeticity bug. Both were
`debug_assertions` used as a proxy for something it does not mean, and both were
silent because CI builds `dev` while local self-dev builds `selfdev`, which
**inherits `release` while pinning `opt-level = 0`**.

**1. `debug_assertions` as a proxy for "unoptimized"** — the headless
side-panel latency test asserted a 60fps (16ms) budget whenever assertions were
off, so unoptimized code was held to optimized timings. It failed at 16.55ms as
soon as it had to share CPU with the rest of the suite.

Cargo hands the real optimization level to build scripts only, so
`jcode-build-meta` (already a `jcode-tui` dependency, already forwarding build
facts this way) now exports `OPT_LEVEL` and `is_optimized_build()`. Verified in
both directions rather than assumed: `selfdev` reports `0/false`, `release`
reports `1/true`.

Two earlier attempts were discarded because measurement contradicted them: a
`cfg` nothing defines, and a runtime speed probe that assumed optimized and
unoptimized builds sit ~30x apart. Measured, they are 1.94 vs 3.58 ns/iter,
far too close to classify.

**2. `debug_assertions` as a proxy for "test build"** — both fault-injection
hooks in `jcode-base::session` were gated on it, so under `selfdev` they were
compiled out, the forced save failure never fired, and four
`client_disconnect_cleanup` tests asserted `Failed` while observing `Persisted`.

This one looked exactly like the env-toggle race the CI comment predicted. It
was not a race: it reproduced identically single-threaded. The hooks are
test-only by construction and now use
`cfg(any(test, feature = "test-support"))`, matching how the crate already gates
its other test-support surfaces, so they behave correctly under every profile
rather than under whichever one CI happens to pick.

## The `video_export` leak was real, and worse than "cosmetic"

The node described this as a cosmetic multi-App view-state leak. Reading the
code, it is an unconditional-reset bug: both export paths in
`crates/jcode-tui/src/video_export.rs` (single-session and swarm) set the
process-global mermaid video-export mode, then clear it several statements
later, with `?` error propagation in between. Any failed replay returned early
and left the flag set, changing how every subsequent render in the process
behaved. A panic leaked it too.

Replaced with a `VideoExportModeGuard` RAII scope, so the reset is unconditional
including on early return and panic. Both call sites converted; the only
remaining references to `set_video_export_mode` are inside the guard itself.

## Gate status

| gate | status |
|---|---|
| Render/cache tests hold the lock; static scan finds zero unlocked mutators | met (45 locked, 0 unlocked, scan in CI) |
| `video_export` multi-App view-state leak removed or proven inert | met: removed, replaced with an RAII guard |
| Full suite green over >=3 parallel rounds without `--test-threads=1` | met, both crates |
| fork-ci rails no longer need `--test-threads=1` | met, all four rails |
