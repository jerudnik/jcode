#!/usr/bin/env python3
"""Prove, per pull request, that every guard this repository claims is
load-bearing can still fail.

Background. D034 recorded that commit 621f4d44d shrank the `Governance Root`
protected-path set from 32 entries to 5, so a pull request could weaken a
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

What this does not cover, stated so the gap is inherited rather than
rediscovered. A claim binds one guard to one defect, so a defect no claim names
passes. Guard dependencies are covered only where a claim names them: the budget
guard's line classifier is named, because weakening it moves lifecycle panics
11 -> 0 and swallowed errors 440 -> 91 with the digest, the file counts and the
exit status unchanged. Nothing enumerates the rest. The registry pins behaviour,
not bytes, so a rewrite preserving the planted behaviour and losing everything
else is invisible here. Widening coverage means adding claims, and a claim that
cannot be shown red is not worth adding.

Plant data is defined here rather than in fixture files on purpose. A plant
expressed as a file outside this module could be weakened in the same pull
request that weakens the guard, and the two would agree with each other.

Exit codes: 0 all claims hold, 1 a claim failed, 2 the harness itself could not
run. Callers must distinguish 1 from 2; `main()` never lets an internal error
surface as a passing run.
"""

from __future__ import annotations

import sys as _sys

# Scrub `scripts/` from sys.path before importing anything else. Python puts
# this file's own directory at the front of sys.path before a line of it runs,
# so `scripts/ast.py` captures the `import ast` below -- and this file's whole
# job is to detect exactly that class of capture. Scanning our own imports is
# not enough: a blinder narrow enough to spare this file is caught by that
# self-scan, so the attacker widens it. Measured on this tree with the scrub
# removed: one `scripts/ast.py` returning empty parse trees, and this file
# printed "10 claim(s) hold" and exited 0. `sys` is a builtin module -- it is
# resolved before sys.path is consulted and cannot be shadowed -- so scrubbing
# through it first is sound. `PYTHONSAFEPATH` / `-P` would do this at the
# interpreter level, but they landed in 3.11 and this tree runs 3.9, so there is
# no interpreter switch to lean on (confirmed independently by piglet).
# Also drops "" (cwd), which shadows the same way when the harness is imported
# rather than run as a script.
_SCRIPTS_DIR = __file__.rsplit("/", 1)[0] if "/" in __file__ else "."
_sys.path[:] = [p for p in _sys.path if p not in ("", _SCRIPTS_DIR)]

import argparse
import ast
import dataclasses
import datetime as dt
import importlib.util
import io
import json
import os
import re
import shutil
import sys
import tempfile
import types
import textwrap
import traceback
from contextlib import contextmanager, redirect_stderr, redirect_stdout
from pathlib import Path
from typing import Any, Callable

REPO_ROOT = Path(__file__).resolve().parent.parent

# Guards that run inside a pull-request-blocking check. Keep this in step with
# GUARDS below; `_check_registry_covers_every_guard` fails if a guard script
# exists on disk and is absent here.
#
# This list is what `_check_dormancy_is_still_true` searches, so a site missing
# from it is a place a guard can run while still being labelled dormant. It
# held three entries and missed five workflows that gate a merge, which is how
# `check_docs_references.py`, `check_reusable_workflow_calls.py` and
# `check_workflow_permissions.py` sat at DORMANT while running on every
# pull request. The justfile entry also searched only the `check` recipe body,
# so `lint-docs` -- which the Docs lint job runs on every pull request -- was
# invisible; it is now the whole file.
#
# The rule for adding one: if a job in the file can turn `PR Gate` red, it
# belongs here. `fork-health.yml`, `main.yml`, `scheduled.yml`, `release.yml`
# and `nix-update.yml` do not run on `pull_request` and so are deliberately
# absent.
BLOCKING_SITES = (
    ("justfile", "a recipe PR Gate runs"),
    (".github/workflows/security.yml", "the security workflow"),
    (".github/workflows/governance-root.yml", "the Governance Root workflow"),
    (".github/workflows/ci.yml", "the PR Gate reusable workflow"),
    (".github/workflows/pr.yml", "the PR workflow"),
    (".github/workflows/fork-ci.yml", "the Fork CI workflow"),
    (".github/workflows/nix.yml", "the Nix workflow"),
    (".github/workflows/freebsd-smoke.yml", "the smoke workflow"),
)


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


@contextmanager
def _guard_import_path() -> Any:
    """Put `scripts/` back on sys.path for the duration of a guard's execution.

    This module scrubs `scripts/` from sys.path at import so its own imports
    cannot be captured by a file in the directory it is policing. Guards,
    however, genuinely run as `python3 scripts/...` in CI, with that directory
    first on the path -- `check_critical_path_budget.py` imports
    `rust_production_filter` and would not otherwise resolve it. Running a
    plant under the real path is the faithful reproduction; the detection above
    stays in this module's own scope, where the path is clean.
    """

    _sys.path.insert(0, _SCRIPTS_DIR)
    try:
        yield
    finally:
        try:
            _sys.path.remove(_SCRIPTS_DIR)
        except ValueError:  # pragma: no cover - a guard mutated sys.path
            pass


def _exec_module(source: str, name: str) -> Any:
    """Build a module from source text, for running a hypothetical variant.

    Used by plants that need to compare the real code against a weakened copy
    of it. Nothing is written to disk and the real module is untouched.
    """

    module = types.ModuleType(name)
    module.__file__ = str(REPO_ROOT / "scripts" / f"{name}.py")
    with _guard_import_path():
        exec(compile(source, module.__file__, "exec"), module.__dict__)  # noqa: S102
    return module


def _load(script: str) -> Any:
    """Import a guard as a module without requiring it to be importable by name.

    The module is registered in `sys.modules` under its synthetic name for the
    duration of the load and removed afterwards. That is not tidiness: a module
    defining a dataclass at import time makes `dataclasses` look itself up in
    `sys.modules[cls.__module__]`, which raises `AttributeError: 'NoneType'
    object has no attribute '__dict__'` when the name was never registered. Any
    guard carrying a `@dataclasses.dataclass` was therefore unplantable, which
    quietly limited which guards could be given a plant at all.
    """

    path = REPO_ROOT / script
    name = f"_guard_{path.stem}"
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {script}")
    module = importlib.util.module_from_spec(spec)
    previous = _sys.modules.get(name)
    _sys.modules[name] = module
    try:
        with _guard_import_path():
            spec.loader.exec_module(module)
    finally:
        if previous is None:
            _sys.modules.pop(name, None)
        else:
            _sys.modules[name] = previous
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


