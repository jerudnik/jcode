---
title: "macOS: stabilize Screen Recording permission across selfdev and Nix upgrades"
status: open
priority: medium
owner: maintainers
opened: 2026-08-28
related:
  - crates/jcode-build-support/src/lib.rs
  - crates/jcode-build-support/src/platform_support.rs
  - crates/jcode-app-core/src/tool/selfdev/build_queue.rs
  - crates/jcode-app-core/src/tool/selfdev/reload.rs
  - crates/jcode-app-core/src/tool/computer/setup.rs
  - nix/package.nix
  - nix/modules/home-manager.nix
  - docs/NIX.md
  - tests/test_nix_distribution_policy.py
---

# macOS: stabilize Screen Recording permission across selfdev and Nix upgrades

## Problem

On macOS, jcode repeatedly asks for Screen Recording permission after a selfdev rebuild and reload. Each grant becomes stale, and Privacy & Security > Screen Recording accumulates dead jcode entries.

The running selfdev binary is currently linker-generated ad-hoc code:

```text
Executable=~/.jcode/current/jcode
Identifier=jcode-0bf3ad9824c44582
flags=0x20002(adhoc,linker-signed)
Signature=adhoc
TeamIdentifier=not set
Internal requirements=none
# designated => cdhash H"76cca6f5057c0e4459f13b374e895f2db3b525fe"
```

The hash-suffixed identifier and CDHash change when the binary changes. The shared jcode process is detached under launchd, with no stable app-bundle ancestor for TCC to use instead. TCC therefore records a grant for a code identity that disappears at the next rebuild.

Explicit ad-hoc re-signing is not sufficient. A scratch test with:

```sh
codesign --force --sign - --identifier com.jrudnik.jcode ./jcode
```

changed the displayed identifier but still produced a CDHash-only designated requirement. The fix needs a persistent signing certificate, not only a stable `--identifier` value.

## Desired outcome

Use one machine-local self-signed code-signing identity, initially named `jcode-dev`, and sign every mutable macOS jcode payload with:

```sh
/usr/bin/codesign --force \
  --sign jcode-dev \
  --identifier com.jrudnik.jcode \
  --timestamp=none \
  <payload>
```

After the first migration reset and grant:

- `Identifier=com.jrudnik.jcode` remains unchanged across rebuilds.
- The designated requirement remains unchanged across rebuilds because it is anchored to the same certificate.
- Screen Recording prompts once, not after every selfdev reload or Nix package upgrade.
- Linux and Windows behavior is unchanged.
- No private key or machine-specific signature enters a Nix derivation, Cachix, Git, or a release artifact.

## Current selfdev path and exact insertion point

The build and publish path is:

1. `crates/jcode-build-support/src/paths.rs:280-319` constructs the TUI selfdev build as `scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode`.
2. `scripts/dev_cargo.sh:1007-1025` and `scripts/dev_cargo.sh:1075-1110` run Cargo locally or delegate to the remote-build wrapper. This script only produces a repository build; it does not install the reload target.
3. After a successful queued build, `crates/jcode-app-core/src/tool/selfdev/build_queue.rs:350-374` calls `build::publish_local_current_build_for_source()`.
4. An explicit reload follows the same path: `crates/jcode-app-core/src/tool/selfdev/reload.rs:325-362` republishes through `build::publish_local_current_build_for_source()` before signaling the server.
5. `crates/jcode-build-support/src/lib.rs:802-845` validates the repository binary, publishes it, writes source metadata, verifies the installed copy, and updates the launcher.
6. `crates/jcode-build-support/src/lib.rs:164-205` owns the one atomic publisher for `~/.jcode/current/jcode`. It copies to a private staged file, smoke-tests it, then rename-publishes it.
7. `crates/jcode-build-support/src/paths.rs:476-495` defines `~/.jcode/current/jcode` as the one real-file publish target. `crates/jcode-build-support/src/paths.rs:543-578` keeps `~/.local/bin/jcode` pointing at that path unless Nix owns the launcher.

The signing hook belongs in `atomic_publish_binary()` in `crates/jcode-build-support/src/lib.rs:178-205`, on the staged copy and before `smoke_test_staged_binary_for_install()` and `publish_staged_binary()`.

Do not put the signing step in `scripts/dev_cargo.sh`. That would miss explicit reload republishing, would sign a repository build instead of the exact payload being activated, and would complicate remote builds. The atomic publisher is the only point shared by queued build, build-reload, and explicit reload.

### Required selfdev implementation

1. Add a macOS-only helper in `crates/jcode-build-support/src/platform_support.rs` beside the existing platform file operations at lines 1-37. Suggested API:

   ```rust
   pub fn sign_macos_tcc_binary(path: &Path) -> anyhow::Result<()>;
   ```

   On non-macOS targets it is a no-op.

