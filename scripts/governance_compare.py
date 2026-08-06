#!/usr/bin/env python3
"""Compare a governance snapshot against the canonical manifest.

This is the comparison half of `scripts/fork-health.sh`; the shell script owns
mode selection and live acquisition, this file owns every judgement about
whether the observed state matches `scripts/required-checks.json`. Splitting it
out is not decoration: the comparison is a deep, order-sensitive-in-places
structural diff plus a fail-closed YAML extractor, and neither is honestly
expressible in shell. R07 design.md section 4 protects "the comparator"; that
noun covers this file as well as the shell entry point (see
docs/fork/ideal-base/evidence/R07/stream-g-protected-paths-proposal.md).

Input is one aggregate snapshot, acquired live by `fork-health.sh --live` or
read from a fixture by `--fixture`. The snapshot shape is:

    {
      "repository": {"id", "full_name",
                     "allow_merge_commit", "allow_squash_merge",
                     "allow_rebase_merge"},
      "rulesets": [<full ruleset body>, ...],
      "effective_main_rules": [{"type": ...}, ...],
      "classic_branch_protection": null | <full protection body>,
      "branches": ["main", ...],
      "workflows": {".github/workflows/x.yml": "<raw text>", ...}   # optional
    }

`workflows` is optional. When absent the extractor reads
`--workflows-dir` (default `.github/workflows`) from disk, which is what live
mode does. Fixture mode carries the text so the comparator can be exercised
against planted workflow mutations without editing the repository's own
workflows.

Exit codes, per design.md section 6:

    0  snapshot matches the manifest
    1  one or more governance mismatches (all are reported, not just the first)
    2  usage, acquisition, or schema failure -- never reported as a pass
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

# Response-only keys. GitHub returns these; they are never desired state, so
# they are stripped recursively before any equality or hash comparison. A key
# added here weakens the comparison, so the list is deliberately short and
# every entry is a documented server-generated field.
VOLATILE_KEYS = frozenset(
    {
        "_links",
        "contexts_url",
        "created_at",
        "current_user_can_bypass",
        "id",
        "node_id",
        "source",
        "source_type",
        "updated_at",
        "url",
    }
)

# A snapshot missing any of these cannot be compared. Absence is a schema
# failure (exit 2), not a mismatch (exit 1): we did not observe a wrong state,
# we failed to observe the state at all.
REQUIRED_SNAPSHOT_KEYS = (
    "repository",
    "rulesets",
    "effective_main_rules",
    "classic_branch_protection",
    "branches",
)


class SchemaError(Exception):
    """Snapshot or manifest could not be interpreted; exit 2."""


class WorkflowParseError(Exception):
    """The constrained extractor met a construct it will not classify; exit 2."""


# ---------------------------------------------------------------------------
# Constrained YAML extraction
# ---------------------------------------------------------------------------
#
# GitHub workflows are the only YAML this reads, and the questions asked of
# them are narrow: which job ids exist, what each declares as `name`, `needs`,
# and `if`, and whether a pull_request trigger carries a path filter. A full
# YAML implementation would accept constructs (anchors, merge keys, multiple
# documents) whose resolution changes what a job means, which is exactly the
# ambiguity a governance check must not absorb quietly. So this parser handles
# the subset workflows actually use and raises on everything else. Raising is
# exit 2: unclassifiable is not the same as compliant.


def _strip_comment(text: str) -> str:
    """Drop a trailing `# ...` comment, respecting quotes."""
    out = []
    quote = None
    i = 0
    while i < len(text):
        ch = text[i]
        if quote:
            if ch == "\\" and quote == '"':
                out.append(ch)
                i += 1
                if i < len(text):
                    out.append(text[i])
                    i += 1
                continue
            if ch == quote:
                quote = None
            out.append(ch)
        elif ch in "\"'":
            quote = ch
            out.append(ch)
        elif ch == "#" and (not out or out[-1] in " \t"):
            break
        else:
            out.append(ch)
        i += 1
    return "".join(out).rstrip()


def _unquote(text: str) -> str:
    text = text.strip()
    if len(text) >= 2 and text[0] == text[-1] and text[0] in "\"'":
        return text[1:-1]
    return text


def _parse_flow_seq(text: str) -> list[str]:
    inner = text.strip()[1:-1].strip()
    if not inner:
        return []
    items = []
    depth = 0
    quote = None
    current = ""
    for ch in inner:
        if quote:
            current += ch
            if ch == quote:
                quote = None
            continue
        if ch in "\"'":
            quote = ch
            current += ch
        elif ch in "[{":
            depth += 1
            current += ch
        elif ch in "]}":
            depth -= 1
            current += ch
        elif ch == "," and depth == 0:
            items.append(_unquote(current))
            current = ""
        else:
            current += ch
    if quote is not None:
        raise WorkflowParseError("unterminated quote in flow sequence")
    items.append(_unquote(current))
    return [item for item in items if item != ""]


class _Line:
    __slots__ = ("indent", "text", "number")

    def __init__(self, indent: int, text: str, number: int) -> None:
        self.indent = indent
        self.text = text
        self.number = number


def _significant_lines(source: str) -> list[_Line]:
    lines = []
    for number, raw in enumerate(source.splitlines(), start=1):
        if "\t" in raw[: len(raw) - len(raw.lstrip())]:
            raise WorkflowParseError(f"line {number}: tab indentation")
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped == "---" or stripped.startswith("--- ") or stripped == "...":
            raise WorkflowParseError(f"line {number}: multi-document YAML")
        lines.append(_Line(len(raw) - len(raw.lstrip()), raw, number))
    return lines


def _parse_block(lines: list[_Line], index: int, indent: int) -> tuple[Any, int]:
    if index >= len(lines):
        return None, index
    if lines[index].text.strip().startswith("- "):
        return _parse_seq(lines, index, lines[index].indent)
    return _parse_map(lines, index, indent)


def _parse_seq(lines: list[_Line], index: int, indent: int) -> tuple[list[Any], int]:
    items: list[Any] = []
    while index < len(lines) and lines[index].indent == indent:
        stripped = _strip_comment(lines[index].text.strip())
        if not stripped.startswith("- "):
            break
        body = stripped[2:].strip()
        item_indent = indent + 2
        if not body:
            index += 1
            if index < len(lines) and lines[index].indent > indent:
                value, index = _parse_block(lines, index, lines[index].indent)
                items.append(value)
            else:
                items.append(None)
            continue
        if ":" in body and not body.startswith(("[", '"', "'")):
            # An inline mapping entry starting a list item: `- uses: x`. Re-parse
            # the item as a mapping whose first key sits on the dash line.
            synthetic = [_Line(item_indent, " " * item_indent + body, lines[index].number)]
            index += 1
            while index < len(lines) and lines[index].indent >= item_indent:
                synthetic.append(lines[index])
                index += 1
            value, _ = _parse_map(synthetic, 0, item_indent)
            items.append(value)
            continue
        items.append(_unquote(body))
        index += 1
    return items, index


def _parse_map(lines: list[_Line], index: int, indent: int) -> tuple[dict[str, Any], int]:
    mapping: dict[str, Any] = {}
    while index < len(lines) and lines[index].indent >= indent:
        if lines[index].indent > indent:
            raise WorkflowParseError(f"line {lines[index].number}: unexpected indentation")
        stripped = lines[index].text.strip()
        if stripped.startswith("- "):
            break
        if stripped.startswith("<<"):
            raise WorkflowParseError(f"line {lines[index].number}: merge key")
        if ":" not in stripped:
            raise WorkflowParseError(f"line {lines[index].number}: not a mapping entry")
        key_part, _, value_part = stripped.partition(":")
        key = _unquote(key_part)
        value_part = _strip_comment(value_part).strip()
        line_number = lines[index].number
        index += 1

        if value_part.startswith(("&", "*")):
            raise WorkflowParseError(f"line {line_number}: YAML anchor or alias")
        if value_part.startswith("{"):
            raise WorkflowParseError(f"line {line_number}: flow mapping")

        if value_part in ("|", ">", "|-", ">-", "|+", ">+"):
            block, index = _consume_block_scalar(lines, index, indent)
            mapping[key] = block
        elif value_part.startswith("["):
            mapping[key] = _parse_flow_seq(value_part)
        elif value_part:
            mapping[key] = _unquote(value_part)
        elif index < len(lines) and lines[index].indent > indent:
            mapping[key], index = _parse_block(lines, index, lines[index].indent)
        elif index < len(lines) and lines[index].indent == indent and lines[index].text.strip().startswith("- "):
            mapping[key], index = _parse_seq(lines, index, indent)
        else:
            mapping[key] = None
    return mapping, index


def _consume_block_scalar(lines: list[_Line], index: int, indent: int) -> tuple[str, int]:
    body = []
    while index < len(lines) and lines[index].indent > indent:
        body.append(lines[index].text)
        index += 1
    return "\n".join(body), index


def parse_workflow(source: str) -> dict[str, Any]:
    lines = _significant_lines(source)
    if not lines:
        raise WorkflowParseError("empty workflow")
    document, _ = _parse_map(lines, 0, lines[0].indent)
    return document


def workflow_job_text(source: str, job_id: str) -> str:
    """Return the raw text of one job block, for checks the parser abstracts away.

    The gate jobs' substance is a shell script inside `run:`. Whether that
    script actually consults every dependency it declares is a property of the
    text, not of the parsed structure, so it is checked textually.
    """
    lines = source.splitlines()
    start = None
    job_indent = 0
    in_jobs = False
    jobs_indent = 0
    for i, raw in enumerate(lines):
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(raw) - len(raw.lstrip())
        if not in_jobs:
            if stripped == "jobs:":
                in_jobs = True
                jobs_indent = indent
            continue
        if indent <= jobs_indent:
            break
        if start is None:
            if stripped == f"{job_id}:" and indent > jobs_indent:
                start = i
                job_indent = indent
            continue
        if indent <= job_indent:
            return "\n".join(lines[start:i])
    if start is None:
        return ""
    return "\n".join(lines[start:])


# ---------------------------------------------------------------------------
# Comparison helpers
# ---------------------------------------------------------------------------


def sanitize(value: Any) -> Any:
    """Recursively drop server-generated keys so only desired state is compared."""
    if isinstance(value, dict):
        return {k: sanitize(v) for k, v in value.items() if k not in VOLATILE_KEYS}
    if isinstance(value, list):
        return [sanitize(item) for item in value]
    return value


def canonical(value: Any) -> str:
    """The pinned encoder from design.md section 4."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def rule_key(rule: Any) -> str:
    if not isinstance(rule, dict) or "type" not in rule:
        raise SchemaError(f"rule is not an object with a type: {rule!r}")
    return str(rule["type"])


