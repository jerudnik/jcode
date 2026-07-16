# R03A Wire compatibility and subscribe handshake: authoritative ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `full` |
| Research budget | `8 decisive checkpoints, exhausted without expansion` |
| Authority today | `fork` for the wire/verdict/action mechanism; `R01` for canonical identity meaning |
| Recommended disposition | `retain-fork` |
| Pilot entry verdict | `blocked` |
| Confidence | `medium-high` for wire authority and additive semantics; `high` for the three present enforcement defects; `medium` for the unexecuted app-core/TUI acceptance floor |
| Last updated | `2026-07-15T10:13:00Z` |

## Review preservation and integrity

The independent reviews were read before adjudication and copied byte-for-byte. They are evidence, not substitute authority. This ledger cites source, tests, and fixed refs for its decisions.

| Review | Absolute source | SHA-256 | Repository copy | Preservation result |
|---|---|---|---|---|
| Opus | `/tmp/jcode-r03a-opus-review.md` | `5d0cf7131f5ff43e932e79aa061d7734ec1399e622b447fa8b4142ab946f689e` | [`opus-review.md`](./opus-review.md) | byte-identical by `cmp -s` |
| Grok | `/tmp/jcode-r03a-grok-review.md` | `c161f8951215b2413bbab62c292cb8864d02da018cb06c79b325cee4be6f0945` | [`grok-review.md`](./grok-review.md) | byte-identical by `cmp -s` |

No live user daemon, network, credential, payment, stash replay, destructive action, source edit, or publication was used. The source tree was read-only. R00 fixes the refs and forbids treating upstream provenance as authority. R11 requires this append-only, hash-anchored record.

## Scope and invariants

- **Owns:** stable `Subscribe` compatibility carriage, protocol/build compatibility verdict, safe short/full compatibility-token comparison, legacy behavior, and the client/server action contract after that verdict.
- **Excludes:** canonical runtime identity meaning and identity writers (R01), provider/model/account entitlement (R02), transport execution and session recovery, reload-target selection, and publication.
- **Must preserve:** legacy clients that omit identity decode as `None`, receive no unknown `HandshakeVerdict`, and retain their pre-NS1 flow. R03A carries an R01-declared projection and never derives competing identity or R02 provider truth.
- **Cross-seam invariant:** R01's source, executable, and activation tuple is canonical. `Subscribe.build_hash` is presently only an R03A compatibility projection. A compatible verdict must never be represented as equality of canonical identities.

## Divergence at a glance

| Concern | Fork | Upstream / merge base | Consequence |
|---|---|---|---|
| Handshake schema and verdict | `PROTOCOL_VERSION`, optional `Subscribe` identity, `HandshakeCompatibility`, and `HandshakeVerdict` exist in the fork. | No corresponding handshake symbols. | Retain fork. There is no upstream behavior to adopt or compose. |
| Legacy additivity | Both carrier fields are `Option`, `serde(default)`, and skipped when absent. Verdict is withheld from a client without `protocol_version`. | Original `Subscribe` has no handshake fields. | Old clients and old request shape remain decodable. |
| Identity projection | TUI and generic client stamp compiled `jcode_build_meta::GIT_HASH`; server compares that token. | No identity carriage. | The token supports short/full hash tolerance, not R01 canonical identity. |
| Incompatible action | Server emits verdict but continues subscription. TUI consumes verdict, but generic `server::Client` only exposes raw events. Re-exec guard selects attach on a second mismatch. | No action contract. | Present fork authority defects block the bounded pilot, not the retain-fork disposition. |
| Tests | Protocol verdict/serde and temp-socket event-order fixtures exist. | No counterpart. | Fixture floor is useful, but no acceptance proof exists for fail-closed incompatible action across every advertising client. |

## Decisive evidence ledger

