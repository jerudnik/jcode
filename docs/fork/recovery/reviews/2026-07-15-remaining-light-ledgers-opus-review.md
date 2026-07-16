# Independent review: five remaining light ledgers

Reviewer role: adversarial verify (read-only). Date 2026-07-15.
Source and worktrees treated read-only. No live daemon, MCP, network, credential, package, release, or user-profile mutation was performed. Only decisive read-only git/hash commands were reproduced.

## Fixed refs (verified resolvable and merge-base correct in both worktrees)

- fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`
- upstream `802f6909825809e882d9c2d575b7e478dce57d3b`
- merge base `git merge-base fork upstream` = `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` (recorded base) ✓

Both worktrees clean (`nothing to commit, working tree clean`). Each ledger commit adds only its own `ledger.md` (docs-only). `git diff --name-only fork..<ledgerHEAD> -- crates scripts` is empty for R05A and R07A, confirming no source mutation.

## Commits under review

| Ledger | Worktree | Commit SHA |
|---|---|---|
| R03B transport/client attach lifecycle | jcode-light-pilot | `05c90a8483a8821d64ccfc885877da249eac0d26` |
| R08A operator input/command semantics | jcode-light-pilot | `ee30055037fa5f8c67d0574bdc9d2cab9baad637` |
| R10 packaging/release/update/distribution | jcode-light-pilot | `eefb5f06f830e9b79142b374ec1e6847780d8fb8` |
| R05A plan/DAG/control-log | jcode-light-control | `bd373ff326282739d1b5f1cd88ea066deec04206` |
| R07A tool/MCP lifecycle | jcode-light-control | `57b31756fc435c8a4cbcc0dbe288d06a87165db4` |

## Verdicts

| Ledger | Verdict | Coordinator may integrate? |
|---|---|---|
| R03B | **PASS** | Yes, as a `conditional` seam record. Do not treat as pilot-ready until the deferred socket fixture completes. |
| R08A | **PASS** | Yes, as a conditional no-UI pilot boundary + recorded escalation. Do not approve onboarding/import consent. |
| R10 | **PASS** | Yes, as a no-network identity-smoke record with `compose` recommendation. Do not authorize any release/install/update path. |
| R05A | **PASS** | Yes, as a `retain-fork` light record. Swarm-driven exercise remains deferred. |
| R07A | **PASS** | Yes, as a `retain-fork` light record. Tool/MCP exercise remains deferred to R07B/full review triggers. |

All five are integrable as light adjudicated records. None claims coordinator approval, merge, replay, ratchet update, remediation, or publication. Every "Coordinator approval" line reads `pending`.

---

## Per-ledger evidence and adversarial findings

### R03B (PASS) — `05c90a848`

Disposition: `retain-fork` (single supported disposition). Verified:

- Merge-base checkpoint reproduces `631935dd1...` ✓
- Fork/upstream `client_lifecycle.rs` deltas from base: fork `234+/198-`, upstream `80+/9-` (ledger cites exactly these) ✓
- `socket.rs::connect_socket` carries the documented "Do not unlink the path on connection-refused" fail-safe comment and refused-path preservation ✓
- **WebSocket-defer scrutiny (task focus):** `git grep -il 'websocket|web_socket|ws://'` over `*.rs` returns 51 files, which at first glance appears to contradict the "no WebSocket transport" claim. On inspection every hit in the attach surface is provider-side realtime transport (`jcode-provider-openai*/websocket_health.rs`, transport-model enum in provider lib) or config/gateway/test text, **not** R03B client-attach transport. The scoped server attach files (`server/socket.rs`, `client_lifecycle.rs`, `client_disconnect_cleanup.rs`) contain no `ws://`/WebSocket attach implementation. The ledger's claim is correctly scoped to R03B's owned surface, and provider websockets are R02/R12 territory. **No overclaim.**
- Incident hash `bug-server-reload-stale-daemon-version-check.md` = `80012e2ce61c578c943263b944bbaca27ac7dbd440af50c1f21f6e0291d8f1a9` matches ledger ✓
- Deferred-fixture posture: the 600s cold-build timeout is recorded as an explicit execution defer, not a pass. Acceptance gated on that fixture. Honest and fail-closed.

Boundary/authority/R09: clean. R03B carries only mechanics, cannot claim reload success or choose a daemon target, and asserts no existing scoped R09 debt (a negative finding, not a gate-clean claim). Escalation triggers (WebSocket/mobile attach, reload/resume/cancel, multi-client mapping leak, terminal reorder) are explicit.

