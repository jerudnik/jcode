# W3 R04 lifecycle widening evidence

Source branch: `recovery/fix-r04-lifecycle-widening-2026-07-16`.
Authoritative source/test HEAD: `221a9474450a00ba761a989cd765c7e16cb85edc`.

This package supersedes the incomplete completion claims in commit `12052949752cd3c88511597827cd74238fe6b866`, which remain preserved in Git history. That commit recorded 12 selected passes without the required guard manifest and omitted two terminal-outcome fixtures. The corrected package uses the coordinator-owned authoritative run below.

## Guarded targeted result

`targeted-fixtures.log` is a byte-identical copy of `/tmp/jcode-w3-r04-authoritative-targeted-221-20260716T060626Z.log`, SHA-256 `00ada583b2905765f31b6e6a7c3870fd31baa2cdb58cd160131f8279219183df`.

- Guard manifest: `FORK_NUDGE_MAX_AGE=2147483647`, `FORK_NUDGE_AUTOSYNC=0`, `CARGO_NET_OFFLINE=true`, `JCODE_NO_TELEMETRY=1`.
- Every fixture used disposable `JCODE_HOME` and `JCODE_RUNTIME_DIR`.
- Exact sections: 14.
- Exact named passes: 14.
- Exit-zero sections: 14.
- Zero-filter passes counted: 0.

The 14 fixtures cover orphan-from-reload, post-reload cancellation, graceful-shutdown initiator and bounded partial checkpoint, interrupted/resumable wait evidence and rendering, exact-once recovery intent, joint restart identity/compatibility observation, exact marker-removal combinations, bounded cleanup lock acquisition, and terminal persistence outcomes `Persisted`, `NotRequired`, `Failed`, and `SkippedLockTimeout`.

## Affected checks and R09

`r09-matrix.log` records exact commands, expected/actual exits, raw-log hashes, and normalized output. Results:

| Gate | Expected | Actual |
|---|---:|---:|
| Affected package check | 0 | 0 |
| Shared Rust classifier, 17 tests | 0 | 0 |
| Dependency boundaries | 0 | 0 |
| Panic budget | 1 | 1 |
| Swallowed-error budget | 1 | 1 |
| Production-size budget | 1 | 1 |
| Test-size budget | 1 | 1 |
| Wildcard re-export budget | 0 | 0 |
| Warning budget | 0 | 0 |
| Shell syntax | 0 | 0 |
| Source-head diff check | 0 | 0 |

No command used `--update`. The expected-red gates remain visible and attributed. Exact touched-file `rustfmt --check` passed. Workspace-wide formatting remains inherited red only on four out-of-declaration files listed in `commands.log`; W3 did not modify them.

## Claim limits

This evidence closes only the deterministic W3 lifecycle-widening fixture contract. It does not claim a live daemon/reload, real provider or network behavior, multi-daemon signal races, R05B swarm-widening approval, a baseline update, or transfer of R01, R03A, R05B, R12, or R09 authority.