class Report:
    """Accumulates every mismatch. Design.md section 6 forbids first-failure exit."""

    def __init__(self) -> None:
        self.failures: list[str] = []
        self.notes: list[str] = []
        self.passes: list[str] = []

    def fail(self, message: str) -> None:
        self.failures.append(message)

    def note(self, message: str) -> None:
        self.notes.append(message)

    def ok(self, message: str) -> None:
        self.passes.append(message)


# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------


def check_repository(manifest: dict[str, Any], snapshot: dict[str, Any], report: Report) -> None:
    repo = snapshot["repository"]
    if not isinstance(repo, dict):
        raise SchemaError("snapshot.repository is not an object")

    for key in ("id", "full_name"):
        if key not in repo:
            raise SchemaError(f"snapshot.repository is missing '{key}'")

    expected_id = manifest["repository_id"]
    if repo["id"] != expected_id:
        report.fail(
            f"repository id is {repo['id']!r}; manifest requires {expected_id!r} "
            "(the comparison is pointed at a different repository)"
        )
    else:
        report.ok(f"repository id {expected_id} matches the manifest")

    expected_name = manifest["repository"]
    if repo["full_name"] != expected_name:
        report.fail(f"repository is {repo['full_name']!r}; manifest requires {expected_name!r}")

    methods = manifest["repository_merge_methods"]
    for key, expected in sorted(methods.items()):
        if key not in repo:
            raise SchemaError(f"snapshot.repository is missing merge setting '{key}'")
        if repo[key] != expected:
            report.fail(f"repository setting {key} is {repo[key]!r}; manifest requires {expected!r}")
    if all(repo.get(k) == v for k, v in methods.items()):
        report.ok("repository merge methods are merge-commit only")


