#!/usr/bin/env python3
"""Run repeated context/cache evaluation matrices locally or on SCO.

This script wraps scripts/context_pipeline_eval.py. It intentionally stays
stdlib-only so it can run on serious-callers-only or inside a VM reached from
that host without extra setup.
"""

from __future__ import annotations

import argparse
import csv
import itertools
import json
import os
import statistics
import subprocess
import time
from pathlib import Path
from typing import Any

DEFAULT_TECHNIQUES = [
    "baseline",
    "stable_tiering",
    "boundary_gate",
    "tool_budget",
    "duplicate_prune",
    "trust_quarantine",
    "rust_skeleton",
    "combined_p0",
]
SCENARIO_KINDS = ("oracle", "negative", "synthetic", "realistic")
DEFAULT_HOST = "serious-callers-only"
NUMERIC_FIELDS = [
    "original_tokens_est",
    "transformed_tokens_est",
    "tokens_saved_est",
    "noise_reduction_ratio",
    "protected_retention_ratio",
    "stale_foreign_retention_ratio",
    "restore_handle_coverage_ratio",
    "placeholders",
    "skeletons",
    "summarized_blocks",
    "latency_ms",
    "practical_score",
]


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def default_out() -> Path:
    return repo_root() / "target" / "context-eval-matrix" / time.strftime("%Y%m%d-%H%M%S")


def parse_csv(path: Path) -> list[dict[str, Any]]:
    with path.open(newline="") as handle:
        rows = list(csv.DictReader(handle))
    for row in rows:
        for field in NUMERIC_FIELDS:
            if field in row and row[field] not in (None, ""):
                try:
                    row[field] = float(row[field])
                except ValueError:
                    pass
    return rows


def save_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def run_command(command: list[str], cwd: Path) -> None:
    print("[run] " + " ".join(command))
    subprocess.run(command, cwd=cwd, check=True)


def assumption_matrix(args: argparse.Namespace) -> list[dict[str, Any]]:
    scenarios = args.scenario_kind
    local_flags = args.include_local_sessions
    budgets = args.tool_budget_chars
    reps = range(args.repetitions)
    matrix = []
    for scenario, include_local, budget, rep in itertools.product(scenarios, local_flags, budgets, reps):
        matrix.append(
            {
                "scenario_kind": scenario,
                "include_local_sessions": include_local,
                "tool_budget_chars": budget,
                "rep": rep,
                "techniques": args.technique or DEFAULT_TECHNIQUES,
            }
        )
    return matrix


def run_one(args: argparse.Namespace, out: Path, assumption: dict[str, Any]) -> list[dict[str, Any]]:
    run_id = (
        f"{assumption['scenario_kind']}-"
        f"local{int(assumption['include_local_sessions'])}-"
        f"budget{assumption['tool_budget_chars']}-"
        f"rep{assumption['rep']:02d}"
    )
    run_out = out / "runs" / run_id
    run_out.mkdir(parents=True, exist_ok=True)
    save_json(run_out / "assumption.json", assumption)

    if args.mode == "remote":
        remote_dir = f"{args.remote_dir.rstrip('/')}/{run_id}"
        command = [
            "python3",
            "scripts/context_pipeline_eval.py",
            "run-remote",
            "--host",
            args.host,
            "--remote-dir",
            remote_dir,
            "--out",
            str(run_out),
            "--scenario-kind",
            assumption["scenario_kind"],
            "--tool-budget-chars",
            str(assumption["tool_budget_chars"]),
        ]
        if args.vm_start_cmd:
            command.extend(["--vm-start-cmd", args.vm_start_cmd])
    else:
        command = [
            "python3",
            "scripts/context_pipeline_eval.py",
            "run-local",
            "--out",
            str(run_out),
            "--scenario-kind",
            assumption["scenario_kind"],
            "--tool-budget-chars",
            str(assumption["tool_budget_chars"]),
        ]

    if assumption["include_local_sessions"]:
        command.append("--include-local-sessions")
    for technique in assumption["techniques"]:
        command.extend(["--technique", technique])

    run_command(command, repo_root())
    rows = parse_csv(run_out / "matrix.csv")
    for row in rows:
        row.update({k: v for k, v in assumption.items() if k != "techniques"})
        row["run_id"] = run_id
        row["mode"] = args.mode
        row["host"] = args.host if args.mode == "remote" else "local"
    save_json(run_out / "annotated_matrix.json", rows)
    return rows


