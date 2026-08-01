# F27 independent hardening review

**Verdict: GAP-FOUND**

Reviewed current `main` at `cdf6c869007a9b2e5bfc84c872f8c3db5de6e53d` against `ACCEPTANCE_STANDARD.md` A6/A7, the F27 contract, every dependency contract, every dependency evidence summary, and the current implementation. The acceptance gate is not met because a critical distribution-policy coverage gap remains. Two additional dependency gates are also not fully satisfied.

## Per-node verification

| Node | Claim | Verified? | Evidence checked |
| --- | --- | --- | --- |
| F22 | Every ignored advisory has complete machine-readable ownership and deterministic expiry enforced in CI and preflight. | **YES** | `scripts/check_advisory_policy.py` parses both `.cargo/audit.toml` and `scripts/security_preflight.sh`, rejects drift/expiry/future acceptance/blank fields/blanket thresholds; `docs/security/advisories.toml`; `.github/workflows/security.yml`; `scripts/preflight.sh`; `tests/test_advisory_policy.py`; evidence README and non-vacuity log. Current-date gate passed and all 35 fixtures passed under Python 3.13. |
| F23 | Critical-path panic/swallow/oversize debt cannot grow, repository debt cannot increase, and downward targets are explicit. | **YES** | `scripts/check_critical_path_budget.py` has pinned domains, ceilings, explicit targets, scope digest, file-count anti-movement checks, and repository high-water marks; `.github/workflows/fork-ci.yml` wiring; F23 README/trend report/non-vacuity evidence. Panic, swallowed-error, and critical-path checks all exited 0. |
| F24 | The declared Nix output is reproducible and companion artifacts expose verifiable source/version provenance and a CycloneDX SBOM. | **YES, with rerun limitation** | `flake.nix:171-218,383-471`; `nix/provenance.nix`; `nix/sbom.nix`; `nix/verify-provenance-sbom.py`; `docs/NIX.md`; F24 README exact-SHA rebuild and schema-validation evidence. Independently executed the current embedded SBOM generator body and verifier: 947 components, 947 unique `bom-ref`s, exit 0. Nix was unavailable locally, so the x86_64-linux clean-build equality measurement was inspected in durable evidence rather than repeated. No critical provenance/SBOM gap found in current source. |
| F25 | Live listener sidecars are preserved, stale sidecars/registry metadata are cleaned only after ownership proof, malformed state warns/quarantines, and terminal logs obey bounded retention without invalidating durable cursors. | **NO: GAP F25-1** | `server/socket.rs`, sidecar matrix tests, `server/swarm_persistence.rs`, hygiene fixtures, `jcode-swarm-core/control_log.rs`, cache-reset logic, F25 README/mutation evidence. Sidecar proof, exact-debug-path cleanup, collision-safe quarantine, complete-line diagnostics, torn-line handling, pending-await retention, and cache reset are present. However, old control logs can be deleted before a valid orphan `.bak` snapshot is loaded; see gap below. |
| F26 | Dead active-PID markers disappear at startup/periodically, telemetry counts live PID markers, and the duplicate telemetry implementation is removed. | **YES** | `jcode-base/src/session.rs:43-66`; indirect periodic call from `server/swarm.rs:269-336`; `jcode-storage/src/active_pids.rs` liveness/sweep and missing-session fixture; `jcode-telemetry-core/src/state_support.rs` `pid=` format plus `is_running`; duplicate file absent; F26 evidence/equivalence record. Full workspace build was not repeated. |
| F28 | Render/cache tests are lock-disciplined, video-export global mode cannot leak, and serialized CI thread caps are removed. | **YES** | `scripts/check_tui_render_lock.py`; Quality Guardrails wiring; no `--test-threads=1` in `fork-ci.yml`; `crates/jcode-tui/src/video_export.rs:123-199` RAII guard; F28 scope/result/stress evidence. Current static scan: 45 locked, 0 unlocked, exit 0. Three full parallel rounds were not repeated. |
| F29 | Direct ambient-root access is eliminated or reasoned on a shrink-only allowlist, and Class A paths honor storage isolation. | **YES** | `scripts/check_ambient_roots.sh` and allowlist; current helper use in memory log, OpenRouter, mobile server, doctor, surface workspaces, and changelog state; F29 result/gate evidence. Current gate: 21 sites, all allowlisted with stated reasons, exit 0. Full workspace suite was not repeated. |
| R05 | Dual attach is surfaced, recovery duplicates collapse, stall-guard cancellation is labeled truthfully, and reconnect working-dir changes are surfaced. | **NO: GAP R05-1** | Dual-attach warning source/tests, recovery-only dedup tests, working-dir notification tests, R05 incident/implementation evidence, current wire and cancellation label paths. Three subclaims are present, but stall-guard cause is still only a client log tag and never reaches the server; the server can still emit a false reload label. The evidence itself records this subclaim as blocked. |
| R06 | `setsid()` failure aborts spawn; process-group signaling preserves descendants; ESRCH-only individual fallback is used for TERM/KILL; other errors surface. | **YES** | `server/socket.rs:311-358`; `jcode-base/src/platform.rs:315-383`; both CLI signal stages; real-process descendant, SIGTERM/SIGKILL fallback, and EPERM fixtures; R06 evidence. Focused cargo fixtures were not repeated. |
| F30 | Retired distribution/native-iOS surfaces are absent and the policy gate is non-vacuous across active docs/workflows; every discovered gap is injected as a separately owned fix node. | **NO: GAP F30-1** | Current policy test, flake/preflight wiring, F30 README/logs, current stale residue. The clean nine-test policy suite exits 0, but the gate still scans only eight opt-in docs and lacks the documented AUR/curl coverage. An isolated `.apm/instructions/retired-channel.md` plant containing `yay -S jcode-git` also exited 0. The four F30-discovered gaps were proposed but not injected into `WORK_GRAPH.json`; `scripts/lib/configure_path.sh` and the active `uninstall.sh` reference remain. |

