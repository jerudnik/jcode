PASS.

Findings:
- IMPORTANT/CRITICAL: none.

Validation:
- Reviewed target `c07654e259ef8bd016df1085437fd26e0e6c7e0d` against base `566d79306`. Current checkout is one docs/evidence commit ahead, but code/test/workflow paths are unchanged from the target.
- `python3 tests/test_r10_release_acquisition.py`: 6/6 passed.
- Target-ref static invariant script: 32 predicates passed.
- `bash -n` passed for target `install.sh`, `install_release.sh`, and `quick-release.sh`.
- `git diff --check 566d79306..c07654e...`: passed.
- No Nix, network, `gh`, credentials, tag/release, live installer/updater, daemon, or external actions invoked.

Evidence:
- Checksum-required acquisition fails before promotion: Rust requires `SHA256SUMS` and verifies before install extraction at `crates/jcode-app-core/src/update.rs:281-305,999`; checksum parser rejects missing, mismatch, invalid, and duplicate entries at `crates/jcode-update-core/src/lib.rs:221-277`; shell installer verifies before promotion at `scripts/install.sh:113-139`; PowerShell installer verifies before promotion at `scripts/install.ps1:148-181,521-529`.
- Reload is explicit-only: `scripts/install.sh:230-240`, `scripts/install_release.sh:90-96`, and `scripts/install.ps1:594-600` gate reload on `JCODE_RELOAD_SERVER=1` and retain `JCODE_SKIP_SERVER_RELOAD`.
- Workflow/quick-release do not make the release public before checksums: draft creation and final checksum-before-publish ordering are at `.github/workflows/release.yml:36-55,380-418`; asset uploads are attached to the draft at `.github/workflows/release.yml:158-165,269-274,360-363`; quick-release stages draft assets and leaves CI to publish at `scripts/quick-release.sh:118-150`.
- Quick-release staging block matches upstream fixed ref `802f6909825809e882d9c2d575b7e478dce57d3b` by local diff/static comparison.
- Tests cover missing/mismatched checksums preserving existing markers, verified promotion, explicit reload, Rust updater checksum requirement, and draft-only release entrypoints at `tests/test_r10_release_acquisition.py:228-313`.
- Invalid unguarded attempt is excluded at `docs/fork/recovery/evidence/2026-07-16-w6-r10/README.md:24-26,66`; R09 expected-red state is preserved at `README.md:28-37` and no R09 source/ledger paths changed.

Confidence: high.

Untested surfaces:
- Real GitHub Actions execution and `actionlint`.
- PowerShell parser/runtime on Windows, because local `pwsh` was unavailable.
- Live GitHub releases/assets, real installer/updater downloads, daemon reload against a running daemon, Nix/package builds, signing, Homebrew, and AUR downstream jobs.