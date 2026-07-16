# R01 Runtime build identity and reload authority: authoritative ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `full` |
| Research budget | `8 decisive checkpoints, exhausted without expansion` |
| Authority today | `fork` |
| Recommended disposition | `retain-fork` |
| Confidence | `medium-high` |
| Last updated | `2026-07-15T09:00:22Z` |

## Review preservation and integrity

The independent reviews were copied byte-for-byte from the designated untracked artifacts. They are evidence, not substituted source authority.

| Artifact | Absolute source | SHA-256 | Preservation result |
|---|---|---|---|
| Opus independent review | `/tmp/jcode-r01-opus-review.md` | `21918fd79db7a7a9e7360a68699b6283e272ed56a359f912a0a0940d529d1e60` | copied verbatim to `opus-review.md` |
| Grok independent review | `/tmp/jcode-r01-grok-review.md` | `9cb7beb3304074056ad1f2cbcdcbfa1f4293b5c2694d63fe180249e3e86d40b6` | copied verbatim to `grok-review.md` |
| Stale-daemon incident | `/Users/jrudnik/notes/projects/jcode/maintenance/bug-server-reload-stale-daemon-version-check.md` | `80012e2ce61c578c943263b944bbaca27ac7dbd440af50c1f21f6e0291d8f1a9` | hash and decisive facts reproduced locally |

The two reviews were read independently before adjudication. No external network, credentials, live user daemon, or destructive operation was used. Fork/upstream provenance is comparison evidence only and is not authority by itself.

## Scope and invariants

- Owns: the meaning of canonical executable/source/build identity; the identity of `current`, published, `stable`, and `shared-server` targets; pending activation; reload-target selection; and the projection of that identity consumed by other seams.
- Excludes: R03A wire encoding and compatibility verdict policy, R04 session/restart handoff, R10 publication/distribution mechanics, client handoff, and release publication.
- Must preserve: a selected executable and its source state cannot be silently conflated with a channel marker; non-forced reload only selects a strictly newer candidate; Nix-managed non-selfdev resolution bypasses the self-managed build shadow; forced reload remains an explicit override; pending activation restores the prior markers on failure.
- Cross-seam invariant: manifest/launcher, daemon reload state, R03A subscribe, and R04 restart evidence are four projections of one R01-defined identity. R03A and R04 consume the definition and must not derive competing truth.

### Identity meaning established today

R01 canonical runtime identity is a tuple, not any one string:

1. **Source provenance:** compiled `GIT_HASH`, source `short_hash`/`full_hash`, `dirty`, `fingerprint`, and immutable `version_label`.
2. **Executable provenance:** selected executable path and resolved payload, compiled `VERSION`, and `BUILD_SOURCE_DIR` where meaningful.
3. **Activation provenance:** channel (`current`, `stable`, or `shared-server`), immutable published version, and pending-activation predecessor values.

`Request::Subscribe.build_hash` is currently only the R03A **compatibility projection** of that tuple, since it is stamped from `jcode_build_meta::GIT_HASH`. It is not sufficient to be called the full canonical identity for dirty same-commit builds. `ReloadSignal.hash` is a closer R01 projection because selfdev reload assigns `source.version_label`; `PendingActivation` retains `source_fingerprint`. Current R04 restart/session fields (`jcode_version`, optional `jcode_git_hash`, optional dirty flag) also do not carry the full fingerprint/label/channel tuple. This semantic split blocks pilot readiness until a projection contract and a narrow test prove it rather than merely naming it.

## Divergence at a glance

| Concern | Fork | Upstream | Consequence |
|---|---|---|---|
| Reload authority | Adds Nix-managed launcher override to client/shared-server/preferred target selection, so a managed non-selfdev process ignores the `builds/` shadow. | Retains the merge-base candidate functions with no `paths.rs` delta. | Fork policy is the only reviewed authority for the incident class. |
| Canonical source identity | Rich `SourceState`, fingerprinted dirty labels, dev metadata, published and pending source fingerprints. | Shared data types are unchanged from the merge base. | No upstream identity model replaces the fork’s protected observables. |
| R03A carriage | Subscribe carries `protocol_version` plus short compiled `build_hash`. | Overlap is a compatibility dependency, not canonical source truth. | Dirty same-commit builds collapse at this projection. |
| R04/reload observables | Reload signal/state carries a `hash`; selfdev assigns the version label and pending activation stores fingerprint. | Overlapping reload/recovery work requires R04 review. | R04 needs a consumer contract, not an R01 authority transfer. |
| Tests and operations | Build-support library suite covers stale shared-server repair, no-downgrade target selection, dirty fingerprint labels, and activation rollback. | No upstream R01 test evidence was accepted as authority. | Fork tests are a regression floor, but do not prove four-way runtime agreement. |