def check_rulesets(manifest: dict[str, Any], snapshot: dict[str, Any], report: Report) -> None:
    observed_list = snapshot["rulesets"]
    if not isinstance(observed_list, list):
        raise SchemaError("snapshot.rulesets is not a list")

    observed: dict[str, dict[str, Any]] = {}
    for entry in observed_list:
        if not isinstance(entry, dict):
            raise SchemaError("snapshot.rulesets contains a non-object entry")
        if "name" not in entry:
            raise SchemaError("a ruleset in the snapshot has no name")
        # A credential without ruleset write access gets a body with no
        # bypass_actors at all. Treating that as "no bypass actors" would turn
        # an unauthorized read into a green result, which is the single most
        # valuable thing an attacker could get from this comparator.
        if "bypass_actors" not in entry:
            raise SchemaError(
                f"ruleset {entry['name']!r} has no bypass_actors key; the credential "
                "cannot see bypass actors, so this read is unauthorized, not empty"
            )
        if "rules" not in entry:
            raise SchemaError(f"ruleset {entry['name']!r} has no rules key")
        if "conditions" not in entry:
            raise SchemaError(f"ruleset {entry['name']!r} has no conditions key")
        observed[str(entry["name"])] = entry

    expected_all = manifest["rulesets"]
    expected_names = set(expected_all)
    active_observed = {
        name for name, body in observed.items() if body.get("enforcement") == "active"
    }

    for name in sorted(active_observed - expected_names):
        report.fail(f"unknown active ruleset {name!r} exists on the repository")
    for name in sorted(expected_names - set(observed)):
        report.fail(f"required ruleset {name!r} is absent")

    for name in sorted(expected_names & set(observed)):
        expected = sanitize(expected_all[name])
        actual = sanitize(observed[name])
        _compare_ruleset(name, expected, actual, report)


