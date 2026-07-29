# Remote worktree build proof — 2026-07-29 (PR #53)

This is the bounded evidence behind D033's claim that worktree remote builds were silently falling back to the laptop and that PR #53 restored offload to SCO. The reviewed PR head is `386f219ddf2481ac2184da55c97e38b49ac6067e`; the functional commits are:

- `ac3465cd7f93a1197a7cc6b08329273db83f337b` — change the rsync exclusion from `.git/` to `.git`, covering both directories and worktree pointer files.
- `8aa709c3fb56c76587b8eeaa5357ef043d4f844c` — make local fallback loud.

## Before: worktree `.git` file copied and remote evaluation fails

Retained raw log: `/private/tmp/remote-probe.log` at capture time, 3050 lines / 159175 bytes, SHA-256 `8589f46f27253665826473a8528b0cbce3a9ef3e0f14c9eea747e5a3b3f835e8`.

```text
=== Remote Cargo on sco-mesh ===
Local:   /tmp/w4-f23
Remote:  .cache/remote-builds/jcode/w4-f23
Command: cargo check -p jcode-app-core --profile selfdev
Mode:    selfdev
SSH timeout: 5s

[0/3] Checking remote SSH...

[1/3] Syncing source files...
building file list ... done
./
.fork.toml
.git
.gitignore
CONTRIBUTING.md
```

The run reaches the remote and fails before Cargo because the transferred `.git` file points back to the laptop's absolute worktree metadata path:

```text
[2/3] Running on remote...
error:
       … while fetching the input 'git+file:///Users/john/.cache/remote-builds/jcode/w4-f23'

       error: opening Git repository "/Users/john/.cache/remote-builds/jcode/w4-f23": failed to resolve path '/Users/jrudnik/labs/jcode/.git/worktrees/w4-f23': No such file or directory (libgit2 error code = 2)
```

## After: `.git` is excluded and Cargo finishes on SCO

Retained raw log: `/private/tmp/remote-fix.log` at capture time, 3494 lines / 176299 bytes, SHA-256 `6dacd7f0b8f8a06ef98a8e4e47b3b7483c5ecadcd9bae74ecb695f62a55c9f74`.

```text
=== Remote Cargo on sco-mesh ===
Local:   /tmp/w4-remotefix
Remote:  .cache/remote-builds/jcode/w4-remotefix
Command: cargo check -p jcode-base --profile selfdev
Mode:    selfdev
SSH timeout: 5s

[0/3] Checking remote SSH...

[1/3] Syncing source files...
building file list ... done
./
.fork.toml
.gitignore
CONTRIBUTING.md
Cargo.lock
```

No line equal to `.git` appears in the post-fix transfer list. The same remote run completes:

```text
    Finished `selfdev` profile [unoptimized] target(s) in 1m 32s
```

A direct read-only check of the successful cache binds the alias to the remote machine and verifies the synced tree and build output:

```text
$ ssh -o BatchMode=yes -o ConnectTimeout=5 sco-mesh 'printf "hostname="; hostname; printf "home="; printf "%s\n" "$HOME"; p="$HOME/.cache/remote-builds/jcode/w4-remotefix"; printf "remote_dir=%s\n" "$p"; test -d "$p"; if test -e "$p/.git"; then ls -ld "$p/.git"; exit 3; else echo ".git=absent"; fi; test -d "$p/target/selfdev"; echo "target/selfdev=present"'
hostname=serious-callers-only
home=/Users/john
remote_dir=/Users/john/.cache/remote-builds/jcode/w4-remotefix
.git=absent
target/selfdev=present
```

The before/after commands intentionally target different packages because the defect occurs during source synchronization and flake evaluation, before Cargo package selection. The discriminating condition is the transferred `.git` entry and resulting absolute-path failure, followed by a remote Cargo completion with `.git` absent.
