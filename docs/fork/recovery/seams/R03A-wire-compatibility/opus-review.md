# R03A Wire compatibility and subscribe handshake: independent Opus full-seam review

| Field | Value |
|---|---|
| Seam | R03A (Wire compatibility and subscribe handshake) |
| Review mode | `full`, independent (Opus) |
| Review head | `16921ace18cf5c25368a376357b7636478d3928f` |
| Fixed refs | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Verified merge base | `git merge-base fork upstream` = `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` (matches) |
| Supported disposition | **`retain-fork`, pilot entry BLOCKED on one named projection test** |
| Confidence | `high` on divergence/authority facts and legacy additivity; `medium-high` on the identity-projection hazard; `medium` on the incompatible-client action being safe for the resume/takeover path |
| Constraints honored | read-mostly; no source/ref/stash/worktree edits; no live user daemon, secrets, network, or payment. Only narrow deterministic tests were run. No future Grok R03A artifact was read. |

## Scope reviewed (behavior, not file buckets)

Per the authoritative `RESPONSIBILITIES.md` R03A row (line 24): R03A owns the
**stable wire schema, protocol/build compatibility verdict, safe short/full
hash comparison, legacy handling, and carriage of R01/R02 identity**. It
explicitly **excludes** the source of build or provider truth, transport
execution, and session recovery. Cross-seam invariant 1 and 6, and the
adjudicated R01/R03A split (index lines 63, 68, 93) bind this review: R01 owns
canonical identity meaning; R03A carries a declared projection and must not
derive competing truth.

Behaviors examined:

1. Wire encoding/decoding of the `Subscribe` request and `HandshakeVerdict`
   event (serde stability, additive fields).
2. The protocol version constant and its comparison.
3. The `Subscribe` build/protocol carrier fields.
4. The server `HandshakeVerdict` verdict (`HandshakeCompatibility::evaluate` +
   `evaluate_and_notify`).
5. Legacy/additive compatibility (clients that advertise nothing).
6. The client action contract for an incompatible verdict (re-exec / refuse /
   attach).

## Evidence ledger

