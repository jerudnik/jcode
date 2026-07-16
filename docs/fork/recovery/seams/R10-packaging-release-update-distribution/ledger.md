# R10 Packaging, release, update, and distribution: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light`, with release-publication and installer-acquisition escalation triggers |
| Research budget | `6 decisive checkpoints, exhausted without expansion` |
| Recommended disposition | `compose` |
| Confidence | `high` for fixed-ref workflow and acquisition evidence; `medium` for unexecuted cross-platform package/update paths |

R10 owns Nix/package outputs, launchers, install and update channels, release metadata, and distribution artifacts. It excludes canonical live-daemon target/reload selection (R01), wire/build compatibility verdicts (R03A), session migration (R04), quality-gate policy (R09), and sync governance (R00). Source was read-only. No release, tag, publish, download, live daemon, credential, external network, update, installation, shell-profile mutation, or destructive action was performed.

## Six decisive checkpoints

| # | Finding | Evidence and deterministic reproduction | Consequence |
|---:|---|---|---|
| 1 | Fixed refs and R10's narrow pilot role are reproducible. | All three refs resolve to the table above and `git merge-base 7ff4fc6be 802f69098` is `631935dd1`. `RESPONSIBILITIES.md:39` limits R10 pilot relevance to an identity smoke, while the R10 mapper record (`reviews/2026-07-15-responsibility-mapper-luna.md:327-339`) limits the evidence budget to updater tests, package metadata comparison, and a non-mutating launcher/version check. | R10 cannot use a release or installer execution to establish pilot readiness. R00 fixes provenance and R01 remains the runtime identity authority. |
| 2 | Fork-only Nix packaging is a coherent downstream asset, not upstream behavior to overwrite. | From merge base, fork adds `flake.nix` `154+/0-`, `nix/package.nix` `137+/0-`, `nix/modules/home-manager.nix` `135+/0-`, and `flake.lock` `98+/0-`; upstream changes none of them. `flake.nix:39-105` exposes `packages.default`, `packages.jcode`, an overlay, and Home Manager module for three pinned systems. `nix/package.nix:49-123` uses `Cargo.lock`, fixed-output git hashes, `--locked --bin jcode`, and deterministic Nix build metadata. | Retain the fork package/output side of R10. It supports reproducible acquisition and identity inspection without claiming release authority. |
| 3 | Fork retains a pre-public release sequence that upstream replaced with a safer draft-and-finalize sequence. | Direct fork/upstream R10 diff is `594+/205-` across `.github/workflows/release.yml`, Cargo metadata, Nix, and release helpers. Fork `release.yml:36-43` creates a public release before platform builds attach assets. Upstream fixed ref `release.yml:36-39` stages assets on a **draft** and publishes only after required builds, signature, and checksum pass. Both refs generate and upload `SHA256SUMS` later, but fork/base can expose an early asset before that manifest is available. | Compose in upstream's draft-to-final, all-required-artifacts publication control. Do not adopt upstream by overwriting the fork's independent Nix package surface. |
| 4 | Checksum code exists, but release acquisition is not uniformly fail-closed. | `jcode-update-core/src/lib.rs:221-270` parses strict SHA256SUMS and rejects mismatch/missing asset entries; its tests at `:365-416` cover matching, mismatch, missing, and malformed digests. App update `update.rs:277-305` verifies only **if available** and logs then proceeds when a release lacks `SHA256SUMS`. `scripts/install.sh:55-130` fetches the latest GitHub tag and artifact, but contains no checksum, signature, or attestation verification. | In-app stable updates may benefit from the workflow manifest, but the curl installer bypasses it and the updater’s missing-manifest path is permissive. Treat remote acquisition as unapproved until the full review establishes a mandatory integrity contract. |
| 5 | Distribution activation crosses protected R01 ownership and mutates user state. | `scripts/install_release.sh:60-98` installs an immutable version path then moves `stable`, `current`, and launcher symlinks and invokes `server reload` unless skipped. `scripts/install.sh:156-203` similarly activates `stable`, removes macOS quarantine, invokes setup/reload, and `:221-287` appends launcher PATH settings to shell configuration files. The R01 incident summarized in `PRESCREEN.md:117-119` records a live daemon whose Nix, selfdev, stable, and mapped executables diverged. | Version-store retention is a useful rollback primitive, but update activation and daemon handoff must call into R01 policy and installer mutations need an explicit reversible/opt-in review. R10 cannot declare the running daemon correct merely because a launcher was repointed. |
| 6 | A bounded static identity fixture passes, but it is not a release/update proof and R09 debt remains binding. | `bash -n scripts/install.sh scripts/install_release.sh scripts/quick-release.sh` passed. `nix flake metadata --offline --json` resolved the locked Nixpkgs rev `4100e830e085863741bc69b156ec4ccd53ab5be0` and the expected flake description without evaluating or building packages. `R09-quality-gates/ledger.md:20-30` keeps four red ratchets visible and forbids `--update`; no R10-specific debt attribution was located. | The identity smoke can inspect a locked package surface, not acquire or run it. Any R10 source change must preserve R09 debt visibility and run trusted applicable gates without `--update`. |

