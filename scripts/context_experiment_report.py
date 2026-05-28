#!/usr/bin/env python3
"""Generate HTML/Markdown reports for context-eval artifacts.

The reporter is intentionally stdlib-only. It consumes deterministic pipeline
artifacts from scripts/context_pipeline_eval.py (matrix.json) and optional model
evaluation artifacts from scripts/context_model_eval.py (model_eval/results.json
and summary.json by default), then writes shareable reports with aggregate
metrics, gate failures, and a recommendation.
"""

from __future__ import annotations

import argparse
import html
import json
import statistics
import time
from pathlib import Path
from typing import Any

DETERMINISTIC_FIELDS = [
    "technique",
    "practical_score",
    "tokens_saved_est",
    "noise_reduction_ratio",
    "protected_retention_ratio",
    "stale_foreign_retention_ratio",
    "restore_handle_coverage_ratio",
    "placeholders",
    "skeletons",
    "summarized_blocks",
    "latency_ms",
]

MODEL_FIELDS = [
    "technique",
    "calls",
    "passed",
    "failed",
    "pass_rate",
    "avg_latency_ms",
    "forbidden_hits",
]


class ReportError(RuntimeError):
    """Raised for user-actionable report generation failures."""


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise ReportError(f"missing required artifact: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ReportError(f"invalid JSON artifact {path}: {exc}") from exc


def save_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def as_float(value: Any, default: float = 0.0) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def percent(value: Any) -> str:
    return f"{as_float(value) * 100:.1f}%"


def number(value: Any) -> str:
    if isinstance(value, float):
        return f"{value:.3f}".rstrip("0").rstrip(".")
    return str(value)


def find_default_model_dir(root: Path) -> Path | None:
    candidate = root / "model_eval"
    if (candidate / "results.json").exists():
        return candidate
    matches = sorted(root.rglob("results.json"))
    return matches[0].parent if matches else None


def summarize_model_results(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    for row in rows:
        grouped.setdefault(str(row.get("technique") or "unknown"), []).append(row)

    summaries: list[dict[str, Any]] = []
    for technique, items in grouped.items():
        passed = sum(1 for item in items if bool(item.get("passed")))
        failed = len(items) - passed
        latencies = [as_float(item.get("latency_ms")) for item in items if item.get("latency_ms") is not None]
        forbidden_hits = sorted({str(hit) for item in items for hit in item.get("forbidden_hits", [])})
        summaries.append(
            {
                "technique": technique,
                "calls": len(items),
                "passed": passed,
                "failed": failed,
                "pass_rate": passed / max(1, len(items)),
                "avg_latency_ms": round(statistics.mean(latencies), 3) if latencies else 0.0,
                "forbidden_hits": forbidden_hits,
            }
        )
    return sorted(summaries, key=lambda item: (item["pass_rate"], -item["failed"], item["technique"]), reverse=True)


def deterministic_failures(rows: list[dict[str, Any]], args: argparse.Namespace) -> list[str]:
    failures: list[str] = []
    for row in rows:
        technique = row.get("technique", "unknown")
        if as_float(row.get("protected_retention_ratio")) < args.min_protected_retention:
            failures.append(f"{technique}: protected retention {percent(row.get('protected_retention_ratio'))} below {percent(args.min_protected_retention)}")
        if as_float(row.get("stale_foreign_retention_ratio")) > args.max_stale_retention:
            failures.append(f"{technique}: stale/foreign retention {percent(row.get('stale_foreign_retention_ratio'))} above {percent(args.max_stale_retention)}")
        if as_float(row.get("restore_handle_coverage_ratio"), 1.0) < args.min_restore_coverage:
            failures.append(f"{technique}: restore handle coverage {percent(row.get('restore_handle_coverage_ratio'))} below {percent(args.min_restore_coverage)}")
    return failures


def model_failures(rows: list[dict[str, Any]], args: argparse.Namespace) -> list[str]:
    failures: list[str] = []
    for row in rows:
        technique = row.get("technique", "unknown")
        if as_float(row.get("pass_rate")) < args.min_model_pass_rate:
            failures.append(f"{technique}: model pass rate {percent(row.get('pass_rate'))} below {percent(args.min_model_pass_rate)}")
        hits = row.get("forbidden_hits") or []
        if hits:
            failures.append(f"{technique}: forbidden model hits present: {', '.join(map(str, hits))}")
    return failures


def choose_recommendation(deterministic: list[dict[str, Any]], model: list[dict[str, Any]]) -> str:
    model_by_technique = {row["technique"]: row for row in model}

    def rank(row: dict[str, Any]) -> tuple[float, float, float, float]:
        technique = str(row.get("technique"))
        model_row = model_by_technique.get(technique, {})
        return (
            as_float(model_row.get("pass_rate"), 1.0 if not model else 0.0),
            -as_float(row.get("stale_foreign_retention_ratio")),
            as_float(row.get("protected_retention_ratio")),
            as_float(row.get("practical_score")),
        )

    if not deterministic:
        return "No deterministic matrix rows were available, so no recommendation can be made."
    best = max(deterministic, key=rank)
    parts = [f"Recommend `{best.get('technique')}`"]
    parts.append(f"practical score {number(best.get('practical_score'))}")
    parts.append(f"protected retention {percent(best.get('protected_retention_ratio'))}")
    parts.append(f"stale/foreign retention {percent(best.get('stale_foreign_retention_ratio'))}")
    if best.get("technique") in model_by_technique:
        parts.append(f"model pass rate {percent(model_by_technique[str(best.get('technique'))].get('pass_rate'))}")
    return "; ".join(parts) + "."


def markdown_table(rows: list[dict[str, Any]], fields: list[str]) -> str:
    if not rows:
        return "_No rows available._\n"
    lines = ["| " + " | ".join(fields) + " |", "| " + " | ".join("---" for _ in fields) + " |"]
    for row in rows:
        values = []
        for field in fields:
            value = row.get(field, "")
            if field.endswith("ratio") or field == "pass_rate":
                value = percent(value)
            elif isinstance(value, list):
                value = ", ".join(map(str, value)) or "-"
            else:
                value = number(value)
            values.append(str(value).replace("|", "\\|"))
        lines.append("| " + " | ".join(values) + " |")
    return "\n".join(lines) + "\n"


def html_table(rows: list[dict[str, Any]], fields: list[str]) -> str:
    if not rows:
        return "<p><em>No rows available.</em></p>"
    head = "".join(f"<th>{html.escape(field)}</th>" for field in fields)
    body_rows = []
    for row in rows:
        cells = []
        for field in fields:
            value = row.get(field, "")
            if field.endswith("ratio") or field == "pass_rate":
                value = percent(value)
            elif isinstance(value, list):
                value = ", ".join(map(str, value)) or "-"
            else:
                value = number(value)
            cells.append(f"<td>{html.escape(str(value))}</td>")
        body_rows.append("<tr>" + "".join(cells) + "</tr>")
    return f"<table><thead><tr>{head}</tr></thead><tbody>{''.join(body_rows)}</tbody></table>"


def render_markdown(report: dict[str, Any]) -> str:
    gate_failures = report["gate_failures"]
    failure_text = "\n".join(f"- {item}" for item in gate_failures) if gate_failures else "- None"
    return f"""# Context Experiment Report

Generated: {report['generated_at']}
Artifacts: `{report['artifacts']}`

## Recommendation

{report['recommendation']}

## Gate Failures

{failure_text}

## Deterministic Metrics

{markdown_table(report['deterministic'], DETERMINISTIC_FIELDS)}

## Model Metrics

{markdown_table(report['model'], MODEL_FIELDS)}
"""


def render_html(report: dict[str, Any]) -> str:
    gate_items = "".join(f"<li>{html.escape(item)}</li>" for item in report["gate_failures"]) or "<li>None</li>"
    return f"""<!doctype html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\">
<title>Context Experiment Report</title>
<style>
body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; margin: 2rem; line-height: 1.45; }}
table {{ border-collapse: collapse; width: 100%; margin: 1rem 0 2rem; font-size: 0.9rem; }}
th, td {{ border: 1px solid #ddd; padding: 0.45rem 0.6rem; text-align: left; vertical-align: top; }}
th {{ background: #f5f5f5; }}
code {{ background: #f5f5f5; padding: 0.1rem 0.25rem; }}
</style>
</head>
<body>
<h1>Context Experiment Report</h1>
<p><strong>Generated:</strong> {html.escape(report['generated_at'])}<br>
<strong>Artifacts:</strong> <code>{html.escape(report['artifacts'])}</code></p>
<h2>Recommendation</h2>
<p>{html.escape(report['recommendation'])}</p>
<h2>Gate Failures</h2>
<ul>{gate_items}</ul>
<h2>Deterministic Metrics</h2>
{html_table(report['deterministic'], DETERMINISTIC_FIELDS)}
<h2>Model Metrics</h2>
{html_table(report['model'], MODEL_FIELDS)}
</body>
</html>
"""


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    artifacts = Path(args.artifacts).resolve()
    matrix_path = Path(args.matrix).resolve() if args.matrix else artifacts / "matrix.json"
    deterministic = load_json(matrix_path)
    if not isinstance(deterministic, list):
        raise ReportError(f"matrix artifact must be a JSON list: {matrix_path}")
    deterministic = sorted(deterministic, key=lambda row: as_float(row.get("practical_score")), reverse=True)

    model_dir = Path(args.model_eval).resolve() if args.model_eval else find_default_model_dir(artifacts)
    model_results: list[dict[str, Any]] = []
    if model_dir is not None and (model_dir / "results.json").exists():
        raw_model = load_json(model_dir / "results.json")
        if not isinstance(raw_model, list):
            raise ReportError(f"model results artifact must be a JSON list: {model_dir / 'results.json'}")
        model_results = summarize_model_results(raw_model)

    gate_failures = deterministic_failures(deterministic, args) + model_failures(model_results, args)
    return {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "artifacts": str(artifacts),
        "deterministic": deterministic,
        "model": model_results,
        "gate_failures": gate_failures,
        "recommendation": choose_recommendation(deterministic, model_results),
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", required=True, help="Context eval artifact root containing matrix.json")
    parser.add_argument("--out", default=None, help="Output directory for report.md/report.html, defaults to artifacts/report")
    parser.add_argument("--matrix", default=None, help="Override deterministic matrix.json path")
    parser.add_argument("--model-eval", default=None, help="Optional model_eval directory containing results.json")
    parser.add_argument("--format", choices=("markdown", "html", "both"), default="both")
    parser.add_argument("--min-protected-retention", type=float, default=1.0)
    parser.add_argument("--max-stale-retention", type=float, default=0.0)
    parser.add_argument("--min-restore-coverage", type=float, default=1.0)
    parser.add_argument("--min-model-pass-rate", type=float, default=1.0)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        report = build_report(args)
    except ReportError as exc:
        parser.error(str(exc))
    out = Path(args.out).resolve() if args.out else Path(args.artifacts).resolve() / "report"
    written: list[Path] = []
    if args.format in {"markdown", "both"}:
        path = out / "report.md"
        save_text(path, render_markdown(report))
        written.append(path)
    if args.format in {"html", "both"}:
        path = out / "report.html"
        save_text(path, render_html(report))
        written.append(path)
    print(json.dumps({"written": [str(path) for path in written], "gate_failures": len(report["gate_failures"]), "recommendation": report["recommendation"]}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
