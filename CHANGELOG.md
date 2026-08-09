# Changelog

## v1.0.0

Status: planned official first fork release. Use `v1.0.0` unless the operator
chooses one explicit fork-prefixed version name before tagging.

### Release name

`v1.0.0` is the first official fork release name. Old `v0.*` tags are pre-fork
history and are not accepted as proof that a fork release was promoted, even when
those tags still exist locally or on a remote. The release commit must be on the
authoritative `main` branch before tagging.

### Promotion proof

Before publication, the operator checks the candidate with:

```bash
python3 scripts/release_check.py --release v1.0.0 --rollback-ref <previous-promoted-main-ref>
```

For the tag publication job, the workflow runs the same gate with `--require-tag`.
The gate verifies the main-branch relationship, root `Cargo.toml` version,
changelog entry, tag, cached Nix output path, SBOM output, provenance output,
metadata-only GitHub Release guard, install command, and rollback command.

### Distribution

Nix and Cachix are the only binary distribution authority. The release workflow
must verify or use cached Nix output for `packages.<system>.jcode`; it must not
attach executables, installers, archives, checksums, or package-manager payloads
to GitHub Releases. GitHub Releases are metadata-only GitHub Release records.

The release evidence includes:

- `packages.<system>.jcode`, built by Nix and served from Cachix when cached.
- `packages.x86_64-linux.jcode-sbom`, the SBOM output.
- `packages.x86_64-linux.jcode-provenance`, the provenance output tying the
  release to the exact source revision and package output.

### Install test

```bash
nix run github:jerudnik/jcode/v1.0.0 --accept-flake-config -- version
```

The command must print `1.0.0` or the operator-selected replacement release name
before the metadata-only GitHub Release is marked final.

### Rollback test

Rollback stays in the consumer's Nix configuration. The previous promoted ref
must be a main ancestor selected before tagging, not an old `v0.*` tag outside
main history.

```bash
nix run github:jerudnik/jcode/<previous-promoted-main-ref> --accept-flake-config -- version
```

If the release is rolled back for a pinned flake consumer, change the input back
to `<previous-promoted-main-ref>` and rebuild the owning Nix profile, Home
Manager, NixOS, or nix-darwin configuration. This is testable before promotion
because the rollback ref is passed to `scripts/release_check.py` and must resolve
as a main-branch ancestor.
