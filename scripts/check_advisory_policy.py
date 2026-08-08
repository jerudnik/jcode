#!/usr/bin/env python3
"""Enforce structured ownership of every accepted security advisory.

The fork suppresses advisories in exactly one machine-readable place:

  .cargo/audit.toml            [advisories].ignore, read by cargo-audit

Neither ownership nor expiry metadata can live there, because cargo-audit
validates that file against a closed schema. So the ownership record lives in
`docs/security/advisories.toml`, and this checker proves the record matches the
ignore list and still carries the owner/expiry bookkeeping.

Failures (each independently fatal):

  undocumented  an ID suppressed in `.cargo/audit.toml` with no record
  stale         a record whose ID is no longer suppressed (clean it up)
  incomplete    a record missing id/owner/rationale/affected_surface/
                expires/retire_when, or with a blank one
  malformed     a non-ISO `expires`/`accepted`, or a duplicate ID
  postdated     `accepted` is in the future, which would park a suppression
                outside the expiry window it is supposed to be bounded by
  expired       `expires` is on or before the effective current date
  overlong      `expires` is more than [policy].max_expiry_days past `accepted`
  ungoverned    a blanket `severity_threshold` suppressing whole severity
                classes with no record to justify or retire it

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

# A severity threshold is a blanket suppression: it hides every advisory below
# the named level, including ones nobody has ever seen or triaged. If the fork
# ever sets one, it needs the same ownership story as a single ID.
THRESHOLD_FIELDS = ("owner", "accepted", "expires", "rationale", "retire_when")

DEFAULT_MAX_EXPIRY_DAYS = 365

AUDIT_TOML = ".cargo/audit.toml"
RECORD_TOML = "docs/security/advisories.toml"


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


def _strip_toml_comment(line: str) -> str:
    in_double = False
    in_single = False
    out: list[str] = []
    for char in line:
        if char == '"' and not in_single:
            in_double = not in_double
        elif char == "'" and not in_double:
            in_single = not in_single
        elif char == "#" and not in_double and not in_single:
            break
        out.append(char)
    return "".join(out).rstrip()


def _parse_toml_value(raw: str) -> object:
    raw = raw.strip()
    if not raw:
        return ""
    if raw.startswith("[") and raw.endswith("]"):
        inner = raw[1:-1].strip()
        if not inner:
            return []
        values: list[object] = []
        item = []
        in_double = False
        in_single = False
        for char in inner:
            if char == '"' and not in_single:
                in_double = not in_double
            elif char == "'" and not in_double:
                in_single = not in_single
            if char == "," and not in_double and not in_single:
                values.append(_parse_toml_value("".join(item).strip()))
                item = []
                continue
            item.append(char)
        if item:
            values.append(_parse_toml_value("".join(item).strip()))
        return values
    if (raw.startswith('"') and raw.endswith('"')) or (raw.startswith("'") and raw.endswith("'")):
        return raw[1:-1]
    if raw.lower() == "true":
        return True
    if raw.lower() == "false":
        return False
    if re.fullmatch(r"-?\d+", raw):
        return int(raw)
    return raw


def load_toml_document(path: pathlib.Path) -> dict:
    document: dict = {}
    current: dict | None = document
    pending_key: str | None = None
    pending_value: str = ""
    for line in path.read_text().splitlines():
        line = _strip_toml_comment(line).strip()
        if not line:
            continue
        if pending_key is not None:
            pending_value = f"{pending_value} {line}".strip()
            if line.endswith("]"):
                current[pending_key] = _parse_toml_value(pending_value)
                pending_key = None
                pending_value = ""
            continue
        if line.startswith("[[") and line.endswith("]]"):
            name = line[2:-2].strip()
            bucket = document.setdefault(name, [])
            if not isinstance(bucket, list):
                raise SystemExit(f"error: {path}: [{name}] is both a table and an array of tables")
            current = {}
            bucket.append(current)
            continue
        if line.startswith("[") and line.endswith("]"):
            name = line[1:-1].strip()
            bucket = document.setdefault(name, {})
            if not isinstance(bucket, dict):
                raise SystemExit(f"error: {path}: [{name}] is both a table and an array of tables")
            current = bucket
            continue
        if "=" not in line:
            raise SystemExit(f"error: {path}: cannot parse line: {line!r}")
        key, raw_value = line.split("=", 1)
        if current is None:
            raise SystemExit(f"error: {path}: key-value pair outside of a table: {line!r}")
        if raw_value.strip().startswith("[") and not raw_value.strip().endswith("]"):
            pending_key = key.strip()
            pending_value = raw_value.strip()
            continue
        current[key.strip()] = _parse_toml_value(raw_value)
    return document


def audit_toml_ignores(path: pathlib.Path) -> list[str]:
    """IDs cargo-audit is told to ignore, in file order."""
    data = load_toml_document(path)
    ignores = data.get("advisories", {}).get("ignore", [])
    if not isinstance(ignores, list):
        raise SystemExit(f"error: {path}: [advisories].ignore must be an array")
    # cargo-audit accepts `ID/package` scoping; the ID is what we govern.
    return [str(entry).split("/")[0] for entry in ignores]


def audit_toml_severity_threshold(path: pathlib.Path) -> str | None:
    data = load_toml_document(path)
    value = data.get("advisories", {}).get("severity_threshold")
    return None if value is None else str(value).strip().lower()


def _as_date(value: object) -> dt.date | None:
    # tomllib decodes bare TOML dates to datetime.date already.
    if isinstance(value, dt.datetime):
        return value.date()
    if isinstance(value, dt.date):
        return value
    try:
        return dt.date.fromisoformat(str(value).strip())
    except ValueError:
        return None


def check(root: pathlib.Path, today: dt.date) -> list[str]:
    problems: list[str] = []

    audit_toml = root / AUDIT_TOML
    record_toml = root / RECORD_TOML
    for path in (audit_toml, record_toml):
        if not path.is_file():
            problems.append(f"missing required file: {path.relative_to(root)}")
    if problems:
        return problems

    suppressed = set(audit_toml_ignores(audit_toml))

    document = load_toml_document(record_toml)
    policy = document.get("policy", {})
    max_days = int(policy.get("max_expiry_days", DEFAULT_MAX_EXPIRY_DAYS))
    records = document.get("advisory", [])
    if not isinstance(records, list):
        return [f"{RECORD_TOML}: [[advisory]] must be an array of tables"]

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
            parsed = _as_date(record[field])
            if parsed is None:
                problems.append(
                    f"{advisory_id}: {field} is not an ISO YYYY-MM-DD date: {record[field]!r}"
                )
            else:
                dates[field] = parsed
        if len(dates) != 2:
            continue

        # Expiry is an interval between two self-declared dates, so a
        # future-dated acceptance would park a suppression indefinitely and
        # still satisfy every window check below.
        if dates["accepted"] > today:
            problems.append(
                f"{advisory_id}: accepted is dated {dates['accepted'].isoformat()}, "
                f"in the future (today is {today.isoformat()}); an acceptance cannot "
                f"begin before it is made"
            )
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

    # Every suppression needs a record.
    for advisory_id in sorted(suppressed):
        if advisory_id not in by_id:
            problems.append(
                f"{advisory_id}: suppressed in {AUDIT_TOML} but has no record in "
                f"{RECORD_TOML} (add owner, rationale, affected surface, expiry, and retirement condition)"
            )

    # Every record must correspond to a live suppression, so retiring an
    # advisory cannot be done halfway.
    for advisory_id in by_id:
        if advisory_id not in suppressed:
            problems.append(
                f"{advisory_id}: has a record in {RECORD_TOML} but is suppressed on no "
                f"surface ({AUDIT_TOML}); delete the stale record"
            )

    problems.extend(_check_severity_threshold(audit_toml, document, today))
    return problems


def _check_severity_threshold(
    audit_toml: pathlib.Path, document: dict, today: dt.date
) -> list[str]:
    """A severity threshold hides whole classes of advisory, including ones
    nobody has triaged, so it needs an owner and an expiry like any other
    acceptance."""
    threshold = audit_toml_severity_threshold(audit_toml)
    record = document.get("severity_threshold")

    if threshold is None:
        if record is not None:
            return [
                f"severity_threshold: {RECORD_TOML} carries a [severity_threshold] record "
                f"but {AUDIT_TOML} sets no threshold; delete the stale record"
            ]
        return []

    if not isinstance(record, dict):
        return [
            f"severity_threshold: {AUDIT_TOML} sets severity_threshold = {threshold!r}, "
            f"which silently suppresses every advisory below that severity, but "
            f"{RECORD_TOML} has no [severity_threshold] record (add owner, rationale, "
            f"expiry, and retirement condition)"
        ]

    problems: list[str] = []
    missing = [f for f in THRESHOLD_FIELDS if not str(record.get(f, "")).strip()]
    if missing:
        problems.append(
            f"severity_threshold: incomplete record, missing or blank: {', '.join(missing)}"
        )
        return problems

    declared = str(record.get("threshold", "")).strip().lower()
    if declared != threshold:
        problems.append(
            f"severity_threshold: {AUDIT_TOML} sets {threshold!r} but {RECORD_TOML} "
            f"documents {declared!r}"
        )

    expires = _as_date(record["expires"])
    accepted = _as_date(record["accepted"])
    if expires is None or accepted is None:
        problems.append("severity_threshold: accepted/expires must be ISO YYYY-MM-DD dates")
        return problems
    if accepted > today:
        problems.append(
            f"severity_threshold: accepted is dated {accepted.isoformat()}, in the future "
            f"(today is {today.isoformat()})"
        )
    if expires <= today:
        problems.append(
            f"severity_threshold: acceptance expired on {expires.isoformat()} "
            f"(today is {today.isoformat()}); owner {record['owner']} must re-argue it "
            f"or retire it ({record['retire_when']})"
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