2. On macOS, the helper must:

   - Resolve `/usr/bin/codesign` directly.
   - Require a valid `jcode-dev` code-signing identity. If it is missing, fail with an actionable message that names the setup helper described below.
   - Run `codesign --force --sign jcode-dev --identifier com.jrudnik.jcode --timestamp=none <staged>`.
   - Run `codesign --verify --strict --verbose=2 <staged>`.
   - Reopen and `sync_all()` the staged file after signing. The existing copy is synced before signing at `crates/jcode-build-support/src/lib.rs:225-270`; signing mutates it, so the signature must be flushed before rename.

3. Call the helper in `atomic_publish_binary()` after `run_after_install_stage_hook(source, &staged)` and before the staged smoke test at `crates/jcode-build-support/src/lib.rs:190-194`.

4. Preserve the existing failure contract. A signing or verification failure must remove only the staged temp and leave the previously published `~/.jcode/current/jcode` untouched, as `crates/jcode-build-support/src/lib.rs:197-204` already does.

5. Do not sign desktop application bundles in this change. The affected path is the TUI/CLI payload published to `~/.jcode/current/jcode`; desktop signing has a different bundle identity and distribution contract.

## Bootstrap the machine-local certificate

Add a new setup helper named `setup_macos_codesign.sh` under the repository `scripts/` directory. This is a development/setup helper, not an installer or publication channel.

The script must be idempotent and must not place private material in the repository:

1. Exit with a clear unsupported-platform message unless `uname -s` is `Darwin`.
2. Resolve the user's login keychain, defaulting to `~/Library/Keychains/login.keychain-db`.
3. If `security find-identity -v -p codesigning <keychain>` already reports a valid identity named `jcode-dev`, print the identity and exit successfully.
4. Create a mode-0700 temporary directory and remove it with a trap.
5. Generate a ten-year self-signed RSA certificate and private key with OpenSSL. The certificate must contain:

   ```text
   Subject/Common Name: jcode-dev
   Basic Constraints: critical, CA:FALSE
   Key Usage: critical, Digital Signature
   Extended Key Usage: Code Signing
   Signature Algorithm: SHA-256
   ```

   A validated noninteractive generation shape is:

   ```sh
   openssl req -x509 -newkey rsa:2048 -sha256 -nodes \
     -keyout "$tmp/jcode-dev.key.pem" \
     -out "$tmp/jcode-dev.cert.pem" \
     -days 3650 \
     -subj '/CN=jcode-dev' \
     -addext 'basicConstraints=critical,CA:FALSE' \
     -addext 'keyUsage=critical,digitalSignature' \
     -addext 'extendedKeyUsage=codeSigning'
   ```

6. Export the certificate and key to a temporary password-protected PKCS#12 file. Generate the password in memory and never print it.
7. Import the PKCS#12 identity into the login keychain with `/usr/bin/security import ... -T /usr/bin/codesign`, so detached selfdev builds can invoke codesign without a new key-access prompt each time.
8. Mark the certificate trusted for code signing in the user trust domain with `/usr/bin/security add-trusted-cert -r trustRoot -p codeSign -k <keychain> <cert.pem>`.
9. Verify success with `security find-identity -v -p codesigning <keychain>` and fail if `jcode-dev` is not valid.
10. Delete all temporary key, certificate, password, and PKCS#12 files through the trap.

Do not use `security create-keypair` or `certtool` without proving the resulting certificate has the Code Signing EKU. The installed `certtool` interface exposes Any/SSL/SMIME EKUs, not an explicit code-signing EKU; the OpenSSL plus `security import` flow is explicit and testable.

The selfdev publisher should fail closed on macOS when the identity is absent. Publishing another unsigned binary silently recreates the TCC churn this issue is meant to stop.

## Nix release/distribution path

### Constraints found in the current package

- The flake exposes the package, overlay, and Home Manager module at `flake.nix:45-57` and wires `nix/package.nix` into `packages.default`/`packages.jcode` at `flake.nix:193-196` and `flake.nix:242-254`.
- `nix/package.nix:112-146` builds the release payload in the Nix store and applies `wrapProgram` so Reasonix is on the wrapped process PATH.
- The Home Manager module currently only adds `cfg.package` to `home.packages` at `nix/modules/home-manager.nix:117-152`. It has no activation hook.
- `docs/NIX.md:37-76` documents `nix run`, `nix build`, and `nix profile install`; `docs/NIX.md:171-208` documents the Home Manager module.
- `tests/test_nix_distribution_policy.py:26-43`, `tests/test_nix_distribution_policy.py:170-185`, and `tests/test_nix_distribution_policy.py:216-280` enforce the Nix-only publication contract and ban restored installer/release channels.