_RUST_SAMPLE = """\
fn live_one() { panic!("a"); }

#[cfg(test)]
mod tests {
    #[test]
    fn t() { panic!("b"); }
}

fn live_two() { panic!("c"); }
"""


def plant_production_filter(source: str | None = None) -> Outcome:
    """The guard's input set must keep excluding only the test item.

    `check_critical_path_budget.py` imports `production_lines` from
    `rust_production_filter.py` to decide which lines of a file are production
    code. That module is upstream of every ceiling: it does not set the budget,
    it decides what the budget is measured over. Nothing else guards it. It is
    not in the protected set, `scope_digest()` does not hash it, its own test is
    wired to no blocking job, and -- measured -- broadening its `#[cfg(test)]`
    handling so that any file containing that attribute yields no production
    lines drops lifecycle panics from 11 to 0 and lifecycle swallowed errors
    from 440 to 91, with the in-scope file counts, the digest and the guard's
    exit status all unchanged. The guard passes and reports a healthy budget.

    So the guard cannot be asked to reject that defect; it genuinely does not.
    What can be pinned is the classifier's behaviour. Against a fixed sample the
    two top-level functions are production and the `#[cfg(test)] mod` is not.
    The plant is the weakening itself, applied to the module source: if it
    changes what the sample classifies as production, this fails. That is a
    claim about meaning rather than bytes, so a comment or a rename does not
    trip it, and a widened exclusion does.
    """

    if source is None:
        source = (REPO_ROOT / "scripts/rust_production_filter.py").read_text(
            encoding="utf-8"
        )
    clean_panics = sum(
        1
        for line in _exec_module(source, "_filter").production_lines_from_text(
            _RUST_SAMPLE
        )
        if "panic!" in line
    )

    weakened_source = source.replace(
        "def production_lines_from_text(source: str) -> list[str]:\n"
        "    masked_code = _mask_rust_non_code(source)",
        "def production_lines_from_text(source: str) -> list[str]:\n"
        "    masked_code = _mask_rust_non_code(source)\n"
        '    if "#[cfg(test)]" in masked_code:\n'
        "        return []",
        1,
    )
    if weakened_source == source:
        return Outcome(
            rejected=False,
            accepted=False,
            evidence="",
            detail=(
                "could not apply the weakening: production_lines_from_text no "
                "longer starts with the masking call this plant edits. The "
                "plant, not the filter, needs updating -- but until it is, this "
                "claim is unproven and must not read as green."
            ),
        )

    weakened = _exec_module(weakened_source, "_weakened_filter")
    weak_panics = sum(
        1
        for line in weakened.production_lines_from_text(_RUST_SAMPLE)
        if "panic!" in line
    )

    return Outcome(
        rejected=weak_panics != clean_panics,
        accepted=clean_panics == 2,
        evidence=(
            f"weakened classifier saw {weak_panics} production panic site(s) "
            f"where the current one sees {clean_panics}"
        ),
        detail=(
            f"sample has 2 production panics and 1 test panic; current filter "
            f"reports {clean_panics}, weakened filter reports {weak_panics}. "
            f"Equal counts would mean the exclusion boundary is no longer "
            f"observable, so this claim could not detect a widened one."
        ),
    )


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


def plant_docs_references() -> Outcome:
    """A documentation link pointing at nothing must be reported.

    This guard was registered DORMANT, on the recorded reason that it was
    "invoked only from scripts/d01_scoreboard.sh, which no workflow runs". That
    file does not exist. The guard is invoked from the justfile's `lint-docs`
    recipe, which the Docs lint job runs on every pull request -- so the entry
    claimed no coverage for a guard that had been gating all along, and the
    dormancy check could not see it because its search was narrowed to the
    `check` recipe body.

    Nothing was broken by that; a guard running while labelled dormant still
    runs. It is the accounting that was wrong, and accounting is the whole
    point of this registry.
    """

    guard = _load("scripts/check_docs_references.py")

    def findings(text: str) -> list[Any]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "docs").mkdir()
            page = root / "docs" / "page.md"
            page.write_text(text, encoding="utf-8")
            (root / "docs" / "real.md").write_text("# real\n", encoding="utf-8")
            return guard.check_links(root, page, text)

    planted = findings("# page\n\nSee [the missing one](./nowhere.md).\n")
    clean = findings("# page\n\nSee [the real one](./real.md).\n")

    return Outcome(
        rejected=bool(planted),
        accepted=not clean,
        evidence="; ".join(str(getattr(f, "message", f)) for f in planted),
        detail="; ".join(str(getattr(f, "message", f)) for f in clean),
    )


def plant_reusable_workflow_calls() -> Outcome:
    """A local `uses:` pointing at a workflow that is not there must be reported.

    Registered DORMANT on the reason "flake check `workflow-syntax`; never built
    in a PR-blocking job". It is executed at `.github/workflows/nix.yml:75`, in
    the Validate job, which runs on every pull request that is not docs-only.
    The dormancy check missed it because `nix.yml` was not among the sites it
    searched.
    """

    guard = _load("scripts/check_reusable_workflow_calls.py")

    def check(caller: str, *, with_target: bool) -> str:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            workflows = root / ".github" / "workflows"
            workflows.mkdir(parents=True)
            (workflows / "caller.yml").write_text(caller, encoding="utf-8")
            if with_target:
                (workflows / "target.yml").write_text(
                    "name: Target\non:\n  workflow_call:\njobs:\n"
                    "  work:\n    runs-on: ubuntu-latest\n"
                    "    steps:\n      - run: echo ok\n",
                    encoding="utf-8",
                )
            checker = guard.ReusableWorkflowCallChecker(root)
            try:
                checker.check()
            except guard.ReusableWorkflowCallError as error:
                return str(error)
            return ""

    caller = (
        "name: Caller\non:\n  pull_request:\njobs:\n"
        "  call:\n    uses: ./.github/workflows/target.yml\n"
    )
    planted = check(caller, with_target=False)
    clean = check(caller, with_target=True)

    return Outcome(
        rejected=bool(planted),
        accepted=not clean,
        evidence=planted,
        detail=clean,
    )


