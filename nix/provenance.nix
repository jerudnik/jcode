{
  runCommand,
  jq,
  nix,
  jcode,
  sbom,
  version,
  sourceFullRevision,
  sourceDisplayRevision,
  flakeLockSha256,
  system,
}:

runCommand "jcode-provenance-${version}"
  {
    nativeBuildInputs = [
      jq
      nix
    ];
    inherit
      version
      sourceFullRevision
      sourceDisplayRevision
      flakeLockSha256
      system
      ;
    jcodeDrvPath = jcode.drvPath;
    jcodeOutPath = jcode.outPath;
  }
  ''
    mkdir -p "$out/share/jcode"
    nar_hash=$(nix --extra-experimental-features nix-command hash path --sri ${jcode})
    nar_size=$(nix-store --dump ${jcode} | wc -c | tr -d ' ')
    sbom_sha256=$(sha256sum ${sbom}/share/jcode/sbom.cdx.json | cut -d' ' -f1)

    jq -n \
      --arg sourceFullRevision "$sourceFullRevision" \
      --arg sourceDisplayRevision "$sourceDisplayRevision" \
      --arg flakeLockSha256 "$flakeLockSha256" \
      --arg version "$version" \
      --arg system "$system" \
      --arg drvPath "$jcodeDrvPath" \
      --arg outPath "$jcodeOutPath" \
      --arg narHash "$nar_hash" \
      --argjson narSize "$nar_size" \
      --arg sbomSha256 "$sbom_sha256" \
      '{
        schema: "https://jerudnik.github.io/jcode/schemas/nix-provenance/v1",
        scope: {
          artifact: "packages.x86_64-linux.jcode",
          claim: "installed output only",
          nix_system: $system,
          exclusions: [
            "scripts/build_linux_compat.sh",
            "compatibility bundle and Linux archive assets are excluded unless made reproducible separately",
            "release assets"
          ]
        },
        release_assets: { included: false },
        source: {
          full_revision: $sourceFullRevision,
          display_revision: $sourceDisplayRevision,
          flake_lock_sha256: $flakeLockSha256
        },
        version: {
          cargo: $version,
          runtime_display: "short jcode --version remains authoritative for humans"
        },
        nix: {
          system: $system,
          rebuild_guidance: "nix build .#packages.x86_64-linux.jcode --rebuild --keep-failed --no-substitute --print-build-logs"
        },
        derivation: {
          drv_path: $drvPath
        },
        output: {
          store_path: $outPath,
          nar_hash: $narHash,
          nar_size: $narSize
        },
        sbom: {
          path: "share/jcode/sbom.cdx.json",
          sha256: $sbomSha256,
          store_path: "${sbom}"
        }
      }' > "$out/share/jcode/provenance.json"
  ''