| # | Finding | Evidence and reproduction | Confidence | Decides |
|---:|---|---|---|---|
| 1 | Fixed refs are reproducible and the recorded base is the merge base. | Terra: `git merge-base 7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4 802f6909825809e882d9c2d575b7e478dce57d3b` returned `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`. | H | Bounds all authority conclusions. |
| 2 | The NS1 handshake surface is fork-only. | Terra: fixed-ref `git grep -l HandshakeCompatibility` returned no source path at base/upstream and returned `wire.rs`, `lib.rs`, `server/handshake.rs`, TUI handshake code, and its fixture at fork. | H | `retain-fork`, not `adopt-upstream` or `compose`. |
| 3 | The identity carriers are additive and legacy-safe. | Fork `crates/jcode-protocol/src/wire.rs:226-232` applies `#[serde(default, skip_serializing_if = "Option::is_none")]`; `protocol_tests/misc_events.rs:382-418` decodes an old-shaped Subscribe to `None`. Fixed-ref base/upstream `Subscribe` bodies contain no fields and upstream is byte-identical to base for that body. | H | Wire compatibility is retained. |
| 4 | A verdict is only sent to advertising clients, and the evaluator is total. | `server/handshake.rs:23-74` gates event send on `client_protocol_version.is_some()`; `wire.rs:66-124` gives legacy `Compatible`, mismatch `IncompatibleReconnect`, and symmetric prefix tolerance. Temp-socket fixture `tool/communicate_tests/end_to_end.rs:690-866` checks mismatch, match, and no legacy verdict. | H | Legacy event safety and compatibility-token semantics. |
| 5 | `build_hash` is a false-compatible R01 projection for dirty same-commit builds. | `tui/backend.rs:338-339` and `server/handshake.rs:72-74` use only `jcode_build_meta::GIT_HASH`; `jcode-build-meta/src/lib.rs:9-11` exposes no dirty/fingerprint/channel component; R01 ledger lines 35-41 defines the richer tuple. | H | **Present authority defect and pilot blocker.** R03A must not certify canonical identity from this token. |
| 6 | Server subscribe side effects continue after an incompatible verdict. | `client_lifecycle.rs:1338-1373` discards `evaluate_and_notify`'s return; normal connection/session handling continues through `client_subscribed = true` at `:1527`. The existing mismatch fixture expressly observes verdict followed by `Done` at `end_to_end.rs:749-756`. | H | **Present authority defect and pilot blocker.** Event ordering is not enforcement. |
| 7 | Generic `server::Client` advertises identity without consuming or enforcing its verdict. | `server/client_api.rs:84-101` sends identity; `:107-115` exposes raw `read_event()` only. Its higher-level event helpers can observe a queued `HandshakeVerdict` rather than their expected response. No production `subscribe_with_info` caller was found by Grok. | H for implementation, M for production exposure | **Present authority defect and pilot blocker for any generic-client pilot.** Until fixed, generic clients must not advertise identity or be in the pilot. |
| 8 | The re-exec guard deliberately attaches after a second still-incompatible verdict. | `tui/app/handshake.rs:60-66` returns `Attach` when `already_reexeced`; test `incompatible_after_reexec_attaches_to_avoid_loop` asserts it. A target equal to current also attaches at `:70-77`. | H | **Present action-policy defect and pilot blocker** because it silently contradicts the incompatible verdict. It is bounded to the TUI retry path, but cannot be accepted as safety behavior. |
| 9 | `HandshakeVerdict.compatibility` is an enum event, so incompatible vocabulary evolution is not wire-additive. | `wire.rs:837-849` serializes the enum in a known server event; older advertising clients cannot parse a newly introduced enum variant. | H | Future verdict variants, required identity semantics, or changed incompatible action require protocol-bump governance. |
| 10 | R02 truth is intentionally absent from this handshake. | R02 ledger lines 24-29 excludes R03A verdicts and reserves provider/model/account outcomes to R02/R12. Neither independent review found a provider identity carrier. | H | Negative finding, not a gap: do not add provider checks to R03A. |

### Negative findings and bounded gaps

