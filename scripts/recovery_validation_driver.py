#!/usr/bin/env python3
"""Run a checked-in recovery validation plan and preserve reproducible evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shlex
import shutil
import signal
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parent.parent
PROMPT_PATH = "docs/fork/recovery/ORCHESTRATOR_PROMPT.md"
VENDOR_UPSTREAM_REF = "refs/heads/vendor/upstream"
REQUIRED_ENV = {
    "CARGO_NET_OFFLINE": "true",
    "JCODE_NO_TELEMETRY": "1",
}
FORBIDDEN_COMMAND_FRAGMENTS = (
    "--update",
    "http://",
    "https://",
    "curl ",
    "wget ",
    "cargo install",
    "cargo publish",
    "nix flake update",
    "nix profile",
    "selfdev",
    " server reload",
    " release",
)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run_capture(argv: list[str], *, cwd: Path = REPO_ROOT) -> str:
    result = subprocess.run(
        argv,
        cwd=cwd,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return result.stdout


def git_output(*args: str) -> str:
    return run_capture(["git", *args])


def vendor_upstream_head() -> str:
    return git_output("rev-parse", "--verify", f"{VENDOR_UPSTREAM_REF}^{{commit}}").strip()


def prompt_diff_sha256() -> str:
    return sha256_bytes(
        subprocess.run(
            ["git", "diff", "--", PROMPT_PATH],
            cwd=REPO_ROOT,
            check=True,
            stdout=subprocess.PIPE,
        ).stdout
    )


def dirty_paths() -> list[str]:
    output = git_output("status", "--porcelain=v1", "-z")
    paths: list[str] = []
    records = output.split("\0")
    index = 0
    while index < len(records):
        record = records[index]
        index += 1
        if not record:
            continue
        status = record[:2]
        path = record[3:]
        if status[0] in "RC" and index < len(records):
            path = records[index]
            index += 1
        paths.append(path)
    return sorted(paths)


def process_rows() -> list[dict[str, Any]]:
    output = run_capture(["ps", "-axo", "pid=,ppid=,command="])
    rows = []
    for line in output.splitlines():
        fields = line.strip().split(None, 2)
        if len(fields) != 3:
            continue
        rows.append({"pid": int(fields[0]), "ppid": int(fields[1]), "command": fields[2]})
    return rows


def ancestor_pids(rows: list[dict[str, Any]]) -> set[int]:
    parents = {row["pid"]: row["ppid"] for row in rows}
    ancestors = {os.getpid()}
    pid = os.getppid()
    while pid > 1 and pid not in ancestors:
        ancestors.add(pid)
        pid = parents.get(pid, 1)
    return ancestors


def active_build_processes() -> list[dict[str, Any]]:
    rows = process_rows()
    ignored = ancestor_pids(rows)
    active = []
    for row in rows:
        if row["pid"] in ignored:
            continue
        command = row["command"]
        try:
            executable = Path(shlex.split(command)[0]).name if command.strip() else ""
        except ValueError:
            executable = ""
        lower = f" {command.lower()} "
        if executable in {"cargo", "rustc", "nix-build"} or any(
            fragment in lower for fragment in (" nix build ", " selfdev build ")
        ):
            active.append(row)
    return active


def tool_paths() -> dict[str, str | None]:
    return {name: shutil.which(name) for name in ("bash", "cargo", "git", "nix", "python3", "rustc")}


def repository_snapshot() -> dict[str, Any]:
    disk = shutil.disk_usage(REPO_ROOT)
    stash_list = git_output("stash", "list")
    worktrees = git_output("worktree", "list", "--porcelain")
    branches = git_output(
        "for-each-ref", "--format=%(refname:short) %(objectname)", "refs/heads"
    )
    vendor_head = vendor_upstream_head()
    return {
        "utc": utc_now(),
        "head": git_output("rev-parse", "HEAD").strip(),
        "branch": git_output("branch", "--show-current").strip(),
        "dirty_paths": dirty_paths(),
        "prompt_diff_sha256": prompt_diff_sha256(),
        "stash_count": len([line for line in stash_list.splitlines() if line]),
        "stash_list_sha256": sha256_bytes(stash_list.encode()),
        "worktrees_sha256": sha256_bytes(worktrees.encode()),
        "branches_sha256": sha256_bytes(branches.encode()),
        "vendor_upstream_head": vendor_head,
        "disk_free_bytes": disk.free,
        "disk_total_bytes": disk.total,
        "active_build_processes": active_build_processes(),
        "tool_paths": tool_paths(),
    }


def validate_plan(plan: dict[str, Any]) -> None:
    if plan.get("schema_version") != 1:
        raise ValueError("plan schema_version must be 1")
    if not isinstance(plan.get("steps"), list) or not plan["steps"]:
        raise ValueError("plan must contain at least one step")
    required_strings = ("name", "branch", "prompt_diff_sha256", "vendor_upstream_head")
    for key in required_strings:
        if not isinstance(plan.get(key), str) or not plan[key]:
            raise ValueError(f"plan must contain nonempty {key}")
    if not isinstance(plan.get("expected_stash_count"), int):
        raise ValueError("plan expected_stash_count must be an integer")
    if not isinstance(plan.get("minimum_free_gib"), int) or plan["minimum_free_gib"] < 0:
        raise ValueError("plan minimum_free_gib must be a nonnegative integer")
    forbidden_output = plan.get("forbidden_output_substrings", [])
    if not isinstance(forbidden_output, list) or not all(
        isinstance(value, str) and value for value in forbidden_output
    ):
        raise ValueError("forbidden_output_substrings must be nonempty strings")
    names: set[str] = set()
    for step in plan["steps"]:
        name = step.get("name")
        command = step.get("command")
        expected_exit = step.get("expected_exit")
        timeout_seconds = step.get("timeout_seconds")
        if not isinstance(name, str) or not name or name in names:
            raise ValueError(f"invalid or duplicate step name: {name!r}")
        names.add(name)
        if not isinstance(command, str) or not command.strip():
            raise ValueError(f"step {name} has no command")
        lower = f" {command.lower()} "
        for fragment in FORBIDDEN_COMMAND_FRAGMENTS:
            if fragment in lower:
                raise ValueError(f"step {name} contains forbidden command fragment {fragment!r}")
        if not isinstance(expected_exit, int) or not 0 <= expected_exit <= 255:
            raise ValueError(f"step {name} has invalid expected_exit")
        if not isinstance(timeout_seconds, int) or timeout_seconds <= 0:
            raise ValueError(f"step {name} has invalid timeout_seconds")


def validate_outer_environment(plan: dict[str, Any]) -> None:
    expected = dict(plan.get("environment", {}))
    for key, value in REQUIRED_ENV.items():
        if expected.get(key) != value:
            raise ValueError(f"plan must require {key}={value}")
    for key, value in expected.items():
        if os.environ.get(key) != value:
            raise ValueError(f"outer environment must set {key}={value!r}")
    if "substitute = false" not in os.environ.get("NIX_CONFIG", ""):
        raise ValueError("NIX_CONFIG must disable substitution for the offline run")


def validate_preflight(plan: dict[str, Any], snapshot: dict[str, Any]) -> None:
    errors = []
    if snapshot["branch"] != plan.get("branch"):
        errors.append(f"branch={snapshot['branch']!r}")
    if snapshot["dirty_paths"] != [PROMPT_PATH]:
        errors.append(f"dirty_paths={snapshot['dirty_paths']!r}")
    if snapshot["prompt_diff_sha256"] != plan.get("prompt_diff_sha256"):
        errors.append("prompt diff hash mismatch")
    if snapshot["stash_count"] != plan.get("expected_stash_count"):
        errors.append(f"stash_count={snapshot['stash_count']}")
    if snapshot["vendor_upstream_head"] != plan.get("vendor_upstream_head"):
        errors.append("vendor/upstream HEAD mismatch")
    minimum = int(plan.get("minimum_free_gib", 0)) * 1024**3
    if snapshot["disk_free_bytes"] < minimum:
        errors.append(f"free disk below {plan.get('minimum_free_gib')} GiB")
    if snapshot["active_build_processes"]:
        errors.append("active build process found")
    if errors:
        raise RuntimeError("preflight failed: " + "; ".join(errors))


def preservation_projection(snapshot: dict[str, Any]) -> dict[str, Any]:
    keys = (
        "head",
        "branch",
        "dirty_paths",
        "prompt_diff_sha256",
        "stash_count",
        "stash_list_sha256",
        "worktrees_sha256",
        "branches_sha256",
        "vendor_upstream_head",
    )
    return {key: snapshot[key] for key in keys}


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def progress(message: str, **fields: Any) -> None:
    payload = {"message": message, **fields}
    print("JCODE_PROGRESS " + json.dumps(payload, sort_keys=True), flush=True)


def terminate_process_group(proc: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(proc.pid, signal.SIGTERM)
        proc.wait(timeout=5)
    except ProcessLookupError:
        return
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
        proc.wait()


def run_step(
    index: int,
    total: int,
    step: dict[str, Any],
    output_dir: Path,
    environment: dict[str, str],
    forbidden_output_substrings: list[str],
) -> dict[str, Any]:
    name = step["name"]
    expected = step["expected_exit"]
    command = step["command"]
    timeout = step["timeout_seconds"]
    log_path = output_dir / f"{index:02d}-{name}.log"
    started_utc = utc_now()
    started = time.monotonic()
    progress(f"Running {name} ({index}/{total})", current=index, total=total, unit="step")
    header = {
        "name": name,
        "command": command,
        "command_sha256": sha256_bytes(command.encode()),
        "expected_exit": expected,
        "started_utc": started_utc,
        "timeout_seconds": timeout,
    }
    with log_path.open("wb") as log:
        log.write(("JCODE_STEP " + json.dumps(header, sort_keys=True) + "\n").encode())
        log.flush()
        proc = subprocess.Popen(
            ["/bin/bash", "-c", command],
            cwd=REPO_ROOT,
            env=environment,
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            actual = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            terminate_process_group(proc)
            actual = 124
        elapsed = time.monotonic() - started
        footer = {
            "actual_exit": actual,
            "elapsed_seconds": round(elapsed, 3),
            "ended_utc": utc_now(),
            "name": name,
        }
        log.write(("JCODE_STEP_RESULT " + json.dumps(footer, sort_keys=True) + "\n").encode())
    observation_count = 0
    log_text = log_path.read_text(errors="replace")
    forbidden_hits = [value for value in forbidden_output_substrings if value in log_text]
    if forbidden_hits:
        actual = 126
        with log_path.open("a") as log:
            log.write(
                "JCODE_DRIVER_ERROR forbidden output substring detected: "
                + json.dumps(forbidden_hits)
                + "\n"
            )
    if name == "pilot_fixture" and actual == expected:
        observation_count = sum(
            1 for line in log_text.splitlines() if line.startswith("PILOT_OBSERVATION ")
        )
        if observation_count != 1:
            actual = 125
            with log_path.open("a") as log:
                log.write(
                    f"JCODE_DRIVER_ERROR expected one PILOT_OBSERVATION, found {observation_count}\n"
                )
    return {
        "name": name,
        "expected_exit": expected,
        "actual_exit": actual,
        "elapsed_seconds": round(elapsed, 3),
        "command": command,
        "command_sha256": sha256_bytes(command.encode()),
        "log": log_path.name,
        "log_sha256": sha256_file(log_path),
        "pilot_observation_count": observation_count,
        "forbidden_output_hits": len(forbidden_hits),
    }


def write_manifest(path: Path, rows: list[dict[str, Any]]) -> None:
    columns = (
        "name",
        "expected_exit",
        "actual_exit",
        "elapsed_seconds",
        "command_sha256",
        "log",
        "log_sha256",
        "pilot_observation_count",
        "forbidden_output_hits",
    )
    with path.open("w") as handle:
        handle.write("\t".join(columns) + "\n")
        for row in rows:
            handle.write("\t".join(str(row[column]) for column in columns) + "\n")


def write_sha256sums(output_dir: Path) -> None:
    sums_path = output_dir / "SHA256SUMS"
    members = sorted(
        path for path in output_dir.iterdir() if path.is_file() and path.name != sums_path.name
    )
    with sums_path.open("w") as handle:
        for path in members:
            handle.write(f"{sha256_file(path)}  {path.name}\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check-plan", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    plan_path = args.plan.resolve()
    plan = json.loads(plan_path.read_text())
    validate_plan(plan)
    if args.check_plan:
        print(f"Plan OK: {plan_path}")
        return 0
    if args.output is None:
        raise SystemExit("--output is required unless --check-plan is used")

    validate_outer_environment(plan)
    output_dir = args.output.resolve()
    if output_dir.exists():
        raise SystemExit(f"output path already exists: {output_dir}")
    output_dir.mkdir(parents=True)
    shutil.copyfile(plan_path, output_dir / "plan.json")

    started_utc = utc_now()
    preflight = repository_snapshot()
    write_json(output_dir / "preflight.json", preflight)
    validate_preflight(plan, preflight)

    environment = os.environ.copy()
    environment.update({str(key): str(value) for key, value in plan.get("environment", {}).items()})
    rows: list[dict[str, Any]] = []
    failure: str | None = None
    for index, step in enumerate(plan["steps"], start=1):
        row = run_step(
            index,
            len(plan["steps"]),
            step,
            output_dir,
            environment,
            plan.get("forbidden_output_substrings", []),
        )
        rows.append(row)
        if row["actual_exit"] != row["expected_exit"]:
            failure = (
                f"{row['name']} exit {row['actual_exit']} != expected {row['expected_exit']}"
            )
            break

    postflight = repository_snapshot()
    write_json(output_dir / "postflight.json", postflight)
    if preservation_projection(postflight) != preservation_projection(preflight):
        failure = failure or "postflight preservation snapshot changed"
    if postflight["active_build_processes"]:
        failure = failure or "postflight found an active build process"

    write_manifest(output_dir / "manifest.tsv", rows)
    run_meta = {
        "schema_version": 1,
        "name": plan["name"],
        "started_utc": started_utc,
        "ended_utc": utc_now(),
        "repo_root": str(REPO_ROOT),
        "plan_source": str(plan_path),
        "plan_sha256": sha256_file(output_dir / "plan.json"),
        "head": preflight["head"],
        "branch": preflight["branch"],
        "architecture": platform.machine(),
        "python": sys.version,
        "tool_paths": preflight["tool_paths"],
        "environment": plan.get("environment", {}),
        "steps_planned": len(plan["steps"]),
        "steps_run": len(rows),
        "result": "failed" if failure else "passed",
        "failure": failure,
    }
    write_json(output_dir / "run.meta.json", run_meta)
    write_sha256sums(output_dir)
    progress("Recovery validation complete", current=len(rows), total=len(plan["steps"]), unit="step")
    if failure:
        print(f"FAILED: {failure}", file=sys.stderr)
        return 1
    print(f"PASS: evidence written to {output_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
