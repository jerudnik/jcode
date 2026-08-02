# R09 — remote session stalls on "loading session…"

Source issue: `docs/fork/ideal-base/human-noticed-issues/REMOTE_LOADING_SESSION_TTL.md`

The client sat on `loading session…` and then advised `/restart`, while the
server was healthy and would have answered. Both halves of that are the same
defect shape this program keeps finding: **the system reported something
true-sounding that wasn't**. The cache reported "stale" for an entry that was
still correct, and the client reported "unavailable" for a session that was
merely slow.

## What was verified before changing anything

The stall chain was confirmed end to end in shipped code, not inferred:

`Subscribe` → `try_available_models_snapshot` → `try_available_models_updated_event`
→ `available_models_updated_event_from_agent` → `model_catalog_snapshot`
→ `model_routes` → `fresh_routes_memo_entry`

Every hop is synchronous on the server's single sequential `match` loop
(`crates/jcode-app-core/src/server/client_lifecycle.rs:88`), so one slow route
build blocks the client's bootstrap.

Three facts constrained the fix:

1. **The TTL is not the correctness guard.** `auth_pricing_generation` and
   `CATALOG_GENERATION` are bumped on auth change and on prefetch/refresh
   completion (`invalidate_routes_memo_globally`, `provider/mod.rs:1175` and
   `:2124`), which invalidates every entry immediately. The TTL only bounds
   how long a *still-valid* entry may be reused.
2. **The build is local synchronous work** (credential and config reads), not
   network fetches.
3. **`MultiProvider` is not `Clone`**, so there is no owned handle to hand to a
   background refresh task. This ruled out the background-refresh approach the
   source doc suggested; the fix instead never queues a caller behind an
   in-flight build.

## Measurement, not reasoning

The old TTL was a fixed `3s`. Taken from `~/.jcode/logs/*.log`
(`[TIMING] model_routes`), n=96:

| stat | ms |
| --- | --- |
| min | 516 |
| p50 | 3185 |
| p90 | 3691 |
| p99 / max | 17039 |

**59.4% of builds took longer than the TTL that cached them.** The median build
outlived its own cache entry, so the steady state was: build for ~3.2s, cache
for 3s, expire, rebuild. The cache was reporting "stale" about work that was
still perfectly valid.

## The fix

`crates/jcode-base/src/provider/mod.rs`:

- `OBSERVED_MAX_ROUTES_BUILD_MS` atomic, fed by `record_routes_build_duration()`
  around `multiprovider_model_routes`.
- `routes_memo_ttl()` = observed max × `ROUTES_MEMO_TTL_BUILD_MULTIPLE` (10),
  clamped to `ROUTES_MEMO_MIN_TTL` (30s) / `ROUTES_MEMO_MAX_TTL` (600s). The TTL
  is now derived from what builds actually cost on this machine rather than from
  a guess.
- Single-flight lock changed from `.lock()` to `.try_lock()`. On `WouldBlock`,
  serve `generation_current_routes_memo_entry(&shared_key)` if one exists. Only
  the very first build in a process has no honest alternative and still blocks.
- `generation_current_routes_memo_entry()` returns an entry that is
  content-valid regardless of TTL age, so serving during a build still respects
  generation invalidation.

`crates/jcode-tui/src/tui/app/remote.rs` (gate 4): the watchdog no longer
advises `/restart` for a session that is merely slow. Whether the last history
re-request was *accepted by the server* is recorded in
`remote_history_recovery_last_send_ok`; a successful send proves the socket is
alive. Connected-but-slow now reports "still starting up… no need to restart";
a genuinely failed send still advises `/restart`.

## Gates

All four in `crates/jcode-base/src/provider/tests/routes_memo_ttl.rs` and
`crates/jcode-tui/src/tui/app/remote_tests.rs`:

| gate | test | control |
| --- | --- | --- |
| 1 | `r09_gate1_ttl_exceeds_every_measured_build_time` | restoring the 3s const fails: `TTL 3000ms must exceed the measured p99 build of 17039ms` |
| 1 | `r09_gate1_ttl_is_derived_from_measurement_not_a_fixed_guess` | — |
| 2 | `r09_gate2_a_request_is_not_queued_behind_an_in_flight_build` | restoring blocking `.lock()` **hangs** the test (caller queues behind the build) |
| 3 | `r09_gate3_serving_during_a_build_still_respects_generation_invalidation` | — |
| 4 | `r09_gate4_slow_startup_is_reported_as_slow_not_as_unavailable` | restoring the old message fails on its exact text (see below) |

Full suites green: `jcode-base --lib` 1230 passed, `jcode-tui --lib` 1884 passed.

### A control caught a worthless test again

Gate 2's first version **passed with the fix reverted** — it proved nothing. The
priming build left a TTL-*fresh* entry (30s floor), so the fast path returned
before the build lock was ever reached; the test never executed the code it
claimed to cover. Fixed by aging both the shared and instance memos past the TTL
without touching generations. Only after that repair did the control do real
work: with blocking `.lock()` restored, the test hangs, which is exactly the
user-visible stall being fixed.

This is the third bad test of mine caught by insisting the control fail first.

### Gate 4 control output

With the old always-`/restart` branch restored:

```
a connected-but-slow server must not be reported as needing a restart:
⚠ Still loading session… the server hasn't sent the conversation history.
This usually clears on its own; if it persists, run /restart to reconnect.
```

Gate 4 asserts **both** branches in one test on purpose: a fix that always says
"still starting up" would be just as much of a lie as the old always-`/restart`
message, and only the contrast catches that.

## Not verified

- **No live remote-session reproduction.** The stall was diagnosed from shipped
  code plus 96 timing samples, and the fix is covered by unit gates, but I did
  not stand up a slow remote server and watch the client recover. Gate 4's
  connected/disconnected split is exercised through app state, not a real
  socket.
- The 10× multiple and the 30s/600s clamps are judgement calls anchored to the
  measured p99; they are not themselves derived from a failure threshold.

## Re-deriving the measurement

The TTL is only as good as the distribution behind it, so the numbers above are
re-derivable rather than quoted from a scrollback:

```sh
grep -h '\[TIMING\] model_routes:' ~/.jcode/logs/*.log \
  | grep -oE 'total=[0-9]+ms' | grep -oE '[0-9]+' > /tmp/r09_samples.txt
python3 - <<'EOF'
xs = sorted(int(l) for l in open('/tmp/r09_samples.txt'))
pct = lambda p: xs[min(len(xs) - 1, int(len(xs) * p))]
over = sum(1 for x in xs if x > 3000)
print(f"n={len(xs)} min={xs[0]} p50={pct(.50)} p90={pct(.90)} p99={pct(.99)} max={xs[-1]}")
print(f"over the old 3000ms TTL: {over}/{len(xs)} = {100 * over / len(xs):.1f}%")
EOF
```

Verified by running it: `n=97 min=516 p50=3198 p90=3691 p99=17039 max=17039`,
`over the old 3000ms TTL: 58/97 = 59.8%`. That is one sample more than the n=96
reading taken earlier in the session, because the binary kept running and logged
another build; the shape is unchanged and the p99 that sizes the TTL is the same
17039ms. The first version of this snippet that I wrote into the doc matched
**zero** lines (it grepped a `[^m]*` span that stopped before `total=`), which is
exactly why it is recorded here as executed output rather than as a command that
ought to work. A machine with faster credential reads will print a
smaller distribution and a correspondingly smaller derived TTL, which is the
point of deriving it rather than pinning it: the constant that was wrong here
was wrong *because* it was a single number chosen for every machine.
