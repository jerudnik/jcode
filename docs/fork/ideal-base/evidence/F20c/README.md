# F20c evidence — retire the dead distribution surface

**Node:** F20c (implement / deterministic / W3), depends on F20b + F17.
**Branch:** `f20c-retire-distribution`.

## Gap

F20a made a nix-installed jcode decline self-update. F20b made self-dev publish
to ONE atomic fixed path, `~/.jcode/current/jcode`, and made every client and
daemon resolver prefer it. Together those two nodes removed the last *reader* of
the old distribution machinery but deliberately left it in place:

- the GitHub-release acquisition subsystem (download / resume / checksum /
  install-over-self, `crates/jcode-update-core`),
- the hand-rolled version store (`~/.jcode/builds/versions/<version>/`),
- the channel matrix (`stable`, `current`, `shared-server`, `canary`) with its
  symlink writers, version marker files, promote/advance/repair paths, and the
  pending-activation + canary-rollback state machine.

Dead code is not the real hazard here. The hazard is **stale writers**: code and
scripts that keep *producing* state nothing consumes. A launcher symlink still
pointing into `builds/current/` keeps executing a binary that no code path can
ever replace, while every in-process resolver reports the fixed path as
authoritative. That is a silently un-updatable install.

This machine proved it is not hypothetical (`retired-layout-detection.txt`):
**4.5 GiB across 8 leftover entries, and `~/.local/bin/jcode` still resolving
into the retired layout.**

## Change

**Deleted** (`removal-grep-clean.txt`, 0 surviving references):
`crates/jcode-update-core` entirely; the version store
(`install_binary_at_version` / `install_version` / `install_local_release`); all
channel symlink writers (`update_{stable,current,shared_server,canary}_symlink`,
`promote_version_to_shared_server`, `advance_shared_server_if_tracking_stable`,
`repair_stale_shared_server_channel`); the pending-activation/canary state
machine (`reconcile_stale_pending_activation`, `PendingActivation`,
`CanaryStatus`, `CrashInfo`, `BinaryChoice`); every channel path helper; and
`examples/promote_build.rs`. `BuildManifest` shrank to `history`.

**Rewritten, not deleted** — the installers. After the channel removal
`install.sh`, `install.ps1`, `install_release.sh` and `uninstall.sh` became
exactly the stale writers described above, which is worse than dead code. They
now stage into a private temp dir and publish via atomic rename to the single
fixed path, then point the launcher there.

**Added — honest migration.** Deleting readers is only half a removal.
`retired_layout_residue()` enumerates pre-F20c leftovers, sizes them
recursively, and flags the entry the launcher resolves into. `jcode doctor`
reports them and escalates to a WARNING when the launcher is stranded (that is a
broken install, not just wasted disk). `jcode doctor --clean-retired-layout`
removes them and **refuses** while the launcher still resolves into them, rather
than deleting the binary the user is currently running.

**Update is now nix-or-source only.** `update.rs` retains no download path at
all; `tests/test_r10_release_acquisition.py` asserts the strictly stronger
property that replaced the old checksum requirement: there is no release-asset
fetch left to verify.

## Coverage preserved, not dropped

42 channel-era tests were removed. They were replaced with tests for the
surviving invariant, not deleted:

| Invariant | Test |
|---|---|
| client and daemon resolve to the same fixed target | `build-support::tests` resolver tests |
| selfdev falls back to an unpublished repo build | `selfdev_falls_back_to_an_unpublished_repo_build` |
| nix-managed sessions ignore a self-managed publish | `nix_managed_sessions_ignore_the_self_managed_publish_target` |
| failed publish leaves nothing staged and preserves the previous binary | `atomic_publish_tests.rs` |
| the real publish writer lands where both resolvers read | `fixed_path_resolver_tests.rs` |
| status reports the published build from its sidecar | `selfdev::tests` |
| find-config advertises one binary, not a channel matrix | `find_config_reports_key_paths` |
| pre-F20c leftovers are detected, sized, stranded-launcher flagged | `retired_version_store_is_detected_and_sized`, `launcher_still_pointing_into_retired_layout_is_flagged_as_stranded` |
| no installer recreates the retired layout | R10 python fixture, `test_install_release.sh`, `verify_windows_install.ps1` |

## Three real defects surfaced while cutting

1. **`find_config_reports_key_paths` asserted `"Build channels"`.** `setup.rs`
   already reported the one published binary, so the test was green against a
   label F20c had deleted.
2. **`selfdev_falls_back_to_an_unpublished_repo_build` pinned the wrong repo.**
   `get_repo_dir()` validates `JCODE_REPO_DIR` with `is_jcode_repo()` and
   silently falls back to `CARGO_MANIFEST_DIR`; the bare-tempdir fixture
   resolved the developer's real checkout and passed only while that checkout
   had no `target/selfdev/jcode`. It failed in the serial suite while still
   passing when filtered.
3. **The TUI resume fixture wrote `builds/current/jcode`** to pin `JCODE_HOME`,
   a path no resolver reads after F20c, so it kept "passing" only by falling
   through to `current_exe`.

## Scope decision (see DECISIONS.md)

The declared `owned_paths` named three `.github/workflows/` files, but
`docs/BRANCHING.md` gives `distro/nix` sole ownership of CI and
`scripts/fork-health.sh` check 5 fails when `main` carries a workflow diff.
Rather than silently violate the branch model, the node was amended: those paths
were dropped in favour of `.github/scripts/verify_windows_install.ps1`, which
was the only genuine coupling in CI (it asserted the deleted
`builds/versions` + `builds/stable` layout). `depends_on` gained `F17` to
serialize the `.github/scripts/**` ownership overlap the railway validator
correctly flagged. Both `expansions` and `all_nodes` copies verified identical.

## Gates

- `./scripts/preflight.sh` — **all 9 gates pass** (`preflight.txt`). The four
  ratchets that initially failed were cleared by changing the code, never by
  `--update`: `env::var_os` instead of `.ok()` for an absent env var; a
  diagnostic that returns `Result` instead of silently swallowing its own scan
  failure; folding the new flag into `run_doctor_command` instead of adding a
  wrapper; and splitting the over-budget `selfdev/tests.rs` into
  `reconcile_tests.rs`.
- Deterministic suite run twice, second run after `cargo clean` of the touched
  crates: `suite-run-1.txt`, `suite-run-2.txt`. Identical results both times
  (38 + 32 Rust tests, 6 R10 python tests, installer publish test, `bash -n`).

## Evidence artifacts

| File | Contents |
|---|---|
| `removal-grep-clean.txt` | 28 retired symbols, 0 surviving references; deleted-artifact checks; pointer to the executable layout assertions |
| `retired-layout-detection.txt` | Real pre-F20c machine: 8 entries / 4.5 GiB detected, stranded launcher flagged, clean correctly refused |
| `suite-run-1.txt`, `suite-run-2.txt` | Deterministic suite, double green, second from clean |
| `preflight.txt` | All 9 preflight gates green |

## Reproduce

```
./scripts/preflight.sh
./scripts/dev_cargo.sh test -p jcode-build-support --lib -- --test-threads=1
./scripts/dev_cargo.sh test -p jcode-app-core --lib -- selfdev:: --test-threads=1
python3 tests/test_r10_release_acquisition.py
./scripts/test_install_release.sh
jcode doctor          # reports retired-layout residue when present
```
