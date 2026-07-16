# Phase 3 bounded pilot plan

Date: 2026-07-15
State: `designed`; execution remains blocked until the dedicated validation driver and fixture slices are committed and validated.

## Authority and fixed evidence

- Independent G2 verdict: **PASS**, [`reviews/2026-07-15-g2-pilot-gate-opus.md`](./reviews/2026-07-15-g2-pilot-gate-opus.md), SHA-256 `abb7b2694abccb0c32385fc552dcc29bf0eba854d439c5c43dc82ba4f3991e4f`.
- G2 preservation commit: `5bcb8884022d3892ac4283f1d355a8d9188dc74e`.
- Integrated source checkpoint reviewed by G2: `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`.
- Fixed comparison refs: fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`, upstream `802f6909825809e882d9c2d575b7e478dce57d3b`, merge base and `vendor/upstream` target `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`.
- Preserved prompt diff SHA-256: `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`.
- Current R09 truth: panic `46`, swallowed `3,074`, production-size `61`, test-size `31`, wildcard `16`, warnings `0`, classifier `17/17`.

## One permitted question

On a disposable environment and fixed recovery history, can one in-process product-owned accepted-tier fixture select one symbolic subscription route, encode and decode one compatible Subscribe carrying an additive R01 runtime projection, execute exactly one noninteractive no-tool/no-memory agent turn, and persist/replay exactly one correlated provider request and response without telemetry, secrets, external effects, compaction, retry, cancellation, or gate drift?

No broader behavior is adjudicated.

## Exact fixture boundary

The fixture is one `#[tokio::test(flavor = "current_thread")]` named:

`agent_tests::recovery_pilot_one_fixture_route_subscribe_turn_evidence`

It runs in one Rust test process and creates exactly one `Agent` session. It does not start the live server or daemon and does not open a TCP, Unix, or WebSocket listener.

Fixture environment:

1. Acquire the existing process-wide test environment lock.
2. Create one fresh temporary `JCODE_HOME` and export `JCODE_NO_TELEMETRY=1`.
3. Set only symbolic `JCODE_API_KEY=fixture-key`; set ambient `JCODE_TIER=flagship` as an adversarial non-authoritative value.
4. Disable memory on the agent and set `allowed_tools` to an empty set.
5. Do not configure MCP, discovery, updater, reload, swarm, cancellation, retry, manual compaction, or a daemon/socket.

## Bounded implementation slices

These slices remain separate commits:

1. **Tooling:** add the generic noninteractive validation driver. It records plan, command, expected/actual exit, timestamps, tool paths, disk and build-process snapshots, preservation hashes, logs, manifest, and SHA-256 sums. It rejects any command containing `--update` and stops on the first unexpected exit or preservation change.
2. **Testability refactor:** extract the successful `/v1/me` body application from `fetch_subscription_me` into one private shared helper with unchanged production behavior. Under `cfg(any(test, feature = "test-support"))`, expose a thin in-process fixture wrapper so app-core tests can exercise live-tier/auth side effects without opening a socket.
3. **Fixture:** add the one exact app-core test. No production identity writer, evidence schema field, retry path, cancellation status, compaction path, or external-effect path may change.
4. **Evidence/status:** run the checked-in plan through the driver, preserve exact output and hashes, then append pilot results. No implementation and result documentation share a commit.

If slice 2 requires more than a helper extraction and test-support wrapper, stop. If slice 3 requires production server mutation or a new identity writer, stop.

## Exact in-process observations

The fixture passes only if every assertion below holds.

### 1. R02 entitlement and route provenance

- Parse and apply a fixed successful `/v1/me` JSON body in process, with `account_id=acct_fixture`, `tier=plus`, and `status=active`.
- Auth readiness is `CredentialPresent` before applying the body and `RequestValid` afterward.
- Tier truth is `Plus` with freshness `Live`; the adversarial ambient `JCODE_TIER=flagship` does not substitute for it.
- A Flagship-only model remains denied while `gpt-5.5` is admitted for Plus.
- The selected structured route is fixed to model `gpt-5.5`, provider label `jcode`, runtime key `Other("jcode-subscription")`, and API method `jcode-subscription`.
- The provider reports `ResolvedCredential::Oauth`; no credential bytes are copied into the route or evidence.

### 2. R01/R03A Subscribe composition

- Construct one `Request::Subscribe` with `protocol_version=PROTOCOL_VERSION`, `build_hash=jcode_build_meta::GIT_HASH`, and a nonempty fixture `RuntimeIdentityProjection` whose canonical fields are intentionally distinct from the running server projection.
- Serialize with `serde_json`, decode with `jcode_protocol::decode_request`, and assert the runtime projection is preserved exactly and remains optional/additive.
- Evaluate compatibility using only protocol plus build hash and assert `Compatible` with a typed `HandshakeVerdict` carrying the server projection.
- Assert compatibility is not canonical runtime-identity equality: the fixture client projection differs from the server projection while the compatibility verdict remains `Compatible`.

This proves wire carriage and verdict semantics without creating a daemon session, disconnect path, takeover path, or generic-client identity claim.

### 3. One R12/R06A turn

