# 2026-07-15 combined prerequisite evidence

Status: durable append-only copy of the surviving coordinator validation, R09, and G0 recheck artifacts at source HEAD `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`.

These files were copied byte-for-byte from `/tmp` before the recovery status documents were edited. Every source/destination pair passed `cmp -s`. Each directory contains a sorted `SHA256SUMS` file. The manifest-file hashes are:

| Evidence set | Original path | Repository copy | `SHA256SUMS` SHA-256 |
|---|---|---|---|
| Sequential combined prerequisites | `/tmp/jcode-combined-prereq-validation` | [`2026-07-15-combined-prereq-validation/`](./2026-07-15-combined-prereq-validation/) | `41ece4820891461de774dbc5ab06d8e8a66c00630be62274d00dc1f5a9952291` |
| Final R09 run and infrastructure attempts | `/tmp/jcode-final-r09` | [`2026-07-15-final-r09/`](./2026-07-15-final-r09/) | `113817813b49815d00a10b716e66ab3ed094b28ff6d02fcc60c6d8584c70940a` |
| G0 non-build R09 rerun | `/tmp/jcode-g0-r09-20260715T1929Z` | [`2026-07-15-g0-r09/`](./2026-07-15-g0-r09/) | `eadb5441bfdf5aef353a2356b2f04454a33912924a07c8eb7e207146ba992614` |

The preserved working-tree invariant remained one user-controlled modification, `docs/fork/recovery/ORCHESTRATOR_PROMPT.md`, whose diff SHA-256 was and remains `8e8e6a92dad180b3925bc0b2a3b7b951bc6a6f5c9e4f8a57c9f522d03ad85c00`.

## Integrated prerequisite chains

### R02 configuration/provider/tier chain

| Category | Coordinator commit(s) |
|---|---|
| Source behavior | `3063fe0fa2d574171871320fe63c80050b764a7b`, `6396c429af29e04fecb0561f9d4b9c4a03c0d3ef`, `3aa644624538ba04318889253a7c67552a8163e0` |
| Tests/fixtures | `285f7ac79856edbcbd2753cc92a50fa13df66e24` |
| Documentation and review preservation | `8b5f2998c16b48185dd2c5e40eac42ea395400ef`, `f86e2d7738ea002388242b53beb1072b0a5f0c72`, `0e7a4b3bf866930b64f63167ba4275849cd452d9`, `cb924b3ae72459c58f5f39654bd9fda4595422b8` |
| Independent correction reviews | Opus PASS `2e5e3c0e0acc63fd22bade8015fdafb003c7fcfb1d0884088345ed92b25388a2`; Fable PASS `1a5ac839a8ea5a83fda1323427e6688210f7921a30f32dcfbbd8d3d6a513dcf3` |
| Validation manifest | Combined manifest `41ece4820891461de774dbc5ab06d8e8a66c00630be62274d00dc1f5a9952291` |
| Intentionally absent commit categories | No separate synchronization or refactor commit. The bounded work was behavior fixes, tests, append-only documentation, and review preservation. |

### R01/R03A runtime identity and compatibility chain

| Category | Coordinator commit(s) |
|---|---|
| Source behavior | `615ab1d9aee979c4744eae86b71dcd7c638ab09e`, `2010b53c8a70a191bd38fc3978c2f760983992aa`, `d5e3fc7ef7d21745f8bd940f18538b53ce934c4a` |
| Tests/fixtures | `0dd9efc13d61635a52d489129f6cf806fcaa2a7e` |
| Documentation and review preservation | `11309fe863d6040ae6732ce58ca421ba6ec74f02`, `9bf52b2b01eaa1e0a282bfa3170de3d1bf69d701`, `3c6c91871c659bd81813e31a71957d6ce499cef2`, `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3` |
| Initial review history | Opus PASS `b1eed52b6112a3c55fb787de15cf82eadb005230cf7b5233507a1f3e07df2f9d`; Fable provider failure `d0f9b9ef56483b2ba2c29f72063ab12f679ec1f4c78554cdd1482ab9c025f1bd`; Grok FAIL `07349da7d17649fb7cfdc9cafc13cf93891f231037a6db2adc2916823d3738d7` |
| Independent correction reviews | Opus PASS `f382998ca7fd56dbc302a43a7f234b3189e8d56979b58175fec342393fdd17f2`; Grok PASS `9b265115ace7786b3698e4affeb006463a0b33903f266ccca73f031af77eafc6` |
| Validation manifest | Combined manifest `41ece4820891461de774dbc5ab06d8e8a66c00630be62274d00dc1f5a9952291` |
| Intentionally absent commit categories | No separate synchronization commit. The helper-export correction is a behavior fix, not an approved broad refactor. |