- No current upstream or merge-base R03A implementation exists. No patch-equivalence claim is made, and R00's `b3ed82a6b` ancestry gap remains binding.
- No test models two same-commit dirty R01 identities through the R03A projection.
- No completed end-to-end temp-socket test proves TUI re-exec/refusal, nor one proves a generic client drains/enforces its verdict before another high-level request.
- No live daemon, network, credential, user data, wrapper replacement, or real `exec` was exercised. These omissions are deliberate stop boundaries, not invitations to expand this seam.
- The protocol does not carry R02 provider/account/model truth. That is correct for scope and must remain so.

## Adjudication

| Disagreement | Opus position | Grok position | Terra resolution | Deciding evidence |
|---|---|---|---|---|
| Fork disposition | Retain fork because all NS1 behavior is fork-only. | Retain fork because neither comparison ref supplies a counterpart. | **Agreement upheld: retain-fork.** The three defects below change pilot acceptance, not implementation authority. | Evidence 1-2. |
| Dirty-build projection | One named joint R01/R03A/R04 blocker. | Blocker because the token omits dirty/source/channel identity. | **Agreement upheld: present authority defect and pilot blocker.** The exact contract must type `build_hash` as compatibility-only until R01 supplies a projection that the wire tests distinguish. | Evidence 5 and R01 ledger lines 35-41. |
| Server continuation after incompatible verdict | TUI action is source-safe enough for the resume/takeover path. | Server continues state mutation and completion after verdict. | **Grok upheld. Present authority defect and pilot blocker.** The protocol's incompatible action is not safe while the server mutates before any consumer can decline attach. | Evidence 6. |
| Generic client advertises but does not enforce | Not identified as a separate blocker. | Generic `server::Client` has raw-event only handling. | **Grok upheld, narrowed. Present authority defect and a blocker only for a pilot including generic clients.** Safest repair is one handshake-aware API or no generic advertisement. | Evidence 7. |
| Re-exec guard attaches after a still-incompatible verdict | Bounded loop guard, treated as safe action policy. | Silent attach requires visibility or refusal. | **Grok upheld. Present action-policy defect and pilot blocker.** Loop prevention is required, but the terminal action after a second mismatch must be refusal with diagnostic, not attach. | Evidence 8. |

**Terra reproduction:** the fixed-ref merge-base command in evidence 1 returned the recorded base. Targeted source inspection reproduced the discarded server verdict, generic raw-event API, and `already_reexeced -> Attach`. This separates current implementation facts from either reviewer's risk judgment.

## Exact wire and action contract

1. **Legacy additivity.** A client omitting `protocol_version` is legacy-compatible. The server emits no `HandshakeVerdict` to it. Optional request fields retain `serde(default)` and omit-on-`None`; no new unknown event reaches an old client.
2. **Advertised compatible request.** A client sending `protocol_version` receives one `HandshakeVerdict { id, Compatible, server_protocol_version, server_build_hash, detail }` before normal subscription completion. Its `build_hash` comparison is only a short/full-compatible token comparison, never a canonical R01 identity assertion.
3. **Advertised incompatible request.** `HandshakeVerdict { id, IncompatibleReconnect, ... }` is terminal for that Subscribe. The server must not set subscribed state, resume/take over a session, mutate connection/member/PID/tool state, or send a successful `Done` after it. It may close after delivering the verdict. This fail-closed rule resolves the current server-side defect.
4. **Client action.** Any client that advertises identity must consume the verdict before treating the subscribe as successful. `Compatible` permits attach. `IncompatibleReconnect` permits exactly one re-exec to a distinct matching launcher. No launcher, a target equal to current, a failed re-exec, or a second incompatible verdict must refuse with a user-visible diagnostic and no attach. A generic client must either expose this enforced result or omit identity advertisement.
5. **R01 projection.** R01 owns the projection type and its source. Until it is changed, `build_hash` is documented as `compatibility_token`, not build identity. A future richer field must be optional and additive, preserve legacy behavior, and prove dirty same-commit/source/channel distinctions in a deterministic fixture.
6. **Protocol-bump governance.** Bump `PROTOCOL_VERSION`, document migration, and extend the fixture matrix before introducing a new verdict enum variant, making fields required, changing the interpretation of legacy `None`, changing the incompatible terminal action, or relying on a richer projection for safety. Optional request-field addition alone remains additive only if no existing advertised client must understand a new response semantic.

