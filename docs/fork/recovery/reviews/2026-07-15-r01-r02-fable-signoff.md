# Fable sign-off: R01 and R02 full seam ledgers

Reviewer: Fable (independent verify agent)
Date: 2026-07-15T09:19Z
Mode: read-only. No repository, ref, stash, or worktree was edited. No network, credentials, live daemon, or destructive action. Test artifacts were written only to `/tmp/fable-r0{1,2}-target`.

Reviewed commits:

- R01: `b25e902eed532156c54ca14f7da6b8e780bde234` in `/Users/jrudnik/labs/jcode-seam-r01` (HEAD of that worktree; adds exactly 3 files under `docs/fork/recovery/seams/R01-runtime-build-identity/`).
- R02: `3217dbcbf22ea6ef13525e7f3f1571b0a49132d6` in `/Users/jrudnik/labs/jcode-seam-r02` (HEAD of that worktree; adds exactly 3 files under `docs/fork/recovery/seams/R02-config-provider-routing/`).

Decisive checkpoints consumed: 10 of 10.

1. Commit identity and docs-only footprint of both commits (`git show --stat/--name-only`).
2. R01 fixed refs: fork/upstream/base all resolve; `merge-base` = `631935dd1...`; base is ancestor of both fork and upstream.
3. R02 fixed refs: identical triple resolves; `merge-base` = `631935dd1...`.
4. R01 preservation: committed `opus-review.md`/`grok-review.md` blobs hash to `21918fd7...` / `9cb7beb3...`, equal to ledger-recorded values and to the `/tmp/jcode-r01-*.md` artifacts. Incident note hashes to `80012e2c...` as recorded.
5. R02 preservation: committed review blobs hash to `122b647b...` / `8a6c18e5...` and are `cmp`-byte-identical to `/tmp/jcode-r02-*.md`. The five trailing two-space lines are proven present in the committed Grok blob (lines 3-7), so they are verbatim source bytes, not normalization drift. Byte identity proven; acceptable.
6. R01 decisive static check: `git diff --unified=0 631935dd1 802f690982 -- crates/jcode-build-support/src/paths.rs` is empty (0 bytes); base..fork numstat is `333/1`; `nix_managed_launcher_override` is defined at fork `paths.rs:545` and called at 565, 603, 698 (the three selection points); upstream retains inherited `shared_server_update_candidate` (437) and `preferred_reload_candidate` (527). The ledger's narrowed "no post-base policy delta" claim is exact.
7. R01 decisive test: `bash scripts/dev_cargo.sh test -p jcode-build-support --lib` = 45 passed, 0 failed, including `dirty_source_state_uses_fingerprint_in_version_label` and pending-activation rollback coverage. Matches ledger.
8. R02 decisive test: `bash scripts/dev_cargo.sh test -p jcode-base --lib subscription` = 25 passed, 0 failed, including `effective_tier_defaults_to_plus_when_unknown` and the flagship-gating guards. Matches ledger's claim that existing tests pass while the stale-Flagship regression is absent.
9. R02 decisive code reproduction: fork `subscription_catalog.rs` parses only `plus`/`flagship` with `effective_tier()` defaulting to Plus; fork `subscription_api.rs` stores cache only under `if let Some(tier)` (successful unknown response leaves stale cache untouched); the `mystery -> None` test exists exactly as cited; upstream carries Pro/Max/Ultra plus changed constants; `catalog_routes.rs` diffs confirm the two-sided collision (fork adds `resolve_current_model_spec`/`primary_only`; upstream adds `standard_catalog_lists_model(...) != Some(false)` suppression). The blocking defect and the compose verdict are both real.
10. R01 projection-claim reproduction: `Subscribe.protocol_version`/`build_hash` in wire.rs; TUI sends `jcode_build_meta::GIT_HASH` (`backend.rs:339`); handshake compares compiled hashes; selfdev reload assigns `hash = source.version_label` (`reload.rs:284`); `client_session.rs:714-725` gates non-forced reload on strictly-newer; `server_events.rs` makes client-proven-older win over `Some(false)`. All citations accurate.

## R01: R01-runtime-build-identity, ledger at `b25e902e`

Verdict: **PASS**. Integration-ready as an adjudication document. Pilot correctly recorded as blocked.