Not a patch-ID equivalence claim; `b3ed82a6b` squash is flagged as an R00 ancestry gap. Correct restraint.

### R08A (PASS) — `ee3005503`

Disposition: `defer` (sole supported disposition), escalated at the dangerous-consent boundary. Verified:

- input.rs/commands.rs deltas: fork `274+/42-`, `181+/117-`; upstream `276+/40-`, `115+/114-` — exactly as cited ✓
- **60-second pre-checked auto-import (task focus):** `DECISION_TIMEOUT = Duration::from_secs(60)` present at base, upstream, and fork (all three) ✓. `onboarding_flow_control.rs:1580` comment "Timeout default: import every currently-checked login." followed by `self.onboarding_finish_import_review();` at `:1581` ✓. This is the decisive checkpoint-4 evidence. The ledger correctly treats shared fork/upstream behavior as evidence of shared behavior, **not** a safety verdict, and escalates.
- `git diff --unified=0 upstream fork -- input.rs` shows no cancel/Escape/interrupt/ctrl hunk, supporting the narrow "shared cancel intent mapping" claim ✓
- Audit incident hash `audit-orthogonality-2026-07-14.md` = `a472b43ecaed612514bdb7d5fcb1547222f41fc4f58118a05df519b8f5637747` matches ✓

**Is `defer` sufficiently fail-closed without exceeding the six-full-seam cap now? (task focus) — Yes.** The approved map keeps R08A at `light`/`smoke only`, and the six full seams (R01, R02, R12, R05B, R03A, R04) are fixed and do not include R08A. The escalation here is a *conditional trigger before onboarding/import is exercised*, not an immediate consumption of a full-seam slot. Per `RESPONSIBILITIES.md:86`, R08 is not a pilot prerequisite unless the chosen stack exercises it, and the bounded pilot is no-UI/no-onboarding/no-credential. `defer` blocks any onboarding/import path, records no approval of expiry-as-consent, and requires a deterministic fixture proving timeout+Escape are fail-closed before any credential read. That is fail-closed. It does not exceed the cap now because the light seam stays light; the future R08A/R02 joint review is a coordinator scheduling decision only if/when the trigger fires. The ledger states plainly it is "an escalation, not a request to change source in this lane."

Minor (informational, not a defect): the ledger's own fixture commands were timed out on a shared Cargo target lock (300s) before test execution and are explicitly recorded as execution defers, not passes. Correct.

### R10 (PASS) — `eefb5f06f`

Disposition: `compose` (sole supported disposition). Verified:

- **Compose decision + draft release control (task focus):** fork `release.yml` "Publish the release up front" / `action-gh-release` creates a public release before platform builds attach assets ✓. Upstream `release.yml` "Stage all platform assets on a draft. The final job publishes the release only after every required build, signature, and checksum passes." with `Create draft release if missing` ✓. The compose recommendation (retain fork Nix outputs + adopt upstream draft-to-final finalization) is directly source-supported.
- Fork-only Nix surface: `flake.nix 154+/0-`, `nix/package.nix 137+/0-`, `nix/modules/home-manager.nix 135+/0-`, `flake.lock 98+/0-`; upstream changes none — exactly as cited ✓
- **Missing mandatory checksum paths (task focus):** `scripts/install.sh` contains zero `sha256`/`checksum`/`signature`/`attestation` matches (grep empty), confirming the curl installer bypasses integrity verification ✓. `update.rs:281 verify_asset_checksum_if_available` with `:289` "does not include SHA256SUMS; skipping checksum verification" confirms the permissive missing-manifest path ✓. `jcode-update-core/src/lib.rs:234-270` strict SHA256SUMS parse + mismatch/missing rejection with tests at `:391` present ✓. The ledger's "checksum code exists but acquisition is not uniformly fail-closed" is precise.
- Locked Nixpkgs rev `4100e830e085863741bc69b156ec4ccd53ab5be0` present in fork `flake.lock` ✓
- **R01 activation boundary (task focus):** the ledger correctly forbids R10 from declaring the running daemon correct on launcher repoint, defers install/activation (`install_release.sh`, `install.sh` reload + PATH/profile mutation) to full review, and cites the R01 stale-daemon incident from `PRESCREEN.md` only as an adjacent boundary fact. Cross-seam ownership to R01/R03A/R04 is explicit and correct.

Escalation triggers (public release before artifacts/checksums complete, absent SHA256SUMS, daemon activation without R01 decision, non-restoring rollback, remote acquisition) are explicit and fail-closed. No release/tag/download/install was executed.