## Authority and supported disposition

`compose` is the sole supported disposition. Keep the fork’s pinned, reusable Nix package and declarative Home Manager surface. Adopt upstream’s release finalization principle, namely do not make a release available until the required artifacts and their integrity controls have completed. Do not treat source agreement on the legacy curl installer as authority: base, upstream, and fork all fetch and activate without verifying a release manifest.

This ledger does not authorize a workflow edit, tag, release, download, installation, or updater invocation. The next source change, if approved after full review, belongs in a separate R10 implementation lane and must preserve R01’s canonical running-binary authority.

## Pilot relevance, fixture boundary, and cross-seam contract

- **Pilot relevance:** R10 provides identity smoke only. The selected pilot may statically inspect the pinned flake/package metadata and an already-local binary’s version/doctor identity, but must not run `scripts/install.sh`, `scripts/install_release.sh`, `quick-release.sh`, `jcode update`, `gh release`, or any remote release lookup/download.
- **Deterministic fixture:** the completed no-network syntax and locked-flake metadata check is the light fixture. Before remote acquisition is allowed, a hermetic fixture must supply an asset and SHA256SUMS, assert a mismatched/missing manifest leaves `stable`, `current`, launcher, and version markers unchanged, then assert a verified asset promotes exactly one version and delegates any daemon decision to R01. Existing focused unit-test entry point: `bash scripts/dev_cargo.sh test -p jcode-update-core --lib -- --test-threads=1`. It was not run here, to avoid a Cargo build/network side effect beyond the static pilot boundary.
- **R00:** release workflow, cache, and package changes live on the documented distribution rail. No fixed-ref provenance or sync claim is changed by this ledger.
- **R01/R03A/R04:** R10 can prepare a verified candidate and update launcher/channel metadata only. R01 selects/reloads the canonical executable; R03A decides compatibility; R04 owns handoff/session survival and terminal outcome.
- **R09:** no baseline or classifier changed. Any future packaging/change slice must run relevant trusted gates without `--update` and record any red debt against R10 rather than hiding it.
- **R11:** the R01 stale-daemon incident is cited only as an adjacent boundary fact from `PRESCREEN.md`; no external release or artifact claim is made.

## Negative findings and gaps

- No Nix build, `nix flake check`, package execution, platform matrix, release draft, artifact, release metadata, checksum asset, signature, Windows installer, update download/resume, daemon reload, or rollback was exercised.
- The static fixture proves shell parseability and offline lock readability, not package correctness, artifact integrity, launchability, or user-state restoration.
- No R10-specific current R09 attribution was located. This is a negative finding, not a claim that R10 is gate-clean.
- The automatic installer reload and profile mutation were inspected only statically. The ledger does not claim their behavior is safe or reversible.

## Acceptance, rollback, and escalation

- **Acceptance or retirement condition:** accept this light ledger only for the no-network identity smoke. Before a release/update/install path is exercised, a full R10 review must verify manifest-required acquisition, publication ordering, explicit user-state behavior, cross-platform artifact/launcher identity, and a rollback that preserves the previous channel and does not override R01’s newer-daemon protection.
- **Rollback or stop condition:** no source change in this lane. Stop rather than broaden if verification requires a tag, GitHub/Cachix action, remote artifact, credential, live daemon, user profile mutation, package build on another platform, or a quality-baseline update.
- **Escalate to full review if:** any release is made public before all required artifacts/checksums/signatures are complete; SHA256SUMS is absent or an installer/update proceeds without verifying it; a candidate activates/reloads a daemon without R01’s canonical-target decision; installer rollback cannot restore prior launcher/channel and user configuration; or the pilot needs remote acquisition rather than identity inspection.
- **Coordinator approval:** pending.
- **Fable review:** pending independent Phase 4 architecture review.

## 2026-07-15 W0 approval amendment

