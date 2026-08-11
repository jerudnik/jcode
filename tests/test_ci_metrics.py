#!/usr/bin/env python3
"""Tests for the read-only CI metrics collector.

The collector is intentionally API-driven and read-only, so the tests exercise
the page-walking and aggregation logic with local fixtures instead of live GitHub
calls.
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
import urllib.parse
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))

import ci_metrics as cmi


def iso8601(value: datetime) -> str:
    return value.isoformat().replace("+00:00", "Z")


class FixtureFetcher:
    def __init__(self, responses: dict[tuple[str, int], list[dict[str, object]]]):
        self.responses = responses
        self.calls: list[str] = []

    def __call__(self, path: str):
        self.calls.append(path)
        parsed = urllib.parse.urlparse(path)
        query = urllib.parse.parse_qs(parsed.query)
        page = int(query.get("page", [1])[0])
        return self.responses.get((parsed.path, page), [])


class CiMetricsTests(unittest.TestCase):
    def setUp(self) -> None:
        self._old_per_page = cmi.DEFAULT_PER_PAGE
        cmi.DEFAULT_PER_PAGE = 2

    def tearDown(self) -> None:
        cmi.DEFAULT_PER_PAGE = self._old_per_page

    def test_collect_metrics_paginates_across_pages(self) -> None:
        now = datetime(2026, 8, 8, 12, tzinfo=timezone.utc)
        repo = "octo/ci"

        responses = {
            (f"/repos/{repo}/pulls", 1): [
                {"number": 1, "created_at": iso8601(datetime(2026, 8, 1, 0, tzinfo=timezone.utc)), "merged_at": iso8601(datetime(2026, 8, 2, 0, tzinfo=timezone.utc))},
                {"number": 2, "created_at": iso8601(datetime(2026, 8, 3, 0, tzinfo=timezone.utc)), "merged_at": iso8601(datetime(2026, 8, 3, 12, tzinfo=timezone.utc))},
            ],
            (f"/repos/{repo}/pulls", 2): [
                {"number": 3, "created_at": iso8601(datetime(2026, 8, 4, 0, tzinfo=timezone.utc)), "merged_at": iso8601(datetime(2026, 8, 4, 1, tzinfo=timezone.utc))},
                {"number": 4, "created_at": iso8601(datetime(2026, 4, 20, 0, tzinfo=timezone.utc)), "merged_at": iso8601(datetime(2026, 4, 21, 0, tzinfo=timezone.utc))},
            ],
            (f"/repos/{repo}/pulls", 3): [],
            (f"/repos/{repo}/actions/runs", 1): [
                {
                    "id": 11,
                    "name": "PR Gate",
                    "created_at": iso8601(datetime(2026, 8, 1, 1, tzinfo=timezone.utc)),
                    "run_started_at": iso8601(datetime(2026, 8, 1, 1, 10, tzinfo=timezone.utc)),
                    "updated_at": iso8601(datetime(2026, 8, 1, 1, 40, tzinfo=timezone.utc)),
                    "conclusion": "success",
                    "pull_requests": [{"number": 1}],
                    "jobs_url": f"https://api.github.com/repos/{repo}/actions/runs/11/jobs",
                },
                {
                    "id": 12,
                    "name": "PR Gate",
                    "created_at": iso8601(datetime(2026, 8, 3, 1, tzinfo=timezone.utc)),
                    "run_started_at": iso8601(datetime(2026, 8, 3, 1, 10, tzinfo=timezone.utc)),
                    "updated_at": iso8601(datetime(2026, 8, 3, 1, 30, tzinfo=timezone.utc)),
                    "conclusion": "cancelled",
                    "pull_requests": [{"number": 2}],
                    "jobs_url": f"https://api.github.com/repos/{repo}/actions/runs/12/jobs",
                },
            ],
            (f"/repos/{repo}/actions/runs", 2): [
                {
                    "id": 13,
                    "name": "PR Gate",
                    "created_at": iso8601(datetime(2026, 8, 3, 2, tzinfo=timezone.utc)),
                    "run_started_at": iso8601(datetime(2026, 8, 3, 2, 15, tzinfo=timezone.utc)),
                    "updated_at": iso8601(datetime(2026, 8, 3, 2, 45, tzinfo=timezone.utc)),
                    "conclusion": "success",
                    "pull_requests": [{"number": 2}],
                    "jobs_url": f"https://api.github.com/repos/{repo}/actions/runs/13/jobs",
                },
                {
                    "id": 14,
                    "name": "PR Gate",
                    "created_at": iso8601(datetime(2026, 4, 20, 1, tzinfo=timezone.utc)),
                    "run_started_at": iso8601(datetime(2026, 4, 20, 1, 5, tzinfo=timezone.utc)),
                    "updated_at": iso8601(datetime(2026, 4, 20, 1, 20, tzinfo=timezone.utc)),
                    "conclusion": "success",
                    "pull_requests": [{"number": 4}],
                    "jobs_url": f"https://api.github.com/repos/{repo}/actions/runs/14/jobs",
                },
            ],
            (f"/repos/{repo}/actions/runs", 3): [],
            (f"/repos/{repo}/actions/runs/11/jobs", 1): [
                {"name": "linux", "started_at": iso8601(datetime(2026, 8, 1, 1, 10, tzinfo=timezone.utc)), "completed_at": iso8601(datetime(2026, 8, 1, 1, 20, tzinfo=timezone.utc)), "conclusion": "success"},
                {"name": "macos", "started_at": iso8601(datetime(2026, 8, 1, 1, 20, tzinfo=timezone.utc)), "completed_at": None, "conclusion": "cancelled"},
            ],
            (f"/repos/{repo}/actions/runs/11/jobs", 2): [],
            (f"/repos/{repo}/actions/runs/12/jobs", 1): [
                {"name": "linux", "started_at": iso8601(datetime(2026, 8, 3, 1, 10, tzinfo=timezone.utc)), "completed_at": iso8601(datetime(2026, 8, 3, 1, 20, tzinfo=timezone.utc)), "conclusion": "startup_failure"},
            ],
            (f"/repos/{repo}/actions/runs/12/jobs", 2): [],
            (f"/repos/{repo}/actions/runs/13/jobs", 1): [
                {"name": "linux", "started_at": iso8601(datetime(2026, 8, 3, 2, 15, tzinfo=timezone.utc)), "completed_at": iso8601(datetime(2026, 8, 3, 2, 30, tzinfo=timezone.utc)), "conclusion": "success"},
            ],
            (f"/repos/{repo}/actions/runs/13/jobs", 2): [],
            (f"/repos/{repo}/actions/runs/14/jobs", 1): [
                {"name": "linux", "started_at": iso8601(datetime(2026, 4, 20, 1, 5, tzinfo=timezone.utc)), "completed_at": iso8601(datetime(2026, 4, 20, 1, 20, tzinfo=timezone.utc)), "conclusion": "success"},
            ],
            (f"/repos/{repo}/actions/runs/14/jobs", 2): [],
            (f"/repos/{repo}/releases", 1): [
                {"created_at": iso8601(datetime(2026, 8, 1, 0, tzinfo=timezone.utc)), "published_at": iso8601(datetime(2026, 8, 1, 2, tzinfo=timezone.utc))},
                {"created_at": iso8601(datetime(2026, 8, 3, 0, tzinfo=timezone.utc)), "published_at": iso8601(datetime(2026, 8, 3, 1, tzinfo=timezone.utc))},
            ],
            (f"/repos/{repo}/releases", 2): [
                {"created_at": iso8601(datetime(2026, 8, 4, 0, tzinfo=timezone.utc)), "published_at": iso8601(datetime(2026, 8, 4, 3, tzinfo=timezone.utc))},
                {"created_at": iso8601(datetime(2026, 4, 20, 0, tzinfo=timezone.utc)), "published_at": iso8601(datetime(2026, 4, 20, 1, tzinfo=timezone.utc))},
            ],
            (f"/repos/{repo}/releases", 3): [],
        }

        fetcher = FixtureFetcher(responses)
        window = cmi.MetricWindow(repo=repo, workflow_name="PR Gate", lookback_days=30, release_lookback_days=90)
        report = cmi.collect_metrics(window, fetcher, now=now)

        self.assertEqual(report["supporting_counts"]["pull_requests"], 3)
        self.assertEqual(report["supporting_counts"]["pr_gate_runs"], 3)
        self.assertEqual(report["supporting_counts"]["releases"], 3)
        self.assertEqual(report["supporting_counts"]["missing_pr_runs"], 1)
        self.assertEqual(report["supporting_counts"]["cancelled_jobs"], 1)
        self.assertEqual(report["starting_values"]["infrastructure_failures"], 1)
        self.assertEqual(report["starting_values"]["runner_minutes"], 55.0)
        self.assertEqual(report["starting_values"]["first_pass_success_pct"], 50.0)
        self.assertAlmostEqual(report["starting_values"]["cancellation_rate_pct"], 33.3333333333, places=6)
        self.assertAlmostEqual(report["starting_values"]["pull_request_lead_time_hours"], 12.3333333333, places=6)
        self.assertAlmostEqual(report["starting_values"]["required_check_delay_minutes"], 132.5, places=6)
        self.assertAlmostEqual(report["starting_values"]["release_time_hours"], 2.0, places=6)

        summary = cmi.render_summary(report)
        self.assertIn("Starting values and target SLOs:", summary)
        self.assertIn("Pull request lead time:", summary)
        self.assertIn("Required-check delay:", summary)
        self.assertIn("Runner-minutes:", summary)
        self.assertIn("Cancellation:", summary)
        self.assertIn("Infrastructure failures:", summary)
        self.assertIn("First-pass success:", summary)
        self.assertIn("Release time:", summary)
        self.assertIn("target <=", summary)
        self.assertIn("target >=", summary)

        expected_pages = {
            "/repos/octo/ci/pulls?state=closed&sort=updated&direction=desc&per_page=2&page=2",
            "/repos/octo/ci/actions/runs?event=pull_request&status=completed&per_page=2&page=2",
            "/repos/octo/ci/releases?per_page=2&page=2",
        }
        self.assertTrue(expected_pages.issubset(set(fetcher.calls)))
        self.assertIn("/repos/octo/ci/pulls?state=closed&sort=updated&direction=desc&per_page=2&page=3", set(fetcher.calls))

    def test_collect_metrics_handles_missing_runs_and_cancelled_jobs(self) -> None:
        now = datetime(2026, 8, 8, 12, tzinfo=timezone.utc)
        repo = "octo/ci"

        responses = {
            (f"/repos/{repo}/pulls", 1): [
                {"number": 10, "created_at": iso8601(datetime(2026, 8, 1, 0, tzinfo=timezone.utc)), "merged_at": iso8601(datetime(2026, 8, 2, 0, tzinfo=timezone.utc))},
                {"number": 11, "created_at": iso8601(datetime(2026, 8, 3, 0, tzinfo=timezone.utc)), "merged_at": iso8601(datetime(2026, 8, 3, 6, tzinfo=timezone.utc))},
            ],
            (f"/repos/{repo}/pulls", 2): [],
            (f"/repos/{repo}/actions/runs", 1): [
                {
                    "id": 21,
                    "name": "PR Gate",
                    "created_at": iso8601(datetime(2026, 8, 1, 1, tzinfo=timezone.utc)),
                    "run_started_at": iso8601(datetime(2026, 8, 1, 1, 5, tzinfo=timezone.utc)),
                    "updated_at": iso8601(datetime(2026, 8, 1, 1, 20, tzinfo=timezone.utc)),
                    "conclusion": "success",
                    "pull_requests": [{"number": 10}],
                    "jobs_url": f"https://api.github.com/repos/{repo}/actions/runs/21/jobs",
                }
            ],
            (f"/repos/{repo}/actions/runs", 2): [],
            (f"/repos/{repo}/actions/runs/21/jobs", 1): [
                {"name": "linux", "started_at": iso8601(datetime(2026, 8, 1, 1, 5, tzinfo=timezone.utc)), "completed_at": None, "conclusion": "cancelled"},
            ],
            (f"/repos/{repo}/actions/runs/21/jobs", 2): [],
            (f"/repos/{repo}/releases", 1): [],
        }

        fetcher = FixtureFetcher(responses)
        window = cmi.MetricWindow(repo=repo, workflow_name="PR Gate", lookback_days=30, release_lookback_days=90)
        report = cmi.collect_metrics(window, fetcher, now=now)

        self.assertEqual(report["supporting_counts"]["pull_requests"], 2)
        self.assertEqual(report["supporting_counts"]["missing_pr_runs"], 1)
        self.assertEqual(report["supporting_counts"]["cancelled_jobs"], 1)
        self.assertEqual(report["starting_values"]["runner_minutes"], 15.0)
        self.assertEqual(report["starting_values"]["cancellation_rate_pct"], 0.0)
        self.assertEqual(report["starting_values"]["first_pass_success_pct"], 100.0)

        with tempfile.TemporaryDirectory() as tmpdir:
            summary_path = Path(tmpdir) / "summary.md"
            artifact_path = Path(tmpdir) / "ci-metrics.json"
            cmi.write_outputs(report, str(summary_path), str(artifact_path))

            summary = summary_path.read_text(encoding="utf-8")
            artifact = json.loads(artifact_path.read_text(encoding="utf-8"))

        self.assertIn("CI metrics for octo/ci", summary)
        self.assertIn("Starting values and target SLOs:", summary)
        self.assertEqual(artifact["window"]["repo"], repo)
        self.assertEqual(artifact["slo_targets"], cmi.SLOS)
        self.assertEqual(artifact["supporting_counts"]["missing_pr_runs"], 1)


if __name__ == "__main__":
    unittest.main()
