#!/usr/bin/env python3
"""Enforce structured ownership of every accepted security advisory.

The fork suppresses advisories on more than one surface, and a suppression is
only as governed as its *weakest* surface:

  .cargo/audit.toml            [advisories].ignore, read by cargo-audit
  scripts/security_preflight.sh  audit_ignores=(--ignore ...), what CI actually
                                 executes (ci.yml, security.yml --strict,
                                 governance-root.yml)

Neither can carry ownership metadata. cargo-audit validates audit.toml against
a closed schema and rejects any extra key, and the preflight array is a vendor
file kept pristine. So the ownership record lives in
`docs/security/advisories.toml`, and this checker proves every surface agrees
with it, and with the others, in both directions.

Failures (each independently fatal):

  undocumented  an ID suppressed on any surface with no record
  stale         a record whose ID is suppressed on no surface (clean it up)
  drift         an ID suppressed on one surface but not another
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
import tomllib

ADVISORY_ID = re.compile(r"RUSTSEC-\d{4}-\d{4}")

# `--ignore RUSTSEC-YYYY-NNNN` inside the preflight array, ignoring trailing
# comments. Lines that are themselves commented out must not count as active.
PREFLIGHT_IGNORE = re.compile(r"--ignore\s+(RUSTSEC-\d{4}-\d{4})")

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
PREFLIGHT_SH = "scripts/security_preflight.sh"
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


def audit_toml_ignores(path: pathlib.Path) -> list[str]:
    """IDs cargo-audit is told to ignore, in file order."""
    data = tomllib.loads(path.read_text())
    ignores = data.get("advisories", {}).get("ignore", [])
    if not isinstance(ignores, list):
        raise SystemExit(f"error: {path}: [advisories].ignore must be an array")
    # cargo-audit accepts `ID/package` scoping; the ID is what we govern.
    return [str(entry).split("/")[0] for entry in ignores]


def audit_toml_severity_threshold(path: pathlib.Path) -> str | None:
    data = tomllib.loads(path.read_text())
    value = data.get("advisories", {}).get("severity_threshold")
    return None if value is None else str(value).strip().lower()


def preflight_ignores(path: pathlib.Path) -> list[str]:
    """IDs the preflight script passes to cargo-audit on the command line.

    This is the surface CI actually executes, so it has to be governed even
    though it is a shell array rather than structured config. Only active
    lines count: a commented-out `--ignore` is not a suppression.
    """
    found: list[str] = []
    inside = False
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not inside:
            if re.match(r"^audit_ignores=\(", line):
                inside = True
                # A one-line array declaration still carries entries.
                if ")" in line:
                    found.extend(PREFLIGHT_IGNORE.findall(line))
                    inside = False
            continue
        if line.startswith(")"):
            break
        if line.startswith("#"):
            continue
        found.extend(PREFLIGHT_IGNORE.findall(line))
    return found


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
    preflight_sh = root / PREFLIGHT_SH
    record_toml = root / RECORD_TOML
    for path in (audit_toml, preflight_sh, record_toml):
        if not path.is_file():
            problems.append(f"missing required file: {path.relative_to(root)}")
    if problems:
        return problems

    surfaces: dict[str, list[str]] = {
        AUDIT_TOML: audit_toml_ignores(audit_toml),
        PREFLIGHT_SH: preflight_ignores(preflight_sh),
    }
    suppressed: set[str] = set().union(*(set(ids) for ids in surfaces.values()))

    document = tomllib.loads(record_toml.read_text())
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

    # Every suppression, on every surface, needs a record.
    for advisory_id in sorted(suppressed):
        if advisory_id not in by_id:
            where = ", ".join(sorted(s for s, ids in surfaces.items() if advisory_id in ids))
            problems.append(
                f"{advisory_id}: suppressed in {where} but has no record in "
                f"{RECORD_TOML} (add owner, rationale, affected surface, "
                f"expiry, and retirement condition)"
            )

    # Every record must correspond to a live suppression, so retiring an
    # advisory cannot be done halfway.
    for advisory_id in by_id:
        if advisory_id not in suppressed:
            problems.append(
                f"{advisory_id}: has a record in {RECORD_TOML} but is suppressed on no "
                f"surface ({AUDIT_TOML}, {PREFLIGHT_SH}); delete the stale record"
            )

    # The surfaces must also agree with each other: an ID dropped from one and
    # left in the other is a half-finished retirement that the union check
    # above would otherwise hide.
    for surface, ids in surfaces.items():
        for advisory_id in sorted(suppressed - set(ids)):
            others = ", ".join(sorted(s for s, other in surfaces.items() if advisory_id in other))
            problems.append(
                f"{advisory_id}: suppressed in {others} but not in {surface}; "
                f"the suppression surfaces must agree"
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
