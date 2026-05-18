#!/usr/bin/env python3
"""Regression tests for scripts/dev_cargo.sh setup selection.

Run with: python tests/test_dev_cargo.py
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import tempfile

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
DEV_CARGO = REPO_ROOT / "scripts" / "dev_cargo.sh"


def run_print_setup(env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(DEV_CARGO), "--print-setup"],
        cwd=REPO_ROOT,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )


def fake_sccache_dir() -> tempfile.TemporaryDirectory[str]:
    temp = tempfile.TemporaryDirectory()
    fake = pathlib.Path(temp.name) / "sccache"
    fake.write_text(
        "#!/usr/bin/env bash\n"
        "if [[ \"${1:-}\" == \"--start-server\" ]]; then exit 0; fi\n"
        "echo 'fake sccache compiler invocation is unexpected in setup tests' >&2\n"
        "exit 77\n",
        encoding="utf-8",
    )
    fake.chmod(0o755)
    return temp


def base_env_with_fake_sccache(temp_path: str) -> dict[str, str]:
    env = os.environ.copy()
    env["PATH"] = f"{temp_path}{os.pathsep}{env['PATH']}"
    env.pop("RUSTC_WRAPPER", None)
    env.pop("JCODE_REMOTE_CARGO", None)
    return env


def test_auto_sccache_is_skipped_when_cargo_incremental_is_set() -> None:
    with fake_sccache_dir() as temp:
        env = base_env_with_fake_sccache(temp)
        env["CARGO_INCREMENTAL"] = "1"

        result = run_print_setup(env)

    assert "sccache_status=skipped-cargo-incremental-env" in result.stdout
    assert "rustc_wrapper=<unset>" in result.stdout
    assert "CARGO_INCREMENTAL is set; using direct rustc" in result.stderr


def test_auto_sccache_is_enabled_when_incremental_env_is_unset() -> None:
    with fake_sccache_dir() as temp:
        env = base_env_with_fake_sccache(temp)
        env.pop("CARGO_INCREMENTAL", None)

        result = run_print_setup(env)

    assert "sccache_status=enabled" in result.stdout
    assert "rustc_wrapper=sccache" in result.stdout


if __name__ == "__main__":
    test_auto_sccache_is_skipped_when_cargo_incremental_is_set()
    test_auto_sccache_is_enabled_when_incremental_env_is_unset()
    print("dev_cargo setup tests passed")