def _compare_ruleset(
    name: str, expected: dict[str, Any], actual: dict[str, Any], report: Report
) -> None:
    if actual.get("enforcement") != expected["enforcement"]:
        report.fail(
            f"ruleset {name!r} enforcement is {actual.get('enforcement')!r}; "
            f"manifest requires {expected['enforcement']!r}"
        )

    if actual.get("target") != expected["target"]:
        report.fail(
            f"ruleset {name!r} target is {actual.get('target')!r}; "
            f"manifest requires {expected['target']!r}"
        )

    actual_bypass = actual.get("bypass_actors")
    if canonical(actual_bypass) != canonical(expected["bypass_actors"]):
        report.fail(
            f"ruleset {name!r} bypass_actors is {canonical(actual_bypass)}; "
            f"manifest requires {canonical(expected['bypass_actors'])}"
        )

    expected_ref = expected["conditions"]["ref_name"]
    actual_ref = (actual.get("conditions") or {}).get("ref_name") or {}
    for field in ("include", "exclude"):
        want = sorted(expected_ref.get(field, []))
        got = sorted(actual_ref.get(field, []))
        if want != got:
            report.fail(
                f"ruleset {name!r} conditions.ref_name.{field} is {got!r}; "
                f"manifest requires {want!r}"
            )

    expected_rules = {rule_key(r): r for r in expected["rules"]}
    actual_rules_list = actual.get("rules") or []
    actual_rules: dict[str, Any] = {}
    for rule in actual_rules_list:
        key = rule_key(rule)
        if key in actual_rules:
            report.fail(f"ruleset {name!r} declares rule type {key!r} more than once")
        actual_rules[key] = rule

    for key in sorted(set(expected_rules) - set(actual_rules)):
        report.fail(f"ruleset {name!r} is missing required rule {key!r}")
    for key in sorted(set(actual_rules) - set(expected_rules)):
        report.fail(f"ruleset {name!r} carries unexpected rule {key!r} (not in the manifest)")

    for key in sorted(set(expected_rules) & set(actual_rules)):
        want_params = expected_rules[key].get("parameters")
        got_params = actual_rules[key].get("parameters")
        if want_params is None and got_params is None:
            continue
        if want_params is None or got_params is None:
            report.fail(f"ruleset {name!r} rule {key!r} parameters presence differs")
            continue
        _compare_rule_parameters(name, key, want_params, got_params, report)


def _compare_rule_parameters(
    ruleset: str, rule: str, expected: dict[str, Any], actual: dict[str, Any], report: Report
) -> None:
    for key in sorted(set(expected) | set(actual)):
        if key not in actual:
            report.fail(f"ruleset {ruleset!r} rule {rule!r} is missing parameter {key!r}")
            continue
        if key not in expected:
            report.fail(f"ruleset {ruleset!r} rule {rule!r} has unexpected parameter {key!r}")
            continue
        want, got = expected[key], actual[key]
        if key == "required_status_checks":
            _compare_required_contexts(ruleset, want, got, report)
            continue
        if isinstance(want, list) and isinstance(got, list):
            if sorted(map(canonical, want)) != sorted(map(canonical, got)):
                report.fail(
                    f"ruleset {ruleset!r} rule {rule!r} parameter {key!r} is "
                    f"{canonical(got)}; manifest requires {canonical(want)}"
                )
            continue
        if want != got:
            report.fail(
                f"ruleset {ruleset!r} rule {rule!r} parameter {key!r} is {got!r}; "
                f"manifest requires {want!r}"
            )


def _compare_required_contexts(
    ruleset: str, expected: list[Any], actual: list[Any], report: Report
) -> None:
    want = {}
    for entry in expected:
        want[str(entry["context"])] = entry.get("integration_id")
    got = {}
    for entry in actual:
        if not isinstance(entry, dict) or "context" not in entry:
            raise SchemaError(f"ruleset {ruleset!r} has a malformed required status check entry")
        got[str(entry["context"])] = entry.get("integration_id")

    for context in sorted(set(want) - set(got)):
        report.fail(f"required context {context!r} is not required by ruleset {ruleset!r}")
    for context in sorted(set(got) - set(want)):
        report.fail(
            f"ruleset {ruleset!r} requires unexpected context {context!r} "
            "(stale or unknown context names are a mismatch, not an extra)"
        )
    for context in sorted(set(want) & set(got)):
        if got[context] != want[context]:
            report.fail(
                f"required context {context!r} is pinned to integration_id {got[context]!r}; "
                f"manifest requires {want[context]!r} (an unpinned context is spoofable)"
            )


def check_effective_main_rules(
    manifest: dict[str, Any], snapshot: dict[str, Any], report: Report
) -> None:
    observed = snapshot["effective_main_rules"]
    if not isinstance(observed, list):
        raise SchemaError("snapshot.effective_main_rules is not a list")

    got = set()
    for rule in observed:
        if isinstance(rule, str):
            got.add(rule)
        else:
            got.add(rule_key(rule))

    target = manifest["target_branch"]
    want = set()
    for name, body in manifest["rulesets"].items():
        include = body["conditions"]["ref_name"].get("include", [])
        exclude = body["conditions"]["ref_name"].get("exclude", [])
        applies = f"refs/heads/{target}" in include or (
            "~ALL" in include and f"refs/heads/{target}" not in exclude
        )
        if applies and body["enforcement"] == "active":
            want.update(rule_key(r) for r in body["rules"])

    for missing in sorted(want - got):
        report.fail(f"effective rules on {target!r} are missing {missing!r}")
    for extra in sorted(got - want):
        report.fail(f"effective rules on {target!r} carry unexpected {extra!r}")
    if want == got:
        report.ok(f"effective rules on {target!r} are exactly {sorted(want)}")


def check_classic_protection(
    manifest: dict[str, Any], snapshot: dict[str, Any], report: Report
) -> None:
    expected = manifest["classic_branch_protection"]
    observed = snapshot["classic_branch_protection"]
    if expected != "absent":
        raise SchemaError("manifest.classic_branch_protection must be 'absent'")
    if observed in (None, "absent"):
        report.ok("classic branch protection is absent")
        return
    report.fail(
        "classic branch protection still exists alongside the ruleset "
        f"(a contradictory second layer): {canonical(sanitize(observed))}"
    )