## Deterministic acceptance fixtures

All fixtures use a `TempDir`, isolated `JCODE_RUNTIME_DIR` and `JCODE_SOCKET`, a fake provider, and no credentials/network/live daemon.

| Fixture | Required observable | Current status |
|---|---|---|
| Legacy serde | Old-shaped Subscribe decodes both handshake fields to `None`; new shape round-trips. | Existing regression floor. |
| Pure verdict matrix | Legacy, protocol mismatch, equal token, short/full, token mismatch, unknown/empty token have exact verdicts. | Existing regression floor. |
| R01 projection | Two canonical R01 identities with same hash but different dirty/fingerprint/version-label/channel either yield different agreed projection or are explicitly rejected as noncanonical. | **Missing, blocker.** |
| Server temp socket | Mismatched advertised client sees terminal incompatible verdict and no subscribe state mutation or successful `Done`. Matching and legacy paths still succeed. | Existing fixture proves opposite post-verdict `Done`; **must change, blocker.** |
| Generic client temp socket | Identity-advertising `server::Client` consumes/enforces verdict, or does not advertise. No higher-level request misreads queued verdict. | **Missing, blocker if generic client in scope.** |
| TUI action | Pure action chooses one re-exec only, then refusal with visible diagnostic on still-incompatible verdict, target-equals-current, or missing target. | Existing test asserts attach after guard; **must change, blocker.** |
| No-network guard | Fixture socket is inside temp runtime and fake provider does not reach network. | Required pilot harness property. |

## Recommendation

- **Disposition:** `retain-fork`.
- **Why:** the fork is the sole authority for the additive NS1 mechanism. Retention keeps real legacy safety and a useful deterministic fixture base. It does not approve the current incompatible path.
- **Pilot verdict:** **blocked** until the projection fixture and server terminal-action fixture pass. A generic-client pilot is additionally blocked until its enforcement fixture passes. The TUI pilot is blocked until guard-path refusal is made visible and tested.
- **Cross-seam dependencies:** R01 defines an approved projection and joins its fixture. R02 remains excluded. R04 joins the identity/restart interpretation. R09 requires no `--update`, visible debt, and gate attribution. R11 retains these reviewed artifacts and hashes.
- **Upstream opportunity:** none. No upstream patch is identified because upstream has no implementation. If a future protocol-bump design is reusable, propose it upstream separately after the fork contract is proven.
- **Quality-of-life ideas:** better mismatch text and diagnostic metadata may be recorded only. They are not authority or pilot work.

## Bounded implementation slices

| Slice | Class | Change | Acceptance | Rollback or stop condition |
|---|---|---|---|---|
| 1 | `sync` | No sync. Preserve fork-only NS1 behavior and compare only against the fixed refs. | Fixed-ref symbol search still finds no upstream counterpart. | Stop rather than invent a compose if an upstream counterpart appears or provenance becomes ambiguous. |
| 2 | `fix` | With R01, type and carry the approved compatibility projection or explicitly constrain `build_hash`; add dirty same-commit fixture. | R01/R03A deterministic fixture distinguishes the approved condition and never calls token equality canonical identity. | Stop if this needs an unowned R01 writer, live builds, or a protocol semantic change without governance. |
| 3 | `fix` | Make `IncompatibleReconnect` terminal server-side before subscribe mutations; make generic `server::Client` handshake-aware or stop advertising. | Temp-socket mismatch leaves no subscription side effects and generic fixture cannot misread the verdict. | Stop if preserving existing success `Done` would weaken terminal safety or if recovery ownership crosses into R04. |
| 4 | `fix` | Replace post-guard and same-target attach with user-visible refusal after an incompatible verdict. | Pure TUI tests cover distinct re-exec, no target, same target, failed re-exec, and second mismatch with refusal. | Stop if loop prevention cannot be shown without real process replacement. |
| 5 | `refactor` | Centralize identity-advertising subscribe and terminal-verdict consumption so TUI and generic clients cannot diverge. | One shared contract test exercises every advertising client surface. | Stop if refactor broadens transport/session recovery or changes legacy wire shape. |
| 6 | `docs` | Document projection limits, terminal incompatible action, protocol-bump rule, R09 debt attribution, and exact fixture commands. | Ledger and API docs agree with passing fixture behavior. | Stop if documentation would claim a stronger identity or test result than observed. |

