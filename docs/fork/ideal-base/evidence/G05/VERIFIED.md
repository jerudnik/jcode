# G05: unauthenticated public acquisition

Result: **the gate's real question is answered, and the answer is a defect.**
The cache is genuinely public and unauthenticated, but the documented end-user
command does not use it. `docs/NIX.md` has been corrected.

Environment: disposable OrbStack VM `jcode-g05-arm` (Ubuntu, aarch64), Nix
installed from `nixos.org/nix/install` inside the VM. Fork revision
`a632aed2c6da0c640463e15e1d70707b48987143` (`main`).

## Preconditions asserted, not assumed

    whoami: jrudnik
    trusted-users: (empty -> root only, so this user is NOT trusted)
    credential scan: ~/.config/nix/netrc /etc/nix/netrc ~/.cachix ~/.netrc
                     ~/.git-credentials -- none present

The absence of credentials is the assertion. A run with credentials available
would prove nothing about the end-user path.

## Measurement

Target `x86_64-linux`, the arch Cachix is populated for. `--dry-run` still
queries substituters, so it reports fetch-vs-build without needing emulation
(amd64 emulation is unavailable on this arm64 host; see Limitations).

| # | Command (all as the untrusted user) | Plan |
|---|---|---|
| 1 | `nix build --dry-run` (exactly as documented) | **1620 derivations built** |
| 2 | + `--accept-flake-config` | **1620 derivations built** |
| 3 | + root `trusted-substituters` in `/etc/nix/nix.conf`, daemon restarted | **7 paths fetched**, including `jcode-0.46.0` |

Run 1 and 2 emit:

    warning: ignoring untrusted flake configuration setting 'extra-substituters'
    warning: ignoring untrusted substituter 'https://jerudnik-jcode.cachix.org',
             you are not a trusted user.

Run 3 fetched `/nix/store/gjcikpji7kr4xa3zpjckd2npdin0xwax-jcode-0.46.0`
(57.0 MiB across 7 paths) with zero derivations built.

**The control fired.** Between run 2 and run 3 the only change was trust
configuration: same revision, same command, same VM, same network. 1620 built
-> 7 fetched isolates the cause to trust config alone, not to network reach,
not to cache contents, not to the package.

## What this proves and does not prove

Proven: the cache is reachable and usable with no credentials of any kind, and
the published `jcode-0.46.0` closure for `x86_64-linux` is complete enough to
install without compiling.

Disproven: `docs/NIX.md:87`'s claim that `--accept-flake-config` is sufficient.
It is not. Nix discards substituters requested by non-trusted users regardless
of that flag. Corrected in the same commit as this file.

Not proven: end-to-end launch of the fetched `x86_64-linux` binary. It cannot
execute on this arm64 host. G03 separately covers "the binary runs".

## Limitations, stated rather than papered over

- **Architecture.** `orb create -a amd64` fails on this arm64 host
  (`machine didn't start in 30s`); the arm64 control (`orb create ubuntu`)
  succeeded immediately, isolating the failure to amd64 emulation rather than
  OrbStack. So `x86_64-linux` was measured by resolution plan, not by execution.
- **aarch64-linux is uncached by design** (`nix.yml:5`, `docs/NIX.md:30`), so a
  native VM run here would compile from source no matter what. That is expected
  behavior, not a cache failure.
- **The host cannot substitute for the VM.** On this machine
  `trusted-users = root @admin` and the operator is an admin, so client-side
  substituter settings are always honored and an untrusted end user cannot be
  simulated. An attempt to do so is recorded below as a failed control.

## Recorded controls, including ones that failed

1. **Chroot-store attempt on the host** (`nix build --store ... --max-jobs 0`,
   Cachix removed from `substituters`). It still fetched from Cachix. The
   control failing to fire is what exposed that the operator is a trusted user,
   which is why the VM is required. A control that does not fire proves nothing
   until you know why.
2. **A prior session's run** concluded G05 passed. Its own transcript contains
   `warning: ignoring untrusted substituter ... you are not a trusted user`, and
   it waited on `pgrep cargo || pgrep rustc`, so it compiled from source: the
   opposite of the gate. It also tested aarch64-linux, which is uncached by
   design, at a revision 199 commits old. `nix` exits 0 on a cache miss, so the
   failure was silent. That acceptance was reverted. This is the exact failure
   mode this run was built to make loud.

## Process failure to record

Before spawning a reviewer I did not check for other agents in the shared
checkout. Two agents then moved HEAD concurrently. Artifacts were rescued to
`/tmp/g05-rescue/` before any destructive step, and the branch was restored with
`--force-with-lease` pinned to an explicit SHA. `main` was never touched.
