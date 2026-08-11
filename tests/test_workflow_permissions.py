#!/usr/bin/env python3

from __future__ import annotations

import pathlib
import json
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from check_workflow_permissions import (  # noqa: E402
    PERMISSION_LEVELS,
    PermissionCheckError,
    WorkflowPermissionChecker,
    _permission_vector,
)


FIXTURES = ROOT / "tests" / "fixtures" / "workflow_permissions"


class WorkflowPermissionTests(unittest.TestCase):
    def test_github_com_permission_scope_matrix_is_complete(self) -> None:
        self.assertEqual(
            {
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
            },
            PERMISSION_LEVELS,
        )

    def test_every_github_com_scope_accepts_exactly_its_supported_levels(self) -> None:
        for scope, levels in PERMISSION_LEVELS.items():
            for level in levels:
                with self.subTest(scope=scope, level=level):
                    self.assertEqual(level, _permission_vector({scope: level})[scope])

            for invalid in {"read", "write", "admin"} - set(levels):
                with self.subTest(scope=scope, invalid=invalid):
                    with self.assertRaisesRegex(
                        PermissionCheckError, f"invalid {scope} permission"
                    ):
                        _permission_vector({scope: invalid})

    def test_permission_aggregates_expand_using_each_scope_maximum(self) -> None:
        read_all = _permission_vector("read-all")
        write_all = _permission_vector("write-all")

        self.assertEqual(set(PERMISSION_LEVELS), set(read_all))
        self.assertEqual(set(PERMISSION_LEVELS), set(write_all))
        for scope, levels in PERMISSION_LEVELS.items():
            with self.subTest(scope=scope):
                self.assertEqual("read" if "read" in levels else "none", read_all[scope])
                self.assertEqual(levels[-1], write_all[scope])

        self.assertEqual("write", write_all["code-quality"])
        self.assertEqual("read", write_all["models"])
        self.assertEqual("write", write_all["repository-projects"])
        self.assertEqual("read", write_all["vulnerability-alerts"])

    def test_unknown_permission_scope_fails_closed(self) -> None:
        with self.assertRaisesRegex(PermissionCheckError, "unknown permission scope"):
            _permission_vector({"future-scope": "read"})

    def check_fixture(self, name: str):
        return WorkflowPermissionChecker(FIXTURES / name).check()

    def check_documents(self, documents: dict[str, str]):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            workflows = root / ".github" / "workflows"
            workflows.mkdir(parents=True)
            for name, content in documents.items():
                (workflows / name).write_text(content, encoding="utf-8")
            return WorkflowPermissionChecker(root).check()

    def check_documents_with_metadata(
        self, documents: dict[str, str], metadata: dict[str, object]
    ):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            workflows = root / ".github" / "workflows"
            workflows.mkdir(parents=True)
            for name, content in documents.items():
                (workflows / name).write_text(content, encoding="utf-8")
            (root / ".github" / "reusable-workflow-secrets.json").write_text(
                json.dumps(metadata), encoding="utf-8"
            )
            return WorkflowPermissionChecker(root).check()

    @staticmethod
    def caller(*targets: str) -> str:
        jobs = "\n".join(
            f"  call_{index}:\n"
            "    permissions:\n"
            "      contents: read\n"
            f"    uses: ./.github/workflows/{target}\n"
            for index, target in enumerate(targets)
        )
        return f"on: push\npermissions:\n  contents: read\njobs:\n{jobs}"

    @staticmethod
    def callee(target: str | None = None, *, write: bool = False) -> str:
        permission = "write" if write else "read"
        if target is not None:
            job = (
                "  call:\n"
                "    permissions:\n"
                f"      contents: {permission}\n"
                f"    uses: ./.github/workflows/{target}\n"
            )
        else:
            job = (
                "  build:\n"
                "    permissions:\n"
                f"      contents: {permission}\n"
                "    runs-on: ubuntu-latest\n"
                "    steps:\n"
                "      - run: echo build\n"
            )
        return f"on:\n  workflow_call:\njobs:\n{job}"

    def test_ordinary_callee_job_cannot_exceed_inbound_budget(self) -> None:
        findings = self.check_fixture("ordinary-job")
        self.assertEqual(1, len(findings))
        self.assertEqual(("cleanup", "actions", "none", "write"), (
            findings[0].job,
            findings[0].scope,
            findings[0].allowed,
            findings[0].requested,
        ))

    def test_disabled_callee_job_is_still_validated(self) -> None:
        findings = self.check_fixture("disabled-ordinary-job")
        self.assertEqual(1, len(findings))
        self.assertEqual("cleanup", findings[0].job)
        self.assertEqual("actions", findings[0].scope)

    def test_job_permissions_replace_workflow_default(self) -> None:
        self.assertEqual([], self.check_fixture("job-precedence"))

    def test_disconnected_direct_cycle_is_inconclusive(self) -> None:
        documents = {
            "caller.yml": self.caller("leaf.yml"),
            "leaf.yml": self.callee(),
            "cycle.yml": self.callee("cycle.yml"),
        }
        with self.assertRaisesRegex(PermissionCheckError, "reusable-workflow cycle"):
            self.check_documents(documents)

    def test_disconnected_indirect_cycle_is_inconclusive(self) -> None:
        documents = {
            "caller.yml": self.caller("leaf.yml"),
            "leaf.yml": self.callee(),
            "cycle-a.yml": self.callee("cycle-b.yml"),
            "cycle-b.yml": self.callee("cycle-a.yml"),
        }
        with self.assertRaisesRegex(PermissionCheckError, "reusable-workflow cycle"):
            self.check_documents(documents)

    def test_all_workflows_called_cycle_is_inconclusive(self) -> None:
        documents = {
            "cycle-a.yml": self.callee("cycle-b.yml"),
            "cycle-b.yml": self.callee("cycle-a.yml"),
        }
        with self.assertRaisesRegex(PermissionCheckError, "reusable-workflow cycle"):
            self.check_documents(documents)

    def test_unreachable_permission_escalation_fails_closed(self) -> None:
        documents = {
            "caller.yml": self.caller("leaf.yml"),
            "leaf.yml": self.callee(),
            "cycle-a.yml": self.callee("cycle-b.yml"),
            "cycle-b.yml": self.callee("cycle-a.yml", write=True),
        }
        with self.assertRaisesRegex(PermissionCheckError, "reusable-workflow cycle"):
            self.check_documents(documents)

    def test_eleven_workflow_levels_are_inconclusive(self) -> None:
        documents = {"caller.yml": self.caller("level-01.yml")}
        for level in range(1, 11):
            target = f"level-{level + 1:02}.yml" if level < 10 else None
            documents[f"level-{level:02}.yml"] = self.callee(target)
        with self.assertRaisesRegex(PermissionCheckError, "exceeds 10 workflow levels"):
            self.check_documents(documents)

    def test_ten_workflow_levels_are_allowed(self) -> None:
        documents = {"caller.yml": self.caller("level-01.yml")}
        for level in range(1, 10):
            target = f"level-{level + 1:02}.yml" if level < 9 else None
            documents[f"level-{level:02}.yml"] = self.callee(target)
        self.assertEqual([], self.check_documents(documents))

    def test_fifty_one_unique_reusable_workflows_are_inconclusive(self) -> None:
        targets = [f"callee-{index:02}.yml" for index in range(51)]
        documents = {"caller.yml": self.caller(*targets)}
        documents.update({target: self.callee() for target in targets})
        with self.assertRaisesRegex(PermissionCheckError, "exceeds 50 unique reusable workflows"):
            self.check_documents(documents)

    def test_fifty_unique_reusable_workflows_are_allowed(self) -> None:
        targets = [f"callee-{index:02}.yml" for index in range(50)]
        documents = {"caller.yml": self.caller(*targets)}
        documents.update({target: self.callee() for target in targets})
        self.assertEqual([], self.check_documents(documents))

    def test_sha_pinned_remote_workflow_is_inconclusive_without_metadata(self) -> None:
        with self.assertRaisesRegex(
            PermissionCheckError,
            "SHA-pinned but opaque.*interface and permission metadata",
        ):
            self.check_fixture("remote-sha")

    def test_mutable_remote_workflow_ref_is_inconclusive(self) -> None:
        with self.assertRaisesRegex(
            PermissionCheckError,
            "mutable and opaque.*immutable SHA",
        ):
            self.check_fixture("remote-mutable-ref")

    def test_local_callee_with_remote_descendant_is_inconclusive(self) -> None:
        with self.assertRaisesRegex(
            PermissionCheckError,
            "remote reusable workflow.*mutable and opaque",
        ):
            self.check_fixture("remote-descendant")

    def test_actionlint_secrets_inherit_startup_semantics(self) -> None:
        findings = self.check_documents(
            {
                "caller.yml": """on: push
permissions:
  contents: read
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/callee.yml
""",
                "callee.yml": """on:
  workflow_call:
    secrets:
      TOKEN:
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo build
""",
            }
        )
        self.assertEqual(1, len(findings))
        self.assertIn("absent from the secrets mapping", findings[0].reason)

    def test_top_level_inherit_cannot_prove_required_local_secret(self) -> None:
        findings = self.check_fixture("top-level-inherit-required-secret")

        self.assertEqual(1, len(findings))
        self.assertEqual("TOKEN", findings[0].secret)
        self.assertIn("does not prove the top-level caller has it", findings[0].reason)
        self.assertIn("explicit secrets mapping", findings[0].reason)

    def test_actionlint_secrets_inherit_policy_integration(self) -> None:
        findings = self.check_documents(
            {
                "caller.yml": """on: push
permissions:
  contents: read
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/middle.yml
    secrets:
      OTHER: ${{ secrets.OTHER }}
""",
                "middle.yml": """on:
  workflow_call:
    secrets:
      OTHER:
        required: false
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/leaf.yml
    secrets: inherit
""",
                "leaf.yml": """on:
  workflow_call:
    secrets:
      DEPLOY_TOKEN:
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo build
""",
            }
        )
        self.assertEqual(1, len(findings))
        self.assertEqual("DEPLOY_TOKEN", findings[0].secret)
        self.assertIn("caller does not receive it", findings[0].reason)

    def test_explicit_forwarding_source_must_exist_in_caller_interface(self) -> None:
        findings = self.check_documents(
            {
                "caller.yml": self.caller("middle.yml"),
                "middle.yml": """on:
  workflow_call:
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/leaf.yml
    secrets:
      TOKEN: ${{ secrets.MISSING }}
""",
                "leaf.yml": """on:
  workflow_call:
    secrets:
      TOKEN:
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo build
""",
            }
        )
        self.assertEqual(1, len(findings))
        self.assertIn("source secret 'MISSING'", findings[0].reason)

    def test_explicit_secret_mapping_matches_required_name_case_insensitively(self) -> None:
        findings = self.check_documents(
            {
                "caller.yml": """on: push
permissions:
  contents: read
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/callee.yml
    secrets:
      token: ${{ secrets.token }}
""",
                "callee.yml": """on:
  workflow_call:
    secrets:
      TOKEN:
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo build
""",
            }
        )
        self.assertEqual([], findings)

    def test_nested_forwarding_and_inherit_match_names_case_insensitively(self) -> None:
        findings = self.check_documents(
            {
                "caller.yml": """on: push
permissions:
  contents: read
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/middle.yml
    secrets:
      token: ${{ secrets.token }}
""",
                "middle.yml": """on:
  workflow_call:
    secrets:
      TOKEN:
        required: true
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/leaf.yml
    secrets: inherit
""",
                "leaf.yml": """on:
  workflow_call:
    secrets:
      ToKeN:
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo build
""",
            }
        )
        self.assertEqual([], findings)

    def test_nested_forwarded_source_matches_caller_interface_case_insensitively(self) -> None:
        findings = self.check_documents(
            {
                "caller.yml": """on: push
permissions:
  contents: read
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/middle.yml
    secrets:
      TOKEN: ${{ secrets.TOKEN }}
""",
                "middle.yml": """on:
  workflow_call:
    secrets:
      token:
        required: true
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/leaf.yml
    secrets:
      deploy: ${{ secrets.ToKeN }}
""",
                "leaf.yml": """on:
  workflow_call:
    secrets:
      DEPLOY:
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo build
""",
            }
        )
        self.assertEqual([], findings)

    def test_secret_diagnostics_preserve_declared_and_referenced_spelling(self) -> None:
        findings = self.check_documents(
            {
                "caller.yml": self.caller("middle.yml"),
                "middle.yml": """on:
  workflow_call:
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/leaf.yml
    secrets:
      deploy_token: ${{ secrets.MiSsInG }}
""",
                "leaf.yml": """on:
  workflow_call:
    secrets:
      RequiredToken:
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo build
""",
            }
        )
        self.assertEqual(
            ["RequiredToken", "deploy_token"], [item.secret for item in findings]
        )
        self.assertIn("source secret 'MiSsInG'", findings[1].reason)

    def test_secret_names_differing_only_by_case_are_rejected(self) -> None:
        documents = {
            "caller.yml": """on: push
permissions:
  contents: read
jobs:
  call:
    permissions:
      contents: read
    uses: ./.github/workflows/callee.yml
    secrets:
      TOKEN: ${{ secrets.TOKEN }}
      token: ${{ secrets.token }}
""",
            "callee.yml": """on:
  workflow_call:
    secrets:
      TOKEN:
        required: true
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo build
""",
        }
        with self.assertRaisesRegex(
            PermissionCheckError, "'TOKEN' and 'token' differ only by case"
        ):
            self.check_documents(documents)

        documents["callee.yml"] = documents["callee.yml"].replace(
            "      TOKEN:\n        required: true",
            "      TOKEN:\n        required: true\n      token:\n        required: false",
        )
        documents["caller.yml"] = documents["caller.yml"].replace(
            "      token: ${{ secrets.token }}\n", ""
        )
        with self.assertRaisesRegex(
            PermissionCheckError, "'TOKEN' and 'token' differ only by case"
        ):
            self.check_documents(documents)

    def test_remote_inherit_requires_reviewed_eligibility(self) -> None:
        uses = "other-org/central/.github/workflows/build.yml@main"
        documents = {
            "caller.yml": f"""on: push
permissions:
  contents: read
jobs:
  call:
    permissions:
      contents: read
    uses: {uses}
    secrets: inherit
"""
        }
        metadata = {
            "remote_interfaces": {
                uses: {
                    "review": "SEC-123 cross-org eligibility review",
                    "interface_sha": "0123456789abcdef0123456789abcdef01234567",
                    "required_secrets": ["TOKEN"],
                    "inherit_eligible": False,
                }
            }
        }
        with self.assertRaisesRegex(PermissionCheckError, "without explicit reviewed inherit eligibility"):
            self.check_documents_with_metadata(documents, metadata)

    def test_reviewed_mutable_remote_secret_interface_still_fails_closed(self) -> None:
        uses = "same-enterprise/central/.github/workflows/build.yml@main"
        with self.assertRaisesRegex(
            PermissionCheckError, "permission, input, accessibility, depth, unique-call, and descendant"
        ):
            self.check_documents_with_metadata(
                {
                    "caller.yml": f"""on: push
permissions:
  contents: read
jobs:
  call:
    permissions:
      contents: read
    uses: {uses}
    secrets:
      TOKEN: ${{{{ secrets.TOKEN }}}}
"""
                },
                {
                    "remote_interfaces": {
                        uses: {
                            "review": "SEC-124 enterprise policy and interface review",
                            "interface_sha": "0123456789abcdef0123456789abcdef01234567",
                            "required_secrets": ["TOKEN"],
                            "inherit_eligible": True,
                        }
                    }
                },
            )

    def test_current_repository_has_no_reusable_permission_violation(self) -> None:
        self.assertEqual([], WorkflowPermissionChecker(ROOT).check())


if __name__ == "__main__":
    unittest.main()