## R09 debt, validation, and sign-off

R09's current red gates remain visible. This ledger changed no Rust source and therefore owns no new production/test-size, panic, or swallowed-error debt. Every code slice above must run its targeted fixtures plus the trusted classifier and relevant R09 gates without `--update`, and must attribute any new direct debt to R03A rather than hide it in a baseline.

- **Commands completed before this ledger:** review-preservation `cmp -s` and `shasum -a 256`; fixed-ref merge-base and symbol search; read-only source inspections; protocol verdict tests from the independent review passed `7 passed; 0 failed`.
- **Warmed test attempt:** `bash scripts/dev_cargo.sh test -p jcode-protocol handshake_verdict` passed `7 passed; 0 failed`; `bash scripts/dev_cargo.sh test -p jcode-app-core --lib server::handshake` passed `3 passed; 0 failed` after warming. `timeout 180 bash scripts/dev_cargo.sh test -p jcode-tui --lib handshake` timed out during compilation with no test failure, and was deliberately not expanded. No timeout is treated as a pass.
- **Diff hygiene:** `git diff --check` was run. It reports exactly four trailing-space diagnostics at `grok-review.md:3-6`; those spaces are Markdown hard-breaks in the user-designated source and are retained to preserve the review byte-for-byte. The authored ledger itself has no trailing whitespace.
- **Remaining risks:** the three blockers above, full R01/R04 projection agreement, and the unexecuted app-core/TUI acceptance tests if the warmed attempt does not complete.
- **Opus review:** preserved, and accepted on fork authority, additivity, and dirty-projection blocker. Its broader TUI-safety conclusion is superseded by the evidence-based action constraints above.
- **Grok review:** preserved, and accepted on server continuation, generic-client handling, and re-exec-guard defects, with generic-client scope narrowed as stated.
- **Terra adjudication:** `pass` for `retain-fork`; `fail` for pilot entry pending blockers.
- **Sol sign-off:** `pass` on `a60cc2d6c53153b492f16160a5d64e20fe23f60c`; preserved at [`2026-07-15-r03a-sol-signoff.md`](../../reviews/2026-07-15-r03a-sol-signoff.md).
- **Fable sign-off:** `pass` on `a60cc2d6c53153b492f16160a5d64e20fe23f60c`; preserved at [`2026-07-15-r03a-fable-signoff.md`](../../reviews/2026-07-15-r03a-fable-signoff.md).

## Implementation and validation amendment: R01/R03A identity prerequisite (2026-07-15)

This amendment is append-only and preserves the prior adjudication bytes. It records the R03A side of the joint prerequisite implemented on branch `recovery/fix-r01-r03a-identity-20260715`.

### Commits recorded

| Commit | Purpose | R03A relevance |
|---|---|---|
| `c759e2504` (`fix(identity): project runtime identity in handshake and reload`) | Adds optional `runtime_identity` to `Subscribe`, optional `server_runtime_identity` to `HandshakeVerdict`, enforces fail-closed incompatible advertised Subscribe handling, stops generic clients from advertising unconsumed identity, and hardens TUI one-reexec/refusal behavior. | Makes the wire projection additive while preserving legacy decoding, and makes incompatible advertised Subscribe terminal before subscription side effects. |
| `28a63f9f4` (`fix(identity): export reload identity helpers`) | Source-only follow-up exporting reload identity helpers through the existing server facade. | Fixes app-core compile wiring discovered during validation without changing the R03A wire contract. |