- Responsibility boundary: PASS. Owns/excludes cleanly split R03A wire, R04 persistence, R10 publication, R08D platform reload out of scope, and the four-projection invariant assigns definition authority to R01 without claiming consumer implementations.
- Evidence support: PASS. Every cited line number, diff shape, and test result I sampled reproduced exactly (checkpoints 6, 7, 10). The correction narrowing "upstream lacks every function" to "no post-base policy delta" is itself evidence-accurate.
- Authority today / disposition: PASS. `fork` / `retain-fork` follows from the empty base..upstream paths.rs diff and the fork-only 333/1 incident-defense delta.
- Pilot entry/exit contract: PASS. Blocked on a named, concrete dirty-build projection test with temp-`JCODE_HOME`/temp-socket and no live daemon. Entry and exit conditions are testable, not aspirational.
- Cross-seam: PASS. R03A projection contract, R04 consumer-not-authority framing, R09/R10/R11/R08D all named with direction of dependency.
- R09 debt: PASS. No source change, no debt silently absorbed, `--update` prohibited, per-file attribution deferred to concrete diffs with file-level evidence required.
- Evidence preservation: PASS. Byte hashes of both reviews and the incident note verified against committed blobs and external artifacts.
- Slice separation and stop/rollback: PASS. Four slices each carry acceptance plus stop conditions; slice 3 refactor explicitly unauthorized until the contract test exists.
- Overclaim check: PASS. Negative findings explicitly state four-way runtime agreement was never exercised, mtime is ordering not identity, and no live daemon ran. Nothing claims unrun behavior.
- IMPORTANT (minor, non-blocking): the ledger records "Authored head was `f5a8999d81311d237d1c106a9d980fd86fa34b6`", a 39-character (truncated) hash. It resolves unambiguously today to `f5a8999d...b6e` (R02 spells the full 40), but a fixed-ref document should carry the full hash. Fix on next append; does not affect any conclusion because the adjudication baseline is the full fork/upstream/base triple, which is correct.

Confidence: high.

## R02: R02-config-provider-routing, ledger at `3217dbcb`

Verdict: **PASS**. Integration-ready as an adjudication document. Pilot correctly recorded as blocked.

- Responsibility boundary: PASS. Owns config provenance, credential references (never values), route identity, tier admission, `/v1/me` freshness; excludes R12 execution/persistence, R03A verdicts, R06 durable evidence, R13 compaction. Server-authoritative usage admission is stated, not invented locally.
- Evidence support: PASS. All decisive checkpoints reproduced (checkpoint 9): the two-tier fork parser, the `if let Some(tier)` stale-cache defect, the `mystery` test that documents the gap, the upstream five-tier candidate, and the genuinely two-sided `catalog_routes` collision. Confidence grading (H/M with medium-low on usage admission) matches evidence strength honestly.
- Authority today / disposition: PASS. `split` / `compose` is forced by the two-sided catalog_routes diff, and refusing upstream pricing/floor constants without a product-owned fixture is the correct authority stance.
- Pilot entry/exit contract: PASS. Six numbered gates, all fixture-testable, with the mandatory stale-Flagship regression correctly labeled "currently fails by absence". The fixture contract table names concrete observables and forbids secrets, network, and the user daemon.
- Cross-seam: PASS. R12 exact route-identity handoff, R13 invalidation-writer proof, R06A fixture round-trip before Phase 3, R03A only after both authorities approve. Stashes are evidence only, never replay targets.
- R09 debt: PASS. Per-file panic/swallowed-error/size entries enumerated and owned by R02 by path, `--update` prohibited, docs-only commit changes none of them. This is stronger than R01's deferred attribution and is the model form.
- Evidence preservation: PASS. Byte identity of both reviews proven (`cmp` against external artifacts, SHA-256 match on committed blobs). The five trailing two-space hard breaks are proven verbatim source bytes of the preserved Grok review, satisfying the stated acceptance condition.
- Slice separation and stop/rollback: PASS. No implementation authorized; each of the four slices has acceptance and stop/rollback bounds, and slice 2 explicitly cannot proceed without recorded product truth.
- Overclaim check: PASS. The ledger explicitly says the passing tests validate existing behavior and not the absent regression, that no live `/v1/me`, credential, or router call was made, and that the claim about `git diff --cached --check` cleanliness at authoring time is the only statement I could not re-run (it is superseded by the byte-identity proof, so nothing rests on it).
- Note (trivial): `Last updated` is date-only (`2026-07-15 UTC`) versus R01's full timestamp. Cosmetic inconsistency only.

Confidence: high for provenance/route/stale-tier findings, medium for the catalog product-truth framing (matching the ledger's own grading, which is appropriate).

## Combined verdict

Both ledgers are semantically complete, operationally reversible, internally honest about unrun behavior, and byte-faithful to their preserved reviews. Fixed refs, the decisive static checks, and one decisive test suite per seam all reproduce.

- R01: **PASS**, integration-ready, pilot blocked pending the dirty-build projection test. One minor IMPORTANT: truncated 39-char authored-head hash to correct on next append.
- R02: **PASS**, integration-ready, pilot blocked pending stale-tier fail-closed fix plus product-owned catalog fixture. Trailing-space hard breaks accepted on proven byte identity.
- No CRITICAL findings. Fable sign-off: **granted for both seams** as adjudication documents; neither seam is authorized for implementation or pilot by this sign-off, consistent with their own blocked verdicts.
