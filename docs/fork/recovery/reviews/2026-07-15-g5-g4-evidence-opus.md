# G5 Independent Adversarial Review — G4 Bounded Pilot Evidence

- Reviewer: sole independent G5 adversarial reviewer (verify posture, read-only).
- Fixed commit reviewed: `da7c155b9d34ff719e065c855338eea3574d62a9` on branch `recovery/2026-07-15`.
- Repository: `/Users/jrudnik/labs/jcode`.
- Constraints honored: no repository/Git/stash/branch/worktree/daemon/build mutation; no network, credentials, live providers, tools/MCP, memory, publication, install, or reload. Only local read-only Git, hash, JSON, grep, offline `python3` unittest, and offline source inspection were used. No Cargo/Nix build ran.
- Method: recomputed all evidence and named artifact SHA-256 hashes; verified manifest step structure and exit parity; compared preflight/postflight snapshots; read the fixture test and production diff at source; ran the driver's own offline unit tests; audited commit separation, authority boundary, and preservation state.

## VERDICT: PASS

The G4 bounded pilot evidence at `da7c155b9` is sound. Every SHA-256 recomputes exactly, the manifest is a byte-faithful ten-step run with expected==actual exits, exactly one standalone `PILOT_OBSERVATION` and zero forbidden-output hits, expected-red debt is honestly reported as red (never as green source evidence), the fixture behavior is genuinely behavior-backed (not merely self-asserted), the driver is fail-closed on every required dimension, no production behavior/wiring/authority exceeds the G2 grant, append-only history was not rewritten, and preservation state is intact (only the user-controlled `ORCHESTRATOR_PROMPT.md` diff `8e8e6a92...`, four stashes).

Confidence: high.

## 1. Hash recomputation (independent)

### Bounded-pilot set (`evidence/2026-07-15-g4-bounded-pilot/`)
- All 15 members verified with `shasum -a 256` against the stored `SHA256SUMS`: ALL OK.
- Directory listing exactly matches `SHA256SUMS` membership; no unlisted or missing files.
- Enclosing `SHA256SUMS` file SHA-256 = `b4692dc023075d89fcbe94065d089234fa59bbc5777215082870eb00c3842343` (matches G4_RESULT and PILOT_PLAN/evidence README claims).

### Attempt-history set (`evidence/2026-07-15-g4-attempt-history/`)
- All 13 members (including the nested `01-vendor-ref-preflight-partial/` and `02-observation-framing-driver/` subtrees) verified: ALL OK.
- Directory listing exactly matches `SHA256SUMS` membership.
- Enclosing `SHA256SUMS` file SHA-256 = `f1fa86fdbffca927d0128fda92bdb3ff3cdfa85d2561d02b683cd275941f4944` (matches claim).

### Named artifact hashes (all reproduced exactly)
- `manifest.tsv` = `321c43f51d5cd6e9d953896117d90873adf17b5b4f594ea7fb4f1cb2341eb4e5`
- `run.meta.json` = `b85e34c61e434e956c2a8cdfc51785ddf3b99d111bf59f5cbb7600bdae9140bb`
- Pilot log `01-pilot_fixture.log` = `fdc47ac6cb27cad0dec492990075f98c8a248341fa7de13db00c953c3ae484bf`
- Source plan and evidence `plan.json` both = `28563740d66302e23d70fe28c34c4d6ed563c553a7eec206dd09dbcd8840134b`; `diff` of source vs preserved copy is byte-identical, and this equals `run.meta.json.plan_sha256`.
- G2 review artifact = `abb7b2694abccb0c32385fc552dcc29bf0eba854d439c5c43dc82ba4f3991e4f` (matches PILOT_PLAN authority citation).

## 2. Manifest structure and exit parity

- `manifest.tsv` has exactly ten data rows (steps 1..10), matching `steps_planned=10`, `steps_run=10`, `result="passed"`, `failure=null` in `run.meta.json`.
- For every row `expected_exit == actual_exit`: three green `0/0` (pilot_fixture, classifier, wildcard, warning, shell_syntax, diff_check) and four intentional-red `1/1` (panic, swallowed, code_size, test_size). Precisely: 0,0,1,1,1,1,0,0,0,0.
- `pilot_observation_count` is `1` only for the `pilot_fixture` row and `0` elsewhere; `forbidden_output_hits` is `0` for all ten rows.
- The single `PILOT_OBSERVATION` line in the pilot log is at line 22, on its own line, beginning `PILOT_OBSERVATION {` (the driver counts `line.startswith("PILOT_OBSERVATION ")`; the `\n` prefix fix in commit `505cd8672` makes it standalone). `grep -c` confirms exactly one.

## 3. Preflight vs postflight preservation

Comparing `preflight.json` and `postflight.json`, all `preservation_projection` fields are identical:
- branch `recovery/2026-07-15`; dirty_paths `["docs/fork/recovery/ORCHESTRATOR_PROMPT.md"]`; prompt_diff_sha256 `8e8e6a92...`; stash_count `4`; stash_list_sha256, worktrees_sha256, branches_sha256 all unchanged; vendor_upstream_head `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`; head `505cd8672...`; active_build_processes `[]`.
- Only runtime measurements differ (disk_free_bytes 40231362560 -> 39860142080, both > 20 GiB minimum; utc timestamps). The driver's `preservation_projection` correctly excludes disk/tool/utc, so these do not count as drift. Tool paths identical.
- `vendor_upstream_head` in preflight, postflight, plan, and G4_RESULT all agree and match the live `refs/heads/vendor/upstream` resolution (`631935dd1...`).