| # | Finding | Evidence (file:line, ref, or command) | Confidence | Decides |
|---:|---|---|---|---|
| 1 | Fixed comparison refs are reproducible and base is the merge base. | `git merge-base 7ff4fc6be 802f690982` returned `631935dd1...`; matches the row baseline. | H | All conclusions are bounded to reproducible refs. |
| 2 | **The entire NS1 handshake surface is fork-only.** `HandshakeCompatibility`, `evaluate_and_notify`, `HandshakeVerdict`, and the client action module exist only in the fork. | `git grep -l HandshakeCompatibility <base>` and `<upstream>` returned empty; `<fork>` returned `server/handshake.rs`, `protocol/src/wire.rs`, `protocol/src/lib.rs`, `tui/app/handshake.rs`, and a communicate e2e test. | H | There is no upstream authority to adopt or compose. `retain-fork`, not `adopt-upstream`/`compose`. |
| 3 | `PROTOCOL_VERSION` is fork-only and equals `1`. | `crates/jcode-protocol/src/lib.rs:26` `pub const PROTOCOL_VERSION: u32 = 1;`. `git grep PROTOCOL_VERSION <base>/<upstream> -- crates/jcode-protocol` empty. | H | Version is a fork invention; upstream never diverged the constant. |
| 4 | **Upstream `Subscribe` is byte-identical to the merge base;** the fork added six additive optional fields. | `git show <upstream>:.../wire.rs` Subscribe body == `git show <base>:.../wire.rs`. Fork adds `protocol_version`, `build_hash`, `spawn_swarm_id`, `spawn_session_id`, `client_pid` (`wire.rs:202-249`). Of these R03A owns `protocol_version` + `build_hash`. | H | No two-sided wire collision on the handshake fields; the PRESCREEN's "near-total protocol overlap" is churn elsewhere in R03, not on these fields. |
| 5 | Both handshake carrier fields are strictly additive and legacy-safe on the wire. | `wire.rs:226-232`: `#[serde(default, skip_serializing_if = "Option::is_none")]` on `protocol_version` and `build_hash`. Round-trip test `test_subscribe_request_defaults_optional_flags` (`protocol_tests/misc_events.rs:382-418`) asserts both decode to `None` from `{"type":"subscribe","id":91}`. | H | A pre-NS1 client omits the fields and is never misread as a mismatch. |
| 6 | `HandshakeVerdict` event is additive and only sent to advertising clients. | `wire.rs:837-849` (`server_build_hash` is `skip_serializing_if Option::is_none`); `server/handshake.rs:57-66` gates the send on `client_protocol_version.is_some()`; e2e test `handshake.rs:855-866` asserts a legacy client never receives the event. | H | Additivity holds in both directions: legacy clients never see an unknown event tag. |
| 7 | The compatibility verdict is total and safe over every input, including the legacy no-advertisement case. | `wire.rs:66-113` `HandshakeCompatibility::evaluate`. Rule order: legacy(None)->Compatible; protocol mismatch->IncompatibleReconnect; same protocol + differing known non-empty hashes->IncompatibleReconnect; else Compatible (unknown/empty hash tolerated). No panic path. | H | The verdict cannot brick a legacy client and fails **open** only when the wire contract (protocol) already matches. |
| 8 | Short/full hash comparison is a safe prefix match. | `wire.rs:121-124` `hashes_match(a,b) = a.starts_with(b) || b.starts_with(a)`; tests `matching_protocol_tolerates_short_vs_full_hash`, `same_protocol_different_hash_is_incompatible` (`wire.rs:1602-1620`). | H (with one caveat, finding 12) | Matches `jcode doctor` behavior; short vs full forms of the same commit are Compatible. |
| 9 | The server verdict consumes only R01's declared projection and derives no competing truth. | `server/handshake.rs:29-35,72-74` passes `PROTOCOL_VERSION` and `jcode_build_meta::GIT_HASH` into the pure evaluator. It reads identity, it does not compute or persist any build identity of its own. | H | Satisfies the R01/R03A split (index line 93) and invariant 1: R03A is a carrier/verdict, not an identity source. |
| 10 | The client action contract for an incompatible verdict is a tested pure decision with a loop guard and a fail-safe. | `tui/app/handshake.rs:54-88` `decide_handshake_action`: Compatible->Attach; already-reexeced->Attach (loop guard, `REEXEC_GUARD_ENV`); target==current->Attach (server-side divergence); resolvable distinct target->ReExec; no target->Refuse. Six unit tests plus a unix process-launch smoke (`handshake.rs:195-301`). | H | The incompatible-client action never infinite-loops and never silently attaches to a mismatched daemon when a distinct launcher exists. |
| 11 | Deterministic no-network fixtures already prove the handshake end to end. | `communicate_tests/end_to_end.rs:690-759` spins a real `Server` over a temp Unix socket with a stub provider, subscribes with a mismatched hash, and asserts an `IncompatibleReconnect` verdict + subsequent `Done`; a sibling test asserts `Compatible` for a matching client and a third asserts legacy clients get no event. All use `JCODE_RUNTIME_DIR`/`JCODE_SOCKET` temp env, no real network or secret. | H | A no-network, no-user-daemon acceptance fixture for the verdict + legacy path already exists. |
| 12 | **Identity-projection hazard (inherited from R01, confirmed here): `build_hash` carries only the short commit hash and cannot distinguish two dirty builds from one commit.** | Client stamps `jcode_build_meta::GIT_HASH` (`tui/backend.rs:339`); server compares its own `GIT_HASH` (`server/handshake.rs:72-74`). `jcode-build-meta/src/lib.rs:10-11` defines `GIT_HASH` as the short commit hash with **no** `dirty`/fingerprint/version-label component. | H | Two same-commit dirty builds evaluate as **Compatible** (finding 8 prefix match on identical hashes) even though R01 defines them as distinct identities. This is a false-compatible hazard, exactly the R01 ledger's `build_hash`-is-a-compatibility-projection-only finding (`R01 ledger:41,49,61,79`). |
| 13 | The narrow deterministic suites pass on the review head. | `bash scripts/dev_cargo.sh test -p jcode-protocol --lib` = `80 passed; 0 failed` (includes 7 `handshake_verdict_tests` + 2 subscribe round-trip tests). `bash scripts/dev_cargo.sh test -p jcode-app-core --lib server::handshake` = `3 passed; 0 failed`. | H | The existing regression floor for the pure verdict and the server shell is green. |

## Negative findings

- **No upstream authority exists for any R03A behavior.** The verdict, the
  protocol constant, the two carrier fields, and the client action are all
  fork-only against the fixed refs. Any claim of "compose with upstream" here
  would be unsupported. The PRESCREEN R03 "near-total two-sided protocol
  overlap" (13/16) reflects churn across the broader transport/gateway/reconnect
  surface (R03B), **not** the handshake fields, which are additive and
  one-sided. This narrows R03A's contested-authority pressure well below the
  seed score.
- **No patch-equivalence claim is made.** R00's empty stable patch-ID
  intersection and the `b3ed82a6b` ancestry gap remain binding; file similarity
  is not adoption evidence.
- **No test proves the dirty-build projection.** No fixture creates two
  same-commit dirty builds and asserts the wire either distinguishes them or
  explicitly types `build_hash` as compatibility-only. The R01 ledger already
  named this as the blocking joint R01/R03A/R04 test (`R01 ledger:88, slice 2`).
  This review confirms R03A is the carrier that must be exercised.