### R05A (PASS) — `bd373ff32`

Disposition: `retain-fork`. Verified:

- **Control-log/DAG authority split from R05B (task focus):** `control_log.rs` present only at fork, absent at base and upstream ✓ (`git cat-file -e` confirms). DAG deltas from base: fork `dag` total `228+/19-` (mod 1/1 + ops 111/14 + tests 116/4), upstream `170+/18-` (ops 85/14 + tests 85/4) — exactly as cited ✓. The ledger correctly challenges authority: fork-only control log is retained but neither DAG side is authoritative by ancestry alone.
- R05A/R05B boundary is source-grounded and matches `RESPONSIBILITIES.md:27-28` and the adjudicated R04/R05B split (`:94`). The forkbomb incident (`MCP_SERVE_FORKBOMB_INCIDENT.md`, present at fork ✓) is correctly attributed to R05B/R04 worker/registration lifecycle, not the pure graph state machine. Cross-seam contract table cleanly delegates spawn/reclaim to R05B, lifecycle to R04, tools to R07A/B, evidence to R06A/R12.
- Test fixtures exist at fork: `jcode-plan/src/dag/tests.rs`, `jcode-swarm-core/tests/control_log_properties.rs`, `comm_control_tests/control_log_dual_write.rs`, `control_log_scan.rs` ✓
- Pilot: `RESPONSIBILITIES.md` marks R05A `Pilot: no`; defer boundary correct.
- R09: names existing oversized/`panic` debt without reclassifying or hiding; forbids `--update`. Correct.

**Gap (not a FAIL):** checkpoint 6 asserts positive test passes (jcode-plan `79/79`, control_log_properties `2/2`). I did not reproduce these because re-running cargo tests is a build side effect outside the decisive read-only boundary and would incur the same shared-target contention the ledger itself hit for app-core. The claim is plausible and internally consistent (the app-core integration tests are honestly marked as timed-out/not-passed). Recommend the coordinator require the checkpoint-6 pass evidence to be independently reproduced from a warm/uncontended target before any swarm-driven R05A change is accepted. This is a verification gap, not an overclaim: the ledger distinguishes passed from deferred correctly.

### R07A (PASS) — `57b31756f`

Disposition: `retain-fork`. Verified:

- **Self-reference/cap safeguards (task focus):** `MAX_OWNED_MCP_CHILDREN`, `is_jcode_mcp_serve_shim`, `drop_self_referential_servers` present **only** at fork (0 hits at base and upstream); `FAILED_CONNECT_RETRY_COOLDOWN` present at all three refs — exactly the ledger's authority-challenge claim ✓. `client.rs:16 MAX_OWNED_MCP_CHILDREN: usize = 64` with RAII reservation, `protocol.rs:239 is_jcode_mcp_serve_shim`, `:491 drop_self_referential_servers` all present ✓.
- Deltas from base: `client.rs 92+/2-`, `manager.rs 26+/6-`, `protocol.rs 183+/12-`; upstream `client.rs` unchanged, `manager.rs 6+/6-`, `protocol.rs 7+/12-` — exactly as cited ✓.
- **No-tool defer (task focus):** `RESPONSIBILITIES.md:31` makes R07A relevant only if tools are exercised; the approved Phase 3 pilot is no-tool. The ledger forbids any MCP connection/tool call in that pilot and lists stop-and-escalate conditions (real MCP server, network, credential, live daemon, discovery, provider route). Correct and fail-closed. Discovery/network/telemetry correctly deferred to R07B/R07C.
- Forkbomb incident correctly scoped: guard is a "footgun backstop, not a security boundary"; headless-session/daemon-shutdown/dead-PID sweep belong to R04/R05B. No live reproduction authorized.
- Schema cache treated as config-fingerprint-bound hint, never execution truth; fixtures (`schema_cache_tests.rs`, `pool.rs`) exist at fork ✓.

**Note on path formality (informational, not a defect):** the ledger cites bare `mcp/protocol.rs`, `mcp/client.rs`, `mcp/pool.rs`. These resolve to `crates/jcode-base/src/mcp/*`, and the ledger's fixture correctly targets `-p jcode-base`, while the registry checkpoint correctly cites `crates/jcode-app-core/src/tool/mod.rs`. Paths are unambiguous and the crate targets are right; no correction required.