- Apply the structured route to the one agent with `Agent::set_route_selection`.
- The deterministic provider returns only `TextDelta("fixture answer")`, token usage `7` input and `3` output, then `MessageEnd(end_turn)`.
- Execute exactly one `run_once_capture` call.
- Persisted readback contains exactly four schema-v1 events at sequences `0..=3`: `TurnStarted`, `ProviderRequest`, `ProviderResponse{Ok}`, `TurnFinished{Ok}`.
- Every event shares one turn ID. Request and response share exactly one provider-request ID. Terminal counts are `(1 request, 1 response, 1 finish)`.
- Persisted provider/model/route/tool-count equal the R02-selected provider/model/API method and `0` tools. Usage totals equal `7/3/10`.
- No compaction or route-selection event interleaves.
- Append one malformed fifth line and assert replay returns exactly the original four valid events.

The production R06A schema does not contain account or entitlement fields. The fixture therefore joins the test-side `acct_fixture`/Plus provenance to the persisted provider/model/route assertions without inventing new session-log fields or claiming that account/entitlement are persisted production fields.

### 4. Consent and secret checks

- `jcode_telemetry_core::is_enabled()` is false for the process.
- `$JCODE_HOME/telemetry_share_content` does not exist.
- No file under `$JCODE_HOME/logs`, if present, contains `begin telemetry session`.
- The raw evidence JSONL and captured fixture output do not contain `fixture-key`.
- The fixture records only symbolic account/provider/model/tier/route values and hashes or counts, not prompt credentials.

## Driver entry checks

Before the first command, the driver must fail closed unless:

- branch is `recovery/2026-07-15`;
- the sole pre-existing dirty path is `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`;
- its diff SHA-256 is `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`;
- stash count is `4` and the stash-list hash matches the plan-start snapshot;
- every registered worktree and branch snapshot matches the plan-start snapshot;
- `vendor/upstream` resolves to `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`;
- no Cargo, rustc, Nix build, or selfdev build lane is active;
- at least 20 GiB is free;
- the plan contains no `--update`, network, daemon, reload, publication, install, or update command.

The run must be launched from the already cached Nix dev shell in offline mode with `CARGO_NET_OFFLINE=true` and Nix substitution disabled. A missing cached dependency is a stop, not permission to access the network.

## Exact validation sequence

The machine-readable sequence is [`pilot/2026-07-15-g4-validation-plan.json`](./pilot/2026-07-15-g4-validation-plan.json). It is sequential and stops at the first unexpected exit.

1. Exact pilot fixture: expected exit `0`.
2. Shared classifier: expected exit `0`, 17/17.
3. Panic budget: expected exit `1`, current `46` versus baseline `31`.
4. Swallowed-error budget: expected exit `1`, current `3,074` versus baseline `2,987`.
5. Production-size budget: expected exit `1`, current `61`.
6. Test-size budget: expected exit `1`, current `31`.
7. Wildcard re-export budget: expected exit `0`, total `16`.
8. Warning budget: expected exit `0`, current `0`.
9. Shell syntax: expected exit `0`.
10. Diff check: expected exit `0`.

No red ratchet is rebaselined or interpreted as green.

## Driver exit checks and evidence

After the last command, the driver repeats every preservation and disk/build-lane snapshot. It fails if the prompt hash, stashes, worktrees, branches, merge-base pin, or forbidden-process posture changed.

The evidence directory must contain:

- immutable copy of the JSON plan;
- `run.meta.json` with start/end UTC, branch, HEAD, architecture, tool paths, environment allowlist, and offline controls;
- `preflight.json` and `postflight.json`;
- one numbered log per command containing command, expected exit, start/end timestamps, actual exit, and captured output;
- `manifest.tsv` with expected/actual exit, elapsed time, command SHA-256, and log SHA-256;
- sorted `SHA256SUMS` for every evidence member except `SHA256SUMS` itself.

The fixture log must contain one deterministic `PILOT_OBSERVATION` JSON line summarizing non-secret account/tier/auth/provider/model/route/runtime-projection/terminal-count results. The driver does not synthesize missing observations.

## Immediate stop and rollback

Stop immediately on any missing/duplicate terminal response, correlation/sequence mismatch, wrong route identity, nonzero tool count, compaction/retry/cancellation event, telemetry enablement/log, secret string in output/evidence, write outside disposable paths, unexpected gate exit/count, use of `--update`, network/cache miss, daemon/reload/tool/MCP/memory/discovery/swarm path, disconnect/takeover path, unowned identity writer, or preservation change.

Rollback only the active slice commit and delete disposable runtime paths. Do not reset, rebase, replay, pop stashes, edit refs/worktrees, clean caches, alter the prompt edit, touch the live daemon, or rewrite evidence. Preserve the failed logs and append the failure before retrying a corrected slice.

## Evidence limitation and claim boundary

A PASS proves one deterministic happy-path composition only. It does not prove live provider behavior, actual backend tier truth, cancellation, retry, context-limit recovery, compaction, tools, memory, MCP, discovery, telemetry delivery, daemon/reload, resume, disconnect/takeover, background work, swarm, UI, release, installation, publication, or global terminal-cardinality correctness.
