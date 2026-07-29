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
    import hashlib
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
        source = package.get("source", "")
        if package["name"] == "jcode" and package["version"] == version and not source:
            continue
        identity = "\0".join((package["name"], package["version"], source)).encode("utf-8")
        component = {
            "type": "library",
            "name": package["name"],
            "version": package["version"],
            "bom-ref": "urn:jcode:cargo:%s" % hashlib.sha256(identity).hexdigest(),
        }
        if source.startswith("registry+"):
            component["purl"] = "pkg:cargo/%s@%s" % (package["name"], package["version"])
        elif source.startswith("git+"):
            component["externalReferences"] = [{"type": "vcs", "url": source}]
        components.append(component)

    sbom = {
        "$schema": "http://cyclonedx.org/schema/bom-1.5.schema.json",
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        # CycloneDX serial numbers identify individual generations. Omitting the
        # optional field is more honest than inventing a random or nil UUID for
        # this byte-for-byte reproducible companion output.
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