def check_branches(manifest: dict[str, Any], snapshot: dict[str, Any], report: Report) -> None:
    branch_set = manifest["branch_set"]
    observed = snapshot["branches"]
    if not isinstance(observed, list):
        raise SchemaError("snapshot.branches is not a list")
    names = [str(b) for b in observed]

    for required in branch_set["required"]:
        if required not in names:
            report.fail(f"missing the maintained rail {required!r}")
        else:
            report.ok(f"maintained rail {required!r} is present")

    for stale in branch_set["stale_rails"]:
        if stale in names:
            report.fail(f"retired rail {stale!r} has returned to the repository")


def check_ruleset_rails(manifest: dict[str, Any], snapshot: dict[str, Any], report: Report) -> None:
    """Rulesets must not name a retired rail, and must keep the automation carve-out."""
    branch_set = manifest["branch_set"]
    referenced: set[str] = set()
    for body in snapshot["rulesets"]:
        ref_name = (body.get("conditions") or {}).get("ref_name") or {}
        for value in list(ref_name.get("include", [])) + list(ref_name.get("exclude", [])):
            text = str(value)
            if text.startswith("refs/heads/"):
                referenced.add(text[len("refs/heads/") :])

    allowed = set(branch_set["required"])
    allowed.update(prefix + "**" for prefix in branch_set["allowed_prefixes"])
    for name in sorted(referenced - allowed):
        report.fail(f"a ruleset references branch pattern {name!r}, which is not a rail")

    for prefix in branch_set["allowed_prefixes"]:
        pattern = prefix + "**"
        if pattern not in referenced:
            report.fail(
                f"the {pattern!r} carve-out is absent from every ruleset condition; "
                "topic branches would be blocked or unmanaged"
            )


# ---------------------------------------------------------------------------
# Workflow contracts
# ---------------------------------------------------------------------------


def _routing_outputs(expression: str) -> set[str]:
    """Extract `needs.<job>.outputs.<name>` output names referenced by an expression."""
    found = set()
    marker = ".outputs."
    index = expression.find(marker)
    while index != -1:
        rest = expression[index + len(marker) :]
        name = ""
        for ch in rest:
            if ch.isalnum() or ch in "_-":
                name += ch
            else:
                break
        if name:
            found.add(name)
        index = expression.find(marker, index + 1)
    return found


def _manifest_routing_outputs(expression: str) -> set[str]:
    return {token.strip() for token in expression.split("||") if token.strip()}


def check_workflow_contracts(
    manifest: dict[str, Any],
    workflows: dict[str, str],
    protected_paths: list[str],
    report: Report,
) -> None:
    parsed: dict[str, dict[str, Any]] = {}
    for path, text in sorted(workflows.items()):
        try:
            parsed[path] = parse_workflow(text)
        except WorkflowParseError as exc:
            raise SchemaError(f"{path}: {exc}") from exc

    # A required context is a job *name*. Two workflows may not both define it:
    # branch protection matches by name, so a duplicate lets an unrelated
    # workflow satisfy a required context. integration_id cannot catch this
    # because both are the same GitHub Actions app.
    definitions: dict[str, list[str]] = {}
    for path, document in parsed.items():
        for job_id, job in (document.get("jobs") or {}).items():
            if not isinstance(job, dict):
                continue
            name = job.get("name")
            if isinstance(name, str):
                definitions.setdefault(name, []).append(f"{path}:{job_id}")

    for contract in manifest["workflow_contracts"]:
        context = contract["context"]
        path = contract["file"]
        job_id = contract["job_id"]

        sites = definitions.get(context, [])
        if len(sites) > 1:
            report.fail(
                f"required context {context!r} is defined by more than one job: "
                f"{', '.join(sorted(sites))}"
            )
            continue
        if not sites:
            report.fail(f"required context {context!r} has no job definition in any workflow")
            continue
        if sites[0] != f"{path}:{job_id}":
            report.fail(
                f"required context {context!r} is defined at {sites[0]}; "
                f"manifest requires {path}:{job_id}"
            )
            continue

        document = parsed[path]
        job = document["jobs"][job_id]
        _check_job_contract(contract, document, job, workflows[path], protected_paths, report)
        report.ok(f"required context {context!r} is uniquely defined at {path}:{job_id}")


