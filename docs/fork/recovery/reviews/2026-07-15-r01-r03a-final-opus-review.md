The swarm report tool is not available in my function set, so I'll deliver the completion report directly as my final response.

---

# ADVERSARIAL REVIEW: R01/R03A IDENTITY PREREQUISITE

VERDICT: PASS

Scope reviewed: baseline e7e47e42c; source commits c759e2504, 28a63f9f4; append-only docs e95d3fdfa. Read-only. No build/test/network/reload executed. Working tree clean on branch recovery/fix-r01-r03a-identity-20260715.

## INVARIANT-BY-INVARIANT VERDICT

1. One R01-owned canonical source/runtime identity projection: PASS
   - RuntimeIdentityProjection defined once, crates/jcode-selfdev-types/src/lib.rs:93-119, with SourceState::runtime_identity_projection at lib.rs:121-135. build_hash is explicitly documented as R03A-compatibility-only and distinct at crates/jcode-protocol/src/wire.rs:228-239. Boundary is clean: identity type lives in the leaf types crate, projected by SourceState.

2. Dirty builds from same commit distinguish deterministically: PASS
   - Production version_label embeds fingerprint for dirty builds: crates/jcode-build-support/src/source_state.rs:122-123 (`{short_hash}-dirty-{fingerprint[..12]}`). Test proves same short_hash but distinct fingerprint/version_label/projection: crates/jcode-build-support/src/tests.rs:105-121.

3. R03A compatibility remains distinct and fail-closed: PASS
   - HandshakeCompatibility::evaluate is total and unchanged in intent, crates/jcode-protocol/src/wire.rs:66-114; legacy None protocol returns Compatible (wire.rs:72-77); only two variants (Compatible, IncompatibleReconnect), no soft-warn leak.

4. Incompatible advertised subscribe fails before session/member/PID/tool mutation: PASS
   - Fail-closed break at crates/jcode-app-core/src/server/client_lifecycle.rs:1375-1382 fires before current_client_instance_id assignment (1383), connection mutation (1384-1396), session resume, and handle_subscribe. Break exits the request loop (661) to cleanup_client_connection (2693). Live e2e proof asserts an Error sentinel and no advertised PID marker written: crates/jcode-app-core/src/tool/communicate_tests/end_to_end.rs:747-780.

5. Clients either consume the verdict or omit advertisement: PASS
   - Only the TUI advertises (crates/jcode-tui/src/tui/backend.rs:338-342) and it consumes via act_on_verdict at crates/jcode-tui/src/tui/app/remote.rs:787. Generic Client now sends None/None/None (crates/jcode-app-core/src/server/client_api.rs:93-99) with a guarding test (client_api.rs:384-391). ACP and e2e harness also omit (src/cli/acp.rs, tests/e2e/test_support/mod.rs). Grep confirms no other non-test advertiser.

6. TUI re-exec at most once; refuses missing/same/failed/second-mismatch: PASS
   - Pure decision in crates/jcode-tui/src/tui/app/handshake.rs:53-95: already_reexeced -> Refuse (66-70); same-target -> Refuse (79-84); missing target -> Refuse (89-93); failed exec -> Refuse (156-161); distinct target -> ReExec with REEXEC_GUARD_ENV guard (build_reexec_command 173-181). All four refusal branches have tests (215-311). Refuse is surfaced and quits at remote.rs:789-794.

7. Runtime/reload evidence preserves selected executable/source identity: PASS (with a test-coverage note, LOW)
   - Selfdev reload projects the exact selected target via source.runtime_identity_projection("selfdev", resolve_binary_payload(&target_binary)) at crates/jcode-app-core/src/tool/selfdev/reload.rs:348-357, using target_binary from find_dev_binary (251) not the running exe. Signal carries it (reload_state.rs:520-548); all Failed/Starting writes clone it (reload.rs:96-207); phase transition preserves it at reload_state.rs:110-117.

8. Legacy clients remain deliberate and bounded: PASS
   - Wire fields are #[serde(default, skip_serializing_if="Option::is_none")] (wire.rs:236-239, 854-857). Legacy no-verdict path preserved (handshake.rs legacy_client_gets_no_verdict_event). Non-selfdev reload triggers deliberately pass None (client_session.rs:789, debug_command_exec.rs:608, selfdev/setup.rs:294): they reload to an installed GIT_HASH build, not a dirty source projection.

## FINDINGS

- LOW (missing test): No positive test asserts a Some(runtime_identity) survives the Starting->SocketReady transition. Code is correct (reload_state.rs:116 clones it) but publish_reload_socket_ready_updates_current_process_marker (socket_tests.rs:255-281) only asserts detail with runtime_identity=None. Recommend adding a Some-valued assertion.

- LOW (observed, not a defect): Live self-binary current_runtime_identity_projection sets source_fingerprint=None and source_hash=GIT_HASH (build-support/src/lib.rs:54-66). Two same-commit dirty binaries built without JCODE_BUILD_GIT_DIRTY differentiation could share GIT_HASH/VERSION and thus produce identical live handshake projections. This is honestly documented in the doc comment ("Release/ambient binaries cannot always reconstruct... Dirty selfdev publication paths should prefer SourceState::runtime_identity_projection") and the authoritative dirty path (selfdev reload) uses the full SourceState projection. Not a violation of the stated invariants; noted for completeness.

- INFO: 28a63f9f4 is a pure re-export addition (crates/jcode-app-core/src/server.rs:474-479) with no model change, matching the recorded compile-failure remediation.

## SCOPE / MIGRATION COMPLETENESS

All 25 Request::Subscribe sites carry the new field; the only literal without runtime_identity (crates/jcode-protocol/src/lib.rs:710) is a `{ id, .. }` id accessor. Field additions are additive and wire-compatible. ReloadState/ReloadSignal/ReloadState.write all updated consistently across tests.

## LEGACY-POLICY ASSESSMENT

Sound and bounded. Advertisement is opt-in and confined to the enforcing TUI; every non-consuming client omits identity; legacy decode preserved by serde defaults; non-selfdev reloads intentionally omit projection. No unbounded relaunch: single REEXEC_GUARD_ENV gate with fail-closed refusal on the second pass.

## DOCS ACCURACY (e95d3fdfa)

Append-only, prior bytes preserved. Recorded history matches code: build_hash-as-compat-only, generic-client de-advertisement, fail-closed-before-mutation, TUI one-reexec/refusal, and the 28a63f9f4 export fix are all verifiable in source. Validation-history caveats (invalid initial command, stale shared-target artifacts, real helper-export failure fixed by 28a63f9f4, concurrent test-list sequencing violation) are faithfully framed as non-validation.

## NOT CHECKED (declared)

- Did not run Cargo/Nix; test pass counts (46/46, 81/81, 37/37, R09 17+ratchets) accepted as asserted, not re-executed.
- Did not exercise real exec()/re-exec or a live daemon reload.
- Did not verify R09 ratchet internals or selfdev-profile build output.
- Serialization/deserialization verified by inspection and the roundtrip test (misc_events.rs:257-292), not by runtime execution.

Note: the swarm report tool was not present in my available toolset (Skill 'swarm' not found; no `jcode swarm` subcommand), so this completion report is delivered inline. Blockers: none. Follow-up: add the one Some-valued reload-identity preservation test noted above.