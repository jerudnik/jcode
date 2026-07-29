#!/usr/bin/env python3
"""Enforce structured ownership of every accepted security advisory.

`.cargo/audit.toml` tells cargo-audit which advisories to ignore. It cannot say
who owns them or when the acceptance stops being valid: cargo-audit validates
that file against a closed schema and rejects any extra key. So the ownership
record lives in `docs/security/advisories.toml`, and this checker proves the
two files agree and that no acceptance has gone stale.

Failures (each independently fatal):

  undocumented  an ID ignored in .cargo/audit.toml with no record
  stale         a record whose ID is no longer ignored (clean it up)
  incomplete    a record missing id/owner/rationale/affected_surface/
                expires/retire_when, or with a blank one
  malformed     a non-ISO `expires`/`accepted`, or a duplicate ID
  expired       `expires` is on or before the effective current date
  overlong      `expires` is more than [policy].max_expiry_days past `accepted`

The current date is injected, never guessed, so expiry tests are deterministic:
`--today YYYY-MM-DD` beats `$ADVISORY_POLICY_TODAY`, which beats the system
date (what CI uses).

Usage:
  scripts/check_advisory_policy.py
  scripts/check_advisory_policy.py --root /path/to/tree --today 2030-01-01
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import pathlib
import re
import sys
import tomllib

ADVISORY_ID = re.compile(r"RUSTSEC-\d{4}-\d{4}")

REQUIRED_FIELDS = (
    "id",
    "crate_name",
    "owner",
    "accepted",
    "expires",
    "affected_surface",
    "rationale",
    "retire_when",
)

DEFAULT_MAX_EXPIRY_DAYS = 365


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=pathlib.Path,
        default=pathlib.Path(__file__).resolve().parents[1],
        help="repository root to check (default: this script's repo)",
    )
    parser.add_argument(
        "--today",
        default=None,
        help="ISO date to evaluate expiry against (default: $ADVISORY_POLICY_TODAY or the system date)",
    )
    return parser.parse_args(argv)


def effective_today(explicit: str | None) -> dt.date:
    raw = explicit or os.environ.get("ADVISORY_POLICY_TODAY")
    if raw is None:
        return dt.date.today()
    try:
        return dt.date.fromisoformat(raw)
    except ValueError as exc:
        raise SystemExit(f"error: --today/ADVISORY_POLICY_TODAY is not an ISO date: {raw!r} ({exc})")


def ignored_ids(audit_toml: pathlib.Path) -> list[str]:
    """IDs cargo-audit is told to ignore, in file order."""
    data = tomllib.loads(audit_toml.read_text())
    ignores = data.get("advisories", {}).get("ignore", [])
    if not isinstance(ignores, list):
        raise SystemExit(f"error: {audit_toml}: [advisories].ignore must be an array")
    return [str(entry).split("/")[0] for entry in ignores]


def check(root: pathlib.Path, today: dt.date) -> list[str]:
    problems: list[str] = []

    audit_toml = root / ".cargo/audit.toml"
    record_toml = root / "docs/security/advisories.toml"
    for path in (audit_toml, record_toml):
        if not path.is_file():
            problems.append(f"missing required file: {path.relative_to(root)}")
    if problems:
        return problems

    ignores = ignored_ids(audit_toml)
    document = tomllib.loads(record_toml.read_text())
    max_days = int(document.get("policy", {}).get("max_expiry_days", DEFAULT_MAX_EXPIRY_DAYS))
    records = document.get("advisory", [])
    if not isinstance(records, list):
        return ["docs/security/advisories.toml: [[advisory]] must be an array of tables"]

    by_id: dict[str, dict] = {}
    for index, record in enumerate(records):
        label = record.get("id") or f"[[advisory]] #{index + 1}"

        missing = [f for f in REQUIRED_FIELDS if not str(record.get(f, "")).strip()]
        if missing:
            problems.append(f"{label}: incomplete record, missing or blank: {', '.join(missing)}")
            continue

        advisory_id = str(record["id"]).strip()
        if not ADVISORY_ID.fullmatch(advisory_id):
            problems.append(f"{label}: id is not a RUSTSEC-YYYY-NNNN identifier")
            continue
        if advisory_id in by_id:
            problems.append(f"{advisory_id}: duplicate record")
            continue
        by_id[advisory_id] = record

        dates: dict[str, dt.date] = {}
        for field in ("accepted", "expires"):
            value = record[field]
            # tomllib decodes bare TOML dates to datetime.date already.
            if isinstance(value, dt.datetime):
                dates[field] = value.date()
            elif isinstance(value, dt.date):
                dates[field] = value
            else:
                try:
                    dates[field] = dt.date.fromisoformat(str(value).strip())
                except ValueError:
                    problems.append(f"{advisory_id}: {field} is not an ISO YYYY-MM-DD date: {value!r}")
        if len(dates) != 2:
            continue

        if dates["expires"] <= today:
            problems.append(
                f"{advisory_id}: acceptance expired on {dates['expires'].isoformat()} "
                f"(today is {today.isoformat()}); owner {record['owner']} must re-argue it "
                f"or retire it ({record['retire_when']})"
            )
        if dates["expires"] < dates["accepted"]:
            problems.append(f"{advisory_id}: expires precedes accepted")
        elif (dates["expires"] - dates["accepted"]).days > max_days:
            problems.append(
                f"{advisory_id}: acceptance window "
                f"{(dates['expires'] - dates['accepted']).days} days exceeds "
                f"[policy].max_expiry_days = {max_days}"
            )

    for advisory_id in ignores:
        if advisory_id not in by_id:
            problems.append(
                f"{advisory_id}: ignored in .cargo/audit.toml but has no record in "
                f"docs/security/advisories.toml (add owner, rationale, affected surface, "
                f"expiry, and retirement condition)"
            )
    ignored = set(ignores)
    for advisory_id in by_id:
        if advisory_id not in ignored:
            problems.append(
                f"{advisory_id}: has a record in docs/security/advisories.toml but is no "
                f"longer ignored in .cargo/audit.toml; delete the stale record"
            )

    return problems


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    today = effective_today(args.today)
    problems = check(args.root.resolve(), today)
    if problems:
        print(f"advisory policy: {len(problems)} problem(s) as of {today.isoformat()}", file=sys.stderr)
        for problem in problems:
            print(f"  error: {problem}", file=sys.stderr)
        return 1
    print(f"advisory policy: OK as of {today.isoformat()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