A machine-local private key cannot be used inside the Nix derivation or CI. Re-signing `$out/bin/...` after realization is also invalid because the Nix store is immutable and content-addressed. The Cachix artifact must remain the unsigned/linker-signed reproducible source payload.

### Viable automatic path: opt-in Home Manager activation

Home Manager is the only repository-owned installation surface with a per-user activation phase. Add an opt-in Darwin-only option, for example:

```nix
programs.jcode.macOSStableCodeIdentity.enable = true;
```

Implement it as follows:

1. In `nix/package.nix:141-146`, keep the Nix wrapper as the owner of runtime environment setup, but make its real Mach-O payload available at a stable package path such as `$out/libexec/jcode/jcode`.

   This can be a symlink to the hidden original created by `wrapProgram`; do not duplicate the binary in the store.

2. Extend the package wrapper with an internal payload override, for example `JCODE_NIX_PAYLOAD_OVERRIDE`. When set, the wrapper must exec that payload instead of the store Mach-O while still applying:

   - the existing Reasonix PATH from `nix/package.nix:141-146`;
   - `JCODE_NIX_MANAGED=1`, because the copied payload's `current_exe()` is no longer under `/nix/store` and store-residence detection at `crates/jcode-build-support/src/paths.rs:511-540` would otherwise misclassify it;
   - `JCODE_MOBILE_WEB_ROOT=$out/share/jcode/web/jcode-mobile`, because mobile assets are resolved relative to `current_exe()` unless overridden at `src/cli/commands/mobile_server.rs:180-225`.

3. Add the option in the `programs.jcode` option block at `nix/modules/home-manager.nix:33-115` and the activation logic in the config block at `nix/modules/home-manager.nix:117-152`.

4. The Darwin activation hook should run after Home Manager's write boundary and should:

   - require the `jcode-dev` identity or fail with the bootstrap-script guidance;
   - copy `${cfg.package}/libexec/jcode/jcode` to a private staged file under a user-owned stable prefix such as `$JCODE_HOME/nix-current/` or `~/.jcode/nix-current/`;
   - make the staged copy owner-writable and executable;
   - sign and verify it with the same identity and identifier as selfdev;
   - fsync and atomically rename it over the previous signed payload;
   - record the source package store path in a sidecar so unchanged Home Manager activations can skip needless re-signing;
   - leave the previous signed payload intact when copy, signing, or verification fails.

5. When the option is enabled, add a small Home Manager launcher package to `home.packages` instead of adding `cfg.package` directly. The launcher should set `JCODE_NIX_PAYLOAD_OVERRIDE` to the stable signed path and then exec `${cfg.package}/bin/jcode`. Delegating to the original package wrapper preserves Reasonix and the packaged asset paths.

6. Keep the option opt-in. Certificate creation modifies the user's Keychain and cannot happen as an implicit Nix build side effect.

### Unsupported automatic Nix modes

There is no repository-controlled per-user activation hook for:

- `nix run`;
- direct `nix profile install`;
- downstream `environment.systemPackages` or direct package use without the Home Manager module.

Do not add a curl installer, release asset, Homebrew package, or standalone non-Nix binary installer to cover those modes. That would violate the distribution contract enforced by `tests/test_nix_distribution_policy.py` and the authority described in `docs/NIX.md:1-11`.

For users who need jcode itself to hold Screen Recording permission, document the Home Manager option as the supported automatic Nix path. Other Nix modes continue to execute the immutable store payload and may need a fresh TCC grant after a package identity change. A later generic user-side copy/sign tool may be considered only if it remains a Nix-owned activation mechanism and passes `nix-distribution-policy`; it is not required for this issue.

## TCC migration guidance

Add the migration steps to the bootstrap script output and to the macOS permission setup guidance near `crates/jcode-app-core/src/tool/computer/setup.rs:66-97`.

The one-time cleanup command is:

```sh
tccutil reset ScreenCapture
```

This is intentionally global because the stale entries use changing hash-derived identities and cannot all be named reliably. The command revokes Screen Recording permission for every application, not only jcode. The script must print that consequence and ask the user to run it manually; do not execute it automatically.

After the reset:

1. Start the newly signed jcode.
2. Trigger a capture through `macos_computer_use` setup/check or another jcode screenshot path. The current probe is `/usr/sbin/screencapture` in `crates/jcode-app-core/src/tool/computer/setup.rs:19-32`.
3. Grant `com.jrudnik.jcode` in Privacy & Security > Screen Recording.
4. Restart jcode once if macOS requests it.

For later testing of only the stable identity, `tccutil reset ScreenCapture com.jrudnik.jcode` may be used when macOS accepts the identifier. The first migration should retain the documented global reset because it is the reliable way to flush the accumulated dead entries.