## Evidence ledger

| Finding | Evidence | Confidence | Decides |
|---|---|---|---|
| Fixed comparison refs are stable. | Terra: `git rev-parse --verify <fork>^{commit} <upstream>^{commit} <base>^{commit}`; `git merge-base fork upstream` returned `631935dd1...`; base is ancestor of both. Authored head was `f5a8999d81311d237d1c106a9d980fd86fa34b6e`. | H | All conclusions are bounded to reproducible refs. |
| Upstream has inherited candidate functions, but no post-base `paths.rs` policy change. | Terra: `git diff --unified=0 631935dd1 802f69098 -- crates/jcode-build-support/src/paths.rs` was empty; both base and upstream expose `shared_server_update_candidate` and `preferred_reload_candidate`. | H | Corrects the overbroad claim that upstream lacks every target function. |
| Fork owns the material incident policy delta. | Terra: `git diff --numstat base fork -- paths.rs` = `333/1`; upstream = no delta. Fork adds `nix_managed_launcher_override` and calls it from client, shared-server, and preferred reload selection (`paths.rs:545-704` at fork). | H | `retain-fork`, not `adopt-upstream` or a policy `compose`. |
| Compile-time and rich source identity have different granularities. | `jcode-build-meta/src/lib.rs:9-32` exposes `VERSION`, `GIT_HASH`, `BUILD_SOURCE_DIR`; `jcode-selfdev-types/src/lib.rs:74-112,146-162` exposes `SourceState` fingerprint/version label, `PublishedBuild`, `PendingActivation`, dev metadata, and `BuildInfo`. | H | Canonical identity must be a tuple. |
| R03A carries only a compatibility projection. | `wire.rs:202-232` has `Subscribe.protocol_version` and `build_hash`; TUI sends `PROTOCOL_VERSION` and `jcode_build_meta::GIT_HASH` (`backend.rs:326-340`); server compares the compiled `GIT_HASH` (`handshake.rs:23-73`). | H | Pilot cannot claim build-hash equality proves dirty-build identity equality. |
| R04/reload projections are inconsistent in richness. | `ReloadSignal.hash` and `ReloadState.hash` are in `reload_state.rs:403-431`; selfdev assigns `hash = source.version_label` and persists `PendingActivation.source_fingerprint` (`tool/selfdev/reload.rs:284,312-318`); session metadata has `jcode_version`, optional `jcode_git_hash`, optional dirty (`jcode-session-types/src/lib.rs:195-197`). | H | R03A/R04 must agree a projection contract before a reload pilot. |
| The stale-daemon failure is real and current fork defenses are explicit. | Incident hash above records a mapped old daemon despite an on-disk newer marker and a forced reload repair. `client_session.rs:714-725` gates only non-forced reload on a strictly newer candidate. `server_events.rs:64-99,168-175` makes a client-proven older server win over `Some(false)`. | H | Do not remove/downgrade fork directional and client-repair rules. |
| Cheapest focused regression suite passes. | Terra: `bash scripts/dev_cargo.sh test -p jcode-build-support --lib` returned `45 passed; 0 failed` on this head. Named coverage includes `dirty_source_state_uses_fingerprint_in_version_label`, `pending_activation_can_complete_and_roll_back`, stale shared-server repair, and reload-target divergence tests. | H | Core selection and source-state mechanics are a maintained regression floor. |
| Four-way agreement was not exercised end-to-end. | No disposable daemon/client dirty-build round trip was available in this bounded review. | H | Pilot readiness is blocked, not assumed. |

### Negative findings