- **No live end-to-end four-way identity agreement was exercised.** No live
  daemon/client dirty-build round trip is available in a bounded read-mostly
  review, and none was attempted (constraint).
- **The incompatible action re-exec side effect (`exec()`) was not executed.**
  Only the pure decision and a `/bin/sh` process-launch smoke were run. The real
  `replace_process`/`exec()` is exercised only by the reload path, which is R04
  and out of scope here.

## Backward-compatibility hazard inventory

Every identity field/carrier that crosses the R03A wire, with its
compatibility posture:

| Carrier | Where | Owner of meaning | Additive? | Hazard |
|---|---|---|---|---|
| `Subscribe.protocol_version: Option<u32>` | `wire.rs:227` | R03A (wire contract) | Yes (`skip_if None`) | None. `None` => legacy Compatible; different => IncompatibleReconnect. Correct. |
| `Subscribe.build_hash: Option<String>` | `wire.rs:232` | R01 (build identity), projected | Yes (`skip_if None`) | **Short-hash-only projection cannot distinguish dirty same-commit builds (finding 12).** False-compatible. |
| `HandshakeVerdict.server_protocol_version: u32` | `wire.rs:843` | R03A | Field always present, event gated to advertisers | None. |
| `HandshakeVerdict.server_build_hash: Option<String>` | `wire.rs:845` | R01, projected | Yes (`skip_if None`) | Same short-hash projection limitation; verdict is advisory only. |
| `HandshakeVerdict.compatibility` | `wire.rs:841` | R03A | Snake-case enum, only two variants | Adding a third variant later is a wire change: legacy/older clients would fail to deserialize the event. Since the event is only sent to advertisers of the **same** protocol contract, this is bounded, but a future variant needs a protocol bump. Noted, not blocking. |
| `Subscribe.selfdev: Option<bool>` | `wire.rs:207` | R01/R04 (channel) | Yes | Not compared in the verdict; used for re-exec target selection client-side (`handshake.rs:786, 121-122`). Channel identity is carried but not verified server-side. |

The carriage of R01/R02 identity is **partial by design**: R03A verifies only
`protocol_version` and short `build_hash`. Channel (`selfdev`), provider/model
(R02), fingerprint, dirty flag, and version label are **not** carried into the
verdict. That is consistent with the adjudicated split (R03A carries a declared
projection, R01 owns meaning), but it means the wire verdict can say
"Compatible" for two binaries R01 considers distinct.

## Disposition (one, supported)

**`retain-fork`. Pilot entry BLOCKED on exactly one added projection test.**

Rationale, tied to evidence:

- Upstream contributes nothing to any R03A behavior (finding 2, 3, 4). There is
  no candidate to adopt or compose. `retain-fork` is the only supported call.
- The fork implementation is well-factored, total, legacy-safe, and already
  covered by pure unit tests plus a no-network socket fixture (findings 5-11,
  13). The verdict correctly fails open only when the protocol contract already
  matches and a hash is not comparable, and fails to `Reconnect`/`Refuse` when
  it matters, with a loop guard.
- The single blocking gap is the identity-projection hazard (finding 12): the
  short-hash `build_hash` cannot express the R01 dirty-build distinction, so the
  wire can report a false Compatible. This is a **carriage** defect that R03A
  owns, exactly the joint R01/R03A/R04 contract the R01 ledger blocks the pilot
  on. R03A must either (a) declare/type `build_hash` explicitly as a
  compatibility-only projection (documented contract + a test asserting it is
  never treated as canonical identity), or (b) carry a distinct canonical
  projection (e.g. fingerprint/version-label) that R01 owns the meaning of.
  Either is a small, owned change; neither adopts upstream nor re-derives R01
  truth.

## Pilot prerequisites for R03A

Before R03A enters the bounded Phase 3 pilot (the subscribe-carries-identity
slice of the pilot question, `RESPONSIBILITIES.md:76`):

1. The dirty-build projection test below is green (the one blocker).
2. R01's canonical tuple contract is accepted (R01 ledger `adjudicated`,
   `retain-fork`, pilot blocked on the same joint test) so R03A carries a
   **declared** projection, not a competing one. R03A changes only its owned
   carriers (`build_hash`, verdict fields), never identity meaning.
3. R02 approves before any R02 identity (provider/model/entitlement) is added to
   the wire; today R03A carries **no** R02 field, so no R02 gate is triggered
   unless the pilot adds one. Invariant 2 stays R12's responsibility.
4. Legacy additivity is preserved: the `test_subscribe_request_defaults_optional_flags`
   and the "legacy client gets no verdict event" fixtures stay green.