## Tests and verification

### Automated tests

1. Extend `crates/jcode-build-support/src/atomic_publish_tests.rs:89-158` to cover signing as part of the staged publish contract. Use a test hook or injected command runner; CI must not require a real Keychain identity.
2. Prove that a signing failure behaves like a smoke-test failure: no staged temp remains and the previous published binary is unchanged.
3. Prove that signing happens before smoke-test and rename. The smoke test should observe the signed staged path, never the source or final path.
4. Keep `crates/jcode-build-support/src/fixed_path_resolver_tests.rs:17-62` passing so both client and daemon still resolve the one fixed publish target.
5. Add non-macOS coverage proving the platform helper is a no-op.
6. Add Nix evaluation/build coverage for the stable `libexec` payload, internal wrapper override, and Home Manager option. The test must inspect generated activation/launcher text but must not access a real Keychain.
7. Run:

   ```sh
   scripts/dev_cargo.sh test -p jcode-build-support
   scripts/dev_cargo.sh test -p jcode-app-core selfdev
   python3 tests/test_nix_distribution_policy.py
   nixfmt --check flake.nix nix/*.nix nix/modules/*.nix
   nix flake check --accept-flake-config --no-build --all-systems
   ```

### Manual macOS acceptance test

1. Run the `setup_macos_codesign.sh` helper and confirm:

   ```sh
   security find-identity -v -p codesigning | grep 'jcode-dev'
   ```

2. Run a selfdev build-reload, then capture identity and requirement:

   ```sh
   codesign -dvvv ~/.jcode/current/jcode 2>&1 | \
     grep -E '^(Identifier|Authority|TeamIdentifier|Signature)='
   codesign -d -r- ~/.jcode/current/jcode 2>&1 | tail -1 \
     > /tmp/jcode-requirement-1.txt
   shasum -a 256 ~/.jcode/current/jcode > /tmp/jcode-binary-1.sha256
   ```

3. Make a real code change, run a second selfdev build-reload, and repeat into `requirement-2.txt` and `binary-2.sha256`.
4. Assert:

   - the binary SHA-256 values differ;
   - both displays contain `Identifier=com.jrudnik.jcode` and `Authority=jcode-dev`;
   - neither display says `Signature=adhoc`;
   - `diff -u /tmp/jcode-requirement-1.txt /tmp/jcode-requirement-2.txt` is empty.

5. Run the one-time `tccutil reset ScreenCapture`, start the signed jcode, trigger Screen Recording, and grant it once.
6. Rebuild and reload again. Trigger another screenshot. No new Screen Recording prompt should appear, and the existing permission entry should remain live.
7. If the Home Manager option is implemented, repeat the identity/requirement comparison across two `home-manager switch` operations using different jcode package revisions. Confirm that the launched process is the signed user-owned payload while the source package and assets still come from Nix.

## Documentation

- Update `docs/NIX.md:171-208` with the opt-in Home Manager setting, bootstrap prerequisite, signed payload location, and unsupported direct-Nix modes.
- Update the selfdev loop at `docs/NIX.md:236-258` to state that macOS selfdev publishing requires the one-time `jcode-dev` identity.
- Update `crates/jcode-app-core/src/tool/computer/setup.rs:66-97` messages so repeated permission failure points to code-sign bootstrap before telling the user to toggle the pane again.
- Do not document any non-Nix publication or installer channel.

## Acceptance criteria

- [ ] A fresh macOS contributor can create the `jcode-dev` identity with one idempotent repository script.
- [ ] Every selfdev publish signs the staged `~/.jcode/current/jcode` payload before smoke-test and atomic rename.
- [ ] Missing or failed signing cannot replace the last-known-good published binary.
- [ ] Two changed selfdev builds have different file hashes but the same `com.jrudnik.jcode` designated requirement.
- [ ] Screen Recording is granted once and survives a subsequent selfdev rebuild/reload.
- [ ] The Nix derivation and Cachix artifact contain no private key and are not mutated after realization.
- [ ] Home Manager can opt into an activation-time signed copy while preserving Reasonix PATH, Nix-managed launcher ownership, and mobile web assets.
- [ ] Direct `nix run`, `nix profile`, and non-Home-Manager package installs remain Nix-only and are documented as lacking automatic machine-local signing.
- [ ] `nix-distribution-policy` continues to pass.

## Non-goals

- Apple Developer ID signing, notarization, or App Store distribution.
- Signing the desktop `.app` bundle.
- Restoring release assets, shell installers, Homebrew, AUR, or any other non-Nix publication path.
- Automatically running `tccutil reset ScreenCapture`.
- Sharing or exporting the machine-local private key.