## 4. Fixture behavior — genuinely behavior-backed

Read `crates/jcode-app-core/src/recovery_pilot_tests.rs` at HEAD. The single `#[tokio::test(flavor = "current_thread")]` `recovery_pilot_one_fixture_route_subscribe_turn_evidence` composes and asserts, not merely narrates:

- **Offline accepted-tier route:** `apply_subscription_me_fixture` parses a fixed `/v1/me` body (no socket), asserting `account_id=acct_fixture`, `parsed_tier=Plus`, `freshness=Live`, `cached_tier=Plus`, `effective_tier=Plus`; `gpt-5.5` allowed, `claude-fable-5` denied. Adversarial ambient `JCODE_TIER=flagship` is set and proven not to substitute (effective tier stays Plus).
- **Auth provenance:** `auth_before == CredentialPresent` before applying the body, `auth_after == RequestValid` after cache invalidation. Derived from the fixture response, not key presence.
- **Compatible Subscribe / runtime projection:** builds `Request::Subscribe`, serializes, decodes via `decode_request`, asserts `runtime_identity` preserved exactly and additive; `HandshakeCompatibility::evaluate` returns `Compatible`; asserts client projection `!=` server projection (`assert_ne!`) while verdict remains `Compatible` — compatibility is explicitly not identity equality.
- **One current-thread Agent, empty tools, memory disabled:** `Agent::new_with_session(..., Some(HashSet::new()))`, `set_memory_enabled(false)` then `assert!(!agent.memory_enabled())`.
- **Telemetry disabled:** `assert!(!crate::telemetry::is_enabled())` and asserts no `telemetry_share_content` file.
- **Exactly one provider call:** the mock provider records calls; `assert_eq!(calls.len(), 1)`, `tool_count == 0`, `model == "gpt-5.5"`, `message_count > 0`.
- **Four correlated evidence events:** readback asserts `events.len() == 4`, sequences `[0,1,2,3]`, all schema-v1, kinds TurnStarted/ProviderRequest/ProviderResponse/TurnFinished, shared `turn_id`, one shared `provider_request_id` across events[1]/events[2] with none on events[0]/events[3]. ProviderRequest asserts provider `jcode`, model `gpt-5.5`, route `jcode-subscription`, tool_count 0. ProviderResponse asserts status Ok, usage 7/3/10, no error_class.
- **Terminal cardinality / usage:** `provider_terminal_counts` asserts `(1,1,1,[Ok])`.
- **Secret exclusion:** raw evidence file asserted to not contain `fixture-key`; log dir scanned to not contain `begin telemetry session`.
- **Replay:** appends a malformed fifth line, asserts `read_session_evidence_from_path` returns exactly the original four events.

**On self-assertion (nonblocking):** the emitted `PILOT_OBSERVATION` JSON uses literal field values rather than reading the runtime variables. Taken alone, the printed line is a hardcoded string. However, the driver only counts the line for a pilot step whose exit already matched, and every value in that JSON is independently backed by a preceding `assert!`/`assert_eq!` that panics (failing the test, exit != 0) if false. So the observation is behavior-backed via the surrounding assertions. The only field with no direct binding is aesthetic (`credential: "oauth"` is asserted via `active_resolved_credential == Some(Oauth)`; that too is backed). This is an honest evidence characteristic, not a gap.

## 5. Driver fail-closed audit

`scripts/recovery_validation_driver.py` enforces, and its offline unit tests (`tests/test_recovery_validation_driver.py`, 10 tests, ran green here) cover:
- **Plan schema:** `validate_plan` requires schema_version 1, nonempty steps, required string keys, integer stash count, nonnegative minimum_free_gib, nonempty forbidden-output strings, unique step names, valid expected_exit (0..255) and positive timeout. Tested for duplicate names and invalid forbidden lists.
- **Forbidden commands:** rejects `--update`, `http://`, `https://`, `curl `, `wget `, `cargo install`, `cargo publish`, `nix flake update`, `nix profile`, `selfdev`, ` server reload`, ` release`. Tested (`--update`, network command).
- **Output:** any forbidden-output substring forces exit 126 and appends a driver error; tested.
- **Timeout/process groups:** steps run in `start_new_session=True`; timeout maps to exit 124 and `terminate_process_group` sends SIGTERM then SIGKILL to the group.
- **Preservation:** preflight validates branch, sole dirty path == prompt, prompt hash, stash count, vendor head, disk floor, and no active build; postflight recomputes `preservation_projection` and fails on any change or on an active build process. Tested that the projection excludes runtime measurements.
- **Expected exits:** first `actual != expected` halts and records the failure.
- **Exact observation count:** for `pilot_fixture` with matching exit, non-unity `PILOT_OBSERVATION` count forces exit 125; tested.
- **SHA256SUMS:** written sorted and excluding itself; tested.

