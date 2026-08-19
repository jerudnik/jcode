#!/usr/bin/env python3

from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
# Borrowed, not donated: append, import, remove (see test_ci_workflow_commands).
_SCRIPTS_DIR = str(ROOT / "scripts")
_BORROWED_PATH_ENTRY = _SCRIPTS_DIR not in sys.path
if _BORROWED_PATH_ENTRY:
    sys.path.append(_SCRIPTS_DIR)
try:
    from check_reusable_workflow_calls import (  # noqa: E402
        ReusableWorkflowCallChecker,
        ReusableWorkflowCallError,
    )
finally:
    if _BORROWED_PATH_ENTRY:
        sys.path.remove(_SCRIPTS_DIR)


class ReusableWorkflowCallPolicyTests(unittest.TestCase):
    def check_documents(self, documents: dict[str, str]) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            workflows = root / ".github" / "workflows"
            workflows.mkdir(parents=True)
            for name, content in documents.items():
                target = workflows / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(content, encoding="utf-8")
            ReusableWorkflowCallChecker(root).check()

    def assert_rejected(self, documents: dict[str, str], message: str) -> None:
        with self.assertRaisesRegex(ReusableWorkflowCallError, message):
            self.check_documents(documents)

    @staticmethod
    def caller(uses: str, extra: str = "") -> str:
        return (
            "on: push\n"
            "jobs:\n"
            "  call:\n"
            f"    uses: {uses}\n"
            f"{extra}"
        )

    @staticmethod
    def callee(target: str | None = None, prefix: str = "./") -> str:
        if target is None:
            job = "  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: 'true'\n"
        else:
            job = f"  call:\n    uses: {prefix}.github/workflows/{target}\n"
        return "on:\n  workflow_call:\njobs:\n" + job

    def test_current_repository_passes(self) -> None:
        ReusableWorkflowCallChecker(ROOT).check()

    def test_call_job_rejects_services_and_snapshot(self) -> None:
        fixtures = {
            "services": "    services:\n      db:\n        image: postgres\n",
            "snapshot": "    snapshot: true\n",
        }
        for keyword, extra in fixtures.items():
            with self.subTest(keyword=keyword):
                self.assert_rejected(
                    {"caller.yml": self.caller("owner/repo/.github/workflows/build.yml@v1", extra)},
                    rf"forbidden key\(s\): {keyword}",
                )

    def test_call_job_accepts_complete_supported_keyword_matrix(self) -> None:
        self.check_documents(
            {
                "caller.yml": (
                    "on: push\njobs:\n  call:\n"
                    "    name: Call\n"
                    "    uses: owner/repo/.github/workflows/build.yml@v1\n"
                    "    with:\n      mode: release\n"
                    "    secrets: inherit\n"
                    "    strategy:\n      fail-fast: false\n"
                    "    needs: prepare\n"
                    "    if: success()\n"
                    "    concurrency: calls\n"
                    "    permissions:\n      contents: read\n"
                )
            }
        )

    def test_expressions_in_job_and_step_uses_are_rejected(self) -> None:
        self.assert_rejected(
            {"caller.yml": self.caller("${{ inputs.workflow }}")},
            "must not contain an expression",
        )
        self.assert_rejected(
            {
                "caller.yml": (
                    "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n"
                    "    steps:\n      - uses: actions/${{ inputs.action }}@v1\n"
                )
            },
            "must not contain an expression",
        )

    def test_local_call_path_is_exactly_one_workflow_file(self) -> None:
        invalid = (
            "./workflow.yml",
            "./.github/workflows/nested/build.yml",
            "./.github/workflows/build.yml@main",
            "./.github/workflows/build.json",
            "$/workflow.yml",
            "$/.github/workflows/nested/build.yml",
            "$/.github/workflows/build.yml@main",
            "$/.github/workflows/build.json",
            "../.github/workflows/build.yml",
        )
        for uses in invalid:
            with self.subTest(uses=uses):
                self.assert_rejected({"caller.yml": self.caller(uses)}, "local reusable workflow must be exactly|remote reusable workflow must be|not a reusable-workflow reference")

    def test_valid_local_call_forms_resolve_to_the_same_target(self) -> None:
        for uses in (
            "./.github/workflows/build.yml",
            "./.github/workflows/build.yaml",
            "$/.github/workflows/build.yml",
            "$/.github/workflows/build.yaml",
        ):
            with self.subTest(uses=uses):
                suffix = pathlib.PurePosixPath(uses).suffix
                target = f"build{suffix}"
                self.check_documents(
                    {
                        "caller.yml": self.caller(uses),
                        target: self.callee(),
                    }
                )

        self.check_documents(
            {
                "caller.yml": (
                    "on: push\njobs:\n"
                    "  dot:\n    uses: ./.github/workflows/build.yml\n"
                    "  dollar:\n    uses: $/.github/workflows/build.yml\n"
                ),
                "build.yml": self.callee(),
            }
        )

    def test_remote_call_path_and_ref_are_strict(self) -> None:
        invalid = (
            "owner/repo/workflows/build.yml@v1",
            "owner/repo/.github/workflows/nested/build.yml@v1",
            "owner/repo/.github/workflows/build.yml",
            "owner/repo/.github/workflows/build.yml@",
            "owner/repo/.github/workflows/build.yml@v1@other",
            "owner/repo/.github/workflows/build.yml@refs/heads/main",
            "owner/repo/.github/workflows/build.yml@refs/tags/v1",
        )
        for uses in invalid:
            with self.subTest(uses=uses):
                self.assert_rejected({"caller.yml": self.caller(uses)}, "remote reusable workflow must be|ref must omit|not a reusable-workflow reference")

    def test_valid_remote_call_forms_pass(self) -> None:
        for uses in (
            "owner/repo/.github/workflows/build.yml@main",
            "owner/repo/.github/workflows/build.yaml@v1.2.3",
            "owner/repo/.github/workflows/build.yml@0123456789abcdef",
        ):
            with self.subTest(uses=uses):
                self.check_documents({"caller.yml": self.caller(uses)})

    def test_reusable_workflow_path_is_rejected_at_step_level(self) -> None:
        for uses in (
            "./.github/workflows/build.yml",
            "$/.github/workflows/build.yml",
            "owner/repo/.github/workflows/build.yml@v1",
        ):
            with self.subTest(uses=uses):
                self.assert_rejected(
                    {
                        "caller.yml": (
                            "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n"
                            f"    steps:\n      - uses: {uses}\n"
                        )
                    },
                    "cannot call a reusable workflow",
                )

    def test_missing_and_non_callable_local_targets_are_rejected(self) -> None:
        self.assert_rejected(
            {"caller.yml": self.caller("./.github/workflows/missing.yml")},
            "local target does not exist",
        )
        self.assert_rejected(
            {
                "caller.yml": self.caller("./.github/workflows/target.yml"),
                "target.yml": "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - run: 'true'\n",
            },
            "does not declare on.workflow_call",
        )

    def test_disconnected_cycles_are_rejected(self) -> None:
        self.assert_rejected(
            {
                "root.yml": self.caller("owner/repo/.github/workflows/build.yml@v1"),
                "cycle-a.yml": self.callee("cycle-b.yml", "$/"),
                "cycle-b.yml": self.callee("cycle-a.yml"),
            },
            "reusable-workflow cycle",
        )

    def test_exactly_ten_total_levels_pass_and_eleven_fail(self) -> None:
        valid = {"level-01.yml": "on: push\njobs:\n  call:\n    uses: ./.github/workflows/level-02.yml\n"}
        for level in range(2, 11):
            target = f"level-{level + 1:02}.yml" if level < 10 else None
            valid[f"level-{level:02}.yml"] = self.callee(target)
        self.check_documents(valid)

        invalid = dict(valid)
        invalid["level-10.yml"] = self.callee("level-11.yml", "$/")
        invalid["level-11.yml"] = self.callee()
        self.assert_rejected(invalid, "exceeds 10 total workflow levels")

    def test_exactly_fifty_unique_local_callees_pass_and_fifty_one_fail(self) -> None:
        def documents(count: int) -> dict[str, str]:
            jobs = "".join(
                f"  call_{index:02}:\n    uses: "
                f"{'$/' if index % 2 else './'}.github/workflows/callee-{index:02}.yml\n"
                for index in range(count)
            )
            result = {"caller.yml": "on: push\njobs:\n" + jobs}
            result.update({f"callee-{index:02}.yml": self.callee() for index in range(count)})
            return result

        self.check_documents(documents(50))
        self.assert_rejected(documents(51), "exceeds 50 unique local reusable workflows")


if __name__ == "__main__":
    unittest.main()
