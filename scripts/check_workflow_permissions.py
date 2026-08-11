#!/usr/bin/env python3
"""Check reusable-workflow GITHUB_TOKEN permission monotonicity.

GitHub validates every job in a called workflow against the permission budget
passed by the caller.  This includes ordinary jobs and jobs whose ``if`` later
evaluates false, not only jobs that make another reusable-workflow call.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from governance_compare import WorkflowParseError, parse_workflow


# GitHub.com's accepted GITHUB_TOKEN permission model. Keep this independent of
# actionlint's table: actionlint 1.7.12 and current main (as of 2026-08-10) omit
# code-quality and vulnerability-alerts even though GitHub accepts both. Any
# actionlint compatibility layer must suppress only those two unknown-scope
# diagnostics; this checker must continue validating their access levels.
#
# Sources:
# https://github.com/github/docs/blob/main/data/reusables/actions/github-token-available-permissions.md
# https://github.com/github/docs/blob/main/data/reusables/actions/github-token-scope-descriptions.md
PERMISSION_LEVELS = {
    "actions": ("none", "read", "write"),
    "artifact-metadata": ("none", "read", "write"),
    "attestations": ("none", "read", "write"),
    "checks": ("none", "read", "write"),
    "code-quality": ("none", "read", "write"),
    "contents": ("none", "read", "write"),
    "deployments": ("none", "read", "write"),
    "discussions": ("none", "read", "write"),
    "id-token": ("none", "write"),
    "issues": ("none", "read", "write"),
    "models": ("none", "read"),
    "packages": ("none", "read", "write"),
    "pages": ("none", "read", "write"),
    "pull-requests": ("none", "read", "write"),
    "repository-projects": ("none", "read", "write"),
    "security-events": ("none", "read", "write"),
    "statuses": ("none", "read", "write"),
    "vulnerability-alerts": ("none", "read"),
}
# https://docs.github.com/en/actions/reference/workflows-and-actions/reusable-workflows#limitations
MAX_WORKFLOW_LEVELS = 10
MAX_UNIQUE_REUSABLE_WORKFLOWS = 50


class PermissionCheckError(Exception):
    """A workflow cannot be classified safely."""


@dataclass(frozen=True)
class Finding:
    path: Path
    job: str
    scope: str
    allowed: str
    requested: str

    def render(self, root: Path) -> str:
        relative = self.path.relative_to(root)
        return (
            f"{relative}: job {self.job!r} requests {self.scope}: {self.requested}, "
            f"but its reusable-workflow caller allows only {self.scope}: {self.allowed}"
        )


@dataclass(frozen=True)
class SecretFinding:
    path: Path
    job: str
    secret: str
    reason: str

    def render(self, root: Path) -> str:
        relative = self.path.relative_to(root)
        return (
            f"{relative}: job {self.job!r} cannot supply required secret "
            f"{self.secret!r}: {self.reason}"
        )


def _secret_identifiers(names: Any, context: str) -> dict[str, str]:
    identifiers: dict[str, str] = {}
    for name in names:
        if not isinstance(name, str):
            raise PermissionCheckError(f"{context} names must be strings")
        normalized = name.casefold()
        previous = identifiers.get(normalized)
        if previous is not None and previous != name:
            raise PermissionCheckError(
                f"{context} names {previous!r} and {name!r} differ only by case"
            )
        identifiers[normalized] = name
    return identifiers


def _permission_vector(value: Any) -> dict[str, str]:
    if value is None:
        raise PermissionCheckError("permissions are inherited")
    if isinstance(value, str):
        if value == "read-all":
            return {
                scope: ("read" if "read" in levels else "none")
                for scope, levels in PERMISSION_LEVELS.items()
            }
        if value == "write-all":
            return {scope: levels[-1] for scope, levels in PERMISSION_LEVELS.items()}
        raise PermissionCheckError(f"unsupported permissions value {value!r}")
    if not isinstance(value, dict):
        raise PermissionCheckError("permissions must be a mapping, read-all, or write-all")

    vector = {scope: "none" for scope in PERMISSION_LEVELS}
    for scope, access in value.items():
        if scope not in PERMISSION_LEVELS:
            raise PermissionCheckError(f"unknown permission scope {scope!r}")
        if access not in PERMISSION_LEVELS[scope]:
            raise PermissionCheckError(f"invalid {scope} permission {access!r}")
        vector[scope] = access
    return vector


def _exceeds(requested: dict[str, str], allowed: dict[str, str]) -> list[str]:
    return [
        scope
        for scope, levels in PERMISSION_LEVELS.items()
        if levels.index(requested[scope]) > levels.index(allowed[scope])
    ]


class WorkflowPermissionChecker:
    def __init__(self, root: Path) -> None:
        self.root = root.resolve()
        self.workflows = self.root / ".github" / "workflows"
        self.documents: dict[Path, dict[str, Any]] = {}
        self.remote_interfaces = self._load_remote_interfaces()

    def _load_remote_interfaces(self) -> dict[str, dict[str, Any]]:
        path = self.root / ".github" / "reusable-workflow-secrets.json"
        if not path.exists():
            return {}
        try:
            document = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise PermissionCheckError(f"cannot parse {path}: {error}") from error
        interfaces = document.get("remote_interfaces") if isinstance(document, dict) else None
        if not isinstance(interfaces, dict):
            raise PermissionCheckError(
                f"{path} must contain a remote_interfaces mapping"
            )
        if not all(isinstance(key, str) and isinstance(value, dict) for key, value in interfaces.items()):
            raise PermissionCheckError(f"{path} remote interface entries must be mappings")
        return interfaces

    def _load(self, path: Path) -> dict[str, Any]:
        path = path.resolve()
        if path in self.documents:
            return self.documents[path]
        try:
            document = parse_workflow(path.read_text(encoding="utf-8"))
        except (OSError, WorkflowParseError) as error:
            raise PermissionCheckError(f"cannot parse {path}: {error}") from error
        self.documents[path] = document
        return document

    def _local_target(self, uses: Any) -> Path | None:
        if uses is None:
            return None
        if not isinstance(uses, str):
            raise PermissionCheckError(
                f"reusable-workflow target must be a literal string, got {uses!r}"
            )
        if uses.startswith("./"):
            target = (self.root / uses[2:]).resolve()
        elif uses.startswith("$/"):
            target = (self.root / uses[2:]).resolve()
        else:
            return None
        try:
            target.relative_to(self.root)
        except ValueError as error:
            raise PermissionCheckError(f"local workflow call escapes repository: {uses}") from error
        return target

    @staticmethod
    def _is_remote_target(uses: Any) -> bool:
        return isinstance(uses, str) and not uses.startswith(("./", "$/"))

    @staticmethod
    def _workflow_call_secrets(
        document: dict[str, Any],
    ) -> tuple[dict[str, str], dict[str, str]]:
        triggers = document.get("on")
        workflow_call = triggers.get("workflow_call") if isinstance(triggers, dict) else None
        if workflow_call is None:
            return {}, {}
        if not isinstance(workflow_call, dict):
            raise PermissionCheckError("workflow_call must be a mapping")
        declarations = workflow_call.get("secrets", {})
        if declarations is None:
            declarations = {}
        if not isinstance(declarations, dict):
            raise PermissionCheckError("workflow_call secrets must be a mapping")
        declared = _secret_identifiers(declarations, "workflow_call secret declaration")
        required: dict[str, str] = {}
        for name, declaration in declarations.items():
            if not isinstance(name, str) or not isinstance(declaration, dict):
                raise PermissionCheckError("workflow_call secret declarations must be mappings")
            required_value = declaration.get("required", False)
            if required_value in ("true", "false"):
                required_value = required_value == "true"
            if not isinstance(required_value, bool):
                raise PermissionCheckError(
                    f"workflow_call secret {name!r} required must be boolean"
                )
            if required_value:
                required[name.casefold()] = name
        return required, declared

    @staticmethod
    def _secret_reference(value: Any) -> str | None:
        if not isinstance(value, str):
            return None
        match = re.fullmatch(
            r"\s*\$\{\{\s*secrets\.([A-Za-z_][A-Za-z0-9_]*)\s*\}\}\s*", value
        )
        return match.group(1) if match else None

    def _remote_interface(self, uses: str) -> tuple[dict[str, str], bool]:
        metadata = self.remote_interfaces.get(uses)
        reference = uses.rpartition("@")[2]
        immutable = len(reference) == 40 and all(
            character in "0123456789abcdefABCDEF" for character in reference
        )
        if metadata is None:
            pin = "SHA-pinned but" if immutable else "mutable and"
            raise PermissionCheckError(
                f"remote reusable workflow {uses!r} is {pin} opaque; "
                "vendor or fetch its workflow interface and permission metadata "
                "at an immutable SHA, or add explicit reviewed interface metadata"
            )
        review = metadata.get("review")
        required = metadata.get("required_secrets")
        interface_sha = metadata.get("interface_sha")
        inherit_eligible = metadata.get("inherit_eligible", False)
        if not isinstance(review, str) or not review.strip():
            raise PermissionCheckError(f"remote reusable workflow {uses!r} lacks review metadata")
        if not isinstance(required, list) or not all(isinstance(item, str) for item in required):
            raise PermissionCheckError(
                f"remote reusable workflow {uses!r} lacks determinate required_secrets metadata"
            )
        if not immutable and not (
            isinstance(interface_sha, str)
            and len(interface_sha) == 40
            and all(character in "0123456789abcdefABCDEF" for character in interface_sha)
        ):
            raise PermissionCheckError(
                f"mutable remote reusable workflow {uses!r} requires a reviewed interface_sha"
            )
        if not isinstance(inherit_eligible, bool):
            raise PermissionCheckError(f"remote reusable workflow {uses!r} has invalid inherit_eligible metadata")
        return (
            _secret_identifiers(
                required, f"remote reusable workflow {uses!r} required secret"
            ),
            inherit_eligible,
        )

    @staticmethod
    def _raise_remote_frontier(uses: str) -> None:
        raise PermissionCheckError(
            f"remote reusable workflow {uses!r} remains opaque after secret review; "
            "acceptance also requires locally proven permission, input, accessibility, "
            "depth, unique-call, and descendant metadata"
        )

    def _check_secret_call(
        self,
        path: Path,
        job_name: str,
        job: dict[str, Any],
        required: dict[str, str],
        caller_available: dict[str, str] | None,
        *,
        remote_inherit_eligible: bool = True,
    ) -> list[SecretFinding]:
        supplied = job.get("secrets")
        if supplied == "inherit":
            if not remote_inherit_eligible:
                raise PermissionCheckError(
                    f"{path}: remote call job {job_name!r} uses secrets: inherit without "
                    "explicit reviewed inherit eligibility"
                )
            if caller_available is None:
                return [
                    SecretFinding(
                        path,
                        job_name,
                        secret,
                        "secrets: inherit does not prove the top-level caller has it; "
                        "use an explicit secrets mapping",
                    )
                    for secret in sorted(required.values(), key=str.casefold)
                ]
            return [
                SecretFinding(path, job_name, required[secret], "the caller does not receive it")
                for secret in sorted(required.keys() - caller_available.keys())
            ]
        if supplied is None:
            supplied = {}
        if not isinstance(supplied, dict):
            raise PermissionCheckError(
                f"{path}: call job {job_name!r} secrets must be a mapping or inherit"
            )
        supplied_names = _secret_identifiers(
            supplied, f"{path}: call job {job_name!r} secret mapping"
        )
        findings = [
            SecretFinding(
                path,
                job_name,
                required[secret],
                "it is absent from the secrets mapping",
            )
            for secret in sorted(required.keys() - supplied_names.keys())
        ]
        if caller_available is not None:
            for target_name, value in supplied.items():
                source = self._secret_reference(value)
                if source is not None and source.casefold() not in caller_available:
                    findings.append(
                        SecretFinding(
                            path,
                            job_name,
                            str(target_name),
                            f"forwarded source secret {source!r} is not in the caller interface",
                        )
                    )
        return findings

    @staticmethod
    def _callee_available_secrets(
        job: dict[str, Any], caller_available: dict[str, str] | None
    ) -> dict[str, str] | None:
        supplied = job.get("secrets")
        if supplied == "inherit":
            return caller_available
        if supplied is None:
            return {}
        if isinstance(supplied, dict):
            return _secret_identifiers(supplied, "call secret mapping")
        return {}

    @staticmethod
    def _jobs(document: dict[str, Any]) -> dict[str, dict[str, Any]]:
        jobs = document.get("jobs")
        if not isinstance(jobs, dict):
            raise PermissionCheckError("workflow has no jobs mapping")
        if not all(isinstance(job, dict) for job in jobs.values()):
            raise PermissionCheckError("workflow job is not a mapping")
        return jobs

    def _check_called(
        self,
        path: Path,
        inbound: dict[str, str],
        available_secrets: dict[str, str] | None,
        stack: tuple[Path, ...],
        unique_called: set[Path],
    ) -> list[Finding | SecretFinding]:
        path = path.resolve()
        if path in stack:
            cycle = " -> ".join(str(item.relative_to(self.root)) for item in (*stack, path))
            raise PermissionCheckError(f"reusable-workflow cycle: {cycle}")
        if len(stack) + 1 > MAX_WORKFLOW_LEVELS:
            chain = " -> ".join(
                str(item.relative_to(self.root)) for item in (*stack, path)
            )
            raise PermissionCheckError(
                f"workflow chain exceeds {MAX_WORKFLOW_LEVELS} workflow levels: {chain}"
            )
        unique_called.add(path)
        if len(unique_called) > MAX_UNIQUE_REUSABLE_WORKFLOWS:
            caller = stack[0].relative_to(self.root)
            raise PermissionCheckError(
                f"{caller} exceeds {MAX_UNIQUE_REUSABLE_WORKFLOWS} unique reusable workflows"
            )

        document = self._load(path)
        workflow_default = document.get("permissions")
        findings: list[Finding] = []
        for job_name, job in self._jobs(document).items():
            # Job permissions replace, rather than merge with, workflow permissions.
            # If neither is declared in a called workflow, the inbound token is
            # inherited unchanged.
            declaration = job.get("permissions", workflow_default)
            requested = inbound if declaration is None else _permission_vector(declaration)
            for scope in _exceeds(requested, inbound):
                findings.append(
                    Finding(path, job_name, scope, inbound[scope], requested[scope])
                )

            target = self._local_target(job.get("uses"))
            if target is not None:
                target_document = self._load(target)
                required_secrets, _ = self._workflow_call_secrets(target_document)
                findings.extend(
                    self._check_secret_call(
                        path, job_name, job, required_secrets, available_secrets
                    )
                )
                findings.extend(
                    self._check_called(
                        target,
                        requested,
                        self._callee_available_secrets(job, available_secrets),
                        (*stack, path),
                        unique_called,
                    )
                )
            elif self._is_remote_target(job.get("uses")):
                uses = job["uses"]
                required_secrets, inherit_eligible = self._remote_interface(uses)
                findings.extend(
                    self._check_secret_call(
                        path,
                        job_name,
                        job,
                        required_secrets,
                        available_secrets,
                        remote_inherit_eligible=inherit_eligible,
                    )
                )
                self._raise_remote_frontier(uses)
        return findings

    def _workflow_graph(self, workflow_paths: list[Path]) -> dict[Path, tuple[Path, ...]]:
        graph: dict[Path, tuple[Path, ...]] = {}
        pending = [path.resolve() for path in workflow_paths]
        while pending:
            path = pending.pop()
            if path in graph:
                continue
            targets = tuple(
                target.resolve()
                for job in self._jobs(self._load(path)).values()
                if (target := self._local_target(job.get("uses"))) is not None
            )
            graph[path] = targets
            pending.extend(target for target in targets if target not in graph)
        return graph

    def _assert_acyclic(self, graph: dict[Path, tuple[Path, ...]]) -> None:
        visited: set[Path] = set()
        active: list[Path] = []
        active_set: set[Path] = set()

        def visit(path: Path) -> None:
            if path in active_set:
                start = active.index(path)
                cycle_paths = (*active[start:], path)
                cycle = " -> ".join(
                    str(item.relative_to(self.root)) for item in cycle_paths
                )
                raise PermissionCheckError(f"reusable-workflow cycle: {cycle}")
            if path in visited:
                return
            active.append(path)
            active_set.add(path)
            for target in graph[path]:
                visit(target)
            active.pop()
            active_set.remove(path)
            visited.add(path)

        for path in graph:
            visit(path)

    def check(self) -> list[Finding | SecretFinding]:
        findings: list[Finding | SecretFinding] = []
        workflow_paths = sorted((*self.workflows.glob("*.yml"), *self.workflows.glob("*.yaml")))
        graph = self._workflow_graph(workflow_paths)
        self._assert_acyclic(graph)
        called = {target for targets in graph.values() for target in targets}

        for path in workflow_paths:
            if path.resolve() in called:
                continue
            document = self._load(path)
            unique_called: set[Path] = set()
            for job_name, job in self._jobs(document).items():
                target = self._local_target(job.get("uses"))
                if target is None and self._is_remote_target(job.get("uses")):
                    uses = job["uses"]
                    required_secrets, inherit_eligible = self._remote_interface(uses)
                    findings.extend(
                        self._check_secret_call(
                            path,
                            job_name,
                            job,
                            required_secrets,
                            None,
                            remote_inherit_eligible=inherit_eligible,
                        )
                    )
                    self._raise_remote_frontier(uses)
                    continue
                if target is None:
                    continue
                target_document = self._load(target)
                required_secrets, _ = self._workflow_call_secrets(target_document)
                findings.extend(
                    self._check_secret_call(path, job_name, job, required_secrets, None)
                )
                # At a top-level workflow, job permissions legitimately replace
                # the workflow default.  They become the reusable-call budget.
                declaration = job.get("permissions", document.get("permissions"))
                if declaration is None:
                    raise PermissionCheckError(
                        f"{path}: call job {job_name!r} relies on mutable repository defaults"
                    )
                findings.extend(
                    self._check_called(
                        target,
                        _permission_vector(declaration),
                        self._callee_available_secrets(job, None),
                        (path.resolve(),),
                        unique_called,
                    )
                )
        return findings


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", nargs="?", type=Path, default=Path.cwd())
    args = parser.parse_args(argv)
    try:
        findings = WorkflowPermissionChecker(args.root).check()
    except PermissionCheckError as error:
        print(f"workflow permission check is inconclusive: {error}", file=sys.stderr)
        return 2
    for finding in findings:
        print(finding.render(args.root.resolve()), file=sys.stderr)
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