- No claim of patch equivalence was made. R00's empty stable patch-ID intersection and `b3ed82a6b` ancestry gap remain binding.
- No upstream `paths.rs` change after the merge base was found. This does **not** mean upstream has no inherited candidate code, only no reviewed policy improvement for this seam.
- No test or static evidence proves a live R03A subscriber, reload daemon, manifest/channel markers, and R04 restart record agree for two dirty builds from the same commit.
- Desktop/platform reload semantics were not reviewed because they are R08D. No live daemon or production path was invoked.

## Adjudication

| Disagreement | Opus position | Grok position | Terra resolution | Deciding evidence |
|---|---|---|---|---|
| R01 disposition and authority | `retain-fork`: upstream lacks the relevant reload-target authority. | `compose`: fork identity/reload base, with upstream reload/session recovery as dependency. | **`retain-fork`, authority `fork`.** Upstream retains inherited candidate functions, so the absolute “lacks every function” wording is narrowed. Yet it has no post-base `paths.rs` policy delta, while the fork owns the Nix/shared-server incident defenses. R04 may review upstream recovery behavior as a dependency, but that is not an R01 compose slice. | Empty `base..upstream paths.rs` diff; fork `333/1` delta; fork-only Nix override used at all three selection points. |
| Identity granularity | Accepts fork compile-time identity and stable data model, with four-way runtime proof still open. | Finds dirty-build split: R01 label/fingerprint versus R03A build hash and R04 fields. | **Accepted as a blocking gap.** The fork remains R01 authority, but `build_hash` is a compatibility projection, not canonical runtime identity. R03A/R04 must implement or explicitly test declared projections before pilot. | Exact field carriers cited above and the passing dirty-label unit test. |

Terra reproduction: `git diff --unified=0 631935dd1d3b2e31e167e2b12ad463e54bcf4b8d 802f6909825809e882d9c2d575b7e478dce57d3b -- crates/jcode-build-support/src/paths.rs` produced no output, while the same comparison to fork showed the Nix override calls and `git diff --numstat` reported `333/1`. This decides that location/provenance alone is insufficient, but the fork’s tested semantics are the present R01 authority.

## Recommendation

- Disposition: `retain-fork`.
- Why: retain the fork’s incident-driven, directional reload authority and identity model. A policy compose would import no demonstrated upstream R01 improvement and would blur R04 recovery dependency with R01 target authority. This does not endorse every source location as authority: the exact executable/source/channel tuple above is authoritative only because it is reproduced, guarded, and consistent with the incident defenses.
- Authority today: fork R01 defines canonical identity and reload target semantics. Upstream may supply candidate evidence only. R03A owns wire carriage, R04 owns restart/session persistence, and neither may redefine R01 identity.
- Pilot readiness: **blocked**. Smallest acceptance test: a temp-`JCODE_HOME`, temp-socket **dirty-build identity projection test** that creates two dirty builds from one commit with distinct fingerprints; asserts distinct R01 `version_label`/`source_fingerprint`; asserts R03A labels `build_hash` as compatibility-only or carries a distinct canonical projection; and asserts R04 reload/restart evidence records the corresponding `version_label`, fingerprint, channel, and resolved executable payload. This must use no live user daemon.
- Cross-seam dependencies: R00 fixed refs/preservation and stop budget; R03A projection encoding and compatibility behavior; R04 restart/reload handoff and snapshots; R09 gate/debt policy; R10 launcher/channel publication; R11 append-only evidence; R08D only if platform reload behavior is exercised.
- Upstream opportunity: none in R01 until a symbol-level R04 review identifies a concrete recovery behavior whose observable contract complements, rather than replaces, the fork target-authority rules.
- Quality-of-life ideas: do not implement here. Consider later making the canonical tuple explicit in diagnostics, but only in a separate R01/R03A/R04 design and test lane.

## Bounded implementation slices

