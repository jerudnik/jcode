# Runtime promotion and Nix build runbook

> **Frozen normalization runbook.** It records the transaction used for the
> current immutable runtime. Revalidate it against current source and
> [`../ideal-base/BASELINE.md`](../ideal-base/BASELINE.md) before operational use;
> do not treat its dated identities as automatically current.

This runbook records the supported transaction for promoting a clean source commit
into the local jcode runtime without confusing source promotion, immutable binary
installation, channel selection, or live daemon handoff.

## Four separate transaction stages

1. **Promote source history.** Fast-forward the intended local branch only after
   the fixed candidate and rollback archives pass.
2. **Build and install immutable bytes.** Build from the clean promoted checkout
   inside the declared `nix develop` environment. Set `JCODE_BUILD_GIT_HASH` to
   the exact short commit when an exact runtime identity is required.
3. **Select channels.** Installing a version and moving `current`, `stable`, or
   `shared-server` are separate operations. A channel change does not change an
   already-running process.
4. **Hand off live processes.** Use `jcode server reload` for the shared daemon,
   then deliberately restart retained launcher integrations such as the menubar
   and hotkey listener. Verify executable identity after each handoff.

Do not report the runtime transaction complete until all four stages agree.

## Clean build source

The 2026-07-17 promotion used:

```bash
cd /Users/jrudnik/labs/jcode-normalization-integration
JCODE_BUILD_GIT_HASH=8962bccb3 \
  JCODE_SKIP_SERVER_RELOAD=1 \
  nix develop -c bash scripts/install_release.sh --fast
```

The release source was the clean normalization checkout at
`8962bccb32eede3b6746c42bfe6d265df29e4471`. The dirty canonical recovery
checkout was deliberately not used as the release build source.

Selfdev and release publishing are different lanes. During the first handoff, a
selfdev publication of the same clean source exposed that both lanes had reused
`versions/<git-hash>`, allowing the selfdev build to replace release bytes under
an allegedly immutable path. Commit `8962bccb3` fixes that collision. Release
installs now use `versions/<git-hash>-<profile>` while selfdev retains its source
label, so equal source identity no longer aliases different build profiles.

## Installer exit interpretation

The build and channel installation complete before shell PATH maintenance.
On this host `~/.zshenv` is declaratively protected, so the installer reports:

```text
scripts/lib/configure_path.sh: line 36: /Users/jrudnik/.zshenv: Permission denied
```

That post-install failure must not be misreported as a build or binary-install
failure. Verify the immutable binary, version markers, and channel symlinks
independently. Do not edit the protected file directly. PATH policy belongs in
the declarative Nix/home-manager source.

## Server reload protocol requirement

The hardened server rejects stateful requests until the client sends `Subscribe`
with an absolute `working_dir`. The standalone reload CLI previously connected
and sent `Reload` directly. Commit `1c368592f` makes server-management clients
subscribe first and adds a Unix-socket regression proving the request ordering
and absolute working directory.

A successful handoff must satisfy both cases:

```bash
jcode server reload --force --json  # reloaded=true, handoff_ready=true
jcode server reload --json          # already_current=true, reloaded=false
```

## 2026-07-17 promoted runtime

- Product/runtime commit: `8962bccb32eede3b6746c42bfe6d265df29e4471`
- Release label: `8962bccb3-release`
- Immutable binary:
  `~/.jcode/builds/versions/8962bccb3-release/jcode`
- SHA-256:
  `6cf81221e8c0cee86ae714d2f1fc9fb55fe8715f45ee8082dc2ecf034a2515fc`
- Version: `jcode v0.46.0-dev (8962bccb3)`
- `current`, `stable`, and `shared-server`: all point to the immutable release
- Live daemon: exact `8962bccb3` executable, client/server doctor verdict `same`
- Retained menubar and `com.jcode.hotkey`: restarted onto the same immutable
  release
- Main-socket smoke: subscribed with an absolute working directory, then received
  `ack`, MCP status events, and `pong`
- Active coordinator session survived both daemon handoffs

The home-manager/Nix-managed binary under `/nix/store` remains a separate older
installation. It was not edited directly. The launcher precedence is intentionally
`~/.local/bin/jcode` for this promoted runtime until declarative Nix policy is
updated through its source.

## Rollback

Immediate runtime rollback remains available at:

```text
~/.jcode/builds/versions/02e25ba33-dirty-1706909ba396/jcode
```

The exact pre-cutover targets, hashes, process identities, and post-cutover state
are preserved under:

```text
/Users/jrudnik/labs/jcode-normalization-rollback/
  runtime-pre-promotion-2026-07-17/
  runtime-promotion-8962bccb3-2026-07-17/
```

Rollback must verify the recorded SHA-256 before atomically restoring channel
symlinks and gracefully restarting the daemon, menubar, and hotkey listener.
Do not delete either runtime version or the rollback records during the soak.

## Required verification checklist

```bash
jcode --version
jcode doctor --json
readlink ~/.jcode/builds/current/jcode
readlink ~/.jcode/builds/stable/jcode
readlink ~/.jcode/builds/shared-server/jcode
shasum -a 256 ~/.jcode/builds/versions/8962bccb3-release/jcode
jcode server reload --json
```

Also verify the daemon executable with `lsof -p <pid>`, query `server:info` and
`sessions` through the debug socket, perform a subscribed main-socket ping, and
confirm retained launcher processes execute the same immutable binary.