def _check_job_contract(
    contract: dict[str, Any],
    document: dict[str, Any],
    job: dict[str, Any],
    source: str,
    protected_paths: list[str],
    report: Report,
) -> None:
    context = contract["context"]
    path = contract["file"]

    want_needs = sorted(contract.get("needs") or [])
    raw_needs = job.get("needs")
    if raw_needs is None:
        got_needs: list[str] = []
    elif isinstance(raw_needs, str):
        got_needs = [raw_needs]
    else:
        got_needs = sorted(str(n) for n in raw_needs)
    if sorted(got_needs) != want_needs:
        report.fail(
            f"{context!r} summary dependencies are {sorted(got_needs)!r}; "
            f"manifest requires {want_needs!r}"
        )

    want_if = contract.get("if")
    got_if = job.get("if")
    if want_if is None:
        if got_if is not None:
            report.fail(f"{context!r} declares an `if:` ({got_if!r}); manifest requires none")
    elif got_if != want_if:
        report.fail(f"{context!r} `if:` is {got_if!r}; manifest requires {want_if!r}")

    # A summary job that declares a dependency but never consults its result is
    # a green light wired to nothing. Checked textually because the substance
    # lives inside the shell script, which the extractor treats as opaque.
    job_text = workflow_job_text(source, contract["job_id"])
    for need in want_needs:
        if f"needs.{need}.result" not in job_text:
            report.fail(
                f"{context!r} declares dependency {need!r} but never reads "
                f"needs.{need}.result, so the dependency cannot affect the gate"
            )

    jobs = document.get("jobs") or {}
    for routed_job, expression in sorted((contract.get("routing") or {}).items()):
        if routed_job not in jobs:
            report.fail(f"{context!r} routes {routed_job!r}, which does not exist in {path}")
            continue
        routed_if = jobs[routed_job].get("if")
        if not isinstance(routed_if, str):
            report.fail(f"routed job {routed_job!r} in {path} has no `if:` gate")
            continue
        want_outputs = _manifest_routing_outputs(expression)
        got_outputs = _routing_outputs(routed_if)
        if want_outputs != got_outputs:
            report.fail(
                f"routed job {routed_job!r} gates on outputs {sorted(got_outputs)!r}; "
                f"manifest requires {sorted(want_outputs)!r}"
            )

    if contract.get("pull_request_paths_filter") == "forbidden":
        _check_pull_request_trigger(context, path, document, report)

    if contract.get("declares_protected_paths"):
        _check_protected_path_declaration(context, path, source, protected_paths, report)


def _check_pull_request_trigger(
    context: str, path: str, document: dict[str, Any], report: Report
) -> None:
    triggers = document.get("on")
    if not isinstance(triggers, dict):
        report.fail(f"{path} has no mapping `on:` block; the trigger contract is unverifiable")
        return
    if "pull_request" not in triggers:
        report.fail(f"{path} has no pull_request trigger, so {context!r} cannot be emitted")
        return
    pull_request = triggers["pull_request"]
    if pull_request is None:
        return
    if not isinstance(pull_request, dict):
        report.fail(f"{path} pull_request trigger is not a mapping")
        return
    for filter_key in ("paths", "paths-ignore"):
        if filter_key in pull_request:
            report.fail(
                f"{path} carries a workflow-level pull_request `{filter_key}:` filter; "
                f"{context!r} is required, so a filtered pull request would never emit it "
                "and the branch would be permanently unmergeable"
            )


def _check_protected_path_declaration(
    context: str, path: str, source: str, protected_paths: list[str], report: Report
) -> None:
    """The audit gate's enforced set and the manifest's must be IDENTICAL.

    Containment in one direction is not enough, and reading it that way is what
    let the two sets drift apart unnoticed. A path the gate enforces but the
    manifest omits is invisible to a manifest-subset-of-gate check, yet it makes
    every manifest consumer (`fork-health.sh` most of all) report a smaller
    boundary than the one actually in force: a change touching only that path
    reads clean locally and is then rejected by the gate.

    So parse the gate's own `protected=( ... )` array and compare sets both
    ways. Substring matching cannot do this: it can tell whether a path appears
    somewhere in the file, never whether the file enforces something extra.
    """
    try:
        enforced = _parse_protected_array(source)
    except SchemaError as exc:
        report.fail(f"{context!r} at {path}: {exc}")
        return

    declared = {p.rstrip("/") for p in protected_paths}
    enforced = {p.rstrip("/") for p in enforced}

    missing = sorted(declared - enforced)
    if missing:
        report.fail(
            f"{context!r} at {path} does not name protected path(s) {missing!r}; "
            "the audit gate would stay green on a change it is supposed to flag"
        )
    extra = sorted(enforced - declared)
    if extra:
        report.fail(
            f"{context!r} at {path} enforces protected path(s) {extra!r} that "
            "scripts/required-checks.json does not declare; every manifest "
            "consumer would report a smaller protected set than the gate "
            "actually enforces, so a change touching only those paths reads "
            "clean and is then rejected by the gate"
        )
    if not missing and not extra:
        report.ok(
            f"{context!r} at {path} enforces exactly the {len(enforced)} "
            "protected path(s) the manifest declares"
        )


def _parse_protected_array(source: str) -> set[str]:
    """Extract the paths from the audit gate's inline `protected=( ... )` array.

    A zero-pattern parse is an artifact, never an answer: it would make the
    equality check above compare against an empty set and quietly agree with
    anything. Raise instead.
    """
    matches = re.findall(r"protected=\(\s*(.*?)\s*\)", source, re.DOTALL)
    if not matches:
        raise SchemaError(
            "no `protected=( ... )` array found; the audit gate's enforced "
            "protected-path set is unreadable, so it cannot be compared"
        )
    if len(matches) > 1:
        raise SchemaError(
            f"found {len(matches)} `protected=( ... )` arrays; the enforced "
            "protected-path set is ambiguous"
        )
    paths = {token for token in matches[0].split() if token}
    if not paths:
        raise SchemaError(
            "`protected=( ... )` array is empty; the audit gate enforces "
            "nothing and would stay green on every governance change"
        )
    return paths


