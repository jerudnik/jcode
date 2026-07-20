# F16 implementation review (adversarial, opus-class)

Reviewed 04590233e..8f26fa291. Verdict: **FAIL**, fixed in-cycle at
8971ed1db, coordinator-verified post-fix.

## Blocking (fixed)

1. fork-ci builds with --target triples; find_e2e_binary never probed
   target/<triple>/release, and JCODE_E2E_REQUIRE_BINARY is exported
   nowhere, so all three promoted tests would silently SKIP-and-pass on
   every CI run. Fixed: triple-dir probe. REQUIRE_BINARY export handed
   to F17 (owns workflows).

## Important (fixed / carried)

1. Teardown not panic-safe: asserts inside async blocks unwound past
   kill_child/kill_spawned_server. Fixed: anyhow::ensure! conversion
   (failures flow through the diagnostic + reap path).
2. Even with the path fix, PTY selfdev tests gate on source-match; CI
   confirmation once REQUIRE_BINARY lands is an F17 follow-up.

## Minor

reconnects_quickly latency is log-only (functional assertion is real:
server id changes, exactly one client, session id stable); stale
reload_marker_active log-only call remains in a helper; whitespace diff
was committed separately.

## Gates executed

Suite run 4/4 passed with selfdev binary; residue empty; no #[ignore] on
promoted tests; JCODE_E2E_BINARY hard-panic and REQUIRE_BINARY behavior
verified by inspection; env mutation serialized via JCODE_HOME_LOCK.

## Not checked

Live fork-ci runner execution; repeated-run flake probing; triple-build
dev_binary_matches_source semantics; Linux job parity.
