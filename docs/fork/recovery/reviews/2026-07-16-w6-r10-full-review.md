# W6/R10 full pre-fix review: packaging, release, update, and distribution

| Field | Value |
|---|---|
| Review date | 2026-07-16 |
| Writer | W6 |
| Scope | R10 packaging, release, update, and distribution |
| Branch under repair | `recovery/fix-w6-r10-acquisition-2026-07-16` |
| Base for repair | `566d7930606f96add92aed65564c95b539a03df0` |
| Fork review ref | `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4` |
| Upstream fixed ref | `802f6909825809e882d9c2d575b7e478dce57d3b` |
| Merge base | `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Disposition | `compose` |
| Confidence | High for workflow/acquisition defects; medium for unexecuted cross-platform package/update paths |

This document preserves the full pre-fix R10 review before implementation changes. It is intentionally docs-only. No release, tag, publish, download against real assets, live daemon, credential, update, installer execution against real assets, shell-profile mutation outside fixtures, network access, or destructive action was performed for this review.

## Executive verdict

R10 is **not acceptable as-is** for release acquisition or installer activation. The correct disposition is **compose**:

1. Keep the fork's downstream Nix/package surfaces because upstream does not own or replace them.
2. Adopt upstream's fixed draft-to-final release publication ordering from `802f6909825809e882d9c2d575b7e478dce57d3b` without overwriting fork package/Nix/signing surfaces.
3. Make remote release acquisition fail closed when `SHA256SUMS` is missing, unreadable, malformed, lacks the selected asset, or mismatches the selected asset.
4. Make installer daemon reload default-off and require explicit opt-in, because live daemon target selection is R01-owned.
5. Prove the repair with hermetic local fixtures only.

## Boundaries and non-goals

R10 owns package outputs, release metadata, release assets, updater acquisition, installer acquisition, launcher installation, and distribution-facing workflows.

R10 does **not** own:

- R01 live daemon identity, target selection, or reload policy.
- R03A wire/build compatibility verdicts.
- R04 session migration semantics.
- R09 quality-gate policy or expected-red baselines.
- R00 sync governance.

The review therefore cannot use a successful installer run or daemon reload as proof that the running daemon is correct. It can only establish that R10 either safely acquires and stages a candidate binary or refuses to mutate state.

## Fixed-reference evidence

The lightweight R10 ledger recorded these decisive refs:

- Fork: `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`
- Upstream fixed ref: `802f6909825809e882d9c2d575b7e478dce57d3b`
- Merge base: `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d`

The review budget was bounded to release publication, remote acquisition, installer activation, and package-surface preservation. It did not execute any real release or installer flow.

## Findings

### F1. Release publication can expose assets before integrity metadata is complete

**Severity:** High

The fork workflow creates or updates a public GitHub release before platform jobs finish. Platform jobs then upload assets directly to that public release, while `SHA256SUMS` is generated only in the final job.

**Evidence:** `.github/workflows/release.yml` at the pre-fix branch has `create-release` using `softprops/action-gh-release@v2` with no draft flag, platform jobs upload assets with `gh release upload`, and the `release` finalizer uploads `SHA256SUMS` after artifact download and checksum generation.

**Impact:** Installers and updaters can observe a latest public release in the window where binaries exist but `SHA256SUMS` does not. A fail-open client then installs an unverifiable asset.

**Required fix:** Compose upstream's fixed draft flow: create a draft release, attach assets to the draft, generate and upload `SHA256SUMS`, then publish only after required build/signature/checksum steps pass.

### F2. In-app updater is fail-open when `SHA256SUMS` is absent

**Severity:** High

The Rust updater has strict checksum parsing and verification available through `jcode-update-core::verify_asset_checksum_text`, but the app layer only calls it when a `SHA256SUMS` asset exists. Missing checksum metadata is logged and treated as success.

**Evidence:** `crates/jcode-app-core/src/update.rs` pre-fix function `verify_asset_checksum_if_available` returns `Ok(())` when `checksum_asset(release)` is `None`.

**Impact:** During a partial release, compromised release metadata, or a workflow regression, the updater can promote an unchecked binary to `stable`, `current`, launcher symlinks, and update metadata.

**Required fix:** Require the `SHA256SUMS` asset before installation and reuse the existing strict `verify_asset_checksum_text` parser/verifier. Do not invent a parallel checksum format or parser.

### F3. POSIX installer downloads release assets without checksum verification

**Severity:** High

`scripts/install.sh` fetches the latest release tag and downloads either `$ARTIFACT.tar.gz` or `$ARTIFACT`, then installs/promotes it. It does not download or verify `SHA256SUMS`.

**Impact:** The one-line installer bypasses the integrity contract that the workflow and Rust updater are supposed to enforce.

**Required fix:** After selecting the exact downloaded asset name and before creating or modifying install/build directories, download `SHA256SUMS`, require a line for that asset, verify the digest with local `sha256sum` or `shasum -a 256`, and fail before state mutation on missing/mismatch.

### F4. Windows installer downloads release assets without checksum verification

**Severity:** High

`scripts/install.ps1` downloads a Windows `.exe` or archive and promotes it to the version store/stable launcher without checking `SHA256SUMS`.

**Impact:** Windows acquisition has the same fail-open integrity gap as POSIX acquisition. Because Windows release assets include both `.exe` and `.tar.gz`, the selected asset's exact filename must be checked.

**Required fix:** Download `SHA256SUMS` for remote artifacts, parse strict 64-hex entries, require the selected filename, compute SHA-256 with `Get-FileHash`, and fail before promotion on missing/mismatch. Local artifact parameters may remain local-fixture paths and should not require network checksum fetches.

### F5. Installer daemon reload is default-on and crosses R01 ownership

**Severity:** High

`scripts/install.sh`, `scripts/install.ps1`, and `scripts/install_release.sh` all attempt `server reload` by default unless `JCODE_SKIP_SERVER_RELOAD=1` is set.

**Impact:** R10 installation can mutate the live daemon target without an explicit user opt-in. This overlaps with R01, where daemon identity and reload target selection are protected responsibilities.

**Required fix:** Default to no reload. Use explicit opt-in such as `JCODE_RELOAD_SERVER=1`, still allowing `JCODE_SKIP_SERVER_RELOAD=1` as a hard disable if present. Hermetic tests must show default-off and explicit-only behavior without contacting a live daemon.

### F6. Mutating steps occur too early for a fail-closed acquisition contract

**Severity:** Medium

Installers create destination directories and version directories before or during acquisition and extraction. A strict acquisition contract should verify the selected remote asset before promotion and avoid touching stable/current/launcher/version markers on missing or mismatched checksums.

**Impact:** Failed or partial acquisitions can leave confusing state, and tests can pass by only checking process exit rather than marker preservation.

**Required fix:** The repair must include fixtures proving missing checksum and checksum mismatch leave `stable`, `current`, launcher, and version markers unchanged.

### F7. Fork package/Nix surfaces are legitimate downstream additions and must be preserved

**Severity:** Medium

The fork carries Nix/package surfaces not present upstream, including `flake.nix`, Nix package metadata, and Home Manager module wiring. These are R10 assets for reproducible acquisition and local identity inspection.

**Impact:** A wholesale upstream overwrite would regress fork-specific distribution surfaces.

**Required fix:** Sync only the upstream release ordering semantics needed for publication safety while preserving fork package/Nix/signing surfaces.

## Required implementation sequence

1. Commit this pre-fix review as a docs-only preservation commit.
2. Commit source fixes:
   - Rust updater requires `SHA256SUMS` and uses `verify_asset_checksum_text`.
   - POSIX and Windows installers verify `SHA256SUMS` before promotion.
   - Installer reload is default-off and explicit-only.
3. Commit hermetic tests:
   - Missing checksum leaves stable/current/launcher/version markers unchanged.
   - Mismatched checksum leaves stable/current/launcher/version markers unchanged.
   - Verified asset promotes exactly one version.
   - Reload is not invoked by default and is invoked only with explicit opt-in.
   - No real network, profile, install path, daemon, or credentials.
4. Commit upstream workflow sync:
   - Draft release creation.
   - Assets remain hidden until final checksum/signature completion.
   - Publish final release after `SHA256SUMS` upload.
   - Preserve fork package/Nix/signing surfaces.
5. Commit final evidence and ledger updates.

## Validation requirements

The final repair must validate offline with:

- Exact hermetic installer/updater tests.
- `bash -n` for touched shell scripts.
- PowerShell parser/static checks if `pwsh` is locally available.
- `actionlint` or explicit workflow invariant checks if `actionlint` is unavailable.
- Affected Rust checks/tests.
- Full unchanged R09 expected-exit matrix without `--update`.

Expected-red results must be attributed to pre-existing R09 ratchets, not hidden or updated.

## Stop conditions

Stop before any of the following:

- Real tag creation.
- GitHub release creation, edit, publication, or asset upload.
- Real updater or installer run against live assets.
- Real shell profile mutation outside hermetic temp fixtures.
- Credential use.
- Live daemon reload.
- External network access.

## 2026-07-16 Grok FAIL declaration and scoped path expansion

Independent Grok review of HEAD `4b2e8ee62` returned **FAIL** with one IMPORTANT finding: `scripts/quick-release.sh` remained unchanged and could race CI by creating a public GitHub release with Linux/macOS assets before `SHA256SUMS` exists.

This FAIL is preserved as a corrective declaration. The prior five commits are not rewritten. The only authorized path expansion for the correction is exactly:

- `scripts/quick-release.sh`

The correction remains bounded by the original R10 stop conditions: no network, no `gh` execution, no tag, no release creation/edit/publication, no credentials, no live updater/installer run, no profile mutation, and no daemon reload. The allowed source behavior is only to compose upstream fixed ref `802f6909825809e882d9c2d575b7e478dce57d3b` quick-release draft staging: `gh release create --draft`, draft asset uploads, and CI alone publishing after checksums.