# ---------------------------------------------------------------------------
# Protected paths
# ---------------------------------------------------------------------------


# Protected paths are declared relative to the root of the repository being
# governed, which is the repository this script lives in (<root>/scripts/).
# Deliberately not derived from --manifest: the manifest may legitimately be a
# copy in a scratch directory (tests, or a proposed-manifest dry run), and the
# paths it names still refer to the real tree.
_REPO_ROOT = Path(__file__).resolve().parent.parent


def manifest_root() -> Path | None:
    return _REPO_ROOT


def check_protected_paths(manifest: dict[str, Any], report: Report) -> list[str]:
    """Return the protected list the audit gate is required to declare.

    `proposed_additions` are Stream G's finding about helper scripts that
    execute inside required contexts but are missing from design.md section 4's
    list. Until the integration gate adjudicates them they are reported, not
    enforced: enforcing an unadjudicated list here would let this file widen a
    governance boundary the design fixed elsewhere.
    """
    protected = manifest["protected_paths"]
    required = list(protected["required"])
    additions = list(protected.get("proposed_additions") or [])

    # A protected path that does not exist protects nothing, and reads as
    # coverage. Typos here fail silently in the worst direction, so treat an
    # unresolvable path as a schema error rather than a mismatch: it means the
    # manifest is wrong, not that the remote surface drifted.
    root = manifest_root()
    if root is not None:
        missing = [p for p in required + additions if not (root / p).exists()]
        if missing:
            raise SchemaError(
                "protected_paths names path(s) that do not exist in the working "
                "tree: " + ", ".join(sorted(missing))
            )

    if protected.get("additions_adjudicated"):
        required.extend(additions)
        report.note(
            f"protected-path additions are adjudicated; enforcing {len(required)} paths"
        )
    elif additions:
        report.note(
            f"{len(additions)} proposed protected-path addition(s) are pending adjudication "
            "and are reported, not enforced (see "
            "docs/fork/ideal-base/evidence/R07/stream-g-protected-paths-proposal.md)"
        )
    return required


# ---------------------------------------------------------------------------
# Live acquisition
# ---------------------------------------------------------------------------
#
# Acquisition lives here rather than in the shell entry point so that the
# snapshot schema has exactly one definition, and so every `gh` invocation is a
# bare `gh api <path>` returning raw JSON. No `--jq`, no `--paginate`: the
# transformations happen in Python where they are testable, and a test shim only
# has to key on the request path. A shim that had to reimplement jq filters
# would be testing the shim.


class AcquisitionError(Exception):
    """A live endpoint could not be read; exit 2 with the endpoint named."""

    def __init__(self, endpoint: str, detail: str) -> None:
        super().__init__(f"live governance acquisition failed at endpoint: {endpoint}\n      {detail}")
        self.endpoint = endpoint


def gh_binary() -> str:
    return os.environ.get("FORK_HEALTH_GH", "gh")