Coordinator approval: **PASS as a no-network `compose` light record**. The independent five-ledger review is [`../../reviews/2026-07-15-remaining-light-ledgers-opus-review.md`](../../reviews/2026-07-15-remaining-light-ledgers-opus-review.md), SHA-256 `b537bc5674fdb9385e60c2dd18a44db5e61ba4f57146cd57fbf91f7a58a8a55d`.

The stale Fable-pending line is discharged by corrected Phase 4 Fable plan SHA-256 `b0bae9803fa726a489e0560fdc423daefa20bd8478ede0aa2772f7684ea21eb9` and independent plan review SHA-256 `3f2d31cb5fb9ead893ed8b1e4ce451072757cc5d0206236833dac1b3a886fe92`. W6 begins with the mandated full R10 review. No workflow edit, tag, release, download, install, updater execution, profile mutation, daemon reload, or publication is authorized by this approval.

## 2026-07-16 W6 full-review repair amendment

W6 implemented the completed full R10 review in five ordered commits on branch `recovery/fix-w6-r10-acquisition-2026-07-16`:

1. `05f00b671` `docs(r10): preserve W6 full review` added [`../../reviews/2026-07-16-w6-r10-full-review.md`](../../reviews/2026-07-16-w6-r10-full-review.md) before any source change.
2. `c1bf53076` `fix(r10): fail closed on unchecked release assets` changed the updater and installers to require `SHA256SUMS` for remote release assets, reused `jcode-update-core` strict checksum verification in the Rust updater, and made installer daemon reload explicit opt-in via `JCODE_RELOAD_SERVER=1` with `JCODE_SKIP_SERVER_RELOAD=1` retained as hard-disable.
3. `a62516b6f` `test(r10): cover fail-closed release acquisition` added one hermetic local test file, `tests/test_r10_release_acquisition.py`, proving missing and mismatched checksums preserve existing `stable`, `current`, launcher, and version markers; a verified asset promotes exactly one version; and reload is default-off/explicit-only.
4. `9203aaf97` `ci(r10): publish releases after checksum finalization` composed upstream fixed-ref `802f6909825809e882d9c2d575b7e478dce57d3b` draft-to-final release ordering: draft creation, draft asset attachment, checksum upload, then final publication. Fork package/Nix/signing surfaces were not edited.
5. Final evidence/ledger commit records this amendment and [`../../evidence/2026-07-16-w6-r10/`](../../evidence/2026-07-16-w6-r10/).

Accepted validation evidence is under [`../../evidence/2026-07-16-w6-r10/`](../../evidence/2026-07-16-w6-r10/):

| Evidence | Result |
|---|---|
| `hermetic-r10-tests.log` | 5 hermetic tests passed under `CARGO_NET_OFFLINE=true`, `FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`, `JCODE_NO_TELEMETRY=1`, disposable `JCODE_HOME`, and disposable `JCODE_RUNTIME_DIR`. No real network/profile/daemon/install path was used. |
| `guarded-rust-checksum-tests.log` | `scripts/dev_cargo.sh test -p jcode-app-core verify_asset_checksum -- --nocapture` passed 3 checksum tests under the same guard class. |
| `syntax-checks.log` | `bash -n scripts/install.sh scripts/install_release.sh` passed. `pwsh` was not locally available, so PowerShell parser/static validation remains an external review need. |
| `workflow-checks.log` | `actionlint` was not locally available; invariant fallback passed for draft release creation, checksum-before-publication ordering, and removal of `softprops/action-gh-release`. |
| `r09-summary.txt` and `r09/*.log` | Full unchanged R09 expected-exit matrix ran without `--update`: classifier, wildcard, and warning gates exited 0; panic, swallowed-error, production-size, and test-size ratchets exited 1 as expected. |
| `invalid-unguarded-rust-attempt.md` | Preserves invalid background task `702684spm1` append-only. It timed out and lacked mandated guards, so it is not accepted as passing evidence. |

Expected red attribution: the four R09 red ratchets are pre-existing visible debt, not hidden by this work and not updated. No `--update` command was run. W6 did not perform a real tag, GitHub release, publication, credential use, live updater/installer run against real assets, profile mutation outside temp fixtures, or daemon reload.

Remaining review needs: independent Grok/coordinator review of these five commits and evidence; PowerShell parser/runtime validation on a host with `pwsh`; optional `actionlint` validation on a host with that binary; and explicit authorization before any real release publication or live acquisition path is exercised.