def summarize(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[tuple[Any, ...], list[dict[str, Any]]] = {}
    for row in rows:
        key = (
            row["technique"],
            row["scenario_kind"],
            row["include_local_sessions"],
            row["tool_budget_chars"],
            row["mode"],
            row["host"],
        )
        grouped.setdefault(key, []).append(row)

    summaries: list[dict[str, Any]] = []
    for (technique, scenario, include_local, budget, mode, host), items in grouped.items():
        summary: dict[str, Any] = {
            "technique": technique,
            "scenario_kind": scenario,
            "include_local_sessions": include_local,
            "tool_budget_chars": budget,
            "mode": mode,
            "host": host,
            "runs": len(items),
        }
        for field in NUMERIC_FIELDS:
            vals = [float(item[field]) for item in items if isinstance(item.get(field), (int, float))]
            if not vals:
                continue
            summary[f"{field}_mean"] = round(statistics.mean(vals), 6)
            summary[f"{field}_min"] = round(min(vals), 6)
            summary[f"{field}_max"] = round(max(vals), 6)
            summary[f"{field}_stdev"] = round(statistics.pstdev(vals), 6)
        summary["passes_reliability_gates"] = (
            summary.get("protected_retention_ratio_min", 0.0) >= 0.98
            and summary.get("restore_handle_coverage_ratio_min", 0.0) >= 1.0
            and summary.get("stale_foreign_retention_ratio_max", 1.0) <= 0.0
            and summary.get("latency_ms_max", 10**9) < 100.0
        )
        summaries.append(summary)
    return sorted(summaries, key=lambda item: (item["scenario_kind"], -item.get("practical_score_mean", 0.0), item["technique"]))


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    if not rows:
        return
    fields = sorted({key for row in rows for key in row.keys()})
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def print_summary(summaries: list[dict[str, Any]]) -> None:
    cols = [
        "technique",
        "scenario_kind",
        "include_local_sessions",
        "tool_budget_chars",
        "runs",
        "protected_retention_ratio_min",
        "stale_foreign_retention_ratio_max",
        "latency_ms_max",
        "practical_score_mean",
        "passes_reliability_gates",
    ]
    widths = {col: max(len(col), *(len(str(row.get(col, ""))) for row in summaries)) for col in cols}
    print("  ".join(col.ljust(widths[col]) for col in cols))
    print("  ".join("-" * widths[col] for col in cols))
    for row in summaries:
        print("  ".join(str(row.get(col, "")).ljust(widths[col]) for col in cols))


def run_matrix(args: argparse.Namespace) -> None:
    out = Path(args.out or default_out()).resolve()
    out.mkdir(parents=True, exist_ok=True)
    assumptions = assumption_matrix(args)
    save_json(out / "assumptions.json", assumptions)
    save_json(
        out / "run_config.json",
        {
            "mode": args.mode,
            "host": args.host,
            "remote_dir": args.remote_dir,
            "vm_start_cmd_set": bool(args.vm_start_cmd),
            "assumption_count": len(assumptions),
        },
    )

    all_rows: list[dict[str, Any]] = []
    for idx, assumption in enumerate(assumptions, start=1):
        print(f"\n=== assumption {idx}/{len(assumptions)}: {assumption} ===")
        all_rows.extend(run_one(args, out, assumption))

    summaries = summarize(all_rows)
    save_json(out / "all_rows.json", all_rows)
    save_json(out / "summary.json", summaries)
    write_csv(out / "all_rows.csv", all_rows)
    write_csv(out / "summary.csv", summaries)
    print("\n=== aggregate summary ===")
    print_summary(summaries)
    print(f"\nWrote repeated evaluation matrix to {out}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("local", "remote"), default="local")
    parser.add_argument("--host", default=os.environ.get("JCODE_CONTEXT_EVAL_HOST", DEFAULT_HOST))
    parser.add_argument("--remote-dir", default=os.environ.get("JCODE_CONTEXT_MATRIX_REMOTE_DIR", "/tmp/jcode-context-eval-matrix"))
    parser.add_argument("--vm-start-cmd", default=os.environ.get("JCODE_CONTEXT_EVAL_VM_START_CMD", ""))
    parser.add_argument("--out", default=None)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--scenario-kind", action="append", choices=SCENARIO_KINDS, default=None)
    parser.add_argument("--include-local-sessions", dest="include_local_sessions", action="append", choices=("true", "false"), default=None)
    parser.add_argument("--tool-budget-chars", action="append", type=int, default=None)
    parser.add_argument("--technique", action="append", choices=DEFAULT_TECHNIQUES, default=None)
    parser.set_defaults(func=run_matrix)
    return parser


def normalize_args(args: argparse.Namespace) -> argparse.Namespace:
    if args.repetitions < 1:
        raise SystemExit("--repetitions must be >= 1")
    args.scenario_kind = args.scenario_kind or ["oracle", "negative", "synthetic", "realistic"]
    args.include_local_sessions = [value == "true" for value in (args.include_local_sessions or ["false", "true"])]
    args.tool_budget_chars = args.tool_budget_chars or [2_000, 4_000, 8_000]
    return args


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = normalize_args(parser.parse_args(argv))
    args.func(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