**Vendor-ref fix (commit `b1f5d187d`):** replaced `git rev-parse HEAD` in a physical `vendor/upstream` directory with `git rev-parse --verify refs/heads/vendor/upstream^{commit}`, added a focused regression test asserting the exact argv. Verified the ref resolves to `631935dd1...` locally. This is a genuine defect fix, not scope expansion.

**Preserved Python/framing failures (attempt history) are honest:**
- `00-*` two launches fail with `error: tool 'python3' not found` (dev-shell lacked python3) — real, exit 1.
- `01-vendor-ref-*` reaches preflight then raises `FileNotFoundError ... vendor/upstream` from the pre-fix driver; only the copied plan is partial output — real defect, honestly preserved.
- `02-observation-framing-driver` manifest shows `pilot_fixture` expected 0, actual 125, `pilot_observation_count 0`, `result "failed"`, `failure "pilot_fixture exit 125 != expected 0"` — the Cargo-prefixed observation was correctly rejected, then fixed by `505cd8672`. Not converted to a pass.
- None of these are represented as source PASS results.

## 6. Authority conformance (no G2/G3 overreach)

- Reviewed-commit range `f6ca30c1a^..da7c155b9` touches only: `subscription_api.rs` (helper extraction + test-support wrapper + two focused tests), `recovery_pilot_tests.rs` (the one fixture) with `lib.rs` `#[cfg(test)]`-only `mod agent_tests`, the driver + its tests, the plan, and append-only docs/evidence. `fetch_subscription_me` production path is behavior-identical: it now calls the extracted `apply_subscription_me_body`, same parser/tier-cache/validation.
- `apply_subscription_me_fixture` is gated `#[cfg(any(test, feature = "test-support"))]`; `test-support` is enabled only as a dev-dependency feature by test targets (jcode-app-core dev-dep, provider runtimes, tui), never in default/production features. No non-test caller of the fixture exists (grep confirms).
- No new identity writer, no `provider_session_id` mutation, no daemon/listener/reload/`bind`/`TcpListener` added. The only `set_var` additions are the test's scoped env guards on `JCODE_HOME`/`JCODE_API_KEY`. All G2-excluded behaviors (live backend, real credentials, network, running daemon, reload, tools/MCP, memory, publication, install, cancellation, retry, compaction, disconnect/takeover, quality-baseline `--update`) remain excluded. `plan.json` contains no `--update` and no networked command.
- G4_RESULT and PILOT_PLAN explicitly restate the claim limit and that phase advancement stays blocked pending this G5 review.
- **Append-only integrity:** across the range, `git diff` of the four pre-existing recovery docs (PROGRESS, BASELINES, README, RESPONSIBILITIES) shows zero content-line deletions (pure additions). Prior evidence sets, reviews, and ledgers are unchanged. History was not rewritten.

## 7. Commit separation and preserved user state

- Implementation and result documentation are cleanly separated: source/tooling in `f6ca30c1a`, `f796ace46`, `86d3e3214`, `b1f5d187d`, `505cd8672`; evidence/status only in `da7c155b9` (docs-only, 37 files, 0 deletions). No commit mixes fixture implementation with its own result evidence.
- Working tree contains only the unstaged `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`; `git diff <that file> | shasum -a 256` reproduces `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00` exactly.
- Exactly four stashes present (`fix-config-hotpath-spam` parts 3/2/1 and `wip before upstream sync`), not popped.

## Blocking findings

None.

## Nonblocking limitations / observations

1. The `PILOT_OBSERVATION` JSON is printed with literal values rather than variables. It is fully behavior-backed by preceding assertions (a false value fails the test before the print is trusted), but the printed line itself is not a live serialization of the runtime state. Honest to note; not a gap.
2. Nix printed saved Cachix substituter/trusted-key notices even under offline + `substitute = false`. G4_RESULT preserves these in the transcript and correctly claims no fetch/credential/network-backed provider occurred. Consistent with the offline design; the notices are config echoes, not network activity.
3. The evidence proves only the exact bounded question. It does not (and does not claim to) authorize any broader behavior.

## What I did NOT check

- I did not run any Cargo or Nix build or execute the Rust fixture; correctness of the fixture is assessed from source plus the preserved manifest-verified log, not from a fresh compilation. Pass counts (e.g., classifier 17/17) are taken from preserved logs.
- I did not execute the recovery validation driver against the live plan (that would perform a pilot run); I ran only its offline unit tests and `--check-plan`.
- I did not exercise any live daemon, network, provider, tool, telemetry endpoint, or credential.
- I did not independently re-derive the G2/G3 authority beyond reading the cited G2 artifact and confirming its hash; I did not read other G-tier reviewer artifacts (none supplied here beyond G2).
- I did not line-audit every unrelated consumer of tier gating, the desktop crate, telemetry-worker internals, or the full replay/crash-recovery surfaces (all outside the bounded pilot).
- I did not independently verify the older combined-prerequisite/R09/G0 evidence manifests beyond confirming their hashes are cited unchanged; G2 already recomputed those.