## 2026-07-16 W6 quick-release correction amendment

Independent Grok review at HEAD `4b2e8ee62` returned **FAIL** with one IMPORTANT finding: `scripts/quick-release.sh` still created a public release and could race CI before `SHA256SUMS` was available. W6 preserved that FAIL in [`../../reviews/2026-07-16-w6-r10-full-review.md`](../../reviews/2026-07-16-w6-r10-full-review.md) and authorized exactly one path expansion: `scripts/quick-release.sh`.

Correction commits after the original five:

6. `30699fe05` `docs(r10): preserve quick-release Grok failure` records the FAIL and path expansion declaration.
7. `09e23e998` `sync(r10): stage quick releases as drafts` composes upstream fixed ref `802f6909825809e882d9c2d575b7e478dce57d3b` quick-release staging: `gh release create --draft`, upload Linux/macOS assets to the draft, and leave publication to CI after checksums and platform gates.
8. `9e981514b` `test(r10): assert draft-only release entrypoints` adds deterministic static coverage that both `.github/workflows/release.yml` and `scripts/quick-release.sh` are draft-only before checksum publication.
9. Final evidence/ledger correction commit records this amendment and updated evidence under [`../../evidence/2026-07-16-w6-r10/`](../../evidence/2026-07-16-w6-r10/).

Accepted local/static validation for the correction:

| Evidence | Result |
|---|---|
| `quick-release-static-tests.log` | Guarded Python fixture/static suite passed 6 tests. |
| `quick-release-syntax.log` | `bash -n scripts/quick-release.sh scripts/install.sh scripts/install_release.sh` passed. |
| `quick-release-invariants.log` | Static invariant fallback passed for workflow checksum-before-publication and quick-release draft staging. |
| `quick-release-diff-check.log` | `git diff --check` passed at capture time. |

No network, Nix shell, `gh`, tag, release, credential, live updater/installer, profile mutation, or daemon action was performed. The earlier unguarded Rust attempt remains preserved as invalid evidence and was not counted as a pass. Remaining need: Grok/coordinator re-review of HEAD after this correction.

## 2026-07-16 W6 final review artifact amendment

Final independent review artifacts were preserved after HEAD `c07654e25` without source/test changes or validation reruns:

| Artifact | SHA-256 | Meaning |
|---|---|---|
| [`../../reviews/2026-07-16-w6-r10-grok-fail-original.md`](../../reviews/2026-07-16-w6-r10-grok-fail-original.md) | `d5560863860cff632ade2b031b144de03f44619f595f5598df93f8f7b90f7fce` | Original Grok FAIL at `4b2e8ee62`, preserving the quick-release public-release race finding. |
| [`../../reviews/2026-07-16-w6-r10-grok-pass-final.md`](../../reviews/2026-07-16-w6-r10-grok-pass-final.md) | `07ee7c3f8d435377a8e951881e32617fda40308f649b5579f20140582b5b7625` | Final Grok PASS after the quick-release draft-staging correction. |

Coordinator-provided invalid optional-validation logs were also preserved under [`../../evidence/2026-07-16-w6-r10/`](../../evidence/2026-07-16-w6-r10/): `w6-actionlint-offline.log` SHA-256 `65754c820ac6f21899c1770d87cab000886220957ca645755255d5563fa6f4ba` and `w6-pwsh-offline.log` SHA-256 `dcb6d4b033fc422430c1a6b3b33f2245d2abf2c74be8bdf9b28733dc2ad84fcd`. The incident record is `optional-validation-offline-incident.md`: `nix shell --offline` unexpectedly contacted `cache.nixos.org`, the LAN cache, and an SSH builder; tasks were cancelled after 226 seconds; no process, ref, or repository mutation was accepted. These attempts are invalid and not accepted as validation evidence.

## 2026-07-16 W6 final Fable PASS artifact amendment

The final Fable PASS review was preserved without source/test changes or validation reruns:

| Artifact | SHA-256 | Meaning |
|---|---|---|
| [`../../reviews/2026-07-16-w6-r10-fable-pass-final.md`](../../reviews/2026-07-16-w6-r10-fable-pass-final.md) | `0aa72e35b43f7edd92f0658c281f4712e704e9c6627172ed41ab60cfc9c1e8a9` | Final Fable PASS, copied verbatim from the coordinator-provided artifact. |

The W6 evidence README links this artifact by exact hash. The evidence `SHA256SUMS` policy remains scoped to files under the W6 evidence directory, so no evidence manifest refresh was needed for the review file.
