# W6/R10 final evidence manifest

All accepted commands were local/offline and used disposable state where applicable. No tag, release, publication, live asset download, real installer run, profile mutation outside temp fixtures, credential, or live daemon reload was performed.

## Commits

1. `05f00b671` docs(r10): preserve W6 full review
2. `c1bf53076` fix(r10): fail closed on unchecked release assets
3. `a62516b6f` test(r10): cover fail-closed release acquisition
4. `9203aaf97` ci(r10): publish releases after checksum finalization
5. Final evidence/ledger commit: this directory and R10 ledger amendment

## Accepted evidence files

| File | Result |
|---|---|
| `hermetic-r10-tests.log` | `python3 tests/test_r10_release_acquisition.py` under `CARGO_NET_OFFLINE=true`, `FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`, `JCODE_NO_TELEMETRY=1`, temp `JCODE_HOME`, temp `JCODE_RUNTIME_DIR`: 5 tests OK |
| `guarded-rust-checksum-tests.log` | Guarded `scripts/dev_cargo.sh test -p jcode-app-core verify_asset_checksum -- --nocapture`: 3 tests OK |
| `syntax-checks.log` | `bash -n scripts/install.sh scripts/install_release.sh`: pass; `pwsh` unavailable locally, so PowerShell parser check skipped |
| `workflow-checks.log` | `actionlint` unavailable locally; invariant checks pass for draft creation, checksum-before-publish, and no `softprops/action-gh-release` |
| `r09-summary.txt` and `r09/*.log` | Full R09 expected-exit matrix without `--update`: classifier/wildcard/warning green; panic, swallowed-error, production-size, test-size red with expected exit 1 |
| `git-checks.log` | `git diff --check` pass; final docs were uncommitted when captured |

## Invalid preserved evidence

`invalid-unguarded-rust-attempt.md` records background task `702684spm1`. It timed out after 600s during cold Nix/Rust compilation and lacked required guards: `CARGO_NET_OFFLINE=true`, `FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`, `JCODE_NO_TELEMETRY=1`, and disposable `JCODE_HOME` / `JCODE_RUNTIME_DIR`. It is not used as passing evidence.

## Expected-red attribution

The R09 matrix preserved existing expected-red debt:

- `panic`: expected exit 1, actual exit 1.
- `swallowed`: expected exit 1, actual exit 1.
- `prod_size`: expected exit 1, actual exit 1.
- `test_size`: expected exit 1, actual exit 1.

No `--update` command was run. These reds are attributed to existing R09 ratchets, not to a hidden baseline change.

## Remaining independent review needs

- Independent Grok/coordinator review of the five commits and the W6 evidence directory.
- Cross-platform PowerShell parser/runtime validation on a host with `pwsh` and Windows installer semantics available.
- Actionlint validation on a host with `actionlint` installed, if the invariant fallback is not sufficient for sign-off.
- No real release publication or live updater/installer run should occur until that review explicitly authorizes it.

## 2026-07-16 quick-release Grok FAIL correction

Independent Grok review of HEAD `4b2e8ee62` returned **FAIL** with one IMPORTANT finding: unchanged `scripts/quick-release.sh` could race CI by creating a public release before `SHA256SUMS`. The correction is append-only and adds exactly one path expansion, `scripts/quick-release.sh`.

Additional commits:

6. `30699fe05` docs(r10): preserve quick-release Grok failure
7. `09e23e998` sync(r10): stage quick releases as drafts
8. `9e981514b` test(r10): assert draft-only release entrypoints
9. Final evidence/ledger correction commit: this amendment and R10 ledger append

Additional accepted local/static evidence:

| File | Result |
|---|---|
| `quick-release-static-tests.log` | Guarded `python3 tests/test_r10_release_acquisition.py`: 6 tests OK, including static proof that `release.yml` publishes only after checksum upload and `quick-release.sh` stages draft assets only. |
| `quick-release-syntax.log` | `bash -n scripts/quick-release.sh scripts/install.sh scripts/install_release.sh`: pass. |
| `quick-release-invariants.log` | Direct static invariant checks: pass. |
| `quick-release-diff-check.log` | `git diff --check 566d7930606f96add92aed65564c95b539a03df0..HEAD`: pass at capture time. |

No Nix shell, `gh`, network, tag, release, credential, live updater/installer, profile mutation, or daemon action was performed. The earlier invalid unguarded Rust attempt remains preserved in `invalid-unguarded-rust-attempt.md` and remains non-accepted evidence.

## 2026-07-16 final review artifacts and invalid optional-validation incident

Final independent review artifacts were copied verbatim into `docs/fork/recovery/reviews/`:

| Review artifact | SHA-256 | Result |
|---|---|---|
| `2026-07-16-w6-r10-grok-fail-original.md` | `d5560863860cff632ade2b031b144de03f44619f595f5598df93f8f7b90f7fce` | Original Grok FAIL for HEAD `4b2e8ee62`, preserved exactly. |
| `2026-07-16-w6-r10-grok-pass-final.md` | `07ee7c3f8d435377a8e951881e32617fda40308f649b5579f20140582b5b7625` | Final Grok PASS after quick-release correction, preserved exactly. |

Coordinator invalid optional-validation logs were preserved under this evidence directory:

| Evidence | SHA-256 | Accepted? |
|---|---|---|
| `w6-actionlint-offline.log` | `65754c820ac6f21899c1770d87cab000886220957ca645755255d5563fa6f4ba` | No |
| `w6-pwsh-offline.log` | `dcb6d4b033fc422430c1a6b3b33f2245d2abf2c74be8bdf9b28733dc2ad84fcd` | No |
| `optional-validation-offline-incident.md` | recorded in `SHA256SUMS` | Incident summary: `nix shell --offline` unexpectedly contacted `cache.nixos.org`, LAN cache, and SSH builder; tasks cancelled after 226 seconds; no process/ref/repo mutation accepted. |

These optional-validation attempts are invalid and not counted as accepted evidence. No validation reruns were performed for this final docs-only append commit.

## 2026-07-16 final Fable PASS artifact

The final Fable PASS review was copied verbatim into `docs/fork/recovery/reviews/`:

| Review artifact | SHA-256 | Result |
|---|---|---|
| `2026-07-16-w6-r10-fable-pass-final.md` | `0aa72e35b43f7edd92f0658c281f4712e704e9c6627172ed41ab60cfc9c1e8a9` | Final Fable PASS, preserved exactly. |

This docs-only append did not rerun validations and did not change source or test files. The evidence `SHA256SUMS` manifest covers files under this evidence directory only, so this review artifact is linked here by exact hash and is not added to the evidence-directory manifest.
