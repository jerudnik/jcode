# Independent Grok-style R03A review: wire compatibility and subscribe handshake

Date: 2026-07-15 UTC  
Repository: `/Users/jrudnik/labs/jcode-seam-r03a`  
Reviewed head: `16921ace18cf5c25368a376357b7636478d3928f`  
Fixed comparison refs: fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`  
Constraint note: I did not read `/tmp/jcode-r03a-opus-review.md` and do not rely on its conclusions. I used source, tests, fixed-ref diffs, binding ledgers, and incident records only.

## Disposition

**Retain fork for R03A, but block pilot entry until enforcement and projection gaps are closed or explicitly narrowed.**

The fork is the only compared ref that implements protocol/build subscribe identity, `HandshakeCompatibility`, `HandshakeVerdict`, and TUI action on incompatible verdicts. Upstream and the merge base have no corresponding symbols, so there is nothing authoritative to adopt for this seam. However, the current fork behavior does not yet prove the full R03A safety claim because:

1. `build_hash` is only a compatibility token stamped from `jcode_build_meta::GIT_HASH`, not R01's full canonical dirty-build/source/channel identity.
2. The server emits an incompatible verdict but then continues to mutate/complete subscribe state. Safety depends on the client seeing and acting on the verdict before later events are processed.
3. The TUI path acts on the verdict, but the generic app-core `Client` advertises identity without enforcing or draining the handshake verdict in its higher-level API.
4. Existing temp-socket tests prove verdict emission/order, but not end-to-end refusal/re-exec behavior against a real client path without post-verdict attach side effects.

## Binding responsibilities and challenged assumptions

| Checkpoint | Evidence | Observation | Tag |
|---|---|---|---|
| R03A scope | `docs/fork/recovery/RESPONSIBILITIES.md:24` assigns R03A stable wire schema, protocol/build compatibility verdict, safe short/full hash comparison, legacy handling, and carriage of R01/R02 identity. It excludes source of build/provider truth, transport execution, and session recovery. | R03A can own wire carriage and compatibility policy, but cannot redefine R01 canonical identity or R02 provider truth. | pass |
| Cross-seam identity | `RESPONSIBILITIES.md:63-68` says R01 owns identity meaning, R03A evaluates compatibility on reconnect after R01/R04. `RESPONSIBILITIES.md:92-94` explicitly keeps R01 and R03A separate. | Any claim that `Subscribe.build_hash` is full build identity is overreach. R03A must project R01 truth, not create its own. | risk |
| Pilot prerequisites | `RESPONSIBILITIES.md:78-88` requires R01/R02/R03A/R12 full ledgers, R06A/R09/R07C/R13 prerequisites, and stops on live daemon, credentials, network, baseline update, or unowned identity writer. | This review only authorizes a bounded R03A fixture plan, not the pilot. | block |
| R00 overlay | `R00 ledger:26-31` requires fixed refs, provenance for equivalence, no stash replay, rollback/stop budgets. `R00 ledger:18-22` warns `vendor/upstream` is stale and upstream is comparison evidence only. | Fork/upstream conclusions below are fixed-ref and symbol-based. Upstream status alone is not authority. | pass |
| R01 boundary | `R01 ledger:35-41` defines canonical runtime identity as source provenance, executable provenance, and activation provenance. It states `Request::Subscribe.build_hash` is currently only the R03A compatibility projection and blocks pilot readiness until a projection contract proves it. | This is the decisive identity constraint. R03A's current `build_hash` cannot distinguish dirty same-commit builds or channel/source fingerprint differences. | block |
| R02 boundary | `R02 ledger:24-29` excludes R03A wire verdicts and preserves exact R02-to-R12 provider outcome. | R03A carries identity fields but must not infer provider/model/account compatibility. No R02 provider identity is currently carried in the R03A subscribe handshake. | gap |
| R09 overlay | `R09 ledger:24-30` forbids blanket `--update` and requires trusted gates to stay visible. | No gate baseline was changed. Targeted tests were read-only from source perspective. | pass |
| R11 overlay | `R11 ledger:24-29` requires append-only evidence and hashes for external incidents/artifacts. | This report records source evidence and test commands. No external untracked incident artifact was newly relied on beyond the checked-in architecture incident. | pass |

## Source evidence: wire schema and compatibility semantics

### Additive wire schema

- `crates/jcode-protocol/src/wire.rs:200-232` defines `Request::Subscribe` with optional `protocol_version: Option<u32>` and `build_hash: Option<String>`, both `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- `crates/jcode-protocol/src/lib.rs:15-26` defines `PROTOCOL_VERSION: u32 = 1` and says legacy clients advertise `None`, treated as compatible.
- Base and upstream exact `Subscribe` schema at fixed refs only contains `id`, `working_dir`, `selfdev`, `target_session_id`, `client_instance_id`, `client_has_local_history`, `allow_session_takeover`, and `terminal_env`. Command run:

```bash
git show 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d:crates/jcode-protocol/src/wire.rs | nl -ba | sed -n '102,126p'
git show 802f6909825809e882d9c2d575b7e478dce57d3b:crates/jcode-protocol/src/wire.rs | nl -ba | sed -n '102,126p'
git grep -n 'deny_unknown_fields' <ref> -- crates/jcode-protocol/src/wire.rs
```

Observed: no `deny_unknown_fields` in base, upstream, or head, so new clients sending extra subscribe fields to old servers should be serde-compatible on request decode. The reverse direction is protected because new fields default to `None`.

### Verdict vocabulary and comparison

- `crates/jcode-protocol/src/wire.rs:31-47` defines `HandshakeCompatibility::{Compatible, IncompatibleReconnect}`.
- `wire.rs:49-113` defines pure evaluation:
  - `None` protocol means legacy compatible.
  - protocol mismatch means `IncompatibleReconnect`.
  - same protocol with known non-empty hash mismatch means `IncompatibleReconnect`.
  - unknown/empty hash with same protocol remains compatible.
- `wire.rs:121-124` implements short/full hash tolerance using prefix matching both ways.
- `wire.rs:1579-1639` contains unit tests for legacy, stale hash with no protocol, matching hash, short/full hash, protocol mismatch, build mismatch, and unknown hash.

Challenge: prefix matching is intentionally compatible with short/full hashes, but it is not a canonical identity check. It can only say two stamped git-hash strings are prefix-compatible. R01's dirty fingerprint, source directory, activation channel, pending activation, and immutable version label are outside this token.

### Server verdict generation

- `crates/jcode-app-core/src/server/handshake.rs:23-69` evaluates the client fields against `PROTOCOL_VERSION` and `jcode_build_meta::GIT_HASH`, logs `HANDSHAKE_VERDICT`, and sends `ServerEvent::HandshakeVerdict` only when `client_protocol_version.is_some()`.
- `handshake.rs:71-74` defines server build hash as `jcode_build_meta::GIT_HASH`.
- `handshake.rs:81-132` tests no event for legacy clients, incompatible event for mismatched hash, and compatible event for matching hash.
- `crates/jcode-app-core/src/server/client_lifecycle.rs:1338-1373` calls `evaluate_and_notify` when processing `Request::Subscribe`.

Challenge: `evaluate_and_notify` returns `HandshakeCompatibility`, but `client_lifecycle.rs` ignores the return value. The server does not reject, close, or skip subscribe side effects on `IncompatibleReconnect`.

### Server behavior after incompatible verdict

After emitting the verdict, subscribe continues:

- `client_lifecycle.rs:1374-1386` updates connection info and terminal env.
- `client_lifecycle.rs:1387-1524` resumes or handles subscribe.
- `client_lifecycle.rs:1527` sets `client_subscribed = true`.
- `crates/jcode-app-core/src/server/client_session.rs:464-477` ensures a swarm member.
- `client_session.rs:479-482` marks active client PID.
- `client_session.rs:484-613` updates working dir and swarm membership.
- `client_session.rs:615-625` may mark/register selfdev tools.
- `client_session.rs:627-647` may register MCP tools for the directory.
- `client_session.rs:675-687` may mark the member ready.
- `client_session.rs:689-693` sends swarm plan and `Done { id }`.

Current temp-socket test `crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs:683-759` asserts an incompatible verdict arrives before subscribe `Done`, and explicitly expects subscribe to still complete after the verdict (`:749-756`). That proves ordering, but it also proves server-side enforcement is not present.

Risk judgment: acceptable only if the client that advertised identity is guaranteed to process the verdict before any later event and exit/re-exec/refuse. That guarantee is true for the TUI event loop source path, but not proved for every in-process or generic client consumer.