### Implementation outcome

- `Subscribe.build_hash` is explicitly R03A compatibility-token-only. It is not canonical R01 runtime identity.
- `Subscribe.runtime_identity` and `HandshakeVerdict.server_runtime_identity` are optional/additive projections. Legacy clients that omit advertised identity still receive no verdict and preserve prior flow.
- The server now fails closed for incompatible advertised Subscribe before subscribed/session/member/PID/tool mutation and without success `Done`.
- Generic `server::Client` no longer advertises protocol/build/runtime identity because it does not synchronously consume/enforce the verdict.
- TUI remains an advertising client and now refuses instead of attaching on second/same-target mismatch; it may re-exec at most once to a distinct target.

### Honest validation history

| Step | Result |
|---|---|
| Initial Cargo command | Infrastructure/operator error: invalid `--nocapture` placement meant the command compiled nothing. It is not counted as validation. |
| Corrected foundational suites | Passed: `jcode-build-support` `46/46`; `jcode-protocol` `81/81`. |
| Shared-target artifact incident | Branch-switch stale shared-target artifacts first produced false missing-type errors. They were cleared package/profile-specifically before source validation continued. |
| Real compile failure after artifact cleanup | `cargo check -p jcode-app-core` exposed real helper export failures for `write_reload_state_with_runtime_identity` and `send_reload_signal_with_runtime_identity`. Fixed by `28a63f9f4`. |
| App-core and TUI checks | Passed after the follow-up export fix. |
| Focused R03A/R01 behavior suites | Passed `37/37`: server/client verdict paths, fail-before-mutation invariant, reload identity propagation, and TUI one-reexec/refusal matrix. |
| Coordinator sequencing violation | Two test-list inventory commands were accidentally launched concurrently by the coordinator. They serialized on locks and are recorded only as a sequencing infrastructure violation, not validation evidence. |
| R09 gates | Classifier, wildcard, and warning checks green; four expected ratchets visibly red; no `--update`. |
| TUI build without reload | No-reload selfdev-profile TUI build passed after clearing the stale package artifact. |

No reload, activation, network, credentials, live user daemon, publication, or baseline update was performed for this validation amendment.

## Final review preservation and correction blockers amendment (2026-07-15)

This amendment is append-only and preserves prior text. It records the final R01/R03A review split and the correction blockers that gate R03A closure.

### Final reviews preserved

| Artifact | Absolute source | SHA-256 | Repository copy | Result |
|---|---|---|---|---|
| Opus final review | `/tmp/jcode-r01-r03a-final-opus-review.md` | `b1eed52b6112a3c55fb787de15cf82eadb005230cf7b5233507a1f3e07df2f9d` | `docs/fork/recovery/reviews/2026-07-15-r01-r03a-final-opus-review.md` | Opus `PASS`; copied byte-for-byte. |
| Grok final review | `/tmp/jcode-r01-r03a-final-grok-review.md` | `07349da7d17649fb7cfdc9cafc13cf93891f231037a6db2adc2916823d3738d7` | `docs/fork/recovery/reviews/2026-07-15-r01-r03a-final-grok-review.md` | Grok `FAIL`; copied byte-for-byte. |
| Fable provider failure | `/tmp/jcode-r01-r03a-fable-provider-failure.md` | `d0f9b9ef56483b2ba2c29f72063ab12f679ec1f4c78554cdd1482ab9c025f1bd` | `docs/fork/recovery/reviews/2026-07-15-r01-r03a-fable-provider-failure.md` | Provider failure produced no verdict; copied byte-for-byte. |

### Correction blockers accepted