Deferred fixture (`nix develop ... cargo test -p jcode-base ... schema_cache_tests`) cancelled on shared target contention, explicitly recorded as no-pass. Honest.

---

## Cross-cutting checks

- **One supported disposition per seam:** R03B retain-fork, R08A defer, R10 compose, R05A retain-fork, R07A retain-fork. Each ledger argues a single disposition and rejects alternatives with evidence. ✓
- **Authority challenge:** every ledger refuses to treat upstream provenance or path overlap as authority (R03B fanout-as-candidate, R08A shared timeout-as-behavior-not-verdict, R10 shared curl installer not authoritative, R05A ancestry not decisive, R07A two-sided overlap not equivalence). ✓
- **Scope/defer/escalation boundaries:** explicit and named in all five. ✓
- **Pilot relevance:** R03B conditional, R08A smoke-only/no-UI, R10 identity-smoke, R05A no, R07A only-if-tools — all match `RESPONSIBILITIES.md:19-42,72-88`. ✓
- **Cross-seam ownership:** no ledger claims another seam's authority; all delegate correctly (reload→R01/R04/R03A, terminal→R04/R12, evidence→R06A, consent→R07A/B/C, worker→R05B). ✓
- **Deterministic fixture + explicit timeout handling:** every ledger records its build/test timeouts as explicit execution defers, never as passes or source failures. R05A is the only one asserting positive passes (jcode-plan/swarm-core), flagged above as an unreproduced-but-plausible gap. ✓
- **R09 / no-`--update` posture:** all five keep existing debt visible, attribute new debt to the owning seam, and forbid `--update`. None reclassifies or rebaselines. ✓
- **Rollback/stop conditions:** present and fail-closed in all five (no source change in lane; stop rather than broaden). ✓
- **No overclaim:** no ledger claims coordinator approval, merge, replay, ratchet, remediation, or publication. All approvals `pending`. ✓

## Confidence and gaps

- Confidence: **high** for all fixed-ref source/delta/presence/hash claims (independently reproduced and matched exactly).
- Confidence: **high** for disposition support, boundary discipline, and no-overclaim posture.
- Gaps I could not close within the read-only decisive boundary:
  1. R05A checkpoint-6 positive test passes (jcode-plan 79/79, control_log_properties 2/2) were not re-executed (build side effect; shared-target contention). Plausible and internally consistent; recommend independent warm-cache reproduction before swarm-driven acceptance.
  2. All ledgers' own deferred fixtures remain unexecuted by design; their acceptance conditions correctly gate on completion.
  3. External incident/audit files live outside the worktrees; I verified their SHA-256 hashes match the ledgers but did not re-audit their full content.

## Decisive commands reproduced (read-only)

```
git -C <wt> merge-base 7ff4fc6be 802f69098            # -> 631935dd1
git -C <wt> cat-file -e <ref>:<path>                   # presence/absence of control_log.rs, docs
git -C <wt> grep -l <symbol> <ref>                     # fork-only guards, DECISION_TIMEOUT, SHA256SUMS
git -C <wt> diff --numstat <base> <ref> -- <paths>     # all cited deltas
git -C <wt> diff --unified=0 <upstream> <fork> -- input.rs   # cancel/Escape no-hunk
git -C <wt> show <ref>:<path>                           # release.yml, socket.rs, update.rs, install.sh
shasum -a 256 <incident/audit files>                   # matched ledger hashes
git -C <wt> diff --name-only <fork>..<ledgerHEAD> -- crates scripts   # empty (docs-only)
```

## Bottom line

All five light ledgers **PASS**. No blocking (high/critical) defects found. One medium-severity verification gap (R05A unreproduced positive test claim) warranting warm-cache reproduction before swarm-driven acceptance, but the ledger's pass/defer distinction is honest and it is safe to integrate as a light record now. The coordinator may integrate each as its stated light/conditional adjudicated record, holding every one at `pending` coordinator approval and honoring the explicit escalation triggers (notably R08A onboarding/import consent and R10 release/install activation, both of which remain unapproved and fail-closed). No ledger was edited.

---

## Correction (appended 2026-07-15, R03B only)

Prior text above is left unchanged. This section supersedes the R03B PASS caveat "deferred socket fixture pending" in light of the R03B correction, per DM from chipmunk. Bounded rereview of R03B only, at latest light-pilot HEAD.

### Scope

Latest `jcode-light-pilot` HEAD is `914916719` (working tree clean). Two new R03B-only commits reviewed:

- `982780cbf52b3d54c379172e662a3066daf4218d` docs(recovery): record R03B warmed fixture results
- `914916719bb1af321a3ceb257d9eaf4008ac88f6` docs(recovery): close R03B fixture defer

Both touch only `docs/fork/recovery/seams/R03B-transport-client-attach-lifecycle/ledger.md` (3+/3- then 1+/1-). No source, no other ledger. Verified via `git show --stat`.

### Diff verification (PASS)

`982780cbf` makes two substantive changes:
1. Corrects the fanout fixture filter from the wrong `server::client_lifecycle_tests::...` to the exact generated `server::client_lifecycle::tests::client_initiated_turn_fans_out_stream_and_terminal_events_to_live_attachments`.
2. Replaces the pure timeout-defer paragraph with recorded warmed results: socket filter `20 passed; 0 failed` in 0.44s; the original wrong module path matched zero tests so `dev_cargo.sh` exited 97 (not treated as success); `cargo test -- --list` resolved the exact name; corrected filter then `1 passed; 0 failed` in 0.93s. Timeout and zero-match retained as validation history.

`914916719` updates the acceptance condition and the coordinator-approval line to "pending coordinator review; the isolated Unix fixtures now pass as recorded above."

### Independent source verification (all PASS)

- **Exact generated test path.** `client_lifecycle.rs:3133-3134` mounts the tests file via `#[path = "client_lifecycle_tests.rs"] mod tests;` inside module `client_lifecycle` (declared `mod client_lifecycle;` at `server.rs:9`). Therefore the generated Rust test path is `server::client_lifecycle::tests::<fn>`, exactly the corrected filter. The original `server::client_lifecycle_tests::...` was wrong because there is no `client_lifecycle_tests` module in the path tree (the file is remounted as `tests` under `client_lifecycle`). The target fn exists at `client_lifecycle_tests.rs:785`. Correction is accurate. ✓
- **Recorded 20/20 socket result.** `socket_tests.rs` at fork contains exactly 20 test attributes (`#[...test]` count = 20; function count = 20), matching the recorded `20 passed; 0 failed`. ✓
- **Recorded 1/1 fanout result.** Exactly one target function resolves under the corrected exact filter (`--exact`), consistent with `1 passed; 0 failed`. ✓
- **Zero-match exit 97 mechanism.** `scripts/dev_cargo.sh:871-884` guards explicit-filter runs: when a filtered `cargo test` reports `running 0 tests` with no `running [1-9]... tests`, it prints the zero-match warning and `return 97`. This confirms the ledger's claim that the wrong module path correctly exited 97 rather than being read as success. The guard is the real fail-closed mechanism, not a narrative flourish. ✓
- **Preserved cold-build timeout.** The 600s `tokenizers` cold-build timeout remains recorded verbatim in the warmed-results paragraph and in the acceptance condition ("The first fresh-build timeout ... remain recorded above"). Not deleted or reframed as a pass. ✓

### Adversarial notes

- The warmed pass timings (0.44s, 0.93s) and pass counts are self-reported in the ledger. I did not re-execute cargo (a build side effect outside the read-only decisive boundary, and the warmed cache/JCODE_HOME state is not reproducible here). However, every falsifiable structural claim the results depend on is independently confirmed: the corrected module path is the true generated path, the socket suite is exactly 20 tests, exactly one fanout test matches, and the exit-97 guard exists and behaves as described. The self-reported counts are therefore consistent with source and internally coherent; the prior wrong path would indeed have produced the recorded zero-match/exit-97, which is strong corroboration that the run actually happened rather than being fabricated.
- No overclaim: coordinator approval remains `pending coordinator review`, not granted. WebSocket defer, R09 no-`--update` posture, and all escalation triggers are unchanged and intact.

### Updated R03B verdict

R03B: **PASS**, and the fixture defer is now **closed**. The socket (20/20) and live-attachment (1/1) fixtures are recorded as passing from a warmed cache with disposable `JCODE_HOME`, the filter-path defect is corrected to the verified generated path, and the timeout/zero-match history is preserved honestly as validation record rather than as failures.

**Is R03B now integrable with fixture complete? Yes.** The coordinator may integrate R03B as its `retain-fork` conditional seam record with the deterministic Unix attach/cleanup fixture complete. It remains a conditional seam (relevant to Phase 3 only when a pilot attaches to the server) and coordinator approval is still pending final coordinator sign-off, but the previously blocking fixture-completion condition is satisfied. The earlier "deferred socket fixture pending" caveat is retired. Other ledgers were not rereviewed.