## Sequential combined validation

The surviving logs are ordered exactly as the coordinator's twelve-step run. All twelve commands exited `0`. Logs 3 and 4 contain zero-test result lines from other test binaries selected by Cargo, but the intended library filters matched and passed exactly 35 and 4 tests respectively. Those zero-test lines are preserved as infrastructure detail and are not counted as passing evidence.

| Log | Evidence | Result |
|---:|---|---|
| `1.log` | `jcode-build-support` library suite | 48 passed |
| `2.log` | `jcode-protocol` library suite | 81 passed |
| `3.log` | R02 subscription catalog/API filters | 35 selected tests passed; unrelated binaries reported zero matches and were not counted |
| `4.log` | R02 provider-admission filters | 4 selected tests passed; unrelated binaries reported zero matches and were not counted |
| `5.log` | Identity handshake unit filter | 3 passed |
| `6.log` | Incompatible initial Subscribe no-mutation lifecycle regression | 1 passed |
| `7.log` | Starting-to-SocketReady reload identity preservation | 1 passed |
| `8.log` | Live handshake/client matrix | 4 passed |
| `9.log` | TUI one-reexec/refusal matrix | 7 passed |
| `10.log` | TUI denied-tier label | 1 passed |
| `11.log` | `jcode-app-core` and `jcode-tui` library checks | exit 0 |
| `12.log` | Root `jcode` TUI binary check | exit 0 |

The raw logs predate the improved validation driver and therefore do not encode their outer command line, start timestamp, expected exit, disk snapshots, or tool paths internally. This is a preserved evidence limitation, not retroactively invented metadata. The next long validation must use the dedicated driver required by the recovery continuation.

## No-reload selfdev-profile build

The coordinated build task was `006329k9q8`, command `scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode`, exit `0` in 46.25 seconds. The target identity was `6c6a4f2c8-dirty-7b4ec829c656` with source fingerprint prefix `7b4ec829c656` and full fingerprint `7b4ec829c656e856`. No reload or daemon activation occurred.

The immutable publication remains at `/Users/jrudnik/.jcode/builds/versions/6c6a4f2c8-dirty-7b4ec829c656/jcode`:

- SHA-256: `fd6297d9d9b135f7c8233dc27a6119bea767f74256e6dddccd1a0e5f557c6dd9`
- Size: `227257456` bytes
- Recorded mtime: `2026-07-15T14:44:17Z`

The original coordinated-build task log was not present in either surviving `/tmp` directory. The task metadata, immutable path, file hash, selfdev channel report, and combined check logs are preserved, but no raw build transcript is claimed.

## R09 current truth and historical discrepancy

The G0 rerun encoded expected exits before invocation. Its command manifest SHA-256 is `267736890c0152b99bb334ddf6197fadf4f7c1feea55e0ec5926d61e97f44a1e`; the enclosing `SHA256SUMS` manifest hash is `eadb5441bfdf5aef353a2356b2f04454a33912924a07c8eb7e207146ba992614`.

| Gate | Expected/actual exit | Current result |
|---|---:|---|
| Shared classifier | `0 / 0` | 17/17 |
| Panic-prone budget | `1 / 1` | 46 versus baseline 31 |
| Swallowed-error-like budget | `1 / 1` | 3,074 versus baseline 2,987 |
| Production-size ratchet | `1 / 1` | 61 findings |
| Test-size ratchet | `1 / 1` | 31 findings |
| Wildcard re-export budget | `0 / 0` | total 16 |
| Warning budget | `0 / 0` | current 0, baseline 0 |
| `bash -n scripts/*.sh` | `0 / 0` | pass |
| `git diff --check` | `0 / 0` | pass |

No command used `--update`.

The historical Phase 0 documents and logs retain swallowed count `3,077`; an intermediate R09 log generation recorded `3,072`; the final preserved and independently rerun current tree records `3,074`. All three generations remain visible. The current claim is 3,074 because it was reproduced at the fixed G0 HEAD with encoded exits and unchanged preservation state.