- **C1 initial incompatible advertised Subscribe preflight:** current behavior allows initial server/session construction before the advertised compatibility decision. The bounded correction is to preflight initial advertised `Subscribe` before provider fork, Agent construction, session/client/global maps, PID markers, member/tool state, emit exactly one applicable verdict followed by terminal `Error`, and return. Compatible and legacy flows retain their existing paths and verdict cardinality.
- **C2 exact R01 runtime projection carriage:** R03A must continue treating `build_hash` as compatibility-only while carrying the R01 projection. The current projection omits exact dirty sidecar metadata for TUI/server current binaries. The bounded correction is to use immutable executable sidecar metadata when present, without changing `PROTOCOL_VERSION`, verdict policy, or `build_hash` semantics.

No integration, reload, network, stash, branch, publication, protocol bump, or build-hash semantic change is authorized by this amendment.

## Final correction and validation amendment (2026-07-15)

This amendment is append-only. It records the bounded C1/C2 corrections after preserving the Opus PASS, Grok FAIL, and Fable provider-failure artifacts in commit `0a0cb5a06`.

### Final correction commits

| Commit | Scope | R03A relevance |
|---|---|---|
| `023226207` (`fix(identity): preflight initial subscribe and sidecar projection`) | Source-only correction. Factors server Subscribe handshake evaluation/event construction, preflights initial incompatible advertised Subscribe before provider fork/session/client/global/PID/member/tool state mutation, reads exact dev source sidecar metadata for runtime identity projection, and writes the sidecar beside installed immutable versioned binaries. | Closes C1 while preserving the additive R03A contract: advertised incompatible Subscribe is fail-closed and terminal before state mutation; legacy/no-advertisement remains compatible. |
| `db5b4a19d` (`test(identity): cover initial preflight and sidecar projection`) | Tests-only correction. Adds direct `handle_client` no-init incompatible initial Subscribe regression, sidecar projection regressions for same-commit dirty identities and immutable installed binaries, and Starting→SocketReady runtime identity preservation. | Proves the R03A fail-closed path and R01 projection carriage without live daemon/network/credentials. |

### Final validation log

All commands below were run from `/Users/jrudnik/labs/jcode-fix-r01-r03a-identity` with `CARGO_TARGET_DIR=target/r01-r03a-identity-validation`, after the tests-only commit. Cargo commands were run one at a time. `scripts/dev_cargo.sh` re-entered the repo Nix dev shell because `cargo` was not on PATH; it printed the standard trusted Cachix settings, hook installation, rerere import, `fork: main is 5 ahead github/main`, `fork: (remote state refreshing in background; rerun for an updated verdict)`, and `sccache skipped for incremental build` messages. No reload, activation, publication, live daemon, credentials, or intentional network action was performed by the correction itself.

| Step | Command | Result |
|---|---|---|
| Build-support lib | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-build-support --lib -- --nocapture` | Pass: `48 passed; 0 failed`. |
| Protocol lib | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-protocol --lib -- --nocapture` | Pass: `81 passed; 0 failed`. |
| App-core handshake focused | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-app-core --lib server::handshake::tests -- --nocapture` | Pass: `3 passed; 0 failed; 1091 filtered out`. One pre-existing warning: `drop_control_log_handle` dead code. |
| App-core lifecycle mistaken filter | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-app-core --lib server::client_lifecycle_tests::incompatible_initial_subscribe_preflights_before_full_session_initialization -- --nocapture` | Failed validation command: `0 passed; 0 failed; 1094 filtered out`, exit `97`, because the explicit filter matched zero tests. This is recorded as operator/filter error, not source evidence. |
| App-core lifecycle corrected filter | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-app-core --lib server::client_lifecycle::tests::incompatible_initial_subscribe_preflights_before_full_session_initialization -- --nocapture` | Pass: `1 passed; 0 failed; 1093 filtered out`. Confirms initial incompatible advertised Subscribe emits verdict+Error before provider fork, sessions, client connections, global session id, shutdown signals, soft queues, swarm member state, or active PID marker. |
| App-core reload preservation | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-app-core --lib server::reload_state::tests::publish_socket_ready_preserves_starting_runtime_identity -- --nocapture` | Pass: `1 passed; 0 failed; 1093 filtered out`. |
| App-core check | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh check -p jcode-app-core --lib` | Pass. |
| TUI check | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh check -p jcode --bin jcode` | Pass. |

