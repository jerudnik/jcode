#!/usr/bin/env python3
"""Experiment manifest and run registry utilities for context-eval.

This module is intentionally stdlib-only. It does not run model calls or mutate
runtime JCODE state; it records enough metadata to make context-eval runs
repeatable and auditable.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import context_pipeline_eval as pipeline  # noqa: E402

SCHEMA_VERSION = 1
DEFAULT_REGISTRY = Path("target/context-eval/run_registry.jsonl")
DEFAULT_DETERMINISM_ITERATIONS = 5
CACHE_FIXTURE_NAMES = ("repo_alpha", "repo_beta")
REQUIRED_MANIFEST_KEYS = {
    "schema_version",
    "experiment_id",
    "created_at",
    "description",
    "pipeline",
    "scenario",
    "techniques",
    "artifacts",
}
REQUIRED_RUN_KEYS = {
    "schema_version",
    "run_id",
    "registered_at",
    "manifest",
    "artifacts_dir",
    "artifacts",
    "metrics",
}


def utc_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def load_json(path: Path) -> Any:
    return json.loads(path.read_text())


def save_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_value(args: list[str]) -> str | None:
    try:
        completed = subprocess.run(
            ["git", *args],
            cwd=repo_root(),
            check=True,
            text=True,
            capture_output=True,
            timeout=5,
        )
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        return None
    return completed.stdout.strip() or None


def git_metadata() -> dict[str, Any]:
    status = git_value(["status", "--short"])
    return {
        "commit": git_value(["rev-parse", "HEAD"]),
        "branch": git_value(["branch", "--show-current"]),
        "dirty": bool(status),
        "status_short": status.splitlines() if status else [],
    }


def normalize_path(path: str | Path | None) -> str | None:
    if path is None:
        return None
    resolved = Path(path).expanduser().resolve()
    root = repo_root().resolve()
    try:
        return str(resolved.relative_to(root))
    except ValueError:
        return str(resolved)


def artifact_entry(path: Path) -> dict[str, Any]:
    return {
        "path": normalize_path(path),
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
    }


def collect_artifacts(artifacts_dir: Path) -> dict[str, Any]:
    artifacts: dict[str, Any] = {}
    for name in ("matrix.json", "matrix.csv", "summary.json", "summary.csv", "all_rows.json", "all_rows.csv"):
        path = artifacts_dir / name
        if path.exists():
            artifacts[name] = artifact_entry(path)
    scenario_dir = artifacts_dir / "scenarios"
    if scenario_dir.exists():
        artifacts["scenarios"] = [artifact_entry(path) for path in sorted(scenario_dir.glob("*.json"))]
    runs_dir = artifacts_dir / "runs"
    if runs_dir.exists():
        artifacts["contexts"] = [artifact_entry(path) for path in sorted(runs_dir.glob("*.context.json"))]
    model_eval_dir = artifacts_dir / "model_eval"
    if model_eval_dir.exists():
        artifacts["model_eval"] = [artifact_entry(path) for path in sorted(model_eval_dir.glob("*.json"))]
    return artifacts


def infer_techniques(artifacts_dir: Path) -> list[str]:
    matrix_path = artifacts_dir / "matrix.json"
    if not matrix_path.exists():
        matrix_path = artifacts_dir / "summary.json"
    if matrix_path.exists():
        raw = load_json(matrix_path)
        if isinstance(raw, list):
            return [str(item["technique"]) for item in raw if isinstance(item, dict) and item.get("technique")]
    runs_dir = artifacts_dir / "runs"
    if runs_dir.exists():
        return [path.name.removesuffix(".context.json") for path in sorted(runs_dir.glob("*.context.json"))]
    return []


def infer_metrics(artifacts_dir: Path) -> dict[str, Any]:
    matrix_path = artifacts_dir / "matrix.json"
    repeated_summary = False
    if not matrix_path.exists():
        matrix_path = artifacts_dir / "summary.json"
        repeated_summary = matrix_path.exists()
    if not matrix_path.exists():
        return {"matrix_rows": 0, "best_technique": None}
    raw = load_json(matrix_path)
    if not isinstance(raw, list):
        return {"matrix_rows": 0, "best_technique": None}
    best = None
    score_key = "practical_score_mean" if repeated_summary else "practical_score"
    for row in raw:
        if isinstance(row, dict) and (best is None or row.get(score_key, -1) > best.get(score_key, -1)):
            best = row
    return {
        "matrix_rows": len(raw),
        "best_technique": best.get("technique") if best else None,
        "best_practical_score": best.get(score_key) if best else None,
        "repeated_matrix": repeated_summary,
    }


def default_experiment_id(prefix: str) -> str:
    safe = "".join(ch.lower() if ch.isalnum() else "-" for ch in prefix).strip("-") or "context-experiment"
    return f"{safe}-{time.strftime('%Y%m%d-%H%M%S', time.gmtime())}"


def build_manifest(args: argparse.Namespace) -> dict[str, Any]:
    artifacts_dir = Path(args.artifacts_dir).expanduser().resolve() if args.artifacts_dir else None
    techniques = args.technique or (infer_techniques(artifacts_dir) if artifacts_dir else [])
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "experiment_id": args.experiment_id or default_experiment_id(args.title),
        "title": args.title,
        "description": args.description,
        "created_at": utc_now(),
        "owner": args.owner,
        "git": git_metadata(),
        "environment": {
            "python": sys.version.split()[0],
            "platform": platform.platform(),
            "hostname": platform.node(),
        },
        "pipeline": {
            "script": "scripts/context_pipeline_eval.py",
            "scenario_kind": args.scenario_kind,
            "tool_budget_chars": args.tool_budget_chars,
            "include_local_sessions": args.include_local_sessions,
        },
        "scenario": {
            "path": normalize_path(args.scenario) if args.scenario else None,
            "kind": args.scenario_kind,
        },
        "techniques": techniques,
        "model_eval": {
            "enabled": args.model_eval,
            "provider": args.model_provider,
            "model": args.model,
        },
        "artifacts": {
            "directory": normalize_path(artifacts_dir) if artifacts_dir else None,
            "files": collect_artifacts(artifacts_dir) if artifacts_dir and artifacts_dir.exists() else {},
        },
        "notes": args.note or [],
    }
    return manifest


def validate_manifest_data(data: Any) -> list[str]:
    errors: list[str] = []
    if not isinstance(data, dict):
        return ["manifest must be a JSON object"]
    missing = sorted(REQUIRED_MANIFEST_KEYS - data.keys())
    if missing:
        errors.append(f"missing required keys: {', '.join(missing)}")
    if data.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"schema_version must be {SCHEMA_VERSION}")
    if not isinstance(data.get("experiment_id"), str) or not data.get("experiment_id"):
        errors.append("experiment_id must be a non-empty string")
    pipeline = data.get("pipeline")
    if not isinstance(pipeline, dict):
        errors.append("pipeline must be an object")
    else:
        if pipeline.get("scenario_kind") not in {"synthetic", "realistic", None}:
            errors.append("pipeline.scenario_kind must be synthetic or realistic")
        if not isinstance(pipeline.get("tool_budget_chars"), int) or pipeline.get("tool_budget_chars", 0) <= 0:
            errors.append("pipeline.tool_budget_chars must be a positive integer")
    techniques = data.get("techniques")
    if not isinstance(techniques, list) or not all(isinstance(item, str) and item for item in techniques):
        errors.append("techniques must be a list of non-empty strings")
    artifacts = data.get("artifacts")
    if not isinstance(artifacts, dict):
        errors.append("artifacts must be an object")
    elif "files" in artifacts and not isinstance(artifacts["files"], dict):
        errors.append("artifacts.files must be an object")
    return errors


def command_create_manifest(args: argparse.Namespace) -> int:
    manifest = build_manifest(args)
    errors = validate_manifest_data(manifest)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    out = Path(args.out).expanduser().resolve()
    save_json(out, manifest)
    print(out)
    return 0


def command_validate_manifest(args: argparse.Namespace) -> int:
    path = Path(args.manifest).expanduser().resolve()
    errors = validate_manifest_data(load_json(path))
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(f"valid manifest: {path}")
    return 0


def build_run_record(args: argparse.Namespace) -> dict[str, Any]:
    manifest_path = Path(args.manifest).expanduser().resolve()
    manifest = load_json(manifest_path)
    artifacts_dir = Path(args.artifacts_dir).expanduser().resolve()
    run_id = args.run_id or f"{manifest.get('experiment_id', 'context-experiment')}-run-{time.strftime('%Y%m%d-%H%M%S', time.gmtime())}"
    return {
        "schema_version": SCHEMA_VERSION,
        "run_id": run_id,
        "registered_at": utc_now(),
        "manifest": {
            "path": normalize_path(manifest_path),
            "experiment_id": manifest.get("experiment_id"),
            "sha256": sha256_file(manifest_path),
        },
        "artifacts_dir": normalize_path(artifacts_dir),
        "artifacts": collect_artifacts(artifacts_dir),
        "metrics": infer_metrics(artifacts_dir),
        "git": git_metadata(),
        "status": args.status,
        "notes": args.note or [],
    }


def validate_run_record(record: Any) -> list[str]:
    if not isinstance(record, dict):
        return ["run record must be a JSON object"]
    errors: list[str] = []
    missing = sorted(REQUIRED_RUN_KEYS - record.keys())
    if missing:
        errors.append(f"missing required run keys: {', '.join(missing)}")
    if record.get("schema_version") != SCHEMA_VERSION:
        errors.append(f"run schema_version must be {SCHEMA_VERSION}")
    if not isinstance(record.get("run_id"), str) or not record.get("run_id"):
        errors.append("run_id must be a non-empty string")
    if not isinstance(record.get("artifacts"), dict):
        errors.append("artifacts must be an object")
    if not isinstance(record.get("metrics"), dict):
        errors.append("metrics must be an object")
    return errors


def command_register_run(args: argparse.Namespace) -> int:
    manifest_errors = validate_manifest_data(load_json(Path(args.manifest).expanduser().resolve()))
    if manifest_errors:
        for error in manifest_errors:
            print(f"error: manifest {error}", file=sys.stderr)
        return 1
    record = build_run_record(args)
    errors = validate_run_record(record)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    registry = Path(args.registry).expanduser().resolve()
    registry.parent.mkdir(parents=True, exist_ok=True)
    with registry.open("a") as handle:
        handle.write(json.dumps(record, sort_keys=True) + "\n")
    print(json.dumps(record, indent=2, sort_keys=True))
    print(f"registered run in {registry}")
    return 0


def read_registry(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    records = []
    for line_no, line in enumerate(path.read_text().splitlines(), start=1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as exc:
            raise SystemExit(f"invalid registry JSON on line {line_no}: {exc}") from exc
        errors = validate_run_record(record)
        if errors:
            raise SystemExit(f"invalid registry record on line {line_no}: {'; '.join(errors)}")
        records.append(record)
    return records


def command_list_runs(args: argparse.Namespace) -> int:
    records = read_registry(Path(args.registry).expanduser().resolve())
    if args.experiment_id:
        records = [record for record in records if record.get("manifest", {}).get("experiment_id") == args.experiment_id]
    if args.json:
        print(json.dumps(records, indent=2, sort_keys=True))
        return 0
    if not records:
        print("no registered runs")
        return 0
    print("run_id\texperiment_id\tstatus\tbest_technique\tartifacts_dir")
    for record in records[-args.limit :]:
        manifest = record.get("manifest", {})
        metrics = record.get("metrics", {})
        print(
            f"{record.get('run_id')}\t{manifest.get('experiment_id')}\t{record.get('status')}\t"
            f"{metrics.get('best_technique')}\t{record.get('artifacts_dir')}"
        )
    return 0


def canonical_json_bytes(data: Any) -> bytes:
    return (json.dumps(data, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fixture_blocks(repo_id: str, owner: str, branch: str, deploy_rule: str, protected_ticket: str, stale_ticket: str) -> list[dict[str, Any]]:
    """Return context-pipeline blocks that intentionally collide across repos."""
    return [
        {
            "id": "file-src-auth-rs",
            "kind": "file",
            "path": "src/auth.rs",
            "tool": None,
            "status": "raw",
            "trust": "verified",
            "content": (
                f"// {repo_id}: overlapping src/auth.rs fixture\n"
                f"pub const OWNER: &str = \"{owner}\";\n"
                f"pub const DEFAULT_BRANCH: &str = \"{branch}\";\n"
                f"pub const DEPLOY_RULE: &str = \"{deploy_rule}\";\n"
                f"pub const PROTECTED_TICKET: &str = \"{protected_ticket}\";\n"
                "pub fn authenticate(user: &str) -> bool { !user.is_empty() }\n"
            ),
            "metadata": {"fixture": "cache_cross_project", "repo_id": repo_id, "cache_namespace": repo_id},
        },
        {
            "id": "doc-project-state",
            "kind": "file",
            "path": "docs/PROJECT_STATE.md",
            "tool": None,
            "status": "raw",
            "trust": "verified",
            "content": (
                f"# Project State\n\nRepository: {repo_id}\nAuthoritative owner: {owner}\n"
                f"Default branch: {branch}\nProtected deployment fact: {deploy_rule}\n"
                "Never substitute facts from the sibling fake repo.\n"
            ),
            "metadata": {"fixture": "cache_cross_project", "repo_id": repo_id, "cache_namespace": repo_id},
        },
        {
            "id": "task-7-overlap",
            "kind": "message",
            "path": ".backlog/tasks/task-7 - Shared-name.md",
            "tool": None,
            "status": "raw",
            "trust": "verified",
            "content": (
                f"TASK-7 in {repo_id}: preserve {protected_ticket}. "
                f"Deployment instruction is exactly: {deploy_rule}. "
                f"Conflicting sibling ticket {stale_ticket} is stale foreign context here."
            ),
            "metadata": {"fixture": "cache_cross_project", "repo_id": repo_id, "cache_namespace": repo_id},
        },
        {
            "id": "restored-sibling-cache-noise",
            "kind": "tool_output",
            "path": None,
            "tool": "session_restore",
            "status": "raw",
            "trust": "unverified",
            "content": (
                "Restored cache candidate from overlapping repo names. "
                f"If this appears as authoritative for {repo_id}, cache isolation failed. "
                f"Stale sibling fact: {stale_ticket}."
            ),
            "metadata": {"fixture": "cache_cross_project_stale_noise", "repo_id": repo_id},
        },
    ]


def cache_fixture_scenarios() -> dict[str, dict[str, Any]]:
    alpha = ["ALPHA_ONLY_FACT_DEFAULT_BRANCH_MAIN", "ALPHA_DEPLOY_RULE_DO_NOT_PUSH_FROM_CACHE", "TASK-ALPHA-7"]
    beta = ["BETA_ONLY_FACT_DEFAULT_BRANCH_RELEASE", "BETA_DEPLOY_RULE_MANUAL_APPROVAL_REQUIRED", "TASK-BETA-7"]
    return {
        "repo_alpha": {
            "name": "cache_cross_project_repo_alpha",
            "fixture_kind": "cache_cross_project",
            "protected_terms": alpha,
            "stale_foreign_terms": beta,
            "blocks": fixture_blocks("repo_alpha", "alpha-team", alpha[0], alpha[1], alpha[2], beta[2]),
        },
        "repo_beta": {
            "name": "cache_cross_project_repo_beta",
            "fixture_kind": "cache_cross_project",
            "protected_terms": beta,
            "stale_foreign_terms": alpha,
            "blocks": fixture_blocks("repo_beta", "beta-team", beta[0], beta[1], beta[2], alpha[2]),
        },
    }


def command_generate_cache_fixtures(args: argparse.Namespace) -> int:
    out = Path(args.out).expanduser().resolve()
    scenarios_dir = out / "scenarios"
    scenarios = cache_fixture_scenarios()
    manifest: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "name": "cache_cross_project_synthetic_fixtures",
        "description": "Two fake repo/context fixtures with overlapping names and conflicting protected facts.",
        "artifact_layout": {
            "scenarios": "scenarios/*.json",
            "pipeline_runs": "runs/*.context.json after context_pipeline_eval.py run-local",
            "metrics": ["matrix.json", "matrix.csv"],
        },
        "fixtures": [],
    }
    for fixture_name in CACHE_FIXTURE_NAMES:
        scenario = scenarios[fixture_name]
        path = scenarios_dir / f"{scenario['name']}.json"
        save_json(path, scenario)
        manifest["fixtures"].append(
            {
                "repo_id": fixture_name,
                "scenario": str(path.relative_to(out)),
                "overlapping_paths": sorted({block.get("path") for block in scenario["blocks"] if block.get("path")}),
                "protected_terms": scenario["protected_terms"],
                "stale_foreign_terms": scenario["stale_foreign_terms"],
            }
        )
    save_json(out / "cache_cross_project_manifest.json", manifest)
    print(f"wrote cache cross-project fixtures to {out}")
    return 0


def canonicalize_artifact(path: Path, include_latency: bool) -> bytes:
    if path.suffix == ".json":
        data = load_json(path)
        if path.name == "matrix.json" and not include_latency:
            for row in data:
                if isinstance(row, dict):
                    row.pop("latency_ms", None)
        return canonical_json_bytes(data)
    if path.suffix == ".csv":
        rows = list(csv.DictReader(path.open(newline="")))
        if not include_latency:
            for row in rows:
                row.pop("latency_ms", None)
        return canonical_json_bytes(rows)
    return path.read_bytes()


def artifact_digest(root: Path, include_latency: bool) -> dict[str, str]:
    return {
        str(path.relative_to(root)): sha256_bytes(canonicalize_artifact(path, include_latency))
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def comparable_matrix(matrix: list[dict[str, Any]], include_latency: bool) -> list[dict[str, Any]]:
    comparable = []
    for row in matrix:
        item = dict(row)
        if not include_latency:
            item.pop("latency_ms", None)
        comparable.append(item)
    return comparable


def command_determinism(args: argparse.Namespace) -> int:
    scenario = Path(args.scenario).expanduser().resolve()
    out = Path(args.out).expanduser().resolve()
    out.mkdir(parents=True, exist_ok=True)
    techniques = args.technique or pipeline.DEFAULT_TECHNIQUES
    baseline_digest: dict[str, str] | None = None
    baseline_matrix: list[dict[str, Any]] | None = None
    runs = []
    drift = []
    for idx in range(1, args.iterations + 1):
        run_dir = out / f"iteration-{idx:03d}"
        if run_dir.exists():
            shutil.rmtree(run_dir)
        run_dir.mkdir(parents=True)
        matrix = comparable_matrix(pipeline.run_experiment(scenario, run_dir, techniques, args.tool_budget_chars), args.include_latency)
        digest = artifact_digest(run_dir, args.include_latency)
        runs.append({"iteration": idx, "artifact_digest": sha256_bytes(canonical_json_bytes(digest)), "files": digest})
        if baseline_digest is None:
            baseline_digest = digest
            baseline_matrix = matrix
            continue
        if digest != baseline_digest:
            changed = sorted(set(digest) | set(baseline_digest))
            drift.append({"iteration": idx, "drift_files": [name for name in changed if digest.get(name) != baseline_digest.get(name)]})
        if matrix != baseline_matrix:
            drift.append({"iteration": idx, "matrix_drift": True})
    summary = {
        "schema_version": SCHEMA_VERSION,
        "scenario": normalize_path(scenario),
        "iterations": args.iterations,
        "techniques": techniques,
        "include_latency": args.include_latency,
        "stable": not drift,
        "runs": runs,
        "drift": drift,
    }
    save_json(out / "determinism_summary.json", summary)
    if drift:
        print(f"determinism drift detected; wrote {out / 'determinism_summary.json'}")
        return 1
    print(f"determinism stable across {args.iterations} iterations; wrote {out / 'determinism_summary.json'}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    create = sub.add_parser("create-manifest", help="write a context-eval experiment manifest")
    create.add_argument("--out", required=True, help="manifest JSON output path")
    create.add_argument("--title", default="Context evaluation experiment")
    create.add_argument("--experiment-id", default=None)
    create.add_argument("--description", default="Context pipeline evaluation run")
    create.add_argument("--owner", default=os.environ.get("USER") or os.environ.get("USERNAME") or "unknown")
    create.add_argument("--scenario", default=None)
    create.add_argument("--scenario-kind", choices=("synthetic", "realistic"), default="synthetic")
    create.add_argument("--include-local-sessions", action="store_true")
    create.add_argument("--tool-budget-chars", type=int, default=4_000)
    create.add_argument("--technique", action="append", default=None, help="expected technique; repeatable")
    create.add_argument("--artifacts-dir", default=None, help="existing context-eval artifact directory to fingerprint")
    create.add_argument("--model-eval", action="store_true", help="record that model eval is part of this experiment")
    create.add_argument("--model-provider", default=None)
    create.add_argument("--model", default=None)
    create.add_argument("--note", action="append", default=None)
    create.set_defaults(func=command_create_manifest)

    validate = sub.add_parser("validate-manifest", help="validate a manifest JSON file")
    validate.add_argument("manifest")
    validate.set_defaults(func=command_validate_manifest)

    register = sub.add_parser("register-run", help="append a run record to a JSONL registry")
    register.add_argument("--manifest", required=True)
    register.add_argument("--artifacts-dir", required=True)
    register.add_argument("--registry", default=str(DEFAULT_REGISTRY))
    register.add_argument("--run-id", default=None)
    register.add_argument("--status", choices=("planned", "running", "completed", "failed"), default="completed")
    register.add_argument("--note", action="append", default=None)
    register.set_defaults(func=command_register_run)

    list_runs = sub.add_parser("list-runs", help="summarize registered context-eval runs")
    list_runs.add_argument("--registry", default=str(DEFAULT_REGISTRY))
    list_runs.add_argument("--experiment-id", default=None)
    list_runs.add_argument("--limit", type=int, default=20)
    list_runs.add_argument("--json", action="store_true")
    list_runs.set_defaults(func=command_list_runs)

    fixtures = sub.add_parser("generate-cache-fixtures", help="write cache cross-project synthetic scenarios")
    fixtures.add_argument("--out", default="target/context-eval/cache-cross-project")
    fixtures.set_defaults(func=command_generate_cache_fixtures)

    determinism = sub.add_parser("determinism", help="rerun and hash context artifacts to detect ordering/metric drift")
    determinism.add_argument("--scenario", required=True, help="scenario JSON file to replay")
    determinism.add_argument("--out", default="target/context-eval/determinism")
    determinism.add_argument("--iterations", type=int, default=DEFAULT_DETERMINISM_ITERATIONS)
    determinism.add_argument("--tool-budget-chars", type=int, default=4_000)
    determinism.add_argument("--technique", action="append", choices=pipeline.DEFAULT_TECHNIQUES + ["duplicate_prune"], default=None)
    determinism.add_argument("--include-latency", action="store_true", help="include volatile latency_ms in artifact hashes")
    determinism.set_defaults(func=command_determinism)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if getattr(args, "iterations", 1) < 1:
        parser.error("--iterations must be at least 1")
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
