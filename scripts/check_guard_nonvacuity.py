#!/usr/bin/env python3
"""Prove, per pull request, that every guard this repository claims is
load-bearing can still fail.

Background. D034 recorded that commit 621f4d44d shrank the `Governance Root`
protected-path set from 27 entries to 5, so a pull request could weaken a
guard's comparison logic and edit that guard's own test in the same change with
no trip-wire. Reproducing that hole showed the defect is larger than D034
stated: most guards are not wired into any pull-request-blocking check at all,
and three of them fail against clean `main` right now while `main` is green. A
guard nothing runs and a guard whose comparison never trips are the same
failure -- a guard that cannot fail -- and neither is visible from a green tick.

The control. This file is the repository's registry of that claim. For every
guard it records `gating` or `dormant`. For every `gating` guard it asserts:

  wiring    the invocation site the registry names still exists, so a guard
            cannot be silently unplugged from CI;
  plant     the guard rejects a known defect, so a weakened comparison is
            caught;
  clean     the guard accepts the same input with the defect removed, so a
            guard that fails unconditionally cannot masquerade as proof.

The plant and clean pair is the point. F23's earlier harness went vacuous by
reading exit-2 crashes as proof of rejection, and `Fork Health` reported 19 days
of false success because a pipeline swallowed exit 2. Rejection is therefore
asserted as a specific outcome carrying specific evidence, never as "not zero",
and every rejection is paired with an acceptance that must hold.

Why this file rather than a digest manifest. A digest of guard sources forces a
ruleset maintenance window on every guard edit, which is the ergonomic cost that
621f4d44d removed, and it would pin the contents of scripts nothing executes.
This file protects the *claim* -- what gates, where it is wired, what must make
it fail -- and leaves guard logic and guard tests unprotected. Routine guard
work needs no window; changing the claim does.

Plant data is defined here rather than in fixture files on purpose. A plant
expressed as a file outside this module could be weakened in the same pull
request that weakens the guard, and the two would agree with each other.

Exit codes: 0 all claims hold, 1 a claim failed, 2 the harness itself could not
run. Callers must distinguish 1 from 2; `main()` never lets an internal error
surface as a passing run.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import importlib.util
import io
import os
import re
import shutil
import sys
import tempfile
import textwrap
import traceback
from contextlib import redirect_stdout
from pathlib import Path
from typing import Any, Callable

REPO_ROOT = Path(__file__).resolve().parent.parent

# Guards that run inside a pull-request-blocking check. Keep this in step with
# GUARDS below; `_check_registry_covers_every_guard` fails if a guard script
# exists on disk and is absent here.
GATING = "gating"
DORMANT = "dormant"


@dataclasses.dataclass(frozen=True)
class Wiring:
    """One place the registry claims a guard is invoked from.

    `where` is a repository-relative file. `recipe` narrows the search to a
    single justfile recipe body, so moving an invocation out of `check` into an
    unrelated recipe is caught -- a bare grep over the whole file would not see
    the difference.
    """

    where: str
    must_contain: str
    recipe: str | None = None


@dataclasses.dataclass(frozen=True)
class Guard:
    script: str
    status: str
    # gating guards only
    wiring: tuple[Wiring, ...] = ()
    plant: Callable[[], "Outcome"] | None = None
    # dormant guards only
    reason: str = ""


@dataclasses.dataclass(frozen=True)
class Outcome:
    """Result of running a guard against planted and clean input.

    `rejected` and `accepted` are the two directions. `evidence` is the message
    the guard produced when rejecting; a rejection with no evidence is treated
    as a crash, not as proof.
    """

    rejected: bool
    accepted: bool
    evidence: str
    detail: str = ""


def _load(script: str) -> Any:
    """Import a guard as a module without requiring it to be importable by name."""

    path = REPO_ROOT / script
    spec = importlib.util.spec_from_file_location(f"_guard_{path.stem}", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {script}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# --------------------------------------------------------------------------
# Plants. Each returns an Outcome carrying both directions.
# --------------------------------------------------------------------------


def plant_critical_path_budget() -> Outcome:
    """The D034 exemplar.

    D034's reproduction relaxed `value > REPOSITORY_CEILINGS[key]` to
    `... * 2`. The plant sits one over each recorded high-water mark, so it is
    expressed relative to the guard's own constants: raising a ceiling (which
    the `--expect-digest` pin already covers) moves the plant with it and stays
    honest, while widening the comparison stops the plant tripping and turns
    this check red.
    """

    guard = _load("scripts/check_critical_path_budget.py")
    ceilings: dict[str, int] = guard.REPOSITORY_CEILINGS

    over = {key: value + 1 for key, value in ceilings.items()}
    at = dict(ceilings)

    regressions = guard.repository_trend_regressions(over)
    clean = guard.repository_trend_regressions(at)

    return Outcome(
        rejected=len(regressions) == len(over) and all(regressions),
        accepted=clean == [],
        evidence="; ".join(regressions[:2]),
        detail=(
            f"planted {len(over)} budgets one over their mark, "
            f"guard reported {len(regressions)}; "
            f"at-the-mark reported {len(clean)} (want 0)"
        ),
    )


def plant_scope_shrink() -> Outcome:
    """A scope shrink must not read as cleanup.

    Same guard, second comparison. Weakening only `repository_trend_regressions`
    would leave this one intact, so both are planted rather than trusting one to
    stand for the file.
    """

    guard = _load("scripts/check_critical_path_budget.py")
    expected: dict[str, int] = guard.EXPECTED_FILE_COUNTS

    shrunk = {domain: count - 1 for domain, count in expected.items()}
    same = dict(expected)

    regressions = guard.scope_shrink_regressions(shrunk)
    clean = guard.scope_shrink_regressions(same)

    return Outcome(
        rejected=len(regressions) == len(shrunk) and all(regressions),
        accepted=clean == [],
        evidence="; ".join(regressions[:2]),
        detail=(
            f"planted a one-file shrink in {len(shrunk)} domains, "
            f"guard reported {len(regressions)}; "
            f"unchanged reported {len(clean)} (want 0)"
        ),
    )


_AUDIT_CLEAN = "[advisories]\nignore = []\n"
_AUDIT_PLANTED = '[advisories]\nignore = ["RUSTSEC-2021-0000"]\n'
_RECORD_EMPTY = "[policy]\nmax_expiry_days = 365\n"


def plant_advisory_policy() -> Outcome:
    """An advisory suppressed with no ownership record must be rejected.

    `check_advisory_policy.py` takes a root, so the plant is a two-file tree in
    a temporary directory: an ignore in `.cargo/audit.toml` with no matching
    record in `docs/security/advisories.toml`. The clean control is the same
    tree with the ignore removed, which must produce no findings -- if it did,
    the guard would be failing for a reason unrelated to the plant and its
    rejection would prove nothing.
    """

    guard = _load("scripts/check_advisory_policy.py")
    today = dt.date(2026, 1, 1)

    def run(audit: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".cargo").mkdir()
            (root / "docs" / "security").mkdir(parents=True)
            (root / ".cargo" / "audit.toml").write_text(audit, encoding="utf-8")
            (root / "docs" / "security" / "advisories.toml").write_text(
                _RECORD_EMPTY, encoding="utf-8"
            )
            return guard.check(root, today)

    planted = run(_AUDIT_PLANTED)
    clean = run(_AUDIT_CLEAN)

    return Outcome(
        rejected=bool(planted),
        accepted=clean == [],
        evidence="; ".join(planted[:2]),
        detail=(
            f"planted an undocumented advisory ignore, guard reported "
            f"{len(planted)} finding(s); clean tree reported {len(clean)} "
            f"(want 0){': ' + '; '.join(clean[:2]) if clean else ''}"
        ),
    )


def plant_governance_compare() -> Outcome:
    """A ruleset that drifted from its manifest must be reported.

    `check_repository` compares a recorded manifest against an observed
    snapshot. The plant points the snapshot at a different repository and flips
    a merge setting; the clean control makes the two agree.
    """

    guard = _load("scripts/governance_compare.py")

    merge_methods = {
        "allow_merge_commit": True,
        "allow_squash_merge": False,
        "allow_rebase_merge": False,
    }
    manifest = {
        "repository_id": 1,
        "repository": "jerudnik/jcode",
        "repository_merge_methods": merge_methods,
    }
    agreeing = {"repository": {"id": 1, "full_name": "jerudnik/jcode", **merge_methods}}
    drifted = {
        "repository": {
            "id": 1,
            "full_name": "someone-else/jcode",
            **{**merge_methods, "allow_squash_merge": True},
        }
    }

    planted_report = guard.Report()
    guard.check_repository(manifest, drifted, planted_report)

    clean_report = guard.Report()
    guard.check_repository(manifest, agreeing, clean_report)

    return Outcome(
        rejected=len(planted_report.failures) >= 2,
        accepted=clean_report.failures == [],
        evidence="; ".join(planted_report.failures[:2]),
        detail=(
            f"planted an identity drift and a merge-method drift, guard reported "
            f"{len(planted_report.failures)} failure(s) (want >= 2); agreeing "
            f"snapshot reported {len(clean_report.failures)} (want 0)"
        ),
    )


# --------------------------------------------------------------------------
# The registry.
# --------------------------------------------------------------------------

GUARDS: tuple[Guard, ...] = (
    Guard(
        script="scripts/check_critical_path_budget.py",
        status=GATING,
        wiring=(
            Wiring(
                where="justfile",
                recipe="check",
                must_contain="scripts/check_critical_path_budget.py",
            ),
            Wiring(where=".github/workflows/fork-ci.yml", must_contain="just check"),
        ),
        plant=plant_critical_path_budget,
    ),
    Guard(
        script="scripts/check_critical_path_budget.py::scope_shrink",
        status=GATING,
        wiring=(),  # same file as above; wiring asserted once
        plant=plant_scope_shrink,
    ),
    Guard(
        script="scripts/check_advisory_policy.py",
        status=GATING,
        wiring=(
            Wiring(
                where=".github/workflows/security.yml",
                must_contain="scripts/check_advisory_policy.py",
            ),
        ),
        plant=plant_advisory_policy,
    ),
    Guard(
        script="scripts/governance_compare.py",
        status=GATING,
        wiring=(
            Wiring(
                where=".github/workflows/governance-root.yml",
                must_contain="scripts/governance_compare.py",
            ),
        ),
        plant=plant_governance_compare,
    ),
    Guard(
        script="scripts/security_preflight.sh",
        status=GATING,
        wiring=(
            Wiring(
                where=".github/workflows/security.yml",
                must_contain="scripts/security_preflight.sh",
            ),
        ),
        plant=None,
        reason=(
            "resolves its own repo root from BASH_SOURCE and shells out to "
            "gitleaks and cargo-audit, so a plant would need a fake secret "
            "committed to a copy of the tree. Wiring is asserted; non-vacuity "
            "is an open residual, recorded in DECISIONS.md."
        ),
    ),
    # ------------------------------------------------------------------
    # Dormant. None of these run in a pull-request-blocking check. Recorded
    # rather than deleted so the gap is visible and so moving one to `gating`
    # is an explicit edit to this table rather than a silent one.
    # The three marked "fails against clean main" were verified failing while
    # main was green, which is the evidence that they gate nothing today.
    # ------------------------------------------------------------------
    Guard("scripts/check_agent_instructions.py", DORMANT,
          reason="flake check `agent-instructions`; nix.yml builds only nix-distribution-policy"),
    Guard("scripts/check_reusable_workflow_calls.py", DORMANT,
          reason="flake check `workflow-syntax`; never built in a PR-blocking job"),
    Guard("scripts/check_workflow_permissions.py", DORMANT,
          reason="flake check `workflow-syntax`; never built in a PR-blocking job"),
    Guard("scripts/check_ambient_roots.sh", DORMANT,
          reason="referenced only by its own allowlist data file; nothing executes it"),
    Guard("scripts/check_branch_handoff.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs"),
    Guard("scripts/check_code_size_budget.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, which no workflow runs; and it exits 1 against clean main, verified while main was green -- proof it gates nothing today"),
    Guard("scripts/check_config_env_lease.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs"),
    Guard("scripts/check_dependency_boundaries.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs"),
    Guard("scripts/check_docs_references.py", DORMANT,
          reason="invoked only from scripts/d01_scoreboard.sh, which no workflow runs"),
    Guard("scripts/check_env_lease_drop_order.py", DORMANT,
          reason="nothing in the repository references it"),
    Guard("scripts/check_panic_budget.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs"),
    Guard("scripts/check_real_home_isolation.sh", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs; also scripts/run_lifecycle_matrix.sh, likewise unrun"),
    Guard("scripts/check_startup_budget.sh", DORMANT,
          reason="invoked only from scripts/test_fast.sh, which no workflow runs"),
    Guard("scripts/check_swallowed_error_budget.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, which no workflow runs; and it exits 1 against clean main, verified while main was green -- proof it gates nothing today"),
    Guard("scripts/check_test_size_budget.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, which no workflow runs; and it exits 1 against clean main, verified while main was green -- proof it gates nothing today"),
    Guard("scripts/check_tui_render_lock.py", DORMANT,
          reason="nothing in the repository references it"),
    Guard("scripts/check_warning_budget.sh", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs; its test scripts/test_warning_budget.py is itself unrun"),
    Guard("scripts/check_web_mobile.sh", DORMANT,
          reason="nothing in the repository references it"),
    Guard("scripts/check_wildcard_reexport_budget.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs"),
    Guard("scripts/generate_governance_fixture.py", DORMANT,
          reason="fixture generator, not a guard"),
    Guard("scripts/tb_compare.py", DORMANT,
          reason="trace-buffer comparison utility, not a guard"),
    Guard("scripts/check_guard_nonvacuity.py", DORMANT,
          reason=(
              "this harness. It is wired into `just check`, but it cannot be "
              "the thing that proves its own wiring: if the invocation is "
              "removed, nothing runs to notice. That assertion therefore "
              "lives in tests/test_guard_nonvacuity.py, which the same recipe "
              "runs on the line above -- so deleting only the harness is "
              "caught, and deleting both is a two-line diff in the most-read "
              "recipe in the repository. That residue is the root of trust "
              "and is recorded as such rather than papered over."
          )),
    Guard("scripts/rust_production_filter.py", DORMANT,
          reason="shared scanner imported by budget guards, not a guard itself"),
)


# --------------------------------------------------------------------------
# Claim checks.
# --------------------------------------------------------------------------


def _justfile_recipe_body(source: str, recipe: str) -> str | None:
    """Return the indented body of one justfile recipe, or None if absent."""

    lines = source.splitlines()
    for index, line in enumerate(lines):
        if re.match(rf"^{re.escape(recipe)}\s*:", line):
            body: list[str] = []
            for following in lines[index + 1 :]:
                if following and not following[0].isspace():
                    break
                body.append(following)
            return "\n".join(body)
    return None


def _check_wiring(guard: Guard) -> list[str]:
    failures: list[str] = []
    for wiring in guard.wiring:
        path = REPO_ROOT / wiring.where
        if not path.exists():
            failures.append(f"{guard.script}: wiring file {wiring.where} is missing")
            continue
        source = path.read_text(encoding="utf-8")
        if wiring.recipe is not None:
            body = _justfile_recipe_body(source, wiring.recipe)
            if body is None:
                failures.append(
                    f"{guard.script}: {wiring.where} has no `{wiring.recipe}` recipe"
                )
                continue
            source = body
        if wiring.must_contain not in source:
            location = (
                f"{wiring.where} recipe `{wiring.recipe}`"
                if wiring.recipe
                else wiring.where
            )
            failures.append(
                f"{guard.script}: {location} no longer invokes "
                f"{wiring.must_contain!r}; the guard has been unplugged from CI"
            )
    return failures


def _check_registry_covers_every_guard() -> list[str]:
    """A guard added to the tree and never registered is invisible to this check."""

    registered = {
        guard.script.split("::", 1)[0] for guard in GUARDS
    }
    on_disk = {
        str(path.relative_to(REPO_ROOT))
        for pattern in ("check_*.py", "check_*.sh", "*_compare.py", "*_preflight.sh",
                        "*_filter.py", "generate_governance_fixture.py")
        for path in (REPO_ROOT / "scripts").glob(pattern)
    }
    missing = sorted(on_disk - registered)
    stale = sorted(registered - on_disk)
    failures = [
        f"{script}: exists in scripts/ but is not registered; add it as gating "
        f"or dormant"
        for script in missing
    ]
    failures += [
        f"{script}: registered but no longer exists in scripts/" for script in stale
    ]
    return failures


def _run_plant(guard: Guard) -> list[str]:
    """Run one plant, converting a harness crash into a claim failure, never a pass."""

    assert guard.plant is not None
    buffer = io.StringIO()
    try:
        with redirect_stdout(buffer):
            outcome = guard.plant()
    except Exception:  # noqa: BLE001 - a crashing plant is a failed claim
        return [
            f"{guard.script}: plant raised instead of returning a verdict, so it "
            f"proves nothing:\n"
            + textwrap.indent(traceback.format_exc().strip(), "      ")
        ]

    failures: list[str] = []
    if not outcome.rejected:
        failures.append(
            f"{guard.script}: the guard ACCEPTED a planted defect. Its comparison "
            f"no longer fails on input it is supposed to reject. {outcome.detail}"
        )
    elif not outcome.evidence.strip():
        failures.append(
            f"{guard.script}: the guard reported a rejection with no message. A "
            f"rejection carrying no evidence is indistinguishable from a crash. "
            f"{outcome.detail}"
        )
    if not outcome.accepted:
        failures.append(
            f"{guard.script}: the guard REJECTED the clean control. A guard that "
            f"fails unconditionally cannot prove anything by failing. "
            f"{outcome.detail}"
        )
    return failures


def run() -> tuple[list[str], list[str]]:
    """Return (failures, passes)."""

    failures = _check_registry_covers_every_guard()
    passes: list[str] = []

    for guard in GUARDS:
        if guard.status == DORMANT:
            continue
        if guard.status != GATING:
            failures.append(f"{guard.script}: unknown status {guard.status!r}")
            continue

        wiring_failures = _check_wiring(guard)
        failures += wiring_failures
        if guard.wiring and not wiring_failures:
            passes.append(f"{guard.script}: wiring intact")

        if guard.plant is None:
            if not guard.reason:
                failures.append(
                    f"{guard.script}: gating with no plant and no recorded reason"
                )
            continue

        plant_failures = _run_plant(guard)
        failures += plant_failures
        if not plant_failures:
            passes.append(f"{guard.script}: rejects its plant, accepts the control")

    return failures, passes


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--list",
        action="store_true",
        help="print the registry and exit without running plants",
    )
    args = parser.parse_args(argv)

    if args.list:
        gating = [g for g in GUARDS if g.status == GATING]
        dormant = [g for g in GUARDS if g.status == DORMANT]
        print(f"gating ({len(gating)}):")
        for guard in gating:
            mark = "plant" if guard.plant else "wiring only"
            print(f"  {guard.script}  [{mark}]")
        print(f"dormant ({len(dormant)}):")
        for guard in dormant:
            print(f"  {guard.script}  -- {guard.reason}")
        return 0

    try:
        failures, passes = run()
    except Exception:  # noqa: BLE001 - never let an internal error read as success
        traceback.print_exc()
        print("guard non-vacuity: HARNESS ERROR (exit 2)", file=sys.stderr)
        return 2

    for line in passes:
        print(f"ok    {line}")
    for line in failures:
        print(f"FAIL  {line}", file=sys.stderr)

    if failures:
        print(
            f"\nguard non-vacuity: {len(failures)} claim(s) failed. Each line above "
            f"is a guard this repository says is load-bearing that cannot be shown "
            f"to fail. Fix the guard, or amend its entry in "
            f"scripts/check_guard_nonvacuity.py -- an edit that shows up in the "
            f"diff as a weakened claim rather than as an unchanged green tick.",
            file=sys.stderr,
        )
        return 1

    print(f"\nguard non-vacuity: {len(passes)} claim(s) hold.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