def plant_workflow_permissions() -> Outcome:
    """A called workflow needing more token than its caller granted must be reported.

    Registered DORMANT with the same inaccurate reason as its sibling above,
    and executed one line later, at `.github/workflows/nix.yml:77`.

    The property is monotonicity: GitHub validates every job in a called
    workflow against the budget the caller passed, so a callee asking for
    `contents: write` under a caller granting `contents: read` fails at run
    time, not at review time. Planting an escalation rather than an absent
    `permissions:` block is deliberate -- an absent block is refused by a
    different branch of the same guard, and planting it would leave the
    monotonicity comparison itself unproven.
    """

    guard = _load("scripts/check_workflow_permissions.py")

    def findings(granted: str, needed: str) -> list[Any]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            workflows = root / ".github" / "workflows"
            workflows.mkdir(parents=True)
            (workflows / "caller.yml").write_text(
                "name: Caller\non:\n  pull_request:\njobs:\n"
                "  call:\n"
                f"    permissions:\n      contents: {granted}\n"
                "    uses: ./.github/workflows/target.yml\n",
                encoding="utf-8",
            )
            (workflows / "target.yml").write_text(
                "name: Target\non:\n  workflow_call:\njobs:\n"
                "  work:\n    runs-on: ubuntu-latest\n"
                f"    permissions:\n      contents: {needed}\n"
                "    steps:\n      - run: echo ok\n",
                encoding="utf-8",
            )
            checker = guard.WorkflowPermissionChecker(root)
            return list(checker.check())

    planted = findings(granted="read", needed="write")
    clean = findings(granted="write", needed="read")

    return Outcome(
        rejected=bool(planted),
        accepted=not clean,
        evidence="; ".join(str(getattr(f, "message", f)) for f in planted),
        detail="; ".join(str(getattr(f, "message", f)) for f in clean),
    )


# The three routes the classifier can produce, and the ci.yml jobs each one is
# supposed to run. `docs_only=true, product_impacting=true` is unreachable: a
# change set of pure prose cannot impact the build, and `classify` never emits
# it. Only three rows, so they are written out rather than generated.
#
# The third row was observed in production exactly once (#189) before this
# table existed. A route with no assertion and no traffic is a route nobody
# would notice breaking, which is why it is pinned here rather than left to be
# exercised by chance.
_ROUTE_CONTRACT: tuple[tuple[bool, bool, frozenset[str]], ...] = (
    # docs_only, product_impacting, jobs expected to run
    (True, False, frozenset({"docs"})),
    (False, False, frozenset({"docs", "fork-ci", "security", "nix"})),
    (False, True, frozenset({"docs", "fork-ci", "security", "nix", "smoke"})),
)


def _evaluate_route_condition(expression: str, docs_only: bool, impacting: bool) -> bool:
    """Evaluate the small condition language ci.yml actually uses.

    Deliberately not a general GitHub expression engine. It understands the two
    shapes present -- a negated input, and a conjunction of one negated and one
    plain input -- and refuses anything else, so a condition that grows past
    what this can read fails loudly instead of being silently evaluated wrong.
    """

    inner = expression.strip()
    if inner.startswith("${{") and inner.endswith("}}"):
        inner = inner[3:-2].strip()

    values = {"inputs.docs_only": docs_only, "inputs.product_impacting": impacting}

    def term(text: str) -> bool:
        text = text.strip()
        negated = text.startswith("!")
        if negated:
            text = text[1:].strip()
        if text not in values:
            raise ValueError(f"unreadable term {text!r} in condition {expression!r}")
        return not values[text] if negated else values[text]

    return all(term(part) for part in inner.split("&&"))


def plant_route_contract() -> Outcome:
    """Each route must run exactly the jobs it is supposed to run.

    The classifier's two outputs decide which legs execute, and a leg that does
    not execute reports success. Until now nothing asserted the mapping from
    route to leg set: the plants covered what the classifier *emits*, not what
    the workflow *does* with it. So a condition could be widened -- `smoke`
    gaining a `docs_only` term, `fork-ci` losing one -- and every check would
    stay green because the affected leg would simply stop running.

    The planted defect is that widening, applied to the real ci.yml. The clean
    control is the real file, which must match the table for all three routes.
    """

    document = _parse_ci_workflow()
    jobs = document.get("jobs") or {}
    expected_jobs = {job for _, _, names in _ROUTE_CONTRACT for job in names}
    missing = sorted(expected_jobs - set(jobs))
    if missing:
        return Outcome(
            rejected=False,
            accepted=False,
            evidence="",
            detail=f"ci.yml no longer defines {', '.join(missing)}",
        )

    def running(conditions: dict[str, str | None], docs_only: bool, impacting: bool) -> frozenset[str]:
        active = set()
        for job, condition in conditions.items():
            if condition is None or _evaluate_route_condition(condition, docs_only, impacting):
                active.add(job)
        return frozenset(active)

    real = {job: jobs[job].get("if") for job in expected_jobs}
    mismatches = [
        f"{'docs_only' if d else 'not docs_only'}/"
        f"{'impacting' if i else 'not impacting'}: ran {sorted(running(real, d, i))}, "
        f"contract says {sorted(want)}"
        for d, i, want in _ROUTE_CONTRACT
        if running(real, d, i) != want
    ]

    # The plant: give `smoke` the condition `fork-ci` has, which makes it run on
    # the route that is supposed to skip it.
    planted = dict(real)
    planted["smoke"] = "${{ !inputs.docs_only }}"
    planted_mismatches = [
        (d, i) for d, i, want in _ROUTE_CONTRACT if running(planted, d, i) != want
    ]

    return Outcome(
        rejected=bool(planted_mismatches),
        accepted=not mismatches,
        evidence=(
            f"widening smoke's condition breaks {len(planted_mismatches)} of "
            f"{len(_ROUTE_CONTRACT)} routes"
        ),
        detail="; ".join(mismatches) if mismatches else "",
    )