### Client projection and action

- `crates/jcode-tui/src/tui/backend.rs:326-343` sends `Subscribe` with `protocol_version: Some(jcode_protocol::PROTOCOL_VERSION)` and `build_hash: Some(jcode_build_meta::GIT_HASH.to_string())`.
- `crates/jcode-app-core/src/server/client_api.rs:84-101` does the same for the generic `server::Client` API.
- `crates/jcode-tui/src/tui/app/remote.rs:775-797` intercepts `ServerEvent::HandshakeVerdict`, calls `act_on_verdict`, continues on compatible, or sets an error/status and quits on refusal.
- `crates/jcode-tui/src/tui/app/handshake.rs:53-88` decides pure actions:
  - compatible attaches;
  - incompatible with distinct target re-execs;
  - incompatible without target refuses;
  - already re-execed attaches to avoid loop;
  - target equal to current attaches.
- `handshake.rs:112-158` resolves `preferred_reload_candidate`, checks `JCODE_NS1_REEXECED`, and uses `platform::replace_process` for re-exec.
- `handshake.rs:160-173` sets the re-exec guard env on the new command.
- `handshake.rs:195-301` tests compatible attach, incompatible re-exec, no-target refusal, already-reexeced attach, same-target attach, guard env, forwarded args, and Unix child process guard visibility.

Challenge: `already_reexeced -> Attach` prevents an infinite relaunch loop, but it can deliberately attach after a still-incompatible verdict. The comment calls this the lesser evil. For pilot acceptance, this must be an explicit observable: after one failed re-exec, the user must see clear evidence that attach happened despite incompatibility, or the policy should refuse instead. Silent attach after guard would violate the spirit of R03A unless made visible and bounded.

### Generic client API risk

The generic `server::Client` advertises identity but does not itself enforce the handshake verdict:

- `client_api.rs:60-104` `subscribe()`/`subscribe_with_info()` sends the identity and immediately returns the request id without reading or acting on the verdict.
- `client_api.rs:107-115` exposes raw `read_event()` only.
- `client_api.rs:170-203` `get_history()`/`get_history_event()` returns the first non-`Ack` event. If called after `subscribe()`, a `HandshakeVerdict` can be returned instead of `History`, and `get_history()` maps non-history to an empty vector.
- I found no current call site of `subscribe_with_info` outside `client_api.rs` (`agentgrep subscribe_with_info`), so this is primarily a public API and future-client risk, not a proven production path today.

Risk judgment: do not claim "actual client/server action after incompatible verdict" is universally safe. It is TUI-safe by source, generic-client-incomplete by source, and server-non-enforcing by source.

## Fork/upstream symbol-level comparison

Commands run:

```bash
BASE=631935dd1d3b2e31e167e2b12ad463e54bcf4b8d
FORK=7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4
UP=802f6909825809e882d9c2d575b7e478dce57d3b
git merge-base "$FORK" "$UP"
git grep -n -E 'HandshakeCompatibility|HandshakeVerdict|protocol_version: Option|build_hash: Option|act_on_verdict|REEXEC_GUARD_ENV|evaluate_and_notify|PROTOCOL_VERSION' "$BASE" -- crates/jcode-protocol/src crates/jcode-app-core/src/server crates/jcode-tui/src/tui/app crates/jcode-tui/src/tui/backend.rs
git grep -n -E 'HandshakeCompatibility|HandshakeVerdict|protocol_version: Option|build_hash: Option|act_on_verdict|REEXEC_GUARD_ENV|evaluate_and_notify|PROTOCOL_VERSION' "$FORK" -- crates/jcode-protocol/src crates/jcode-app-core/src/server crates/jcode-tui/src/tui/app crates/jcode-tui/src/tui/backend.rs
git grep -n -E 'HandshakeCompatibility|HandshakeVerdict|protocol_version: Option|build_hash: Option|act_on_verdict|REEXEC_GUARD_ENV|evaluate_and_notify|PROTOCOL_VERSION' "$UP" -- crates/jcode-protocol/src crates/jcode-app-core/src/server crates/jcode-tui/src/tui/app crates/jcode-tui/src/tui/backend.rs
```

Results:

- Merge base and upstream produced no R03A handshake symbols.
- Fork and head contain the full R03A symbol set in `wire.rs`, `lib.rs`, `server/handshake.rs`, `client_lifecycle.rs`, `tui/backend.rs`, `tui/app/handshake.rs`, and `tui/app/remote.rs`.
- `git diff --unified=0 "$FORK" HEAD` for the R03A-relevant files produced no relevant symbol changes, so head `16921ace1` preserves the fork-baseline R03A behavior.

Conclusion: upstream cannot be adopted for R03A handshake behavior. Retaining the fork is necessary, but retention is not pilot approval.

## Relevant incident evidence

`docs/architecture/SELFDEV_NIX_DAEMON_DIVERGENCE.md:41-57` records G1, G2, G3, and G4:

- G1: Subscribe carried no build/protocol version, so version-mismatched clients attached blindly.
- G3: no first-class meaning of protocol-compatible vs incompatible reload.
- G4: source-of-truth ambiguity across checkouts.

`SELFDEV_NIX_DAEMON_DIVERGENCE.md:133-159` proposes NS1: add `protocol_version` and `build_hash` to Subscribe, server returns typed verdict, client re-execs/refuses rather than silently attaching. Current fork implements most of NS1, but G4 and R01's full identity projection are not solved by `GIT_HASH` alone.

`docs/fork/patch-ledger.md:22` records this as a permanent downstream patch and states legacy clients advertise nothing and never receive the event. The ledger also admits full wrapper-aware identity is NS2/FR-70, which reinforces that R03A's token is not the full build identity.

## Tests and validation run

Read-only/source-preserving commands run from this checkout:

```bash
export CARGO_TARGET_DIR=/tmp/jcode-r03a-target
export JCODE_HOME=/tmp/jcode-r03a-home
./scripts/dev_cargo.sh test -p jcode-protocol handshake_verdict
./scripts/dev_cargo.sh test -p jcode-app-core 'server::handshake|handshake_emits|handshake_sends'
./scripts/dev_cargo.sh test -p jcode-tui --lib handshake
```

Observed result:

- `jcode-protocol handshake_verdict`: **passed**, 7 tests, 0 failed. Output showed the seven `wire::handshake_verdict_tests::*` cases passing.
- Combined app-core/TUI command: **timed out after 600s** during cold dependency compilation in `/tmp/jcode-r03a-target`, while compiling app-core dependencies (`aws-sdk-sso` at timeout). No app-core/TUI test failure was observed; those suites were not completed.
- The dev shell printed normal hook/rerere setup messages. `git status --short` afterward printed no source changes.
- Document hashes captured for evidence reproducibility:
  - `RESPONSIBILITIES.md`: `8f9ae6329dc0f114801a27e35c0eece8f6cb55a28408a6703918f9fca3fb2f85`
  - `patch-ledger.md`: `08e9995b839ad5bed48a07e2350f845f0c90252512a36fa5b42623361ff604e1`
  - `SELFDEV_NIX_DAEMON_DIVERGENCE.md`: `a712c22c8e995192493902a08a5e8eef3b6b97ea5b757edf2ce917583e4f3ec7`

## Deterministic temp-socket/no-network pilot fixtures

These fixtures should be the bounded R03A pilot surface. They require no live user daemon, no network, no credentials, and no external socket.

1. **Protocol serde fixture**
   - Construct legacy subscribe JSON with no `protocol_version`/`build_hash` and assert decode defaults to `None`.
   - Construct new subscribe JSON with fields and assert exact round-trip.
   - Construct old-ref-shaped subscribe JSON plus extra fields and assert current decode accepts it.
   - Observable: schema is additive and serde-stable.

2. **Pure verdict table fixture**
   - Directly call `HandshakeCompatibility::evaluate` for: legacy none, protocol mismatch, same protocol same hash, short/full hash, same protocol different hash, same protocol unknown/empty hash.
   - Observable: exact verdict plus detail string class.
   - Existing coverage: `wire.rs:1579-1639`; keep as regression floor.

3. **Server temp-socket verdict fixture**
   - Use `TempDir`, isolated `JCODE_RUNTIME_DIR`, fake provider, and `Server::run()` on a temp socket.
   - Raw client sends advertised mismatch.
   - Assert first relevant event is `HandshakeVerdict(IncompatibleReconnect)`.
   - Add missing assertion: incompatible must not perform subscribe side effects unless the accepted policy is explicitly "client-enforced only". Current code sends `Done`; decide and test the intended policy.
   - Observable: either server-enforced refusal or explicitly documented client-enforced model with no unbounded side effects.

