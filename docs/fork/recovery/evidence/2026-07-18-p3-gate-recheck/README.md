# P3 strict one-turn pilot gate recheck (2026-07-18)

Re-evaluation of the strict one-turn bounded pilot gate against the integrated
tree at main HEAD `65175cff4`, with trusted R09 gates, isolated environment, and
no credentials/tools/UI/reload/compaction. Independent approval is preserved.

## Verdict: PASS re-affirmed (with recorded ratchet drift, non-blocking)

## 1. Original independent approval chain recomputed intact

- G2 pilot-gate review hash recomputes: `abb7b2694abccb0c32385fc552dcc29bf0eba854d439c5c43dc82ba4f3991e4f`
  (`reviews/2026-07-15-g2-pilot-gate-opus.md`).
- G5 evidence review hash and size recompute: `37f094d26b196612f2171de98d52238abb72bb8b69d59b149e7bb00999db86d3`, 15,077 bytes.
- G4 bounded-pilot evidence `SHA256SUMS` self-hash `b4692dc0...` matches; all
  members verify OK. Attempt-history `SHA256SUMS` self-hash `f1fa86fd...`
  matches; all members verify OK. Manifest shows 10/10 expected==actual exits,
  1 pilot observation, 0 forbidden output hits.

## 2. Integrated fixes present at current main HEAD

- R01/R03A fail-before-mutation preflight: `crates/jcode-app-core/src/server/client_lifecycle.rs:97` (definition), `:495` (caller returns before session construction).
- R02 fail-closed tier/auth: `crates/jcode-base/src/subscription_api.rs:46` (`tier_truth`), `:97` (authoritative denial), `:121` (malformed-response denial).
- R12 terminal provider-error evidence: `crates/jcode-app-core/src/agent/evidence.rs:143`, 8 call sites across `turn_loops.rs` and `turn_streaming_mpsc.rs`.
- Pilot fixture recovered onto main as `crates/jcode-app-core/src/recovery_pilot_tests.rs` (module `agent_tests`, `lib.rs:57`), commit `135b0ecd7`.

## 3. Fixture re-executed on current main HEAD: PASS

`cargo test -p jcode-app-core --lib agent_tests::recovery_pilot_one_fixture_route_subscribe_turn_evidence -- --exact --test-threads=1`

- Result: `ok. 1 passed; 0 failed`, 4.03s.
- `PILOT_OBSERVATION` emitted exactly once: account `acct_fixture`, tier `plus`
  live, auth `credential_present -> request_valid`, credential `oauth`, provider
  `jcode`, model `gpt-5.5`, route `jcode-subscription`, compatible handshake,
  distinct runtime projections, tools 0, memory disabled, telemetry disabled,
  usage 7/3/10, 4 evidence events, 4 replay events, terminal counts 1/1/1.
- Isolation confirmed by the fixture itself: fresh temp `JCODE_HOME`,
  `JCODE_NO_TELEMETRY=1`, symbolic `fixture-key` (asserted absent from raw
  evidence), adversarial ambient `JCODE_TIER=flagship` granted nothing.

## 4. One-turn compaction avoidance still holds

- `crates/jcode-compaction-core/src/lib.rs:19`: `RECENT_TURNS_TO_KEEP = 10`.
- `crates/jcode-base/src/compaction.rs:857-873` (`should_compact_with`): all
  three modes gate on `active.len() > RECENT_TURNS_TO_KEEP`; a one-turn fixture
  has `active.len() <= 1`, so no compaction trigger is reachable.

## 5. R09 trusted gates at current HEAD, no `--update`

| Gate | Exit | Result |
|---|---:|---|
| Classifier (`test_rust_production_filter.py`) | 0 | 17/17 OK |
| Wildcard re-export budget | 0 | total=16, green |
| Warning budget (`cargo check` via nix develop) | 0 | current=0 baseline=0 |
| Shell syntax (`bash -n scripts/*.sh`) | 0 | green |
| Diff check (`git diff --check`) | 0 | green |
| Panic budget | 1 | expected red; current 48 vs baseline 31 |
| Swallowed-error budget | 1 | expected red; current 3,075 vs baseline 2,987 |
| Production-size budget | 1 | expected red |
| Test-size budget | 1 | expected red |

## 6. Recorded drift (non-blocking, must not be laundered)

- Swallowed-error total is now **3,075**, one above the N2-frozen 3,074
  (growth: `src/cli/tui_launch.rs` 4 -> 5). Panic total 48 matches the N2
  QUALITY_DEBT register (the G4-era 46 is historical).
- Production/test size findings grew since N2 (e.g. `agent_tests.rs`
  1,321 -> 2,309 LOC; new oversized `client_lifecycle_tests.rs`,
  `comm_session_tests.rs`). All remain honestly red; no baseline changed.
- This drift is post-pilot debt growth owned by the touching seams under the
  QUALITY_DEBT no-growth policy. It does not retroactively invalidate the G2/G4/G5
  bounded-pilot approval, whose gate contract was expected-red exits without
  `--update`, which still holds.

## 7. Preservation

- Four stashes present; `refs/heads/vendor/upstream` = `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` (pinned merge base).
- Working tree clean except unrelated untracked `docs/fork/ideal-base/evidence/F01/` (other in-flight work).

## Claim boundary unchanged

This recheck re-affirms only the exact bounded one-turn question. It does not
widen the claim to live providers, real credentials, network, daemon/reload,
tools/MCP, memory, cancellation, retry, compaction, or UI.