| Slice | Class | Change | Acceptance | Rollback or stop condition |
|---|---|---|---|---|
| 1 | `sync` | No R01 upstream sync. Preserve fork target-authority policy while R04 separately evaluates upstream recovery behavior. | `base..upstream paths.rs` remains no policy delta, or any proposed import names one observable and a targeted test. | Stop if import is justified only by upstream provenance, broad merge/rebase, unresolved `b3ed82a6b` ancestry, or it alters target policy without a stale-daemon regression. |
| 2 | `fix` | Joint R01/R03A/R04 dirty-build projection contract, beginning with the smallest acceptance test named above. R01 supplies tuple semantics; R03A/R04 change only their owned carriers. | Two same-commit dirty fixtures have distinct canonical R01 identities and every projection is either equal or explicitly typed/validated as a compatibility projection. | Stop and roll back the isolated slice if it needs the live daemon, a secret/network, incompatible legacy-wire broadening, or unowned writer changes. |
| 3 | `refactor` | None authorized. Do not consolidate identity writers until the contract test identifies the minimal duplicate or ambiguous writer. | Refactor proposal lists every writer/reader and preserves all slice-2 fixtures without gate baseline changes. | Stop if the change becomes semantic redesign, changes R03A encoding or R04 persistence without their seam owners, or exceeds the R00 budget. |
| 4 | `docs` | Preserve both reviews verbatim and publish this adjudication, tuple definition, blocked pilot condition, evidence hashes, and negative findings. | SHA-256 of copied reviews equals the designated artifacts; Markdown, allowed paths, and `git diff --check` pass. | Stop if a review hash mismatches, an earlier decision would be rewritten rather than appended, or any path outside this seam directory changes. |

## Red-debt ownership and quality gate

R09 is binding. It records visible global debt at the current baseline: production-size `60` violations, test-size `31`, panic `46` versus `31`, and swallowed errors `3,077` versus `2,987`. R09 explicitly says per-file assignment is not yet enumerated and each behavior seam must list its entries before implementation.

- This ledger authorizes **no source implementation**, so it accepts no existing source debt as silently owned by R01 and introduces none. Its three documentation files do not modify gate scripts or ratchet JSON.
- Before slice 2 or 3 starts, R01 owns identifying every changed/introduced panic, swallowed-error, production-size, and test-size entry in its concrete diff. Existing entries may be attributed to R01 only with file-level evidence, not because the files concern reload.
- `--update` is prohibited. Trusted classifier tests and green warning/wildcard gates remain required. Any R01 code slice must rerun the R09-required gate matrix and keep existing red debt visible.

## Validation and sign-off

- Commands: fixed-ref/ancestry and preservation hash commands in Evidence ledger; precise `git diff --unified=0 base upstream -- paths.rs`; `git diff --numstat base fork/upstream -- paths.rs`; exact identity-carrier searches; `bash scripts/dev_cargo.sh test -p jcode-build-support --lib` (`45 passed; 0 failed`).
- Failure modes checked: stale on-disk marker versus older mapped daemon; non-forced downgrade/reload loop; Nix-managed build-shadow capture; stale shared-server repair; dirty source labels; pending activation rollback; loss of dirty-build distinction at handshake/restart projections.
- Remaining risks: mtime remains ordering rather than cryptographic identity; no temp-daemon end-to-end four-way test; R03A and R04 consumer implementations are not adjudicated here; `server/util.rs`/`server/reload.rs` historical provenance before merge base was not resolved; platform reload is unexamined.
- Opus review: `pass` as independent evidence. Its fork-authority conclusion is accepted with the narrower correction that upstream retains inherited candidate functions.
- Grok review: `pass` as independent evidence. Its identity-granularity finding is accepted; its `compose` recommendation is narrowed to an R04 dependency rather than an R01 disposition.
- Terra adjudication: `pass`, with pilot readiness blocked on the named dirty-build projection test.
- Sol sign-off: `pass` as an integration-ready adjudication document with the pilot blocker preserved; see [`2026-07-15-r01-r02-sol-signoff.md`](../../reviews/2026-07-15-r01-r02-sol-signoff.md), SHA-256 `84943fd4bc97c1a69ee8e63b7f2df1e05b27f447132618eeb73f06d800a6acdb`.
- Fable sign-off: `pass` as an integration-ready adjudication document with the pilot blocker preserved; see [`2026-07-15-r01-r02-fable-signoff.md`](../../reviews/2026-07-15-r01-r02-fable-signoff.md), SHA-256 `942d282e3245e7493e7dd2c0d816e72982d1d1fc0ebdaa2dfa920fb60c7bf32b`. Its sole IMPORTANT accuracy note, the truncated authored-head hash, was corrected before integration.

## Implementation and validation amendment: R01/R03A identity prerequisite (2026-07-15)

This amendment is append-only and preserves the prior adjudication bytes. It records the implementation and validation history for the joint R01/R03A prerequisite on branch `recovery/fix-r01-r03a-identity-20260715`.

