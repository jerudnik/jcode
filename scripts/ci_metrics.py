#!/usr/bin/env python3
"""Collect read-only CI metrics from the public GitHub API.

The collector keeps to public repository data and writes two outputs when the
caller asks for them:

- a human-readable workflow summary, suitable for GITHUB_STEP_SUMMARY
- a JSON artifact with the raw calculations and SLO targets

The default workflow orientation is the current PR Gate setup, but the script is
generic enough to run against any repository that exposes public Actions and
release data.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Callable, Iterable


DEFAULT_WORKFLOW_NAME = "PR Gate"
DEFAULT_LOOKBACK_DAYS = 30
DEFAULT_RELEASE_LOOKBACK_DAYS = 90
DEFAULT_API_BASE = "https://api.github.com"
DEFAULT_PER_PAGE = 100

SLOS = {
    "pull_request_lead_time_hours": 24.0,
    "required_check_delay_minutes": 30.0,
    "runner_minutes": 30.0,
    "cancellation_rate_pct": 5.0,
    "infrastructure_failures": 0,
    "first_pass_success_pct": 95.0,
    "release_time_hours": 2.0,
}

JsonFetcher = Callable[[str], Any]


@dataclass(frozen=True)
class MetricWindow:
    repo: str
    workflow_name: str
    lookback_days: int
    release_lookback_days: int


def _utc_now() -> datetime:
    return datetime.now(tz=timezone.utc)


def _parse_iso8601(value: str | None) -> datetime | None:
    if not value:
        return None
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def _format_duration_minutes(minutes: float | None) -> str:
    if minutes is None:
        return "n/a"
    if minutes < 1:
        return f"{minutes * 60:.0f}s"
    if minutes >= 60:
        hours = minutes / 60.0
        if hours >= 10:
            return f"{hours:.1f}h"
        return f"{hours:.2f}h"
    return f"{minutes:.1f}m"


def _format_hours(hours: float | None) -> str:
    return "n/a" if hours is None else _format_duration_minutes(hours * 60.0)


def _format_pct(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.1f}%"


def _load_json(url: str, token: str | None = None) -> Any:
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            **({"Authorization": f"Bearer {token}"} if token else {}),
        },
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.loads(response.read().decode("utf-8"))


def make_fetcher(api_base: str, token: str | None = None) -> JsonFetcher:
    def fetch_json(path: str) -> Any:
        url = api_base.rstrip("/") + path
        return _load_json(url, token=token)

    return fetch_json


def _paginated_items(fetch_json: JsonFetcher, path: str, params: dict[str, Any] | None = None) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    page = 1
    base_params = dict(params or {})
    base_params.setdefault("per_page", DEFAULT_PER_PAGE)

    while True:
        query = urllib.parse.urlencode({**base_params, "page": page})
        page_items = fetch_json(f"{path}?{query}")
        if not isinstance(page_items, list):
            raise SystemExit(f"Expected a list from {path}, got {type(page_items).__name__}")
        if not page_items:
            break
        items.extend(page_items)
        if len(page_items) < int(base_params["per_page"]):
            break
        page += 1

    return items


def _run_duration_minutes(run: dict[str, Any]) -> float | None:
    started = _parse_iso8601(run.get("run_started_at")) or _parse_iso8601(run.get("created_at"))
    finished = _parse_iso8601(run.get("updated_at")) or _parse_iso8601(run.get("created_at"))
    if not started or not finished:
        return None
    return max((finished - started).total_seconds() / 60.0, 0.0)


def _job_duration_minutes(job: dict[str, Any], fallback_finish: datetime | None) -> float | None:
    started = _parse_iso8601(job.get("started_at")) or _parse_iso8601(job.get("created_at"))
    finished = _parse_iso8601(job.get("completed_at")) or fallback_finish
    if not started or not finished:
        return None
    return max((finished - started).total_seconds() / 60.0, 0.0)


def _jobs_for_run(fetch_json: JsonFetcher, repo: str, run_id: int) -> list[dict[str, Any]]:
    return _paginated_items(fetch_json, f"/repos/{repo}/actions/runs/{run_id}/jobs")


def _pull_request_runs(fetch_json: JsonFetcher, repo: str, workflow_name: str, lookback_since: datetime) -> list[dict[str, Any]]:
    runs = _paginated_items(fetch_json, f"/repos/{repo}/actions/runs", {"event": "pull_request", "status": "completed"})
    selected: list[dict[str, Any]] = []
    for run in runs:
        created = _parse_iso8601(run.get("created_at"))
        if created and created < lookback_since:
            continue
        if run.get("name") != workflow_name:
            continue
        selected.append(run)
    return selected


def _pull_requests(fetch_json: JsonFetcher, repo: str, lookback_since: datetime) -> list[dict[str, Any]]:
    pulls = _paginated_items(fetch_json, f"/repos/{repo}/pulls", {"state": "closed", "sort": "updated", "direction": "desc"})
    selected: list[dict[str, Any]] = []
    for pull in pulls:
        merged_at = _parse_iso8601(pull.get("merged_at"))
        if merged_at is None or merged_at < lookback_since:
            continue
        selected.append(pull)
    return selected


def _releases(fetch_json: JsonFetcher, repo: str, since: datetime) -> list[dict[str, Any]]:
    releases = _paginated_items(fetch_json, f"/repos/{repo}/releases")
    selected: list[dict[str, Any]] = []
    for release in releases:
        published = _parse_iso8601(release.get("published_at"))
        if published is None or published < since:
            continue
        selected.append(release)
    return selected


def _group_runs_by_pr(runs: Iterable[dict[str, Any]]) -> dict[int, list[dict[str, Any]]]:
    by_pr: dict[int, list[dict[str, Any]]] = {}
    for run in runs:
        for pull in run.get("pull_requests") or []:
            number = pull.get("number")
            if isinstance(number, int):
                by_pr.setdefault(number, []).append(run)
    for pr_runs in by_pr.values():
        pr_runs.sort(key=lambda item: (_parse_iso8601(item.get("created_at")) or datetime.min.replace(tzinfo=timezone.utc)))
    return by_pr


def _summarize_numbers(values: list[float]) -> dict[str, float | None]:
    if not values:
        return {"count": 0, "mean": None, "median": None, "max": None}
    sorted_values = sorted(values)
    count = len(sorted_values)
    mid = count // 2
    median = sorted_values[mid] if count % 2 else (sorted_values[mid - 1] + sorted_values[mid]) / 2.0
    return {
        "count": count,
        "mean": sum(sorted_values) / count,
        "median": median,
        "max": sorted_values[-1],
    }


def collect_metrics(
    window: MetricWindow,
    fetch_json: JsonFetcher,
    *,
    now: datetime | None = None,
) -> dict[str, Any]:
    now = now or _utc_now()
    lookback_since = now - timedelta(days=window.lookback_days)
    release_since = now - timedelta(days=window.release_lookback_days)

    pull_requests = _pull_requests(fetch_json, window.repo, lookback_since)
    pr_runs = _pull_request_runs(fetch_json, window.repo, window.workflow_name, lookback_since)
    releases = _releases(fetch_json, window.repo, release_since)
    runs_by_pr = _group_runs_by_pr(pr_runs)

    pr_lead_times: list[float] = []
    required_check_delays: list[float] = []
    first_pass_succeeded = 0
    prs_with_runs = 0
    missing_runs = 0
    runner_minutes = 0.0
    cancellation_count = 0
    infra_failures = 0
    job_count = 0
    cancelled_job_count = 0
    missing_job_pages = 0

    for run in pr_runs:
        run_conclusion = (run.get("conclusion") or "").lower()
        if run_conclusion == "cancelled":
            cancellation_count += 1
        if run_conclusion in {"startup_failure", "timed_out"}:
            infra_failures += 1

        jobs_url = run.get("jobs_url")
        if jobs_url:
            try:
                jobs = _jobs_for_run(fetch_json, window.repo, int(run["id"]))
            except urllib.error.HTTPError:
                missing_job_pages += 1
                jobs = []
        else:
            jobs = []

        fallback_finish = _parse_iso8601(run.get("updated_at"))
        if jobs:
            for job in jobs:
                job_count += 1
                duration = _job_duration_minutes(job, fallback_finish)
                if duration is not None:
                    runner_minutes += duration
                job_conclusion = (job.get("conclusion") or "").lower()
                if job_conclusion == "cancelled":
                    cancelled_job_count += 1
                if job_conclusion in {"startup_failure", "timed_out"}:
                    infra_failures += 1
        else:
            runner_minutes += _run_duration_minutes(run) or 0.0

    for pull in pull_requests:
        merged_at = _parse_iso8601(pull.get("merged_at"))
        created_at = _parse_iso8601(pull.get("created_at"))
        if created_at and merged_at:
            pr_lead_times.append((merged_at - created_at).total_seconds() / 3600.0)

        pr_number = pull.get("number")
        pr_runs_for_pr = runs_by_pr.get(pr_number, []) if isinstance(pr_number, int) else []
        if not pr_runs_for_pr:
            missing_runs += 1
            continue

        prs_with_runs += 1
        first_run = pr_runs_for_pr[0]
        if (first_run.get("conclusion") or "").lower() == "success":
            first_pass_succeeded += 1

        successful_runs = [run for run in pr_runs_for_pr if (run.get("conclusion") or "").lower() == "success"]
        if successful_runs and created_at:
            first_success = successful_runs[0]
            completed_at = _parse_iso8601(first_success.get("updated_at")) or _parse_iso8601(first_success.get("run_started_at"))
            if completed_at:
                required_check_delays.append((completed_at - created_at).total_seconds() / 60.0)

    release_times: list[float] = []
    for release in releases:
        published = _parse_iso8601(release.get("published_at"))
        created = _parse_iso8601(release.get("created_at"))
        if published and created:
            release_times.append((published - created).total_seconds() / 3600.0)

    pr_lead_stats = _summarize_numbers(pr_lead_times)
    required_check_stats = _summarize_numbers(required_check_delays)
    release_stats = _summarize_numbers(release_times)
    first_pass_rate = (first_pass_succeeded / prs_with_runs * 100.0) if prs_with_runs else None
    cancellation_rate = (cancellation_count / len(pr_runs) * 100.0) if pr_runs else None

    return {
        "window": {
            "repo": window.repo,
            "workflow_name": window.workflow_name,
            "lookback_days": window.lookback_days,
            "release_lookback_days": window.release_lookback_days,
            "generated_at": now.isoformat(),
        },
        "starting_values": {
            "pull_request_lead_time_hours": pr_lead_stats["mean"],
            "required_check_delay_minutes": required_check_stats["mean"],
            "runner_minutes": runner_minutes,
            "cancellation_rate_pct": cancellation_rate,
            "infrastructure_failures": infra_failures,
            "first_pass_success_pct": first_pass_rate,
            "release_time_hours": release_stats["mean"],
        },
        "slo_targets": SLOS,
        "supporting_counts": {
            "pull_requests": len(pull_requests),
            "pr_gate_runs": len(pr_runs),
            "prs_with_runs": prs_with_runs,
            "missing_pr_runs": missing_runs,
            "workflow_jobs": job_count,
            "cancelled_jobs": cancelled_job_count,
            "missing_job_pages": missing_job_pages,
            "releases": len(releases),
        },
        "distribution": {
            "pull_request_lead_time_hours": pr_lead_stats,
            "required_check_delay_minutes": required_check_stats,
            "release_time_hours": release_stats,
        },
    }


def _summary_line(label: str, value: float | None, target: str) -> str:
    return f"- {label}: {_format_duration_minutes(value) if 'minutes' in label.lower() or 'time' in label.lower() and 'lead' not in label.lower() else _format_hours(value)} (target {target})"


def render_summary(report: dict[str, Any]) -> str:
    values = report["starting_values"]
    targets = report["slo_targets"]
    counts = report["supporting_counts"]

    lines = [
        f"## CI metrics for {report['window']['repo']}",
        "",
        f"Workflow: `{report['window']['workflow_name']}` over the last {report['window']['lookback_days']} days.",
        "",
        "Starting values and target SLOs:",
        f"- Pull request lead time: {_format_hours(values['pull_request_lead_time_hours'])} (target <= {_format_hours(targets['pull_request_lead_time_hours'])})",
        f"- Required-check delay: {_format_duration_minutes(values['required_check_delay_minutes'])} (target <= {_format_duration_minutes(targets['required_check_delay_minutes'])})",
        f"- Runner-minutes: {_format_duration_minutes(values['runner_minutes'])} (target <= {_format_duration_minutes(targets['runner_minutes'])})",
        f"- Cancellation: {_format_pct(values['cancellation_rate_pct'])} (target <= {_format_pct(targets['cancellation_rate_pct'])})",
        f"- Infrastructure failures: {values['infrastructure_failures'] if values['infrastructure_failures'] is not None else 'n/a'} (target <= {targets['infrastructure_failures']})",
        f"- First-pass success: {_format_pct(values['first_pass_success_pct'])} (target >= {_format_pct(targets['first_pass_success_pct'])})",
        f"- Release time: {_format_hours(values['release_time_hours'])} (target <= {_format_hours(targets['release_time_hours'])})",
        "",
        "Coverage notes:",
        f"- {counts['pull_requests']} merged PRs, {counts['pr_gate_runs']} completed PR Gate runs, {counts['releases']} releases",
        f"- {counts['missing_pr_runs']} merged PRs without a matching PR Gate run, {counts['cancelled_jobs']} cancelled jobs",
    ]
    return "\n".join(lines).rstrip() + "\n"


def write_outputs(report: dict[str, Any], summary_file: str | None, artifact_file: str | None) -> None:
    summary = render_summary(report)
    if summary_file:
        Path(summary_file).write_text(summary, encoding="utf-8")
    else:
        sys.stdout.write(summary)

    if artifact_file:
        Path(artifact_file).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY", ""), help="repository in owner/name form")
    parser.add_argument("--workflow-name", default=DEFAULT_WORKFLOW_NAME, help="workflow name to inspect for required checks")
    parser.add_argument("--lookback-days", type=int, default=DEFAULT_LOOKBACK_DAYS, help="PR/run lookback window in days")
    parser.add_argument("--release-lookback-days", type=int, default=DEFAULT_RELEASE_LOOKBACK_DAYS, help="release lookback window in days")
    parser.add_argument("--api-base", default=DEFAULT_API_BASE, help="GitHub API base URL")
    parser.add_argument("--summary-file", default=os.environ.get("GITHUB_STEP_SUMMARY", ""), help="path to the workflow summary file")
    parser.add_argument("--artifact-file", default="", help="path to the JSON artifact file")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    if not args.repo:
        print("ci_metrics: --repo or GITHUB_REPOSITORY is required", file=sys.stderr)
        return 2

    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    fetch_json = make_fetcher(args.api_base, token=token)
    window = MetricWindow(
        repo=args.repo,
        workflow_name=args.workflow_name,
        lookback_days=args.lookback_days,
        release_lookback_days=args.release_lookback_days,
    )
    report = collect_metrics(window, fetch_json)
    write_outputs(report, args.summary_file or None, args.artifact_file or None)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