def gh_api(path: str, *, allow_404: bool = False) -> Any:
    """Read one endpoint. Any failure other than a permitted 404 is fatal."""
    binary = gh_binary()
    if shutil.which(binary) is None and not Path(binary).exists():
        raise AcquisitionError(f"GET {path}", f"{binary} is not on PATH")
    try:
        completed = subprocess.run(
            [binary, "api", path],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as exc:
        raise AcquisitionError(f"GET {path}", str(exc)) from exc

    if completed.returncode != 0:
        combined = (completed.stderr or "") + (completed.stdout or "")
        if allow_404 and "404" in combined:
            return None
        raise AcquisitionError(f"GET {path}", combined.strip() or f"exit {completed.returncode}")

    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise AcquisitionError(f"GET {path}", f"response was not JSON: {exc}") from exc


def acquire_live(repo: str, target_branch: str) -> dict[str, Any]:
    binary = gh_binary()
    if shutil.which(binary) is None and not Path(binary).exists():
        raise AcquisitionError("gh", f"{binary} is not on PATH")
    try:
        auth = subprocess.run(
            [binary, "auth", "status"], capture_output=True, text=True, check=False
        )
    except OSError as exc:
        raise AcquisitionError("gh auth status", str(exc)) from exc
    if auth.returncode != 0:
        raise AcquisitionError(
            "gh auth status",
            (auth.stderr or auth.stdout or "").strip() or "gh is not authenticated",
        )

    repository = gh_api(f"repos/{repo}")
    ruleset_index = gh_api(f"repos/{repo}/rulesets")
    if not isinstance(ruleset_index, list):
        raise AcquisitionError(f"GET repos/{repo}/rulesets", "response was not a list")

    rulesets = []
    for entry in ruleset_index:
        if not isinstance(entry, dict) or "id" not in entry:
            raise AcquisitionError(f"GET repos/{repo}/rulesets", "index entry has no id")
        rulesets.append(gh_api(f"repos/{repo}/rulesets/{entry['id']}"))

    effective = gh_api(f"repos/{repo}/rules/branches/{target_branch}")
    # Absent classic protection is the healthy state, so a 404 is the answer,
    # not a failure. Every other error still stops the run.
    classic = gh_api(f"repos/{repo}/branches/{target_branch}/protection", allow_404=True)

    branch_index = gh_api(f"repos/{repo}/branches?per_page=100")
    if not isinstance(branch_index, list):
        raise AcquisitionError(f"GET repos/{repo}/branches", "response was not a list")
    branches = []
    for entry in branch_index:
        if not isinstance(entry, dict) or "name" not in entry:
            raise AcquisitionError(f"GET repos/{repo}/branches", "branch entry has no name")
        branches.append(str(entry["name"]))

    return {
        "repository": repository,
        "rulesets": rulesets,
        "effective_main_rules": effective,
        "classic_branch_protection": classic,
        "branches": branches,
    }


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def load_json(path: Path, label: str) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise SchemaError(f"{label} not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise SchemaError(f"{label} is not valid JSON: {path}: {exc}") from exc


def resolve_workflows(snapshot: dict[str, Any], workflows_dir: Path | None) -> dict[str, str]:
    embedded = snapshot.get("workflows")
    if embedded is not None:
        if not isinstance(embedded, dict):
            raise SchemaError("snapshot.workflows is not an object")
        return {str(k): str(v) for k, v in embedded.items()}
    if workflows_dir is None:
        raise SchemaError("snapshot has no workflows and no --workflows-dir was given")
    if not workflows_dir.is_dir():
        raise SchemaError(f"workflow directory not found: {workflows_dir}")
    found = {}
    for entry in sorted(workflows_dir.glob("*.yml")):
        # Key by the canonical repository-relative path the manifest uses. The
        # directory may be a scratch copy (tests, or a pre-apply dry run); what
        # the contract identifies is the workflow, not where it was read from.
        found[f".github/workflows/{entry.name}"] = entry.read_text(encoding="utf-8")
    if not found:
        raise SchemaError(f"no workflow files under {workflows_dir}")
    return found


def compare(manifest: dict[str, Any], snapshot: dict[str, Any], workflows: dict[str, str]) -> Report:
    report = Report()
    for key in REQUIRED_SNAPSHOT_KEYS:
        if key not in snapshot:
            raise SchemaError(f"snapshot is missing required key {key!r}")

    protected_paths = check_protected_paths(manifest, report)

    check_repository(manifest, snapshot, report)
    check_rulesets(manifest, snapshot, report)
    check_effective_main_rules(manifest, snapshot, report)
    check_classic_protection(manifest, snapshot, report)
    check_branches(manifest, snapshot, report)
    check_ruleset_rails(manifest, snapshot, report)
    check_workflow_contracts(manifest, workflows, protected_paths, report)
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Compare a governance snapshot against scripts/required-checks.json"
    )
    parser.add_argument("--manifest", required=True, type=Path)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--snapshot", type=Path, help="compare an on-disk aggregate snapshot")
    source.add_argument("--live", action="store_true", help="acquire the snapshot from GitHub")
    parser.add_argument(
        "--workflows-dir",
        type=Path,
        default=None,
        help="read workflow text from disk when the snapshot does not embed it",
    )
    parser.add_argument(
        "--dump-snapshot",
        type=Path,
        default=None,
        help="write the acquired live snapshot here (sanitized), for evidence transcripts",
    )
    args = parser.parse_args(argv)

    try:
        manifest = load_json(args.manifest, "manifest")
        if args.live:
            snapshot = acquire_live(manifest["repository"], manifest["target_branch"])
            if args.dump_snapshot is not None:
                args.dump_snapshot.write_text(
                    json.dumps(sanitize(snapshot), indent=2, sort_keys=True) + "\n",
                    encoding="utf-8",
                )
        else:
            snapshot = load_json(args.snapshot, "snapshot")
        if manifest.get("schema_version") != 1:
            raise SchemaError(
                f"manifest schema_version is {manifest.get('schema_version')!r}; this "
                "comparator implements version 1"
            )
        workflows = resolve_workflows(snapshot, args.workflows_dir)
        report = compare(manifest, snapshot, workflows)
    except SchemaError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2
    except WorkflowParseError as exc:
        print(f"ERROR: workflow extraction failed: {exc}", file=sys.stderr)
        return 2
    except AcquisitionError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2

    for message in report.passes:
        print(f"OK:   {message}")
    for message in report.notes:
        print(f"NOTE: {message}")
    for message in report.failures:
        print(f"FAIL: {message}", file=sys.stderr)

    if report.failures:
        print(
            f"=== Governance: {len(report.failures)} mismatch(es) against the manifest ===",
            file=sys.stderr,
        )
        return 1
    print("=== Governance: snapshot matches the manifest ===")
    return 0


if __name__ == "__main__":
    sys.exit(main())