### Commits recorded

| Commit | Purpose | R01 relevance |
|---|---|---|
| `c759e2504` (`fix(identity): project runtime identity in handshake and reload`) | Introduces `RuntimeIdentityProjection`, projects `SourceState` into canonical runtime identity, carries optional projections through `Subscribe`/`HandshakeVerdict`, preserves reload signal/state identity, and updates TUI/client/server contract fixtures. | Establishes the explicit R01-owned source/executable/channel projection rather than treating `build_hash` as canonical identity. |
| `28a63f9f4` (`fix(identity): export reload identity helpers`) | Source-only follow-up exporting `send_reload_signal_with_runtime_identity` and `write_reload_state_with_runtime_identity` through the existing server facade. | Fixes the app-core access path so reload propagation calls use the R01 projection helpers without changing the model. |

### Implementation outcome

- R01 canonical runtime identity is now an explicit `RuntimeIdentityProjection` with `version_label`, optional source fingerprint/dirty/hash fields, activation channel, and resolved executable payload.
- Dirty `SourceState` projection remains R01 authority. R03A `build_hash` remains compatibility-only and is documented as such.
- Selfdev reload paths now pass exact source-state projection into reload signal/state evidence. Existing wrapper APIs preserve `None` for legacy callers.
- Server handshake emits optional server runtime projection for advertising clients while preserving legacy no-verdict behavior.
- TUI advertises the optional runtime projection alongside its compatibility protocol/hash advertisement.

### Honest validation history

| Step | Result |
|---|---|
| Initial Cargo command | Infrastructure/operator error: the initial command placed `--nocapture` incorrectly and compiled nothing. It is not counted as validation. |
| Corrected foundational suites | Passed: `jcode-build-support` `46/46`; `jcode-protocol` `81/81`. |
| Shared-target artifact incident | A branch-switch stale shared-target artifact first produced false missing-type errors. It was cleared package/profile-specifically. This was infrastructure state, not a source defect. |
| Real compile failure after clearing stale artifact | `cargo check -p jcode-app-core` then exposed real reload-helper export failures. Calls through `crate::server` could not reach helpers defined in `reload_state`. Fixed by `28a63f9f4`. |
| App-core and TUI checks | Passed after the export follow-up. |
| Focused behavior suites | Passed `37/37`: server/client verdict behavior, fail-before-mutation behavior, reload projection propagation, and TUI one-reexec/refusal matrix. |
| Coordinator sequencing violation | Two test-list inventory commands were accidentally launched concurrently by the coordinator. They serialized on locks and count only as a sequencing infrastructure violation, not validation evidence. |
| R09 gates | Classifier, wildcard, and warning checks were green. Four expected ratchets were visibly red. No `--update` was used. |
| TUI build without reload | No-reload selfdev-profile TUI build passed after clearing the stale package artifact. |

No reload, activation, network, credentials, live user daemon, or publication was performed for this validation amendment.

## Final review preservation and correction blockers amendment (2026-07-15)

This amendment is append-only and preserves prior text. It records the final identity review split and the two bounded correction blockers accepted before identity review can be treated as closed.

### Final reviews preserved

| Artifact | Absolute source | SHA-256 | Repository copy | Result |
|---|---|---|---|---|
| Opus final review | `/tmp/jcode-r01-r03a-final-opus-review.md` | `b1eed52b6112a3c55fb787de15cf82eadb005230cf7b5233507a1f3e07df2f9d` | `docs/fork/recovery/reviews/2026-07-15-r01-r03a-final-opus-review.md` | Opus `PASS`; copied byte-for-byte. |
| Grok final review | `/tmp/jcode-r01-r03a-final-grok-review.md` | `07349da7d17649fb7cfdc9cafc13cf93891f231037a6db2adc2916823d3738d7` | `docs/fork/recovery/reviews/2026-07-15-r01-r03a-final-grok-review.md` | Grok `FAIL`; copied byte-for-byte. |
| Fable provider failure | `/tmp/jcode-r01-r03a-fable-provider-failure.md` | `d0f9b9ef56483b2ba2c29f72063ab12f679ec1f4c78554cdd1482ab9c025f1bd` | `docs/fork/recovery/reviews/2026-07-15-r01-r03a-fable-provider-failure.md` | Provider failure produced no verdict; copied byte-for-byte. |

