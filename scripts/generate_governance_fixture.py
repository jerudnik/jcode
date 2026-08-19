#!/usr/bin/env python3
"""Regenerate a valid governance fixture from the manifest and workflow text.

Writes a current fixture (default `target/fork-health/governance-valid.json`)
for offline `scripts/fork-health.sh --fixture` runs. No fixture is checked in;
the generated file is derived state, rebuilt on demand.

Regenerate after changing the manifest or the required-context workflows:

    python3 scripts/generate_governance_fixture.py \
      --workflows-dir .github/workflows \
      --output target/fork-health/governance-valid.json

`--workflows-dir` must point at the workflows to check.
"""

from __future__ import annotations

import argparse
import copy
import json
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MANIFEST = REPO_ROOT / "scripts" / "required-checks.json"
DEFAULT_OUTPUT = (
    REPO_ROOT / "target" / "fork-health" / "governance-valid.json"
)


def build(manifest: dict[str, Any], workflows_dir: Path) -> dict[str, Any]:
    rulesets = [copy.deepcopy(body) for _, body in sorted(manifest["rulesets"].items())]

    target = manifest["target_branch"]
    effective: list[dict[str, str]] = []
    seen: set[str] = set()
    for body in rulesets:
        ref = body["conditions"]["ref_name"]
        include, exclude = ref.get("include", []), ref.get("exclude", [])
        applies = f"refs/heads/{target}" in include or (
            "~ALL" in include and f"refs/heads/{target}" not in exclude
        )
        if not applies or body["enforcement"] != "active":
            continue
        for rule in body["rules"]:
            if rule["type"] not in seen:
                seen.add(rule["type"])
                effective.append({"type": rule["type"]})

    workflows = {}
    for entry in sorted(workflows_dir.glob("*.yml")):
        workflows[f".github/workflows/{entry.name}"] = entry.read_text(encoding="utf-8")

    contract_files = {c["file"] for c in manifest["workflow_contracts"]}
    missing = sorted(contract_files - set(workflows))
    if missing:
        raise SystemExit(f"workflow directory is missing contract file(s): {missing}")

    repo = {
        "id": manifest["repository_id"],
        "full_name": manifest["repository"],
    }
    repo.update(manifest["repository_merge_methods"])

    return {
        "_comment": (
            "Valid governance fixture in the aggregate snapshot shape "
            "scripts/governance_compare.py consumes. Regenerate with "
            "scripts/generate_governance_fixture.py. Tests deep-copy this object, "
            "mutate one property, and assert the comparator goes red."
        ),
        "repository": repo,
        "rulesets": rulesets,
        "effective_main_rules": effective,
        "classic_branch_protection": None,
        "branches": ["main"],
        "workflows": workflows,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--workflows-dir", type=Path, default=REPO_ROOT / ".github" / "workflows")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args(argv)

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    fixture = build(manifest, args.workflows_dir)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(fixture, indent=2, sort_keys=False) + "\n", encoding="utf-8")
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
