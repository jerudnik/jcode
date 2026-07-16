FAIL. One IMPORTANT finding.

## IMPORTANT finding

**Release publication can still expose assets before `SHA256SUMS` when `scripts/quick-release.sh` races or pre-creates the release.**

Evidence:
- `.github/workflows/release.yml:44-55` creates a draft only if no release exists.
- `.github/workflows/release.yml:414-418` explicitly leaves an already-public release public.
- `scripts/quick-release.sh:114-131` is unchanged from base and still runs `gh release create` without `--draft`, uploading Linux/macOS assets immediately.
- Upstream fixed ref `802f6909825809e882d9c2d575b7e478dce57d3b` changed `scripts/quick-release.sh` to stage a draft release with `--draft`, upload assets to the draft, and let CI publish after gates.

Impact: the workflow comment at `release.yml:45-46` says `quick-release.sh` can race this job, but the branch did not compose upstream’s quick-release fix. If quick-release wins, assets are public before `SHA256SUMS`, preserving the core R10 defect.

## Explicit adjudications

1. **Updater missing-`SHA256SUMS` direct unit gap:** acceptable, not IMPORTANT. Control flow is clear: `verify_asset_checksum_required` errors when `SHA256SUMS` is absent at `update.rs:287-293`, and `download_and_install_blocking_with_progress` calls it before promotion at `update.rs:999`.
2. **PowerShell/actionlint unavailable:** acceptable only as recovery fallback evidence, not final cross-platform signoff. Static invariants are reasonable but not equivalent to parser/runtime validation.
3. **Failures before mutation:** checksum-missing/mismatch paths fail before stable/launcher/version promotion in inspected installers. Not every conceivable post-checksum failure is proven to avoid version-dir mutation.
4. **Default-off reload/R01:** satisfied in changed installer paths via `JCODE_RELOAD_SERVER=1` opt-in and `JCODE_SKIP_SERVER_RELOAD=1` hard disable.
5. **Workflow cannot publish before artifacts/checksums:** not satisfied because unchanged `quick-release.sh` can create a public release first.
6. **Invalid unguarded attempt:** honestly excluded in `invalid-unguarded-rust-attempt.md`.
7. **R09/path/commit-class:** five-commit class split and R09 evidence are mostly clean, but the quick-release omission is a path/scope failure for the release-finalization claim.

## Validation performed

- Confirmed exact HEAD `4b2e8ee62cc8784cd82321f2eaf8c8488531fd4b` and five commits from base.
- Ran `python3 tests/test_r10_release_acquisition.py`: 5/5 passed offline with disposable env.
- Ran `bash -n scripts/install.sh scripts/install_release.sh`: passed.
- Ran workflow invariant fallback: passed.
- Ran `git diff --check`: passed.
- Verified evidence `SHA256SUMS`: all OK.
- Compared `scripts/quick-release.sh` and `release.yml` against upstream fixed ref.

## Not checked

I did not run `pwsh`, `actionlint`, real release/tag/publication, network downloads, live updater/installer, daemon reload, credentials, Windows runtime behavior, or a fresh Cargo test build. Confidence: high on the blocking finding.