### Final status

- Initial incompatible advertised Subscribe is now fail-closed before heavy initialization or observable session/client/global/PID/member/tool mutation.
- The emitted sequence is the same applicable `HandshakeVerdict` followed by terminal `Error`, then return.
- Compatible advertised Subscribe and legacy/no-advertisement behavior remain unchanged. Verdict cardinality remains exactly one for applicable advertising clients and zero for legacy clients.
- R03A `build_hash` remains a compatibility token only. R01 runtime identity remains the canonical projection and is additive on the wire.
- No protocol version bump, compatibility-token semantic change, reload activation, or live daemon exercise was performed.

## Final correction re-review preservation (2026-07-15)

This amendment is append-only. Two independent read-only re-reviews evaluated exact head `c2eba7796` after the C1/C2 correction and both returned **PASS** with high confidence and no critical or important findings.

| Review | Repository artifact | SHA-256 | Verdict |
|---|---|---|---|
| Opus correction re-review | `../../reviews/2026-07-15-r01-r03a-correction-rereview-opus.md` | `f382998ca7fd56dbc302a43a7f234b3189e8d56979b58175fec342393fdd17f2` | PASS |
| Grok correction re-review | `../../reviews/2026-07-15-r01-r03a-correction-rereview-grok.md` | `9b265115ace7786b3698e4affeb006463a0b33903f266ccca73f031af77eafc6` | PASS |

The re-reviews agree that the initial incompatible advertised Subscribe now returns before provider/session/client/global/PID/member/tool mutation, exact dirty same-commit identity is recovered from executable sidecars when present, immutable selfdev publication writes that sidecar, R03A compatibility semantics remain separate, and `Some(runtime_identity)` survives Starting→SocketReady.

Non-blocking residuals remain explicit: ad-hoc or ambient binaries without sidecars use the documented lossy fallback; generic remote `/reload` still supplies `runtime_identity: None`, while the reviewed selfdev reload path supplies `Some` and the state transition preserves it. The assigned writer process exited before performing this final docs-only copy, so the coordinator preserved the already-completed reviewer artifacts directly. No source, test, integration, reload, network, or activation action occurred in this amendment.

## Coordinator combined-validation amendment (2026-07-15)

At coordinator HEAD `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`, the integrated R01/R03A chain is `615ab1d9a` through `6c6a4f2c8`. Exact source, test, documentation, initial disagreement/provider-failure, correction-review, validation-manifest, and intentionally absent category linkage is preserved in [`../../evidence/README.md`](../../evidence/README.md).

The initial Opus PASS (`b1eed52b...`), Fable provider failure (`d0f9b9ef...`), and Grok FAIL (`07349da7...`) remain binding evidence. The bounded correction re-reviews reproduce as Opus PASS SHA-256 `f382998ca7fd56dbc302a43a7f234b3189e8d56979b58175fec342393fdd17f2` and Grok PASS SHA-256 `9b265115ace7786b3698e4affeb006463a0b33903f266ccca73f031af77eafc6`.

The sequential combined manifest SHA-256 is `41ece4820891461de774dbc5ab06d8e8a66c00630be62274d00dc1f5a9952291`. R03A-relevant results are protocol 81/81; handshake unit filter 3/3; initial incompatible Subscribe fail-before-mutation 1/1; live handshake/client matrix 4/4; and TUI one-reexec/refusal matrix 7/7. Compatible, legacy, protocol-version, and `build_hash` semantics remain unchanged.

The previously named R03A strict prerequisite is closed as a source-fix node. This is not pilot authorization; G2 remains the independent pilot gate.