def _parse_ci_workflow() -> dict[str, Any]:
    guard = _load("scripts/governance_compare.py")
    source = (REPO_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    return guard.parse_workflow(source)


# Which plant covers each required context's workflow contract. The audit that
# prompted this found the same shape twice: a property that decides whether a
# check runs, one detector for it, and that detector inside the blast radius of
# the change it judged. Fixing the two instances did not stop a third from
# being introduced -- adding a required context with no plant would reproduce
# it exactly, and nothing would say so.
#
# `_check_required_contexts_have_plants` reads the manifest and fails if a
# context named there is absent here. Adding a required context is then a
# two-part edit: the manifest, and a plant proving its contract can fail.
CONTRACT_PLANTS = {
    "Governance Root": "scripts/governance_compare.py::job_contract",
    "PR Gate": "scripts/governance_compare.py::pr_gate_contract",
}


def _check_required_contexts_have_plants() -> list[str]:
    """Every required context must have a plant that can prove its contract fails.

    A required context whose `if:` contract is unasserted is not hypothetical:
    a job skipped by a conditional reports success to a ruleset, verified
    directly rather than inferred, so an unasserted contract is a gate that can
    be turned off without turning anything red.
    """

    manifest_path = REPO_ROOT / "scripts/required-checks.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        return [f"cannot read scripts/required-checks.json to check plant coverage: {error}"]

    contexts = {
        contract["context"]
        for contract in manifest.get("workflow_contracts", [])
        if isinstance(contract, dict) and contract.get("context")
    }
    if not contexts:
        # An empty read would make every comparison below vacuously true.
        return [
            "scripts/required-checks.json declares no workflow_contracts; plant "
            "coverage for required contexts cannot be checked"
        ]

    registered = {guard.script for guard in GUARDS}
    failures: list[str] = []
    for context in sorted(contexts):
        plant = CONTRACT_PLANTS.get(context)
        if plant is None:
            failures.append(
                f"required context {context!r} has no entry in CONTRACT_PLANTS, so "
                f"nothing proves its `if:` contract can fail. A context whose "
                f"contract is unasserted can be given a condition that never "
                f"fires, and a skipped required context is reported as success."
            )
        elif plant not in registered:
            failures.append(
                f"required context {context!r} names plant {plant!r}, which is not "
                f"a registered guard"
            )
    for context in sorted(set(CONTRACT_PLANTS) - contexts):
        failures.append(
            f"CONTRACT_PLANTS names {context!r}, which is no longer a required "
            f"context in the manifest; drop it or restore the context"
        )
    return failures


def plant_governance_job_contract() -> Outcome:
    """A required context made to skip must be reported.

    This is the direction `plant_governance_compare` does not cover. That plant
    exercises `check_repository`, which compares rulesets and merge settings;
    the workflow contract -- `Governance Root` carries no `if:`, `PR Gate` is
    `always()` -- lives in `_check_job_contract` and was reached at pull-request
    time only from `tests/test_governance_compare.py`, which this repository
    does not protect.

    That mattered because GitHub reports a job skipped by a conditional as
    *Success*, so a pull request adding `if: false` to a required job turns the
    gate green rather than red, and the pull request that does it can delete its
    own detector in the same change set. Both halves were reproduced before this
    plant was written.

    The plant plants exactly that: the real workflow with a job-level `if:`
    spliced onto the required job. The clean control is the real workflow
    unmodified, so a comparator that stopped reading the contract at all fails
    the accepted side rather than passing quietly.
    """

    guard = _load("scripts/governance_compare.py")
    manifest = {
        "workflow_contracts": [
            {
                "context": "Governance Root",
                "file": ".github/workflows/governance-root.yml",
                "job_id": "governance-root",
                "if": None,
                "needs": [],
                "routing": {},
            }
        ]
    }
    path = REPO_ROOT / ".github/workflows/governance-root.yml"
    clean_source = path.read_text(encoding="utf-8")

    anchor = "  governance-root:\n    name: Governance Root\n"
    if clean_source.count(anchor) != 1:
        # The plant addresses a specific two lines. If they move, say so rather
        # than planting nothing and reporting a pass.
        return Outcome(
            rejected=False,
            accepted=False,
            evidence="",
            detail=f"anchor {anchor!r} appears {clean_source.count(anchor)} times",
        )
    planted_source = clean_source.replace(
        anchor, anchor + "    if: ${{ false }}\n"
    )

    planted_report = guard.Report()
    guard.check_workflow_contracts(
        manifest,
        {".github/workflows/governance-root.yml": planted_source},
        [],
        planted_report,
    )

    clean_report = guard.Report()
    guard.check_workflow_contracts(
        manifest,
        {".github/workflows/governance-root.yml": clean_source},
        [],
        clean_report,
    )

    return Outcome(
        rejected=bool(planted_report.failures),
        accepted=not clean_report.failures,
        evidence="; ".join(planted_report.failures),
        detail="; ".join(clean_report.failures),
    )


def plant_pr_gate_job_contract() -> Outcome:
    """`PR Gate` losing `always()` must be reported.

    The sibling of the check above, and not redundant with it: `Governance Root`
    is contracted to carry *no* `if:` and `PR Gate` to carry `always()`
    specifically, which are opposite branches of `_check_job_contract`. A
    comparator that only ever checked "absent" would satisfy the other plant
    while letting the summary job drop to a condition that never fires.

    `always()` is what makes the gate observe a failed leg at all. Without it a
    failing dependency skips the gate, and a skipped gate reports Success.
    """

    guard = _load("scripts/governance_compare.py")
    manifest = {
        "workflow_contracts": [
            {
                "context": "PR Gate",
                "file": ".github/workflows/pr.yml",
                "job_id": "pr-gate",
                "if": "always()",
                "needs": ["checks", "classify"],
                "routing": {},
            }
        ]
    }
    path = REPO_ROOT / ".github/workflows/pr.yml"
    clean_source = path.read_text(encoding="utf-8")

    anchor = "    if: always()\n"
    if clean_source.count(anchor) != 1:
        return Outcome(
            rejected=False,
            accepted=False,
            evidence="",
            detail=f"anchor {anchor!r} appears {clean_source.count(anchor)} times",
        )
    planted_source = clean_source.replace(anchor, "    if: ${{ false }}\n")

    planted_report = guard.Report()
    guard.check_workflow_contracts(
        manifest,
        {".github/workflows/pr.yml": planted_source},
        [],
        planted_report,
    )

    clean_report = guard.Report()
    guard.check_workflow_contracts(
        manifest,
        {".github/workflows/pr.yml": clean_source},
        [],
        clean_report,
    )

    return Outcome(
        rejected=bool(planted_report.failures),
        accepted=not clean_report.failures,
        evidence="; ".join(planted_report.failures),
        detail="; ".join(clean_report.failures),
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


_WIRED_TEST = """\
import unittest


class Wired(unittest.TestCase):
    def test_something(self) -> None:
        self.assertTrue(True)
"""

# The shape that reports `Ran 0 tests ... OK`: a module-level `def test_*`
# with no TestCase, which unittest does not collect.
_VACUOUS_TEST = """\
def test_something():
    assert True


if __name__ == "__main__":
    test_something()
"""


def plant_test_wiring() -> Outcome:
    """A test file that nothing runs, and one that collects nothing, must be reported.

    Both defects pass silently in the ordinary course: an unwired file is never
    executed, and a script-style file executed by `unittest` prints
    `Ran 0 tests ... OK` and exits 0.

    Two recipe shapes are planted because the justfile uses one and the guard
    accepts both. Under a `tests/test_*.py` glob every module is wired by
    construction, so only the zero-collecting file can be caught; under a
    hand-written list a file can also go unnamed. Planting only the shape the
    repository happens to use today would leave the other branch unproven the
    day someone switches. The clean control -- a glob over one healthy module
    -- matters as much as the plants: findings there would mean the guard was
    failing for an unrelated reason and its rejections would prove nothing.
    """

    guard = _load("scripts/check_test_wiring.py")

    def run(files: dict[str, str], recipe: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "tests").mkdir()
            for name, source in files.items():
                (root / "tests" / name).write_text(source, encoding="utf-8")
            (root / "justfile").write_text(f"check:\n{recipe}\n", encoding="utf-8")
            return guard.problems(root)

    glob_recipe = '    for f in tests/test_*.py; do python3 -m unittest "$f"; done'
    named_recipe = "\n".join(
        f"    python3 -m unittest tests.{module}"
        for module in ("test_wired", "test_vacuous")
    )
    tree = {
        "test_wired.py": _WIRED_TEST,
        "test_unwired.py": _WIRED_TEST,
        "test_vacuous.py": _VACUOUS_TEST,
    }

    under_glob = run(tree, glob_recipe)
    under_names = run(tree, named_recipe)
    clean = run({"test_wired.py": _WIRED_TEST}, glob_recipe)

    # The glob wires every module, so it must catch the zero-collecting file and
    # must not report the file the named recipe leaves out.
    vacuous_seen = any(f.startswith("test_vacuous:") for f in under_glob)
    glob_wires_all = not any(f.startswith("test_unwired:") for f in under_glob)
    unwired_seen = any(f.startswith("test_unwired:") for f in under_names)

    return Outcome(
        rejected=vacuous_seen and glob_wires_all and unwired_seen,
        accepted=clean == [],
        evidence="; ".join((under_glob + under_names)[:2]),
        detail=(
            f"planted a zero-collecting module (seen: {vacuous_seen}) and, under "
            f"a hand-written recipe, an unnamed one (seen: {unwired_seen}); the "
            f"glob recipe wired every module (no false unwired: {glob_wires_all}); "
            f"healthy tree reported {len(clean)} finding(s) (want 0)"
            f"{': ' + '; '.join(clean[:2]) if clean else ''}"
        ),
    )


def plant_lint_docs() -> Outcome:
    """A docs lint that read fewer files than it was given must not pass.

    `vale` with no input files prints its usage banner and exits 0, so the
    recipe this replaced reported a clean lint whenever the pathspec matched
    nothing. The plants stand in for the three ways that ends in a green run
    over unread files: no files selected at all, a summary reporting fewer
    files than were handed over, and no summary at all. Each runs against a
    stub `vale`, because the point is what the checker concludes from a
    linter's report, not what the real linter finds. The control is the same
    stub reporting the honest count, which must pass -- otherwise the
    rejections would just be a checker that always says no.
    """

    guard = _load("scripts/lint_docs.py")
    files = ["a.md", "b.md", "c.md"]

    def run(summary: str | None, listed: list[str]) -> int:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            stub = root / "vale-stub"
            line = f'echo "{summary}"' if summary is not None else "true"
            stub.write_text(f"#!/bin/sh\n{line}\n", encoding="utf-8")
            stub.chmod(0o755)
            listing = root / "files.txt"
            listing.write_text("\n".join(listed), encoding="utf-8")
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                return guard.main(
                    [
                        "--root",
                        str(root),
                        "--vale",
                        str(stub),
                        "--files-from",
                        str(listing),
                    ]
                )

    honest = f"0 errors, 0 warnings and 0 suggestions in {len(files)} files."
    short = "0 errors, 0 warnings and 0 suggestions in 1 file."

    no_files = run(honest, [])
    under_reported = run(short, files)
    no_summary = run(None, files)
    clean = run(honest, files)

    return Outcome(
        rejected=all(rc != 0 for rc in (no_files, under_reported, no_summary)),
        accepted=clean == 0,
        evidence=(
            f"empty selection exited {no_files}, under-reported run exited "
            f"{under_reported}, summaryless run exited {no_summary}"
        ),
        detail=(
            f"planted an empty file list ({no_files}), a linter claiming 1 of "
            f"{len(files)} files ({under_reported}), and one reporting no count "
            f"at all ({no_summary}) -- all want non-zero; the honest count "
            f"exited {clean} (want 0)"
        ),
    )


# The change set a pull request weakening the route classifier necessarily
# carries: the classifier's own table, and the workflow that invokes it.
_ROUTE_SELF = ("scripts/classify_pr_paths.py", ".github/workflows/pr.yml")
# Prose, the route the classifier exists to grant.
_ROUTE_PROSE = ("docs/guide.md", "README.md")
# A governance workflow edit: judged in full by the linters and contract tests,
# but nothing about it needs the release binary rebuilt.
_ROUTE_INERT = (".github/workflows/governance-root.yml", "scripts/required-checks.json")
# Paths whose contents reach the built artifact. The classification table is
# an allowlist of what is *inert*, so widening that allowlist by one line
# reroutes everything the new entry covers. `crates/` is the whole product.
_ROUTE_BUILD = (
    "crates/jcode/src/main.rs",
    "Cargo.toml",
    "Cargo.lock",
    "flake.nix",
)


def plant_pr_route_build_inputs() -> Outcome:
    """A path that reaches the built artifact must keep running the product legs.

    The two plants above cover the classifier exempting *itself* and failing
    closed on input it does not recognise. Neither covers the table, and the
    table is the easiest thing in the file to weaken: `INERT_PREFIXES` gaining
    one entry, `crates/`, reroutes every Rust change to
    `product_impacting=false`, which skips the Nix package build, the smoke
    check, the full-test recipe and the release-check step. Every one of those
    reports success by not running.

    That edit was reproduced against this registry before this plant existed,
    and the registry reported all claims holding. The only thing that caught it
    was `tests/test_classify_pr_paths.py`, which this repository does not
    protect and which the same pull request could have rewritten.

    So the assertion is made here, from a file the protected set covers: the
    real classifier, on real build inputs, must still route them expensive.
    """

    guard = _load("scripts/classify_pr_paths.py")

    impacting = {
        path: guard.classify([path])["product_impacting"] for path in _ROUTE_BUILD
    }
    missed = sorted(path for path, hit in impacting.items() if not hit)
    clean = guard.classify(list(_ROUTE_INERT))

    return Outcome(
        rejected=not missed,
        accepted=not clean["product_impacting"],
        evidence=(
            "every build input still routes product_impacting=true: "
            + ", ".join(_ROUTE_BUILD)
        ),
        # `detail` is what the runner prints when a plant is accepted, so the
        # diagnosis has to live here rather than in `evidence`.
        detail=(
            f"the classification table now treats these build inputs as inert: "
            f"{', '.join(missed)}. Widening INERT_PREFIXES or INERT_FILES "
            f"reroutes them to product_impacting=false, which skips the Nix "
            f"package build, Smoke, full-test and release-check."
            if missed
            else (
                f"{', '.join(_ROUTE_INERT)} -> product_impacting="
                f"{str(clean['product_impacting']).lower()} (want false)"
            )
        ),
    )


def plant_pr_route_self_exemption() -> Outcome:
    """The classifier may not route its own weakening onto the cheap path.

    `pr.yml` checks out `head.sha` and runs the classifier from that checkout,
    so a pull request editing this table is routed by the edited copy. Measured
    on this tree: `is_docs` relaxed to `return True` classifies a change set of
    nothing but `scripts/classify_pr_paths.py` as `docs_only=true`, and
    `docs_only` is what `ci.yml` gates Fork CI on -- the leg that runs this
    harness. The weakening skips its own detector, and no shadow is needed for
    it: the edit is honest and touches no protected path.

    Registering the claim does not by itself close that ordering; it makes the
    property checkable wherever the harness does run, and moves it out of
    tests/test_classify_pr_paths.py, which is unprotected and can be weakened
    in the same pull request as the classifier -- D034's failure mode, which is
    the one this file exists to refuse.

    The clean control is prose. A classifier answering "impacting" to
    everything would satisfy the rejection while destroying the route the
    classifier exists for, so acceptance is asserted as well.
    """

    guard = _load("scripts/classify_pr_paths.py")
    planted = guard.classify(list(_ROUTE_SELF))
    clean = guard.classify(list(_ROUTE_PROSE))

    return Outcome(
        rejected=planted["product_impacting"] and not planted["docs_only"],
        accepted=clean["docs_only"] and not clean["product_impacting"],
        evidence=(
            f"{', '.join(_ROUTE_SELF)} -> "
            f"docs_only={str(planted['docs_only']).lower()}, "
            f"product_impacting={str(planted['product_impacting']).lower()}"
        ),
        detail=(
            f"planted the classifier's own change set, want docs_only=false and "
            f"product_impacting=true, got {planted}; prose control wants "
            f"docs_only=true and product_impacting=false, got {clean}"
        ),
    )


def plant_pr_route_fails_closed() -> Outcome:
    """An unreadable or unrecognised change set must run everything.

    Same file, second comparison, because the two weaken independently: a table
    that still refuses to exempt itself can be relaxed at the fallback so that a
    diff nobody could read, or a path nobody anticipated, routes cheap. Both
    defects are silent -- the skipped leg reports success.

    The clean control is a governance workflow edit, which is the narrow case
    the classifier was built to make cheap. It differs from the prose control
    above on purpose: a guard that accepted only prose would pass that claim
    while having lost this route entirely.
    """

    guard = _load("scripts/classify_pr_paths.py")
    unreadable = guard.classify([])
    unrecognised = guard.classify(["tools/newthing/main.go"])
    clean = guard.classify(list(_ROUTE_INERT))

    return Outcome(
        rejected=(
            unreadable["product_impacting"]
            and not unreadable["docs_only"]
            and unrecognised["product_impacting"]
        ),
        accepted=not clean["product_impacting"],
        evidence=(
            f"empty change set -> product_impacting="
            f"{str(unreadable['product_impacting']).lower()}; unrecognised path -> "
            f"product_impacting={str(unrecognised['product_impacting']).lower()}"
        ),
        detail=(
            f"planted an empty change set ({unreadable}) and an unrecognised path "
            f"({unrecognised}), both want product_impacting=true; the governance "
            f"workflow control wants product_impacting=false, got {clean}"
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
        script="scripts/rust_production_filter.py::production_lines",
        status=GATING,
        # No wiring of its own: it is a library, not a script. It runs whenever
        # check_critical_path_budget.py runs, which is asserted above, and the
        # claim here is about its classification rather than its invocation.
        wiring=(),
        plant=plant_production_filter,
    ),
    Guard(
        script="scripts/classify_pr_paths.py",
        status=GATING,
        wiring=(
            # The whole invocation, not just the path. `-I` is what stops a
            # `scripts/<name>.py` shadow rebinding one of the classifier's
            # imports and forging a route, and until now that requirement lived
            # only in tests/test_classify_pr_paths.py, which this repository does
            # not protect. A guard's own test agreeing with a weakened guard is
            # D034.
            Wiring(
                where=".github/workflows/pr.yml",
                must_contain="python3 -I scripts/classify_pr_paths.py",
            ),
            # The classifier does not get to route a change to itself. The
            # workflow forces the full route when the change set touches a
            # routing-critical path, and these two claims are why that override
            # cannot be removed quietly: deleting it edits a protected path, so
            # Governance Root fires, and the override itself guarantees this
            # harness runs on that pull request rather than being skipped by the
            # very classifier under edit. Assert the decision and the path
            # separately -- an override that kept the grep but stopped forcing,
            # or kept forcing but dropped the classifier from the set, is the
            # failure this is watching for, and either alone still greps.
            Wiring(
                where=".github/workflows/pr.yml",
                must_contain="-e scripts/classify_pr_paths.py",
            ),
            Wiring(
                where=".github/workflows/pr.yml",
                must_contain="printf 'docs_only=false\\nproduct_impacting=true\\n'",
            ),
        ),
        plant=plant_pr_route_self_exemption,
    ),
    Guard(
        script="scripts/classify_pr_paths.py::fails_closed",
        status=GATING,
        wiring=(),  # same file as above; wiring asserted once
        plant=plant_pr_route_fails_closed,
    ),
    Guard(
        script="scripts/classify_pr_paths.py::build_inputs",
        status=GATING,
        wiring=(),  # same file as above; wiring asserted once
        plant=plant_pr_route_build_inputs,
    ),
    Guard(
        script="scripts/classify_pr_paths.py::route_contract",
        status=GATING,
        wiring=(),  # same file as above; wiring asserted once
        plant=plant_route_contract,
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
        script="scripts/check_test_wiring.py",
        status=GATING,
        wiring=(
            Wiring(
                where="justfile",
                recipe="check",
                must_contain="scripts/check_test_wiring.py",
            ),
            Wiring(where=".github/workflows/fork-ci.yml", must_contain="just check"),
        ),
        plant=plant_test_wiring,
    ),
    Guard(
        script="scripts/lint_docs.py",
        status=GATING,
        wiring=(
            Wiring(
                where="justfile",
                recipe="lint-docs",
                must_contain="scripts/lint_docs.py",
            ),
            Wiring(where=".github/workflows/ci.yml", must_contain="just lint-docs"),
        ),
        plant=plant_lint_docs,
    ),
    Guard(
        script="scripts/governance_compare.py",
        status=GATING,
        wiring=(
            # The comparator is reached at pull-request time through its test
            # module, which the `test-python` glob runs and `check` depends on.
            # It used to be claimed against governance-root.yml, where the
            # string appears only inside the `protected=(...)` array: the
            # workflow names the comparator as a protected path and never
            # executes it. That claim passed because wiring was a substring
            # test while `_executes` -- three functions away, and mention-aware
            # -- would have rejected it. Wiring now uses `_executes`, so the
            # claim has to name a site that runs something.
            Wiring(
                where="justfile",
                recipe="test-python",
                must_contain="tests/test_*.py",
            ),
        ),
        plant=plant_governance_compare,
    ),
    Guard(
        script="scripts/governance_compare.py::job_contract",
        status=GATING,
        wiring=(),  # same file as above; wiring asserted once
        plant=plant_governance_job_contract,
    ),
    Guard(
        script="scripts/governance_compare.py::pr_gate_contract",
        status=GATING,
        wiring=(),  # same file as above; wiring asserted once
        plant=plant_pr_gate_job_contract,
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
    Guard(
        script="scripts/check_reusable_workflow_calls.py",
        status=GATING,
        wiring=(
            Wiring(
                where=".github/workflows/nix.yml",
                must_contain="scripts/check_reusable_workflow_calls.py",
            ),
        ),
        plant=plant_reusable_workflow_calls,
    ),
    Guard(
        script="scripts/check_workflow_permissions.py",
        status=GATING,
        wiring=(
            Wiring(
                where=".github/workflows/nix.yml",
                must_contain="scripts/check_workflow_permissions.py",
            ),
        ),
        plant=plant_workflow_permissions,
    ),
    Guard("scripts/check_ambient_roots.sh", DORMANT,
          reason="referenced only by its own allowlist data file; nothing executes it"),
    Guard("scripts/check_branch_handoff.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs"),
    Guard("scripts/check_config_env_lease.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs"),
    Guard("scripts/check_dependency_boundaries.py", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs"),
    Guard(
        script="scripts/check_docs_references.py",
        status=GATING,
        wiring=(
            Wiring(
                where="justfile",
                recipe="lint-docs",
                must_contain="scripts/check_docs_references.py",
            ),
            Wiring(where=".github/workflows/ci.yml", must_contain="just lint-docs"),
        ),
        plant=plant_docs_references,
    ),
    Guard("scripts/check_env_lease_drop_order.py", DORMANT,
          reason="nothing in the repository references it"),
    Guard("scripts/check_real_home_isolation.sh", DORMANT,
          reason="invoked only from scripts/preflight.sh, a developer entry point that no workflow and no justfile recipe runs; also scripts/run_lifecycle_matrix.sh, likewise unrun"),
    Guard("scripts/check_startup_budget.sh", DORMANT,
          reason="invoked only from scripts/test_fast.sh, which no workflow runs"),
    Guard("scripts/check_tui_render_lock.py", DORMANT,
          reason="nothing in the repository references it"),
    Guard("scripts/check_warning_budget.sh", DORMANT,
          reason="invoked only from scripts/preflight.sh, whose full run is still a developer entry point; its test tests/test_warning_budget.py now runs through the tests/test_*.py glob"),
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
    """Assert every wiring claim names a site that actually runs the guard.

    This used to be `must_contain in source`, a plain substring test, and that
    is how a GATING claim came to be satisfied by a path sitting in a bash
    array. `.github/workflows/governance-root.yml` lists
    `scripts/governance_compare.py` among the paths it *protects*; it has never
    executed it. The registry read that mention as wiring and reported the
    comparator gating for as long as the claim existed.

    The asymmetry is the interesting part: `_check_dormancy_is_still_true` was
    already careful here, using `_executes`, which skips a bare list entry as
    data. So the registry was rigorous about "dormant but actually running" and
    credulous about "gating but not actually running" -- the direction where a
    false claim reads as coverage. Both directions now use the same test.
    """

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
        location = (
            f"{wiring.where} recipe `{wiring.recipe}`"
            if wiring.recipe
            else wiring.where
        )
        if wiring.must_contain not in source:
            failures.append(
                f"{guard.script}: {location} no longer invokes "
                f"{wiring.must_contain!r}; the guard has been unplugged from CI"
            )
        elif not _executes(source, wiring.must_contain):
            failures.append(
                f"{guard.script}: {location} mentions {wiring.must_contain!r} but "
                "never executes it -- a name in a list is data, not wiring. Point "
                "the claim at the line that runs it, or drop the claim."
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
                        "*_filter.py", "lint_*.py", "classify_*.py",
                        "generate_governance_fixture.py")
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


# Local modules a gating guard is meant to import from `scripts/`. Any other
# name a gating guard imports must resolve outside `scripts/`; see
# _check_no_import_shadowing.
#
# `governance_compare` is here because the two workflow checkers import its
# YAML parser as a sibling -- `from governance_compare import
# WorkflowParseError, parse_workflow`. That is deliberate reuse of one parser
# rather than a second implementation of it, and it is why those two guards
# cannot be run under `python3 -I`.
INTENDED_LOCAL_IMPORTS = frozenset(
    {"rust_production_filter", "check_guard_nonvacuity", "governance_compare"}
)


def _imported_top_level_names(source: str) -> set[str]:
    names: set[str] = set()
    for node in ast.walk(ast.parse(source)):
        if isinstance(node, ast.Import):
            names |= {alias.name.split(".")[0] for alias in node.names}
        elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
            names.add(node.module.split(".")[0])
    return names


def _shadow_scan_closure(
    seeds: "frozenset[str] | None" = None,
) -> dict[str, set[str] | None]:
    """Every file whose imports a shadow in `scripts/` could capture.

    Starts from the gating guards, plus this file and its own test -- a shadow
    is process-wide, so a control that scans the guards but not itself can be
    switched off by one file it never looks at -- and follows local modules
    transitively. The closure matters rather than one file's import lines:
    before this walked it, `rust_production_filter`'s imports were covered only
    because they happened to be a subset of its caller's, so the day it gained
    an import the caller lacked, the shadow would have come back unseen. An
    unstated precondition that happens to hold is the shape that became G10A.

    Maps each file to its imported top-level names, or to None if it could not
    be parsed.
    """

    if seeds is None:
        seeds = frozenset(
            {g.script.split("::")[0] for g in GUARDS if g.status == GATING}
            | {"scripts/check_guard_nonvacuity.py", "tests/test_guard_nonvacuity.py"}
        )
    queue = sorted(seeds)
    closure: dict[str, set[str] | None] = {}
    while queue:
        script = queue.pop()
        if script in closure:
            continue
        path = REPO_ROOT / script
        if path.suffix != ".py" or not path.exists():
            continue
        try:
            imported = _imported_top_level_names(path.read_text(encoding="utf-8"))
        except SyntaxError:
            closure[script] = None
            continue
        closure[script] = imported
        for name in sorted(imported & INTENDED_LOCAL_IMPORTS):
            queue.append(f"scripts/{name}.py")
    return closure


def _check_no_import_shadowing() -> list[str]:
    """No file in `scripts/` may capture a name a gating guard imports.

    Python puts a script's own directory first on `sys.path`, so adding
    `scripts/<name>.py` silently rebinds `import <name>` for every guard run as
    `python3 scripts/...`. Measured on this tree: a `scripts/hashlib.py`
    returning a fixed `sha256` freezes `scope_digest()`, so raising
    lifecycle/swallowed_error from 441 to 9999 still prints the pinned
    5ed12e31... and `--expect-digest` passes. That defeats the pin itself,
    which is the one mechanism that otherwise forces a ceiling change through
    review, and it unlocks all six constant tables at once.

    The attack adds a file. It edits no guard, touches nothing the ruleset
    covers, and moves no digest, so every other claim here stays green --
    this one was written after watching the harness pass while the budget
    was 22x weaker.

    Adding a legitimate local module means adding its name to
    INTENDED_LOCAL_IMPORTS, which shows up in the diff as a widened allowlist
    rather than as an unchanged green run.
    """

    present = {path.stem for path in (REPO_ROOT / "scripts").glob("*.py")}
    failures: list[str] = []
    for script, imported in sorted(_shadow_scan_closure().items()):
        if imported is None:
            failures.append(f"{script}: could not be parsed to check imports")
            continue
        for name in sorted(
            imported & present - INTENDED_LOCAL_IMPORTS - {Path(script).stem}
        ):
            failures.append(
                f"{script}: imports `{name}`, and `scripts/{name}.py` exists, so "
                f"it resolves to that file rather than to the module it expects. "
                f"A shadow of `hashlib` freezes the scope digest and lets every "
                f"budget ceiling move without moving the pin; a shadow of `ast` "
                f"blinds this check. Either add `{name}` to INTENDED_LOCAL_IMPORTS "
                f"as a deliberate local module, or rename the file."
            )
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


def _executes(source: str, script: str) -> bool:
    """True if `source` appears to RUN `script`, not merely mention it.

    The distinction matters. `.github/workflows/governance-root.yml` lists
    script paths inside a `protected=(...)` array: those are data telling the
    workflow which paths to watch, not commands. Substring matching cannot tell
    the two apart, and treating a mention as an invocation is the same mistake
    that made this file's first draft describe seven dormant scripts as having
    "no invocation site" when they were referenced all along. A line that is
    nothing but the path -- optionally a YAML or shell list item -- is data.
    """

    for line in source.splitlines():
        if script not in line:
            continue
        stripped = line.strip().lstrip("-").strip().strip('"').strip("'")
        if stripped == script:
            continue  # a bare list entry: data, not a command
        return True
    return False


def _check_dormancy_is_still_true(guard: "Guard") -> list[str]:
    """Assert a guard labelled dormant is genuinely unreachable from a gate.

    Dormancy is the registry's escape hatch: a dormant guard is skipped, so it
    needs no plant. That makes the label the softest spot in this file. One
    word -- GATING to DORMANT -- silently drops a live guard from coverage,
    which is D034's own failure mode reproduced inside the control meant to
    catch it. So the label is not taken on trust. If a dormant guard turns out
    to be reachable from a PR-blocking site, this fails and asks for the label
    to be corrected rather than quietly honouring it.
    """

    failures: list[str] = []
    if guard.script == f"scripts/{Path(__file__).name}":
        # This file is deliberately wired into `check`. Its entry is dormant
        # because it carries no plant of its own, not because it never runs,
        # and tests/test_guard_nonvacuity.py asserts that wiring.
        return failures
    for path, label in BLOCKING_SITES:
        target = REPO_ROOT / path
        if not target.exists():
            failures.append(
                f"{guard.script}: cannot verify dormancy, {path} is missing"
            )
            continue
        source = target.read_text(encoding="utf-8")
        if _executes(source, guard.script):
            failures.append(
                f"{guard.script}: registered DORMANT, but {label} references it. "
                f"Either it now gates a PR -- in which case mark it GATING and "
                f"give it a plant -- or the reference is dead and should go. "
                f"Recorded reason was: {guard.reason}"
            )
    return failures


def run() -> tuple[list[str], list[str]]:
    """Return (failures, passes)."""

    failures = _check_registry_covers_every_guard()
    passes: list[str] = []

    contract_coverage = _check_required_contexts_have_plants()
    failures += contract_coverage
    if not contract_coverage:
        passes.append(
            "every required context has a plant proving its `if:` contract can fail"
        )

    shadowing = _check_no_import_shadowing()
    failures += shadowing
    if not shadowing:
        passes.append("scripts/: no gating guard's imports are shadowed")

    for guard in GUARDS:
        if guard.status == DORMANT:
            failures += _check_dormancy_is_still_true(guard)
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
