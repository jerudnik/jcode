# F16: hermetic lifecycle cases promoted to required

Commits: 04590233e, a8db1fad5, e925b8ce9, 8f26fa291 (worker), validated
by the coordinator against a fresh source-matching selfdev binary.

## Promoted tests (no longer #[ignore]d)

| Test | Was ignored because | Now |
|---|---|---|
| binary_integration_reload_handoff | needed target/release/jcode | find_e2e_binary(): JCODE_E2E_BINARY -> release -> selfdev -> debug; loud SKIP if absent; hard fail under JCODE_E2E_REQUIRE_BINARY=1. Runs the real exec handoff end to end. |
| binary_integration_selfdev_reload_reconnects_quickly | same + load-sensitive latency assertion | functional assertion required; latency bound widened to 30s with per-cycle timing logged |
| binary_integration_selfdev_client_reload_resumes_session | same | version assertion compares against the resolved reload target's --version |

`..._full_reload_...` remains ignored (needs two genuinely different
builds; out of scope, classified in F15).

## Latent test bugs found and fixed during promotion

1. reload_handoff sent Reload without Subscribe; the hardened server
   silently refuses that, so the test could never have passed as written.
2. reload_handoff checked reload_marker_active() against the wrong
   runtime dir (vacuous); replaced with debug-socket identity polling.
3. Fixtures must not set JCODE_TEMP_SERVER (temporary servers refuse
   reload by F03 design); teardown reaps daemons via the disposable
   $JCODE_HOME/servers.json registry.
4. Pre-existing e2e compile break in reload_multiclient.rs fixed
   (Server::run() signature change from R04).
5. PTY selfdev fixtures guard against source-stale binaries
   (require_selfdev_e2e_binary): the product's hot_exec would otherwise
   rebuild mid-test for minutes. Clean-tree CI runs execute fully.

## Gates

1. No required lifecycle case ignored for build-layout convenience:
   all three shortlisted tests promoted; remaining ignores are classified
   non-layout reasons (F15 census).
2. Disposable home/runtime, zero residue: per-test TempDirs for
   home/runtime/install; coordinator verified zero fixture daemons and
   temp dirs after three consecutive suite runs.

## Validation (coordinator, fresh binary 8f26fa291-dirty)

    JCODE_E2E_BINARY=target/selfdev/jcode cargo test --test e2e binary_integration
    run 1: 3 passed, 1 failed (concurrent build load; see note)
    runs 2-4: ok. 4 passed; 0 failed; 3 ignored  (x3 consecutive)

Flake note: the single failure occurred while a selfdev rebuild held the
cargo lock and system load was high; three consecutive clean runs
followed. Same load-sensitivity class as F15's table; the 30s bound and
functional/latency split already mitigate. Residue checks clean; one
8-hour-old orphaned fixture daemon from a killed F08 gate run (predating
F16) was found and reaped, recorded as a known hazard of killing gate
scripts mid-run rather than an F16 fixture defect.

## Review round (F16 review FAIL -> fixed)

Review found BLOCKING-1: fork-ci builds with an explicit --target triple
(target/<triple>/release), invisible to find_e2e_binary, so in CI the
promoted tests would silently SKIP-and-pass. Fixed at 8971ed1db: the
probe scans target/*/{release,debug} one level down. Exporting
JCODE_E2E_REQUIRE_BINARY=1 in the workflow belongs to F17 (owns CI
rails) and is recorded as its input.

important-1 (teardown not panic-safe: assertions unwound past
kill_child/kill_spawned_server, leaking PTY child + daemon) fixed in the
same commit: assertions inside async test blocks converted to
anyhow::ensure! so failures flow through the error path, which prints
diagnostics and still reaps.

Post-fix validation: 4 passed, 0 failed, 3 ignored against
target/selfdev/jcode; zero fixture residue.