## Preserved infrastructure events

These events are evidence, not source verdicts:

- Direct dependency-boundary invocation failed because Cargo was absent from `PATH`.
- Two pinned-shell attempts using the macOS Python stub failed with `tool 'python3' not found`.
- One cached Python attempt failed with architecture `Exec format error`.
- The architecture-compatible cached Nix Python plus pinned Cargo passed the dependency-boundary check.
- An initial shell-syntax invocation named nonexistent scripts and failed. The exact recorded command `bash -n scripts/*.sh` passed in G0.
- The earlier combined logs include Nix/dev-shell hook output and preserved zero-test result lines. Neither is silently converted into a source failure or a passing selected test.
- `scripts/clean_target.sh --aggressive` remains untrusted on this macOS environment because it exits 64. It was not invoked during G0 or G1.
- A Serena project-activation tool call returned `Unknown tool: activate_project`; repository work continued with read-only filesystem/Git tools. This is an orchestration-tool infrastructure event only.
- Repository `*.log` ignore rules rejected the first ordinary exact-path staging attempt. The coordinator then used `git add -f` only for the named evidence files; no unrelated ignored file was staged.
- Default cached `git diff --check` reports a final blank line in ten byte-exact combined logs. The source `/tmp` files contain those bytes, and `cmp -s` plus the manifests require preserving them. `git -c core.whitespace=-blank-at-eof diff --cached --check` is the applicable clean check; the logs were not normalized.

## Gate meaning

The previously named strict prerequisite-node count is now **zero** at the integrated source checkpoint. This statement is not Phase 3 authorization. Pilot authorization remains **OPEN, pending independent G2 adversarial adjudication**. G2 may inject new blockers. No pilot may execute from this evidence package alone.

## G2 independent pilot-gate review

The byte-exact independent Opus G2 artifact is [`../reviews/2026-07-15-g2-pilot-gate-opus.md`](../reviews/2026-07-15-g2-pilot-gate-opus.md), copied from `/tmp/jcode-g2-pilot-gate-opus.md` after `cmp -s` verification. SHA-256: `abb7b2694abccb0c32385fc552dcc29bf0eba854d439c5c43dc82ba4f3991e4f`.

Verdict: **PASS** for exactly one bounded fixture pilot. The reviewer independently recomputed all three existing evidence manifests and the named correction-review hashes, rechecked source behavior and preservation, and preserved the evidence limitations. It ran no build or test. G3 must use current gate truth panic `46`, swallowed `3,074`, production-size `61`, test-size `31`; the next long validation requires a dedicated driver with encoded expected exits, disk/tool snapshots, and hashes.

## G4 bounded fixture pilot

The successful byte-exact driver output is [`2026-07-15-g4-bounded-pilot/`](./2026-07-15-g4-bounded-pilot/), captured at source HEAD `505cd86726f86dc0eedaf3998afae6ed83290d5d`.

| Evidence | SHA-256 |
|---|---|
| Successful `SHA256SUMS` | `b4692dc023075d89fcbe94065d089234fa59bbc5777215082870eb00c3842343` |
| Successful `manifest.tsv` | `321c43f51d5cd6e9d953896117d90873adf17b5b4f594ea7fb4f1cb2341eb4e5` |
| Successful `run.meta.json` | `b85e34c61e434e956c2a8cdfc51785ddf3b99d111bf59f5cbb7600bdae9140bb` |
| Successful pilot log | `fdc47ac6cb27cad0dec492990075f98c8a248341fa7de13db00c953c3ae484bf` |
| Attempt-history `SHA256SUMS` | `f1fa86fdbffca927d0128fda92bdb3ff3cdfa85d2561d02b683cd275941f4944` |

All ten expected exits matched actual exits. The pilot observation count is one and forbidden-output hits are zero. Preflight and postflight preserve the same branch, sole dirty prompt path and hash, four stashes, vendor pin, worktree state, and no active build process.

The complete failed and successful launch history is [`2026-07-15-g4-attempt-history/`](./2026-07-15-g4-attempt-history/). It preserves two pre-driver Python failures, one vendor-ref preflight failure and partial plan copy, one observation-framing driver rejection, the corrected standalone-framing verification, and the successful launch transcript. These are infrastructure/evidence-framing events, not passing source evidence. See [`../G4_RESULT.md`](../G4_RESULT.md) for the exact composition and claim limit.