### Correction blockers accepted

- **C1 initial incompatible advertised Subscribe preflight:** current behavior creates the initial Agent/session/client connection/global session/active PID before the compatibility check. The bounded correction is to preflight the initial advertised `Subscribe` before provider fork, Agent construction, session/client/global maps, PID markers, member/tool state, emit the same `HandshakeVerdict` plus terminal `Error`, and return. Compatible and legacy flows must remain unchanged.
- **C2 exact dirty runtime projection:** current TUI/server `current_runtime_identity_projection` is best-effort and omits exact dirty fingerprint even when the selected immutable executable has `DevBinarySourceMetadata`. The bounded correction is to recover that sidecar for the executing/resolved binary when present, publish the sidecar beside immutable versioned executables, and preserve release/ambient fallback when absent.

No integration, reload, network, stash, branch, or publication operation is authorized by this amendment.

## Final correction and validation amendment (2026-07-15)

This amendment is append-only. It records the bounded C1/C2 corrections after preserving the Opus PASS, Grok FAIL, and Fable provider-failure artifacts in commit `0a0cb5a06`.

### Final correction commits

| Commit | Scope | R01 relevance |
|---|---|---|
| `023226207` (`fix(identity): preflight initial subscribe and sidecar projection`) | Source-only correction. Factors server Subscribe handshake evaluation/event construction, preflights initial incompatible advertised Subscribe before provider fork/session/client/global/PID/member/tool state mutation, reads exact dev source sidecar metadata for runtime identity projection, and writes the sidecar beside installed immutable versioned binaries. | Closes C1 and C2 for R01. Canonical runtime identity now recovers exact dirty source fingerprint/hash metadata from the executable sidecar when present, while release/ambient fallback remains best-effort. |
| `db5b4a19d` (`test(identity): cover initial preflight and sidecar projection`) | Tests-only correction. Adds direct `handle_client` no-init incompatible initial Subscribe regression, sidecar projection regressions for same-commit dirty identities and immutable installed binaries, and Starting→SocketReady runtime identity preservation. | Proves the R01 projection and reload evidence behavior without live daemon/network/credentials. |

### Final validation log

All commands below were run from `/Users/jrudnik/labs/jcode-fix-r01-r03a-identity` with `CARGO_TARGET_DIR=target/r01-r03a-identity-validation`, after the tests-only commit. Cargo commands were run one at a time. `scripts/dev_cargo.sh` re-entered the repo Nix dev shell because `cargo` was not on PATH; it printed the standard trusted Cachix settings, hook installation, rerere import, `fork: main is 5 ahead github/main`, `fork: (remote state refreshing in background; rerun for an updated verdict)`, and `sccache skipped for incremental build` messages. No reload, activation, publication, live daemon, credentials, or intentional network action was performed by the correction itself.