4. **TUI client action fixture without live exec**
   - Use pure `decide_handshake_action` for compatible, no target, distinct target, same target, already re-execed.
   - Add a visible-observable assertion for `already_reexeced + incompatible -> Attach`, or change policy to Refuse. Today it silently attaches by design.
   - Observable: no reconnect loop and no silent attach.

5. **Generic client API fixture**
   - Start temp server, call `server::Client::subscribe()`.
   - Then call `get_history_event()` or another high-level API and assert it does not misinterpret `HandshakeVerdict` as the requested response.
   - Current source suggests this will fail or return an empty history if a verdict is queued.
   - Observable: all advertised clients either enforce or transparently drain the handshake verdict.

6. **R01 projection fixture**
   - Build or fake two source identities with same `GIT_HASH` but different dirty fingerprint/version label/channel metadata.
   - Assert R03A labels `build_hash` as compatibility-only, not canonical identity, or extend subscribe to carry the agreed R01 projection.
   - Observable: dirty same-commit builds cannot be falsely certified as same runtime identity.

7. **No-network/no-live-daemon guard fixture**
   - Assert tests bind only temp sockets, use fake provider, and fail if `JCODE_SOCKET` points outside the temp runtime or if provider network code is invoked.
   - Observable: pilot stays within the bounded safety envelope in `RESPONSIBILITIES.md:78-88`.

## Pilot blockers and required observables

Blocking before R03A pilot acceptance:

1. **Projection contract blocker:** R01 must approve what R03A carries. Current `build_hash = GIT_HASH` is a compatibility token, not full canonical identity. Required observable: a test or typed schema that distinguishes compatibility token from dirty-build/source/channel identity.
2. **Incompatible server-side action blocker:** Current server emits `IncompatibleReconnect` then completes subscribe. Required observable: either server refuses/skips side effects on incompatible, or the pilot explicitly proves every identity-advertising client path handles the verdict before any unsafe attach behavior matters.
3. **Generic client blocker:** `server::Client` advertises identity but has no handshake-aware high-level API. Required observable: either do not advertise from generic clients, or make `subscribe()` consume/enforce/drain the verdict, with tests.
4. **Reconnect-loop/silent-attach blocker:** `JCODE_NS1_REEXECED` prevents loops by attaching after a second incompatible verdict. Required observable: user-visible warning and bounded policy, or switch to refusal after one failed re-exec.
5. **Validation blocker:** App-core end-to-end and TUI handshake tests were not completed in this review due cold compile timeout. Required observable: targeted app-core temp-socket tests and TUI pure/action tests pass in a clean bounded environment.

## Negative findings

- I found no R03A handshake implementation in upstream or merge base at the fixed refs.
- I found no evidence that `PROTOCOL_VERSION` is bumped by schema changes beyond the initial value `1`; future changes still need governance.
- I found no R03A carriage of R02 provider/account/model identity. This is acceptable only if the pilot does not require R03A to express provider compatibility.
- I found no server-side rejection on incompatible verdict.
- I found no completed test run proving actual TUI re-exec/refusal against a real temp-socket server. Existing tests cover pure decision and command construction, not an end-to-end exec replacement.
- I found no evidence that `build_hash` can distinguish dirty same-commit builds, source-checkout divergence, or activation channel state.
- I did not use a live user daemon, network, credentials, stash replay, destructive action, or source edits.

## Confidence and gaps

Confidence: **medium-high** for the disposition (`retain-fork, pilot blocked`) because the symbol-level fork/upstream comparison is decisive and the source-level risks are concrete. Confidence is **medium** for runtime impact because app-core/TUI targeted tests did not complete within the bounded compile timeout.

Gaps intentionally left rather than expanded:

- I did not inspect `/tmp/jcode-r03a-opus-review.md`.
- I did not run a live daemon or real re-exec.
- I did not complete app-core/TUI targeted tests because the run timed out during cold compile after protocol tests passed.
- I did not audit all historical commits inside the curated squash. R00 requires per-seam semantic claims to be command-backed; my claim here is symbol-level only for the fixed refs.
- I did not review desktop/platform adapters; R08D owns them unless the pilot adds platform behavior.
