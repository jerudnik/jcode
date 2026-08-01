#!/usr/bin/env python3
"""Verify jcode's reproducibility provenance and CycloneDX SBOM.

The checker is intentionally structural and deterministic: it compares the
provenance document to independently supplied expected facts, validates the SBOM
shape and identity rules, and proves that every locked Cargo package appears in
the SBOM.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


REQUIRED_PROVENANCE_SCHEMA = "https://jerudnik.github.io/jcode/schemas/nix-provenance/v1"
REQUIRED_SBOM_SCHEMA = "http://cyclonedx.org/schema/bom-1.5.schema.json"
REQUIRED_EXCLUSIONS = [
    "scripts/build_linux_compat.sh",
    "compatibility bundle and Linux archive assets are excluded unless made reproducible separately",
    "release assets",
]


class VerificationError(Exception):
    pass


def load_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def expect_eq(errors: list[str], field: str, actual: Any, expected: Any) -> None:
    if actual != expected:
        errors.append(f"{field}: expected {expected!r}, got {actual!r}")


def get_path(document: dict[str, Any], dotted: str) -> Any:
    current: Any = document
    for part in dotted.split("."):
        if not isinstance(current, dict) or part not in current:
            raise VerificationError(f"missing field {dotted}")
        current = current[part]
    return current


def parse_cargo_packages(lock_path: Path, root_version: str) -> list[dict[str, str]]:
    packages: list[dict[str, str]] = []
    current: dict[str, str] = {}
    in_package = False

    for raw_line in lock_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if line == "[[package]]":
            if current and not (
                current.get("name") == "jcode"
                and current.get("version") == root_version
                and not current.get("source")
            ):
                packages.append(current)
            current = {}
            in_package = True
            continue
        if not in_package or " = " not in line:
            continue
        key, value = line.split(" = ", 1)
        if key in {"name", "version", "source"}:
            current[key] = json.loads(value)

    if current and not (
        current.get("name") == "jcode"
        and current.get("version") == root_version
        and not current.get("source")
    ):
        packages.append(current)

    return sorted(packages, key=lambda item: (item.get("name", ""), item.get("version", ""), item.get("source", "")))


def package_key(package: dict[str, str]) -> tuple[str, str, str]:
    source = package.get("source", "")
    if source.startswith("registry+"):
        source = "registry"
    elif not source:
        source = "workspace"
    return (package.get("name", ""), package.get("version", ""), source)


def component_key(component: dict[str, Any]) -> tuple[str, str, str]:
    external_refs = component.get("externalReferences", [])
    source = ""
    for ref in external_refs:
        if ref.get("type") == "vcs":
            source = ref.get("url", "")
            break
    if not source:
        source = "registry" if component.get("purl") else "workspace"
    return (component.get("name", ""), component.get("version", ""), source)


def verify(provenance_path: Path, sbom_path: Path, expected_path: Path, cargo_lock_path: Path) -> list[str]:
    provenance = load_json(provenance_path)
    sbom = load_json(sbom_path)
    expected = load_json(expected_path)
    errors: list[str] = []

    expect_eq(errors, "provenance.schema", provenance.get("schema"), REQUIRED_PROVENANCE_SCHEMA)
    required_scope = f"packages.{expected['nix_system']}.jcode"
    expect_eq(errors, "provenance.scope.artifact", get_path(provenance, "scope.artifact"), required_scope)
    expect_eq(errors, "provenance.scope.nix_system", get_path(provenance, "scope.nix_system"), expected["nix_system"])
    expect_eq(errors, "provenance.source.full_revision", get_path(provenance, "source.full_revision"), expected["source_full_revision"])
    expect_eq(errors, "provenance.source.display_revision", get_path(provenance, "source.display_revision"), expected["source_display_revision"])
    expect_eq(errors, "provenance.source.flake_lock_sha256", get_path(provenance, "source.flake_lock_sha256"), expected["flake_lock_sha256"])
    expect_eq(errors, "provenance.version.cargo", get_path(provenance, "version.cargo"), expected["cargo_version"])
    expect_eq(errors, "provenance.nix.system", get_path(provenance, "nix.system"), expected["nix_system"])
    expect_eq(errors, "provenance.derivation.drv_path", get_path(provenance, "derivation.drv_path"), expected["drv_path"])
    expect_eq(errors, "provenance.output.store_path", get_path(provenance, "output.store_path"), expected["output_path"])
    expect_eq(errors, "provenance.output.nar_hash", get_path(provenance, "output.nar_hash"), expected["output_nar_hash"])
    expect_eq(errors, "provenance.output.nar_size", get_path(provenance, "output.nar_size"), expected["output_nar_size"])
    expect_eq(errors, "provenance.sbom.sha256", get_path(provenance, "sbom.sha256"), expected["sbom_sha256"])

    expect_eq(errors, "provenance.scope.exclusions", get_path(provenance, "scope.exclusions"), REQUIRED_EXCLUSIONS)
    if get_path(provenance, "release_assets.included") is not False:
        errors.append("release_assets.included must be false")

    expect_eq(errors, "sbom.bomFormat", sbom.get("bomFormat"), "CycloneDX")
    expect_eq(errors, "sbom.specVersion", sbom.get("specVersion"), "1.5")
    expect_eq(errors, "sbom.$schema", sbom.get("$schema"), REQUIRED_SBOM_SCHEMA)
    expect_eq(errors, "sbom.version", sbom.get("version"), 1)
    if "serialNumber" in sbom:
        errors.append("sbom.serialNumber must be omitted for the reproducible companion output")
    expect_eq(errors, "sbom.metadata.component.type", get_path(sbom, "metadata.component.type"), "application")
    expect_eq(errors, "sbom.metadata.component.name", get_path(sbom, "metadata.component.name"), "jcode")
    expect_eq(errors, "sbom.metadata.component.version", get_path(sbom, "metadata.component.version"), expected["cargo_version"])
    root_bom_ref = get_path(sbom, "metadata.component.bom-ref")
    expect_eq(errors, "sbom.metadata.component.bom-ref", root_bom_ref, f"pkg:cargo/jcode@{expected['cargo_version']}")
    expect_eq(errors, "sbom.metadata.tools", get_path(sbom, "metadata.tools"), [{"vendor": "jcode", "name": "nix/sbom.nix"}])

    components = sbom.get("components")
    if not isinstance(components, list) or not components:
        errors.append("sbom.components must be a non-empty list")
        components = []

    seen_refs = {root_bom_ref}
    seen_components: set[tuple[str, str, str]] = set()
    for index, component in enumerate(components):
        if not isinstance(component, dict):
            errors.append(f"sbom.components[{index}] must be an object")
            continue
        expect_eq(errors, f"sbom.components[{index}].type", component.get("type"), "library")
        for field in ("name", "version", "bom-ref"):
            if not isinstance(component.get(field), str) or not component[field]:
                errors.append(f"sbom.components[{index}].{field} must be a non-empty string")
        bom_ref = component.get("bom-ref")
        if isinstance(bom_ref, str):
            if bom_ref in seen_refs:
                errors.append(f"duplicate bom-ref in SBOM: {bom_ref}")
            seen_refs.add(bom_ref)
        seen_components.add(component_key(component))

    for package in parse_cargo_packages(cargo_lock_path, expected["cargo_version"]):
        key = package_key(package)
        if key in seen_components:
            continue
        source = package.get("source", "")
        if source.startswith("git+"):
            errors.append(f"locked git dependency omitted from SBOM: {package['name']} {package['version']} {source}")
        else:
            errors.append(f"locked Cargo package omitted from SBOM: {package['name']} {package['version']} {source or 'workspace'}")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--sbom", type=Path, required=True)
    parser.add_argument("--expected", type=Path, required=True)
    parser.add_argument("--cargo-lock", type=Path, required=True)
    args = parser.parse_args()

    try:
        errors = verify(args.provenance, args.sbom, args.expected, args.cargo_lock)
    except (KeyError, VerificationError, json.JSONDecodeError) as exc:
        errors = [str(exc)]

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print("provenance and SBOM verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
