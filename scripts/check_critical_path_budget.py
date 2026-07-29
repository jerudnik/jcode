#!/usr/bin/env python3
"""Enforce zero-growth critical-path quality ceilings and report the debt trend.

Why this exists on top of the repository-wide ratchets
------------------------------------------------------
`check_panic_budget.py`, `check_swallowed_error_budget.py` and
`check_code_size_budget.py` already refuse to let debt grow *in the working
tree*: a new production file with a panic is rejected, a tracked file may not
grow, and the totals may not increase. What they do not do is bound how far a
baseline may be moved. Their baselines
(`panic_budget.json`, `swallowed_error_budget.json`, `code_size_budget.json`,
`test_size_budget.json`, `warning_budget.txt`) are deliberately *unprotected*
governance-wise, so that routine tightening after a cleanup needs no
maintenance window (see
`docs/fork/ideal-base/evidence/R07/integration-adjudication.md`). The accepted
cost was that a *raise* is only "visible in review".

That trade is fine for the repository at large. It is not fine for the paths
acceptance standard A6 calls critical: lifecycle, persistence, updater,
provider-infrastructure and TUI. For those, this script records an explicit
machine-readable scope plus a per-domain ceiling, and CI pins the whole data
block by digest from the protected `.github/workflows/fork-ci.yml`. Weakening
anything here - raising a ceiling, shrinking the scope, relaxing a downward
target, or loosening the oversize threshold - therefore requires editing two
protected paths and turns `Governance Root` red, which is exactly the reviewed
maintenance window such a change deserves. Tightening needs no edit at all:
ceilings are high-water marks, so cleanup simply opens headroom, and the
report records the real current value and the distance to target.

Policy
------
- Existing critical-path debt is grandfathered by the ceilings. No all-at-once
  cleanup is demanded.
- Critical-path panic-prone, swallowed-error and oversize counts may not exceed
  their per-domain ceiling. Debt cannot be shuffled between domains.
- Downward targets are recorded per domain and reported as distance-to-target.
  They are goals, not gates, so they never block unrelated work.
- Non-critical paths are not gated here. New debt there is caught by the
  repository-wide ratchets, and the aggregate trend is reported below.

Usage
-----
    python3 scripts/check_critical_path_budget.py \
        --expect-digest <sha256> --report <path>

`--print-digest` emits the current digest, for use when a ceiling or the scope
legitimately changes inside a maintenance window.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

# These scripts are invoked as `python3 scripts/...`, which puts the scripts
# directory on sys.path and makes this sibling import available.
from rust_production_filter import production_lines, production_rust_files

REPO_ROOT = Path(__file__).resolve().parent.parent
CODE_SIZE_BASELINE = REPO_ROOT / "scripts" / "code_size_budget.json"
PANIC_BASELINE = REPO_ROOT / "scripts" / "panic_budget.json"
SWALLOWED_BASELINE = REPO_ROOT / "scripts" / "swallowed_error_budget.json"
TEST_SIZE_BASELINE = REPO_ROOT / "scripts" / "test_size_budget.json"
WARNING_BASELINE = REPO_ROOT / "scripts" / "warning_budget.txt"

PANIC_PATTERN = re.compile(r"\.unwrap\(|\.expect\(|\b(?:panic!|todo!|unimplemented!)")
SWALLOWED_PATTERNS = (
    re.compile(r"\blet\s+_\s*="),
    re.compile(r"\.ok\(\)"),
    re.compile(r"\.unwrap_or_default\(\)"),
)

# ---------------------------------------------------------------------------
# Pinned data block. Everything below up to DIGEST_FIELDS is covered by the
# digest CI pins, so it cannot be weakened without a maintenance window.
# ---------------------------------------------------------------------------

# Oversize threshold. Asserted equal to code_size_budget.json's threshold_loc so
# that raising the (unprotected) baseline threshold cannot silently retire the
# oversize dimension for critical or non-critical paths alike.
OVERSIZE_THRESHOLD_LOC = 1200

# A6: "Critical lifecycle, persistence, updater, provider-infrastructure, and
# TUI paths". Each domain below names the concrete prefixes that implement it in
# this crate layout. Prefixes are matched in declaration order and each file is
# attributed to exactly one domain, so totals never double count.
CRITICAL_PATHS: dict[str, list[str]] = {
    # A0/A1 runtime ownership and work-aware lifetime: the daemon server module
    # holds shutdown, lifecycle, client lifecycle, runtime, reload, socket and
    # lease-bearing code; jcode-core holds process/panic/activity primitives.
    "lifecycle": [
        "crates/jcode-app-core/src/server/",
        "crates/jcode-core/",
    ],
    # A2/A7 durable background, recovery and telemetry marker state.
    "persistence": [
        "crates/jcode-app-core/src/restart_snapshot.rs",
        "crates/jcode-storage/",
        "crates/jcode-session-types/",
        "crates/jcode-background-types/",
        "crates/jcode-telemetry-core/",
    ],
    # A5 package and updater integrity, including the selfdev activation path
    # that performs the same acquire/activate/rollback role in-tree.
    "updater": [
        "crates/jcode-app-core/src/update.rs",
        "crates/jcode-app-core/src/tool/selfdev/",
        "src/cli/selfdev.rs",
    ],
    # Provider *infrastructure*: transport, selection, failover, retry, auth and
    # metadata shared by every vendor adapter. Individual vendor adapters
    # (jcode-provider-openai, -anthropic, ...) are deliberately excluded; they
    # are leaf integrations, not infrastructure, and remain covered by the
    # repository-wide ratchets.
    "provider_infrastructure": [
        "crates/jcode-provider-core/",
        "crates/jcode-provider-env/",
        "crates/jcode-provider-metadata/",
        "crates/jcode-auth-types/",
    ],
    # The TUI surface this fork actually ships, plus its core and render
    # primitives. Leaf widget crates stay on the repository-wide ratchets.
    "tui": [
        "crates/jcode-tui/",
        "crates/jcode-tui-core/",
        "crates/jcode-tui-render/",
    ],
}

# Zero-growth ceilings. These are the observed counts at the commit that
# introduced this gate; existing debt is grandfathered, growth is not.
CEILINGS: dict[str, dict[str, int]] = {
    "lifecycle": {"panic": 11, "swallowed_error": 441, "oversize_files": 10},
    "persistence": {"panic": 1, "swallowed_error": 85, "oversize_files": 1},
    "updater": {"panic": 0, "swallowed_error": 22, "oversize_files": 0},
    "provider_infrastructure": {"panic": 0, "swallowed_error": 16, "oversize_files": 1},
    "tui": {"panic": 8, "swallowed_error": 597, "oversize_files": 33},
}

# Explicit downward targets. Reported, never gated: A6 asks for explicit
# downward targets, and F23's contract is explicitly "without demanding an
# all-at-once cleanup". A target is the value the domain should reach, with the
# reason that value and not zero.
TARGETS: dict[str, dict[str, Any]] = {
    "lifecycle": {
        "panic": 0,
        "swallowed_error": 220,
        "oversize_files": 5,
        "rationale": "A0/A1 shutdown and lease code must not be able to abort the "
        "daemon; panics go to zero. Halve swallowed-error and oversize debt as the "
        "server module is decomposed.",
    },
    "persistence": {
        "panic": 0,
        "swallowed_error": 42,
        "oversize_files": 0,
        "rationale": "A2 requires error-aware durable writes, so a discarded result "
        "on this path is a lost-state bug; halve it, and drive panics and the one "
        "oversize file (jcode-telemetry-core/src/lib.rs) to zero.",

    },
    "updater": {
        "panic": 0,
        "swallowed_error": 11,
        "oversize_files": 0,
        "rationale": "A5 requires every failed activation to preserve the prior "
        "runtime, which a swallowed error defeats; halve it and hold the rest at "
        "zero.",
    },
    "provider_infrastructure": {
        "panic": 0,
        "swallowed_error": 8,
        "oversize_files": 0,
        "rationale": "Shared transport/selection/failover code is reached by every "
        "provider, so its debt is multiplied; drive to near zero and split the one "
        "oversize file.",
    },
    "tui": {
        "panic": 0,
        "swallowed_error": 300,
        "oversize_files": 20,
        "rationale": "Largest surface and the one A4 only recently made "
        "deterministically testable; panics to zero, and roughly halve the "
        "swallowed-error and oversize debt over the decomposition program.",
    },
}

# Expected number of in-scope production files per domain.
#
# Without this, the ceilings have a shrinking denominator: moving a file out of a
# critical directory removes its debt from the domain, and a count-only check
# reads that as cleanup. An independent review demonstrated it - `git mv`-ing
# server/shutdown.rs out of the critical set dropped lifecycle/panic 11 -> 3 and
# the gate praised it as "headroom from prior cleanup". Debt that LEAVES the
# critical set must not be indistinguishable from debt that was FIXED.
#
# An unexplained decrease therefore fails. An increase is fine and expected:
# adding files to a critical domain is normal work, and their debt is still
# bounded by the ceilings. Legitimate removals (a genuine deletion, or a
# refactor that moves code out of scope) are recorded here inside a maintenance
# window, which is exactly the review such a scope change deserves.
EXPECTED_FILE_COUNTS: dict[str, int] = {
    "lifecycle": 62,
    "persistence": 10,
    "updater": 8,
    "provider_infrastructure": 19,
    "tui": 191,
}

# Repository-wide high-water marks, read from the five ratchet baselines.
#
# The existing ratchets already refuse growth *in the working tree*. What they
# cannot bound is how far their own (deliberately unprotected) baseline may be
# moved. These marks close that: a baseline may be lowered freely, because a
# lower value stays under its mark and needs no edit here, but it cannot be
# raised without also editing this protected script and the protected workflow
# pin. That is the intended asymmetry - tightening is frictionless, loosening is
# reviewed - and it is what makes "the repository debt trend cannot increase"
# true of the recorded budget and not merely of one working tree.
REPOSITORY_CEILINGS: dict[str, int] = {
    "panic": 56,
    "swallowed_error": 3032,
    "oversize_files": 100,
    "oversize_total_loc": 200999,
    "test_oversize_files": 37,
    "warnings": 0,
}

DIGEST_FIELDS = (
    "oversize_threshold_loc",
    "critical_paths",
    "ceilings",
    "targets",
    "expected_file_counts",
    "repository_ceilings",
)

DIMENSIONS = ("panic", "swallowed_error", "oversize_files")


def pinned_data() -> dict[str, Any]:
    return {
        "oversize_threshold_loc": OVERSIZE_THRESHOLD_LOC,
        "critical_paths": CRITICAL_PATHS,
        "ceilings": CEILINGS,
        "targets": TARGETS,
        "expected_file_counts": EXPECTED_FILE_COUNTS,
        "repository_ceilings": REPOSITORY_CEILINGS,
    }


def scope_digest() -> str:
    payload = json.dumps(pinned_data(), sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--expect-digest",
        help="fail unless the pinned scope/ceiling/target block hashes to this value",
    )
    parser.add_argument("--report", help="write the trend report JSON to this path")
    parser.add_argument(
        "--print-digest",
        action="store_true",
        help="print the current scope digest and exit",
    )
    return parser.parse_args()


def domain_for(rel: str) -> str | None:
    for domain, prefixes in CRITICAL_PATHS.items():
        for prefix in prefixes:
            if rel == prefix or (prefix.endswith("/") and rel.startswith(prefix)):
                return domain
    return None


def zero_domain_counts() -> dict[str, dict[str, int]]:
    return {domain: {dim: 0 for dim in DIMENSIONS} for domain in CRITICAL_PATHS}


def rust_file_line_count(path: Path) -> int:
    with path.open("r", encoding="utf-8") as handle:
        return sum(1 for _ in handle)


class Measurement:
    """One scan of the critical scope, retaining per-file detail.

    The scan reads and masks every in-scope Rust file, so it is the expensive
    part of this check. Failure reporting reuses this result rather than
    rescanning, which keeps a red run as cheap as a green one.
    """

    def __init__(self) -> None:
        self.counts = zero_domain_counts()
        self.oversize_files: dict[str, list[str]] = {domain: [] for domain in CRITICAL_PATHS}
        self.per_file: dict[str, list[tuple[str, int]]] = {
            f"{domain}/{dim}": []
            for domain in CRITICAL_PATHS
            for dim in ("panic", "swallowed_error")
        }
        self.file_counts: dict[str, int] = {domain: 0 for domain in CRITICAL_PATHS}
        self.scanned = 0

    def contributors(self, domain: str, dimension: str) -> list[str]:
        """Name the worst contributors so a red gate is actionable."""

        if dimension == "oversize_files":
            return sorted(self.oversize_files[domain])
        hits = sorted(self.per_file[f"{domain}/{dimension}"], reverse=True, key=lambda x: x[1])
        return [f"{rel} ({count})" for rel, count in hits[:10]]


def measure() -> Measurement:
    result = Measurement()
    for path in production_rust_files():
        rel = path.relative_to(REPO_ROOT).as_posix()
        domain = domain_for(rel)
        if domain is None:
            continue
        result.scanned += 1
        result.file_counts[domain] += 1
        lines = list(production_lines(path))
        panics = sum(1 for line in lines if PANIC_PATTERN.search(line))
        swallowed = sum(
            1 for line in lines if any(pattern.search(line) for pattern in SWALLOWED_PATTERNS)
        )
        result.counts[domain]["panic"] += panics
        result.counts[domain]["swallowed_error"] += swallowed
        if panics:
            result.per_file[f"{domain}/panic"].append((rel, panics))
        if swallowed:
            result.per_file[f"{domain}/swallowed_error"].append((rel, swallowed))
        loc = rust_file_line_count(path)
        if loc > OVERSIZE_THRESHOLD_LOC:
            result.counts[domain]["oversize_files"] += 1
            result.oversize_files[domain].append(f"{rel} ({loc} LOC)")
    return result


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def repository_totals() -> dict[str, int]:
    """Repository-wide debt as recorded by the five ratchet baselines.

    These are the *recorded budget*, not a fresh scan. That is deliberate: the
    per-ratchet scripts already prove the working tree matches its baseline, so
    reading the baselines here checks the one thing they cannot check, namely
    whether the recorded budget itself moved up.
    """

    code_size = load_json(CODE_SIZE_BASELINE)
    warnings_text = WARNING_BASELINE.read_text(encoding="utf-8").strip()
    if not warnings_text.isdigit():
        raise SystemExit(f"error: invalid warning baseline in {WARNING_BASELINE}: {warnings_text!r}")
    return {
        "panic": load_json(PANIC_BASELINE)["total"],
        "swallowed_error": load_json(SWALLOWED_BASELINE)["total"],
        "oversize_files": len(code_size["tracked_files"]),
        "oversize_total_loc": sum(code_size["tracked_files"].values()),
        "test_oversize_files": len(load_json(TEST_SIZE_BASELINE)["tracked_files"]),
        "warnings": int(warnings_text),
    }


def build_report(measurement: Measurement) -> dict[str, Any]:
    counts = measurement.counts
    domains: dict[str, Any] = {}
    critical_totals = {dim: 0 for dim in DIMENSIONS}
    target_totals = {dim: 0 for dim in DIMENSIONS}
    ceiling_totals = {dim: 0 for dim in DIMENSIONS}
    for domain, domain_counts in counts.items():
        entry: dict[str, Any] = {"paths": CRITICAL_PATHS[domain], "dimensions": {}}
        for dim in DIMENSIONS:
            current = domain_counts[dim]
            ceiling = CEILINGS[domain][dim]
            target = TARGETS[domain][dim]
            critical_totals[dim] += current
            ceiling_totals[dim] += ceiling
            target_totals[dim] += target
            entry["dimensions"][dim] = {
                "current": current,
                "ceiling": ceiling,
                "headroom": ceiling - current,
                "target": target,
                "distance_to_target": max(0, current - target),
                "at_or_below_target": current <= target,
            }
        entry["rationale"] = TARGETS[domain]["rationale"]
        entry["file_count"] = {
            "current": measurement.file_counts[domain],
            "expected": EXPECTED_FILE_COUNTS[domain],
            "delta": measurement.file_counts[domain] - EXPECTED_FILE_COUNTS[domain],
        }
        entry["oversize_files"] = sorted(measurement.oversize_files[domain])
        domains[domain] = entry

    repo = repository_totals()
    return {
        "version": 1,
        "scope_digest": scope_digest(),
        "oversize_threshold_loc": OVERSIZE_THRESHOLD_LOC,
        "critical_production_files_scanned": measurement.scanned,
        "domains": domains,
        "critical_totals": {
            dim: {
                "current": critical_totals[dim],
                "ceiling": ceiling_totals[dim],
                "target": target_totals[dim],
                "distance_to_target": max(0, critical_totals[dim] - target_totals[dim]),
            }
            for dim in DIMENSIONS
        },
        "repository_totals": repo,
        "repository_trend": {
            key: {
                "recorded": repo[key],
                "high_water_mark": mark,
                "headroom": mark - repo[key],
                "reduced_by": max(0, mark - repo[key]),
            }
            for key, mark in sorted(REPOSITORY_CEILINGS.items())
        },
        "critical_share_percent": {
            dim: round(100.0 * critical_totals[dim] / repo[dim], 1) if repo[dim] else 0.0
            for dim in DIMENSIONS
        },
    }


def scope_shrink_regressions(file_counts: dict[str, int]) -> list[str]:
    """Fail when a domain lost files, so a scope shrink cannot read as cleanup.

    Only a decrease is a regression. Growth is normal work and its debt is still
    bounded by the ceilings.
    """

    return [
        f"{domain} lost in-scope production files: {EXPECTED_FILE_COUNTS[domain]} -> {count}. "
        "Debt that leaves the critical set is not debt that was fixed. If this removal is "
        "intentional, record the new count in EXPECTED_FILE_COUNTS and refresh the workflow "
        "digest pin, which is a protected-path change and belongs in a maintenance window."
        for domain, count in sorted(file_counts.items())
        if count < EXPECTED_FILE_COUNTS[domain]
    ]


def repository_trend_regressions(repo: dict[str, int]) -> list[str]:
    return [
        f"repository {key} budget rose above its recorded high-water mark: "
        f"{REPOSITORY_CEILINGS[key]} -> {value}"
        for key, value in sorted(repo.items())
        if value > REPOSITORY_CEILINGS[key]
    ]


def print_report(report: dict[str, Any]) -> None:
    print("Critical-path debt trend")
    print(f"  scope digest: {report['scope_digest']}")
    print(f"  production files in scope: {report['critical_production_files_scanned']}")
    print("  in-scope file counts (a decrease fails; debt must be fixed, not moved out):")
    for domain, entry in report["domains"].items():
        counts = entry["file_count"]
        delta = counts["delta"]
        marker = "OK" if delta == 0 else (f"+{delta}" if delta > 0 else str(delta))
        print(
            f"    {domain:24s} current={counts['current']:4d} expected={counts['expected']:4d} ({marker})"
        )
    header = f"  {'domain':24s} {'dimension':16s} {'now':>6s} {'ceil':>6s} {'target':>7s} {'to-go':>6s}"
    print(header)
    for domain, entry in report["domains"].items():
        for dim, values in entry["dimensions"].items():
            print(
                f"  {domain:24s} {dim:16s} {values['current']:6d} {values['ceiling']:6d} "
                f"{values['target']:7d} {values['distance_to_target']:6d}"
            )
    for dim, values in report["critical_totals"].items():
        share = report["critical_share_percent"][dim]
        print(
            f"  TOTAL {dim}: now={values['current']} ceiling={values['ceiling']} "
            f"target={values['target']} to-go={values['distance_to_target']} "
            f"repo={report['repository_totals'][dim]} ({share}% of repository)"
        )
    print("  Repository debt trend (recorded budget vs high-water mark):")
    for key, values in report["repository_trend"].items():
        direction = (
            f"reduced by {values['reduced_by']}" if values["reduced_by"] else "unchanged"
        )
        print(
            f"    {key:20s} recorded={values['recorded']:7d} "
            f"mark={values['high_water_mark']:7d} ({direction})"
        )


def main() -> int:
    args = parse_args()

    if args.print_digest:
        print(scope_digest())
        return 0

    if args.expect_digest is not None and args.expect_digest != scope_digest():
        print(
            "Critical-path budget scope digest mismatch.\n"
            f"  expected (pinned in .github/workflows/fork-ci.yml): {args.expect_digest}\n"
            f"  actual   (scripts/check_critical_path_budget.py):   {scope_digest()}\n"
            "The critical-path scope, ceilings, targets, or oversize threshold changed. "
            "Both this script and the workflow pin are protected governance paths, so an "
            "intentional change belongs in a maintenance window; update the workflow pin "
            "with `python3 scripts/check_critical_path_budget.py --print-digest`.",
            file=sys.stderr,
        )
        return 1

    baseline_threshold = load_json(CODE_SIZE_BASELINE)["threshold_loc"]
    if baseline_threshold != OVERSIZE_THRESHOLD_LOC:
        print(
            "Oversize threshold drift: "
            f"scripts/code_size_budget.json threshold_loc={baseline_threshold} but the pinned "
            f"critical-path threshold is {OVERSIZE_THRESHOLD_LOC}. The baseline is unprotected, "
            "so raising it there cannot be allowed to retire the oversize dimension.",
            file=sys.stderr,
        )
        return 1

    measurement = measure()
    counts = measurement.counts
    report = build_report(measurement)

    if args.report:
        report_path = Path(args.report)
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    regressions: list[str] = repository_trend_regressions(report["repository_totals"])
    regressions += scope_shrink_regressions(measurement.file_counts)
    for domain in CRITICAL_PATHS:
        for dim in DIMENSIONS:
            current = counts[domain][dim]
            ceiling = CEILINGS[domain][dim]
            if current <= ceiling:
                continue
            detail = measurement.contributors(domain, dim)
            regressions.append(
                f"{domain}/{dim} grew past its zero-growth ceiling: {ceiling} -> {current}\n"
                + "".join(f"      contributor: {item}\n" for item in detail).rstrip("\n")
            )

    print_report(report)

    if regressions:
        print("\nCritical-path budget exceeded (acceptance standard A6):", file=sys.stderr)
        for entry in regressions:
            print(f"  - {entry}", file=sys.stderr)
        print(
            "Critical paths are zero-growth: existing debt is grandfathered, new debt is not. "
            "Remove the new debt, or move the work outside the critical scope. Raising a ceiling "
            "requires editing two protected paths inside a maintenance window.",
            file=sys.stderr,
        )
        return 1

    improvements = [
        f"{domain}/{dim} is {CEILINGS[domain][dim] - counts[domain][dim]} below its ceiling"
        for domain in CRITICAL_PATHS
        for dim in DIMENSIONS
        if counts[domain][dim] < CEILINGS[domain][dim]
    ]
    if improvements:
        print("\nCritical-path budget OK, with headroom from prior cleanup:")
        for entry in improvements:
            print(f"  - {entry}")
    else:
        print("\nCritical-path budget OK: every domain is exactly at its ceiling.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