## Gaps

### F30-1: active distribution documentation can reintroduce retired channels while the policy gate stays green

**Severity: CRITICAL (policy/provenance of supported distribution surface)**

- `tests/test_nix_distribution_policy.py:27-54` defines an eight-file opt-in `ACTIVE_DISTRIBUTION_DOCS` list and a forbidden-token list that does not include the documented AUR/curl-pipe cases.
- `tests/test_nix_distribution_policy.py:195-202` scans only that opt-in list.
- `docs/fork/ideal-base/evidence/F30/README.md:80-122` already records this escape, the missing token coverage, workflow lint drift, and stale installer residue, then proposes `F30-FIX-1..4` without applying them.
- Current `WORK_GRAPH.json` has no `F30-FIX-*` node even though F30's acceptance gate requires every discovered gap to become an injected, separately owned fix node.
- Current stale residue remains at `scripts/lib/configure_path.sh` and `crates/jcode-build-support/src/paths.rs:1076-1080`.

Independent non-vacuity check in an isolated temporary tree:

1. Copied the current policy test and its eight listed documents.
2. Added active `.apm/instructions/retired-channel.md` containing `Install Jcode with: yay -S jcode-git`.
3. Ran only `test_active_distribution_docs_do_not_advertise_retired_channels`.
4. **Observed exit 0.** The policy gate passed despite the active retired-channel claim.

Why it matters: CI/preflight can report the Nix-only distribution policy green while active agent instructions or other unlisted supported documentation advertise a retired distribution channel. This is exactly the policy gap F30 found and F27 is required not to leave open.

### F25-1: retention can delete a live control-log tail before recovering a valid orphan backup

**Severity: HIGH (durable-state loss / cleanup ordering)**

- `crates/jcode-app-core/src/server/swarm_persistence.rs:385-401` preserves a control log only when the primary `.json` snapshot exists, then deletes an old log otherwise.
- `swarm_persistence.rs:647` runs that pruning before scanning snapshots/backups.
- `swarm_persistence.rs:655-664` explicitly treats an orphan `.bak` with no primary `.json` as loadable state.
- `swarm_persistence.rs:804-807` says the log is the replay tail beyond the snapshot checkpoint.

A crash shape with `swarm.bak` present, `swarm.json` absent, and an old `swarm.control.jsonl` is therefore misclassified as an orphan log. The log is deleted first, then the valid backup is loaded. Any events beyond the backup's covered offset are lost and cannot be replayed. The existing fixtures cover corrupt orphan backups and old orphan logs separately, but not a valid orphan backup plus retained tail.

Required fix direction: make retention backup-aware, or load/classify recoverable snapshots before pruning, and add a fixture proving a valid orphan backup preserves and replays its old control-log tail.

### R05-1: stall-guard cause remains log-only, so a cancellation can still be labeled as server reload

**Severity: HIGH (hardening gate unmet; false recovery provenance)**

- `crates/jcode-tui/src/tui/backend.rs:829-836` builds `Request::Cancel { id }`; the `reason` is supplied only to the logging helper.
- `backend.rs:426-446` serializes the reason-less request and separately formats interrupt log fields.
- `crates/jcode-protocol/src/wire.rs:141-143` still defines `Cancel { id: u64 }` with no cause.
- `crates/jcode-app-core/src/agent/turn_streaming_mpsc.rs:1608-1621` still selects the server-reload tool result from the shared graceful-shutdown signal.
- `docs/fork/ideal-base/evidence/R05/implementation.md:78-119` explicitly states this acceptance sub-issue was not fixed and was escalated.

Why it matters: the accepted R05 node still fails its explicit gate, "A stall-guard cancel is labeled as such, never as a server reload." Mislabeling changes recovery interpretation and obscures the causal chain during the exact multi-client incident class R05 was created to harden.

A smaller contract mismatch also remains: the written gate says all identical queued user messages deliver once, while the implementation intentionally deduplicates recovery-provenance copies only and preserves genuinely repeated user input. That implementation choice is sensible, but the gate text should be narrowed if this is the intended contract.

## What I did not check

- No full Cargo build, full workspace suite, or full `jcode-tui`/`jcode-app-core` parallel stress rerun.
- No Nix installation was available, so I did not repeat x86_64-linux clean realizations, NAR equality, flake checks, or online CycloneDX schema validation. I inspected F24's exact-SHA evidence and ran its current generator/verifier structurally without Nix.
- No live GitHub Actions, live branch-protection query, Cachix publication, or other network-dependent gate. `fork-health.sh` was run against the canonical offline governance fixture.
- No dynamic Rust reproduction of F25-1. The gap is established from the cleanup/load ordering and explicit orphan-backup branch in current source; a focused fixture should be added with the fix.
- No rerun of R06's real-process signal fixtures or F26's full-build duplicate-removal proof.
- No audit of external advisory-database completeness beyond the repository's two governed suppression surfaces.

## Confidence

**High.** The critical F30 policy escape was independently reproduced in an isolated tree. R05's unmet gate is admitted by its own evidence and confirmed in current wire/server code. F25-1 is a direct source-ordering/data-retention defect with a concrete crash-state witness, though it was not dynamically exercised in Rust during this no-build pass.