## G5 independent G4 evidence review

The byte-exact Anthropic Opus review is [`../reviews/2026-07-15-g5-g4-evidence-opus.md`](../reviews/2026-07-15-g5-g4-evidence-opus.md), copied from `/tmp/jcode-g5-g4-evidence-opus.md` after byte comparison. Size: `15,077` bytes. SHA-256: `37f094d26b196612f2171de98d52238abb72bb8b69d59b149e7bb00999db86d3`.

Verdict: **PASS**, high confidence, no blocking findings. The reviewer recomputed both G4 manifests and exact memberships, checked every required exit and preservation field, audited the fixture and driver, ran only the driver's offline unit tests and plan check, and preserved three nonblocking limitations. No Cargo/Nix build, live pilot, provider, daemon, or network path was exercised by the review.
## 2026-07-16 Phase 6 final coordinator audit

The accepted no-Nix, no-network final coordinator audit at source head
`51168d16e9c708ae4afff09a6fc6402642d17782` is preserved under
[`2026-07-16-phase6-final-audit/`](2026-07-16-phase6-final-audit/). Its
`SHA256SUMS` file has SHA-256
`9af58f1563f266066edd6da9208983da62eeb0b1997ec78f9c26318221dcd2a3`.

The accepted 76-entry expected-exit manifest has zero mismatches: 48
build-support tests, 81 protocol tests, 38 R02 subscription tests, 4 R02
provider-filter tests, 14 exact R04 fixtures, 11 R12 fixtures, the six affected
package checks, the trusted R09 green/expected-red matrix, and preservation and
process guards all matched. The first attempt is preserved separately as
invalid because its product suite passed 38 tests while a historical count
guard still required 35. All 17 recovery evidence `SHA256SUMS` manifests,
including this package, reproduce from their owning directories.

Independent Opus spot review of candidate commit `4f96772b6` returned **PASS**
with zero IMPORTANT or CRITICAL findings. The byte-exact report is
[`../reviews/2026-07-16-phase6-spot-check-opus.md`](../reviews/2026-07-16-phase6-spot-check-opus.md),
SHA-256 `092dbf4ec862b23b8d778f029772b46b434202e816622bd1f71c4bfa1f759dcc`.
Its sole LOW finding was the distinction between 62 real checks and 76 TSV
physical lines. That wording is corrected append-only in the active status
documents and final package. The corrected package `SHA256SUMS` SHA-256 is
`ca8ff5b9f3b6c09dc0ff05de9b3c1c426fc2373706eeeca26cad87126f2e14d8`;
the earlier `9af58f15...` hash remains the exact candidate package reviewed by
Opus and is preserved in Git history.

Independent Fable architecture and maintainability review of corrected head
`6cbed3a95450a2b22637c63145b31fb5aeda0d87` also returned **PASS** with zero
IMPORTANT or CRITICAL findings. The byte-exact report is
[`../reviews/2026-07-16-phase6-architecture-fable.md`](../reviews/2026-07-16-phase6-architecture-fable.md),
SHA-256 `3fa06d1109c5fc56c9cf1bc73dcea540cff084b5ef4fcc1a0a8dcd48e3910865`.
Its five LOW maintainability findings are carried into the W7 post-recovery
defer with explicit owners, evidence gaps, and escalation triggers.

Joint final sign-off was then completed at fixed head `17586246a`:

- [`../reviews/2026-07-16-phase6-final-signoff-sol.md`](../reviews/2026-07-16-phase6-final-signoff-sol.md),
  PASS, SHA-256
  `228f5937dd7eafa6570ed857b3a8db43a1ed43c0a3c9ad6dcaf6e2d29ef8ebe4`;
- [`../reviews/2026-07-16-phase6-final-signoff-fable.md`](../reviews/2026-07-16-phase6-final-signoff-fable.md),
  PASS, SHA-256
  `7da9ca6810bde9db1035b68e1d2a46f3c0966c6610db7c19553acc96cacc13d3`.

Both reports contain zero unresolved IMPORTANT or CRITICAL findings. They sign
the completed ledgers and recovery plan together, endorse overlay retirement,
and preserve the same offline/no-live claim boundary as the accepted audit.