| Step | Command | Result |
|---|---|---|
| Build-support lib | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-build-support --lib -- --nocapture` | Pass: `48 passed; 0 failed`. New sidecar tests `same_commit_dirty_sidecars_project_distinct_runtime_identities` and `installed_immutable_binary_sidecar_projects_exact_runtime_identity` passed. |
| Protocol lib | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-protocol --lib -- --nocapture` | Pass: `81 passed; 0 failed`. |
| App-core handshake focused | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-app-core --lib server::handshake::tests -- --nocapture` | Pass: `3 passed; 0 failed; 1091 filtered out`. One pre-existing warning: `drop_control_log_handle` dead code. |
| App-core lifecycle mistaken filter | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-app-core --lib server::client_lifecycle_tests::incompatible_initial_subscribe_preflights_before_full_session_initialization -- --nocapture` | Failed validation command: `0 passed; 0 failed; 1094 filtered out`, exit `97`, because the explicit filter matched zero tests. This is recorded as operator/filter error, not source evidence. |
| App-core lifecycle corrected filter | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-app-core --lib server::client_lifecycle::tests::incompatible_initial_subscribe_preflights_before_full_session_initialization -- --nocapture` | Pass: `1 passed; 0 failed; 1093 filtered out`. Confirms initial incompatible advertised Subscribe emits verdict+Error before provider fork, sessions, client connections, global session id, shutdown signals, soft queues, swarm member state, or active PID marker. |
| App-core reload preservation | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh test -p jcode-app-core --lib server::reload_state::tests::publish_socket_ready_preserves_starting_runtime_identity -- --nocapture` | Pass: `1 passed; 0 failed; 1093 filtered out`. |
| App-core check | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh check -p jcode-app-core --lib` | Pass. |
| TUI check | `CARGO_TARGET_DIR=target/r01-r03a-identity-validation scripts/dev_cargo.sh check -p jcode --bin jcode` | Pass. |

### Final status

- C1 is resolved for the initial advertised incompatible Subscribe path. Compatible advertised and legacy/no-advertisement flows retain their later normal handling, including exactly one verdict for applicable compatible advertising clients and no verdict for legacy clients.
- C2 is resolved for binaries with sidecar metadata. Projection uses exact sidecar fingerprint, dirty flag, short hash, full hash, activation channel, and resolved executable payload. Dirty labels are reconstructed as `<hash>-dirty-<fingerprint-prefix>` from sidecar fields.
- Fallback projection for binaries without sidecars remains best-effort using build metadata and resolved payload.
- No protocol version bump, compatibility-token semantic change, reload activation, or live daemon exercise was performed.

## Final correction re-review preservation (2026-07-15)

This amendment is append-only. Two independent read-only re-reviews evaluated exact head `c2eba7796` after the C1/C2 correction and both returned **PASS** with high confidence and no critical or important findings.

| Review | Repository artifact | SHA-256 | Verdict |
|---|---|---|---|
| Opus correction re-review | `../../reviews/2026-07-15-r01-r03a-correction-rereview-opus.md` | `f382998ca7fd56dbc302a43a7f234b3189e8d56979b58175fec342393fdd17f2` | PASS |
| Grok correction re-review | `../../reviews/2026-07-15-r01-r03a-correction-rereview-grok.md` | `9b265115ace7786b3698e4affeb006463a0b33903f266ccca73f031af77eafc6` | PASS |

The re-reviews agree that the initial incompatible advertised Subscribe now returns before provider/session/client/global/PID/member/tool mutation, exact dirty same-commit identity is recovered from executable sidecars when present, immutable selfdev publication writes that sidecar, R03A compatibility semantics remain separate, and `Some(runtime_identity)` survives Starting→SocketReady.

Non-blocking residuals remain explicit: ad-hoc or ambient binaries without sidecars use the documented lossy fallback; generic remote `/reload` still supplies `runtime_identity: None`, while the reviewed selfdev reload path supplies `Some` and the state transition preserves it. The assigned writer process exited before performing this final docs-only copy, so the coordinator preserved the already-completed reviewer artifacts directly. No source, test, integration, reload, network, or activation action occurred in this amendment.

## Coordinator combined-validation and build amendment (2026-07-15)

At coordinator HEAD `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`, the integrated R01/R03A chain is `615ab1d9a` through `6c6a4f2c8`. Exact source, test, documentation, review, validation-manifest, and intentionally absent category linkage is preserved in [`../../evidence/README.md`](../../evidence/README.md).

The sequential combined manifest SHA-256 is `41ece4820891461de774dbc5ab06d8e8a66c00630be62274d00dc1f5a9952291`. R01-relevant results are: `jcode-build-support` 48/48; identity handshake 3/3; initial incompatible Subscribe preflight 1/1; Starting-to-SocketReady identity preservation 1/1; live handshake/client matrix 4/4; TUI one-reexec/refusal matrix 7/7; app-core/TUI checks and root TUI binary check exit 0.

Coordinated no-reload build task `006329k9q8` passed for `6c6a4f2c8-dirty-7b4ec829c656`, source fingerprint `7b4ec829c656e856`. Immutable executable `/Users/jrudnik/.jcode/builds/versions/6c6a4f2c8-dirty-7b4ec829c656/jcode` is 227,257,456 bytes with SHA-256 `fd6297d9d9b135f7c8233dc27a6119bea767f74256e6dddccd1a0e5f557c6dd9`. The raw build transcript did not survive and is not claimed; task metadata, immutable file identity, channel state, and check logs are the available evidence. No reload or activation occurred.

The previously named R01 strict prerequisite is closed as a source-fix node. This is not pilot authorization; G2 remains the independent pilot gate.
