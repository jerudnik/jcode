# F19 evidence — Package + verify mobile static assets from a share path

**Node:** F19 (implement / deterministic / W3), depends on F18. Issue #22.

## Gap

`jcode mobile-server` serves `web/jcode-mobile`, but:

- The Nix package (`nix/package.nix`) never installed those assets, so an
  installed binary had nothing to serve.
- `mobile_web_root()` in `src/cli/commands/mobile_server.rs` resolved assets by
  trying the **current working directory first**, then an ad-hoc
  `$exe_dir/web/jcode-mobile`. That both (a) failed for an installed binary run
  outside a checkout and (b) let a stray `./web/jcode-mobile` in the CWD **mask**
  a broken install.

## Change

### `nix/package.nix`
- Added `../web/jcode-mobile` to the crane source fileset.
- `postInstall` copies the assets to the FHS share path
  `$out/share/jcode/web/jcode-mobile` (kept out of `commonArgs` so the ~900-crate
  dependency layer is unaffected). `chmod -R u+w` on the copy so crane's
  reference-stripping fixup can rewrite files copied read-only from the store.

### `src/cli/commands/mobile_server.rs`
- Replaced the CWD-first resolver with a **packaging-first**, pure, unit-tested
  `resolve_mobile_web_root(exe, cwd, env_override, exists)`. Precedence:
  1. `JCODE_MOBILE_WEB_ROOT` explicit override (validated, hard-errors if unset
     path is missing — never a silent fallthrough).
  2. `<prefix>/share/jcode/web/jcode-mobile` derived from the executable (the
     install layout).
  3. `<bindir>/web/jcode-mobile` (legacy tree install).
  4. `<cwd>/web/jcode-mobile` **only when the running binary lives inside that
     same checkout** (the `cargo run` dev case). An installed binary can never
     reach this branch, so CWD cannot mask a broken install.
- 6 unit tests cover each branch, including the two acceptance-gate cases
  (`share_path_wins_for_installed_binary`,
  `cwd_cannot_mask_missing_packaged_assets_for_installed_binary`).

## Evidence artifacts

- `nix-build.log.txt` — real `nix build .#packages.aarch64-darwin.jcode` shipping
  the share assets (`signing: .../share/jcode/web/jcode-mobile/...`).
- `http-proof.txt` — **gate #1**: the installed binary, launched from an empty
  temp CWD outside the checkout, serves `HTTP 200` `index.html` from
  `/nix/store/...-jcode-0.46.0/share/jcode/web/jcode-mobile` (see the `serving`
  line). Uses a throwaway `JCODE_HOME`.
- `mask-prevention-proof.txt` — **gate #2**: with a decoy
  `./web/jcode-mobile/index.html` in the CWD, the installed binary still serves
  the packaged assets (`serving /nix/store/...`, response is the real jcode
  index, not the decoy). CWD cannot mask the package.
- Unit tests: `cargo test --lib -- mobile_server` → 6 passed.

## Reproduce (Mac, aarch64-darwin)

```
export PATH="/etc/profiles/per-user/jrudnik/bin:$PATH"
nix build .#packages.aarch64-darwin.jcode --accept-flake-config -o result-f19
PKG=$(readlink -f result-f19)
WORK=$(mktemp -d); export JCODE_HOME="$WORK/home"; mkdir -p "$JCODE_HOME"; cd "$WORK"
"$PKG/bin/jcode" mobile-server serve-internal --port 8791 --bind 127.0.0.1 &
curl -i http://127.0.0.1:8791/    # -> HTTP 200, jcode mobile web index.html
```