5. R09 gates stay green without `--update`; any R03A code slice attributes its
   own panic/swallowed-error/size deltas. This review authorizes no source edit
   and introduces no debt.

## Cheapest acceptance tests (no network, no live daemon)

The smallest fixture that proves protocol/build projection and the
incompatible-client action already largely exists; the one missing test is
additive:

1. **[EXISTS, keep as floor]** Pure verdict matrix:
   `jcode-protocol` `handshake_verdict_tests` (legacy None, matching, short-vs-full,
   protocol mismatch, hash mismatch, unknown/empty hash). Green (finding 13).
2. **[EXISTS, keep as floor]** Wire additivity round trips:
   `test_subscribe_request_roundtrip_preserves_session_takeover_flags` and
   `test_subscribe_request_defaults_optional_flags` (legacy default => `None`).
   Green.
3. **[EXISTS, keep as floor]** No-network socket fixture: `communicate_tests/end_to_end.rs`
   subscribe-with-mismatched-hash -> `IncompatibleReconnect` + `Done`;
   matching-hash -> `Compatible`; legacy -> no event. Green e2e over a temp Unix
   socket with a stub provider.
4. **[MISSING, the blocker]** Dirty-build projection test (pure, no network):
   construct two `Subscribe` identities with the **same** short `build_hash`
   `"abc1234"` but distinct R01 canonical projections (fingerprint/version-label
   supplied as a second field or as a typed wrapper). Assert one of:
   - `build_hash` alone yields `Compatible` (documenting the projection is
     coarse) **and** a companion assertion proves the code never treats
     `build_hash` as canonical identity (grep-level or type-level contract), or
   - if a canonical projection is carried, two same-commit dirty builds yield
     `IncompatibleReconnect` (or an explicit typed compatibility-only marker).
   This is the R03A half of the R01 slice-2 joint test and requires no daemon.
5. **[EXISTS, keep as floor]** Client action contract: `decide_handshake_action`
   unit tests (attach/reexec/refuse/loop-guard/target==current) plus the unix
   `build_reexec_command` process-launch smoke. Green.

## Gaps and residual risk

- **Dirty-build false-compatible (finding 12):** the one blocking gap. Owned by
  R03A carriage, meaning owned by R01. Cheapest fix is a documented projection
  contract + test 4; no upstream import, no live daemon.
- **Channel (`selfdev`) not verified in the verdict:** two builds from the same
  commit on different channels are Compatible on the wire even though they may be
  different launchers. The client re-exec target selection uses `selfdev`
  (`handshake.rs:786`) but the **verdict** does not, so a channel-only divergence
  produces no `Reconnect`. Whether this matters depends on R01/R04 channel
  policy; flagged for the joint contract, not independently blocking.
- **Future verdict-enum evolution:** adding a third `HandshakeCompatibility`
  variant is a wire-breaking change for existing advertisers and must be gated by
  a `PROTOCOL_VERSION` bump. Documented risk, not present today.
- **`exec()` side effect untested here:** the real re-exec is exercised only by
  R04's reload path; out of R03A scope. The pure decision and command
  construction are tested.
- **R03B (transport/attach) is out of scope** and holds most of the PRESCREEN
  R03 churn; R03A's own fields are one-sided and additive.

## Checkpoint budget

8 decisive checkpoints, consumed without expansion:
(1) refs/merge-base; (2) fork-only handshake surface; (3) fork-only
`PROTOCOL_VERSION`; (4) upstream Subscribe == base, fork additive fields;
(5) legacy additivity + verdict totality read; (6) client action contract read;
(7) existing no-network e2e fixture read; (8) narrow deterministic tests run
(`jcode-protocol --lib` 80/0, `jcode-app-core --lib server::handshake` 3/0).
Narrowed rather than broadened: R03B transport and the live four-way identity
round trip were explicitly deferred, not reviewed.

## Validation and sign-off

- Commands run (read-only / narrow tests only): `git merge-base`,
  `git grep -l HandshakeCompatibility <base|upstream|fork>`,
  `git grep PROTOCOL_VERSION <base|upstream>`, `git show <base|upstream>:wire.rs`
  Subscribe body comparison, `bash scripts/dev_cargo.sh test -p jcode-protocol --lib`
  (80 passed; 0 failed), `bash scripts/dev_cargo.sh test -p jcode-app-core --lib server::handshake`
  (3 passed; 0 failed).
- No source, ref, stash, or worktree change; no live user daemon, secret,
  network, or payment. No future Grok R03A artifact was read.
- Independent conclusion: **`retain-fork`, pilot BLOCKED** on the single
  dirty-build projection test (test 4), which is the R03A half of the R01
  ledger's joint slice-2 contract.
