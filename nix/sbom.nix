{
  runCommand,
  python3,
  cargoLock,
  version,
}:

runCommand "jcode-sbom-${version}"
  {
    nativeBuildInputs = [ python3 ];
    inherit cargoLock version;
  }
  ''
    mkdir -p "$out/share/jcode"
    python3 - <<'PY' "$cargoLock" "$version" "$out/share/jcode/sbom.cdx.json"
    import json
    import re
    import sys
    from pathlib import Path

    lock_path = Path(sys.argv[1])
    version = sys.argv[2]
    out_path = Path(sys.argv[3])

    packages = []
    current = {}
    for line in lock_path.read_text(encoding="utf-8").splitlines() + ["[[package]]"]:
        if line.strip() == "[[package]]":
            if current:
                packages.append(current)
            current = {}
            continue
        match = re.match(r'(name|version|source) = "(.*)"', line)
        if match:
            current[match.group(1)] = match.group(2)

    components = []
    for package in sorted(packages, key=lambda item: (item.get("name", ""), item.get("version", ""), item.get("source", ""))):
        component = {
            "type": "library",
            "name": package["name"],
            "version": package["version"],
            "bom-ref": "pkg:cargo/%s@%s" % (package["name"], package["version"]),
        }
        source = package.get("source", "")
        if source.startswith("registry+"):
            component["purl"] = "pkg:cargo/%s@%s" % (package["name"], package["version"])
        elif source.startswith("git+"):
            component["externalReferences"] = [{"type": "vcs", "url": source}]
        components.append(component)

    sbom = {
        "$schema": "https://cyclonedx.org/schema/bom-1.5.schema.json",
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": "urn:uuid:00000000-0000-0000-0000-000000000000",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "name": "jcode",
                "version": version,
                "bom-ref": "pkg:cargo/jcode@%s" % version,
            },
            "tools": [{"vendor": "jcode", "name": "nix/sbom.nix"}],
        },
        "components": components,
    }
    out_path.write_text(json.dumps(sbom, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
    PY
  ''
