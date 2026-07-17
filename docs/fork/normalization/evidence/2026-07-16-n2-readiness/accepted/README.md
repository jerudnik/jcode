# Accepted N2 readiness evidence

This directory preserves the fixed-head validation run for candidate
`62b3946b63eac0a5082b52fed98087ccafc2160c` on branch
`normalize/integration`.

## Result

All 54 manifest gates produced their expected exit code. The four quality debt
gates remain deliberately red and are guarded by exact assertions. No baseline
update command was invoked.

Key results:

- source and recovery-tree equivalence: pass
- enumerated product delta: pass
- all 14 focused R04 lifecycle fixtures: pass
- R12 suite: 11 passed
- W7 provenance suite: 3 passed
- `jcode-app-core --lib`: 1,101 passed, 23 ignored
- `jcode-base --lib`: 1,169 passed, 3 ignored
- `jcode-storage --lib`: 10 passed
- TUI and workspace compilation: pass
- workspace library clippy with `-D warnings`: pass
- built binary: `jcode v0.46.0-dev (62b3946b6)`
- exact source/binary hash assertion: pass
- panic expected-red: `31 -> 48`
- swallowed-error expected-red: `2987 -> 3074`
- production-size and test-size expected-red: exact detector output preserved

## Integrity

- Original manifest SHA-256:
  `2f0752a6218ec93c563a94f5821cef13fa942fda8ad7231cf3f16b23e4c3b091`
- Original `SHA256SUMS` SHA-256:
  `64ad9d6d7f6307ee9f163ec0f9c54119e3db0ef02214f0494f11f0cdd2e8b48e`
- Packaged host manifest SHA-256:
  `f1fd1107b1a602210bbc353043232735d9696d3d598357068df70f64ccb1a6cd`

Run `./verify_raw.sh` from this directory to verify every decompressed raw log
against `RAW_SHA256SUMS`. `SOURCE_SHA256SUMS` preserves the original absolute
output paths for audit provenance. `EVIDENCE_SHA256SUMS` covers the complete
committed package except itself.

## Non-gating TUI diagnostics

`diagnostics/` contains two full `jcode-tui --lib` runs from the earlier
`9f960f835` candidate: one using the normal host environment and one using an
isolated `HOME`. They produced 38 and 37 failures respectively. These logs are
preserved rather than hidden, but they are not trusted N2 gates:

- Phase 6's accepted TUI gate is affected compilation.
- N2 changes no `jcode-tui` product source.
- The fixed candidate passes `cargo check -p jcode-tui`, workspace compilation,
  and workspace library clippy.

The diagnostic failures are therefore recorded as existing host/config-sensitive
fixture debt, not waived normalization regressions.

## Host and rollback state

`host-manifest.txt` records the clean candidate checkout, fast-forward relation
to local `main`, rollback bundle hashes, archive/stash counts, canonical-checkout
state, and remote disposition. It is observational evidence only. It does not
authorize moving `main`, deleting refs/worktrees/stashes, repointing runtime, or
pushing a remote.
