# G05: unauthenticated public acquisition

Result: **gate met end to end, and it surfaced a documentation defect.**
The `x86_64-linux` binary was fetched from public Cachix with no credentials,
with all local building forbidden, and launched. Separately, the command
`docs/NIX.md` told users to run did not use the cache at all; corrected.

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

Target `x86_64-linux`, the arch Cachix is populated for. OrbStack registers
`qemu-x86_64` binfmt, so the VM executes x86-64 binaries transparently.

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

## Acquisition and launch, not just a plan

The table above is `--dry-run`. The gate was then satisfied for real:

    nix build --max-jobs 0 --accept-flake-config ...#packages.x86_64-linux.jcode
      copying path '...-jcode-0.46.0' from 'https://jerudnik-jcode.cachix.org'
      EXIT=0
      -r-xr-xr-x 1 root root 142610888 /tmp/g05out/bin/jcode

`--max-jobs 0` forbids all local building, so exit 0 is only reachable by
substitution. The fetched file is genuinely x86-64, read from the ELF header
rather than inferred:

    magic: b'\x7fELF'   e_machine: 0x3e   (0x3e = x86-64, 0xb7 = aarch64)

It then launched:

    /tmp/g05out/bin/jcode --version  ->  jcode v0.46.0 (a632aed)   EXIT=0

matching the revision under test.

## What this proves and does not prove

Proven: the cache is reachable and usable with no credentials of any kind; the
published `jcode-0.46.0` closure for `x86_64-linux` installs without compiling;
and the acquired binary runs.

Disproven: `docs/NIX.md:87`'s claim that `--accept-flake-config` is sufficient.
It is not. Nix discards substituters requested by non-trusted users regardless
of that flag. Corrected in the same commit as this file.

Not proven: behavior on real x86-64 silicon. Execution here is qemu emulation,
which is sufficient to show the artifact is not corrupt and its entry point
works, but is not a substitute for native hardware.

## Limitations, stated rather than papered over

- **Emulated, not native.** `orb create -a amd64` fails on this arm64 host
  (`machine didn't start in 30s`); the arm64 control (`orb create ubuntu`)
  succeeded immediately, isolating that failure to amd64 VM creation. The
  x86_64 binary still ran, via `qemu-x86_64` binfmt inside the arm64 VM. I
  initially recorded "cannot execute on this host" and that was wrong: the run
  contradicted it, and the binfmt registration explains why.
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
