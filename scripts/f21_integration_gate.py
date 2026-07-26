#!/usr/bin/env python3
"""F21 — deterministic CI/package/updater integration gate.

Runs the full gate from clean state twice at a single commit and proves the two
runs agree. F21's acceptance is not "the suite passes" (F17 already gates that);
it is that the *packaged* artifact and the *installed* behaviour are reproducible
and self-consistent at one source identity.

Four phases per run:

  suites   the blocking test rails fork-ci runs on this platform
  package  a real `nix build` of the flake package
  install  assertions about what actually landed in the store output
  updater  the F20a/F20c matrix, exercised against the *installed* binary

Determinism is judged on a per-check `fingerprint`, not on raw logs. Logs contain
timestamps, durations, paths under a per-run temp dir and test-ordering noise,
none of which are part of the claim. A fingerprint is the normalized fact a check
establishes: the store path, the set of installed asset paths, the updater's
verdict for a given install kind. Two runs agree iff every fingerprint matches.

Usage:
    scripts/f21_integration_gate.py --runs 2
    scripts/f21_integration_gate.py --runs 2 --skip-suites   # package+install+updater only
    scripts/f21_integration_gate.py --self-test              # verify the harness itself
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field, asdict
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


class GateError(RuntimeError):
    """A check could not be evaluated (as opposed to evaluating to a failure)."""


@dataclass
class Check:
    """One verifiable fact.

    `ok` says whether it holds. `fingerprint` is the part that must be identical
    across runs; it deliberately excludes timing and per-run scratch paths.
    """

    name: str
    ok: bool
    fingerprint: str
    detail: str = ""
    duration_s: float = 0.0


@dataclass
class RunResult:
    index: int
    commit: str
    checks: list[Check] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return all(c.ok for c in self.checks)

    def fingerprints(self) -> dict[str, str]:
        return {c.name: c.fingerprint for c in self.checks}


def run_cmd(
    argv: list[str],
    *,
    cwd: Path = REPO,
    env: dict[str, str] | None = None,
    timeout: int = 3600,
) -> subprocess.CompletedProcess:
    full_env = dict(os.environ)
    if env:
        full_env.update(env)
    return subprocess.run(
        argv,
        cwd=cwd,
        env=full_env,
        capture_output=True,
        text=True,
        timeout=timeout,
    )


def git_commit() -> str:
    r = run_cmd(["git", "rev-parse", "HEAD"], timeout=60)
    if r.returncode != 0:
        raise GateError(f"git rev-parse failed: {r.stderr.strip()}")
    return r.stdout.strip()


def git_is_dirty() -> bool:
    r = run_cmd(["git", "status", "--porcelain"], timeout=60)
    return bool(r.stdout.strip())


def digest(*parts: str) -> str:
    h = hashlib.sha256()
    for p in parts:
        h.update(p.encode())
        h.update(b"\0")
    return h.hexdigest()[:16]


# --------------------------------------------------------------------------
# phase: suites
# --------------------------------------------------------------------------

# The blocking library rails fork-ci runs. Each is (check-name, cargo -p args).
# Kept to --lib: the integration binaries (provider_matrix, e2e) need network or
# a built release binary and are attributed to their own CI steps.
SUITE_PACKAGES = [
    ("suites.jcode-base", ["-p", "jcode-base"]),
    ("suites.jcode-tui", ["-p", "jcode-tui"]),
    ("suites.jcode-app-core", ["-p", "jcode-app-core"]),
]


def parse_test_result(stdout: str) -> tuple[int, int]:
    """Return (passed, failed) summed over every `test result:` line."""
    passed = failed = 0
    for line in stdout.splitlines():
        if not line.startswith("test result:"):
            continue
        # test result: ok. 1204 passed; 0 failed; 3 ignored; ...
        for token, label in ((" passed", "p"), (" failed", "f")):
            idx = line.find(token)
            if idx < 0:
                continue
            num = line[:idx].split()[-1]
            if not num.isdigit():
                continue
            if label == "p":
                passed += int(num)
            else:
                failed += int(num)
    return passed, failed


def phase_suites(profile: str) -> list[Check]:
    checks = []
    for name, pkg_args in SUITE_PACKAGES:
        t0 = time.time()
        r = run_cmd(
            ["./scripts/dev_cargo.sh", "test", *pkg_args, "--lib", "--profile", profile],
            timeout=3600,
        )
        passed, failed = parse_test_result(r.stdout)
        ok = r.returncode == 0 and failed == 0 and passed > 0
        checks.append(
            Check(
                name=name,
                ok=ok,
                # Pin the pass count: a suite that silently stops running half
                # its tests between runs is exactly the drift this gate exists
                # to catch, and returncode alone would not notice.
                fingerprint=f"passed={passed} failed={failed}",
                detail=f"exit={r.returncode} passed={passed} failed={failed}",
                duration_s=time.time() - t0,
            )
        )
    return checks


# --------------------------------------------------------------------------
# phase: package
# --------------------------------------------------------------------------


def nix_system() -> str:
    r = run_cmd(["nix", "eval", "--impure", "--raw", "--expr", "builtins.currentSystem"], timeout=300)
    if r.returncode != 0:
        raise GateError(f"cannot determine nix system: {r.stderr.strip()}")
    return r.stdout.strip()


def phase_package(out_link: Path) -> tuple[list[Check], Path | None]:
    system = nix_system()
    attr = f".#packages.{system}.jcode"
    t0 = time.time()
    r = run_cmd(
        [
            "nix",
            "build",
            attr,
            "--accept-flake-config",
            "--builders",
            "",
            "--max-jobs",
            "8",
            "--out-link",
            str(out_link),
        ],
        timeout=14400,
    )
    built = r.returncode == 0 and out_link.exists()
    store_path = out_link.resolve() if built else None
    checks = [
        Check(
            name="package.nix_build",
            ok=built,
            # The store path IS the determinism claim: same source -> same
            # derivation -> same output hash. Nothing weaker would prove it.
            fingerprint=store_path.name if store_path else "BUILD-FAILED",
            detail=(str(store_path) if store_path else r.stderr.strip()[-2000:]),
            duration_s=time.time() - t0,
        )
    ]
    return checks, store_path


# --------------------------------------------------------------------------
# phase: install (assertions about the store output)
# --------------------------------------------------------------------------

# Assets F19 requires the package to install, relative to the store output.
REQUIRED_INSTALLED = [
    "bin/jcode",
    "share/jcode/web/jcode-mobile",
]


def phase_install(store_path: Path | None) -> list[Check]:
    if store_path is None:
        return [
            Check(
                name="install.assets",
                ok=False,
                fingerprint="NO-PACKAGE",
                detail="package phase produced no store path",
            )
        ]
    checks = []

    present = []
    missing = []
    for rel in REQUIRED_INSTALLED:
        (present if (store_path / rel).exists() else missing).append(rel)
    checks.append(
        Check(
            name="install.assets",
            ok=not missing,
            fingerprint="present=" + ",".join(sorted(present)),
            detail=f"missing={missing}" if missing else "all required assets installed",
        )
    )

    # F19: the mobile asset root must actually contain the served entrypoint,
    # not just exist as an empty directory.
    index = store_path / "share/jcode/web/jcode-mobile/index.html"
    checks.append(
        Check(
            name="install.mobile_entrypoint",
            ok=index.is_file(),
            fingerprint=f"index.html={'yes' if index.is_file() else 'no'}",
            detail=str(index),
        )
    )

    # F18 launch gate: the installed binary must run.
    t0 = time.time()
    r = run_cmd([str(store_path / "bin/jcode"), "--version"], timeout=300)
    version = r.stdout.strip().splitlines()[0] if r.stdout.strip() else ""
    checks.append(
        Check(
            name="install.launches",
            ok=r.returncode == 0 and bool(version),
            fingerprint=version or "NO-VERSION",
            detail=f"exit={r.returncode} version={version!r} stderr={r.stderr.strip()[:300]}",
            duration_s=time.time() - t0,
        )
    )
    return checks


# --------------------------------------------------------------------------
# phase: updater matrix
# --------------------------------------------------------------------------


def phase_updater(store_path: Path | None) -> list[Check]:
    """Exercise the F20a/F20c updater matrix against the *installed* binary.

    The store-resident binary must self-declare externally managed and decline
    self-update, and must do so under an isolated JCODE_HOME so the result is a
    property of the binary rather than of this machine's state.
    """
    if store_path is None:
        return [
            Check(
                name="updater.declines_self_update",
                ok=False,
                fingerprint="NO-PACKAGE",
                detail="package phase produced no store path",
            )
        ]

    binary = store_path / "bin/jcode"
    checks = []

    with tempfile.TemporaryDirectory(prefix="f21-home-") as home:
        env = {
            "JCODE_HOME": home,
            # Non-interactive so the gate can never block on a prompt.
            "JCODE_NON_INTERACTIVE": "1",
        }
        # Deliberately NOT setting JCODE_NIX_MANAGED. That env var is an
        # explicit override, and setting it here would force the answer we are
        # trying to verify. F20a's actual claim is that a store-resident binary
        # self-declares managed purely from where it lives, so the only honest
        # test is to run it with the override absent.

        t0 = time.time()
        r = run_cmd([str(binary), "update"], env=env, timeout=600)
        combined = (r.stdout + r.stderr).lower()
        # F20a: a store-resident binary must refuse to self-update and must say
        # so via package-manager guidance, never by silently downloading.
        declined = any(
            marker in combined
            for marker in ("externally managed", "nix profile", "home-manager", "package manager")
        )
        downloaded = "downloading" in combined or "installing update" in combined
        checks.append(
            Check(
                name="updater.declines_self_update",
                ok=declined and not downloaded,
                fingerprint=f"declined={declined} downloaded={downloaded}",
                detail=f"exit={r.returncode} out={(r.stdout + r.stderr).strip()[:600]}",
                duration_s=time.time() - t0,
            )
        )

        # F20c: nothing may recreate the retired version store.
        builds = Path(home) / "builds"
        checks.append(
            Check(
                name="updater.no_retired_layout_written",
                ok=not builds.exists(),
                fingerprint=f"builds_dir={'present' if builds.exists() else 'absent'}",
                detail=str(builds),
            )
        )

        # The binary must report itself as nix-origin, which is what makes the
        # decline above principled rather than incidental.
        t0 = time.time()
        r = run_cmd([str(binary), "doctor", "--json"], env=env, timeout=600)
        origin = ""
        try:
            payload = json.loads(r.stdout)
            origin = str(
                payload.get("client", {}).get("origin")
                or payload.get("origin")
                or ""
            )
        except (json.JSONDecodeError, AttributeError):
            origin = "UNPARSEABLE"
        checks.append(
            Check(
                name="updater.doctor_origin",
                # `Origin` serializes lowercase; a store-resident binary must
                # classify as `nix`. Verified against the enum in
                # src/cli/commands/doctor.rs rather than assumed.
                ok=origin == "nix",
                fingerprint=f"origin={origin}",
                detail=f"exit={r.returncode} out={r.stdout.strip()[:400]}",
                duration_s=time.time() - t0,
            )
        )

    return checks


# --------------------------------------------------------------------------
# driver
# --------------------------------------------------------------------------


def do_run(index: int, args: argparse.Namespace, commit: str) -> RunResult:
    result = RunResult(index=index, commit=commit)

    if not args.skip_suites:
        result.checks.extend(phase_suites(args.profile))

    out_link = Path(tempfile.gettempdir()) / f"f21-result-{index}"
    if args.skip_package:
        store_path = None
    else:
        pkg_checks, store_path = phase_package(out_link)
        result.checks.extend(pkg_checks)
        result.checks.extend(phase_install(store_path))
        result.checks.extend(phase_updater(store_path))

    return result


def compare(runs: list[RunResult]) -> tuple[bool, list[str]]:
    """Every run must agree on every fingerprint."""
    if len(runs) < 2:
        return False, ["fewer than two runs recorded; determinism is unproven"]
    base = runs[0].fingerprints()
    diffs = []
    for other in runs[1:]:
        got = other.fingerprints()
        for name in sorted(set(base) | set(got)):
            a, b = base.get(name, "<absent>"), got.get(name, "<absent>")
            if a != b:
                diffs.append(f"run1.{name}={a!r} != run{other.index}.{name}={b!r}")
    return not diffs, diffs


def render_report(runs: list[RunResult], deterministic: bool, diffs: list[str]) -> str:
    lines = ["# F21 two-run integration gate manifest", ""]
    commits = {r.commit for r in runs}
    lines.append(f"- source identity: `{'` / `'.join(sorted(commits))}`")
    lines.append(f"- single commit across runs: {'yes' if len(commits) == 1 else 'NO'}")
    lines.append(f"- runs: {len(runs)}")
    lines.append(f"- all checks passed: {'yes' if all(r.ok for r in runs) else 'NO'}")
    lines.append(f"- runs agree: {'yes' if deterministic else 'NO'}")
    lines.append("")
    lines.append("| check | " + " | ".join(f"run {r.index}" for r in runs) + " | agree |")
    lines.append("|---|" + "---|" * (len(runs) + 1))
    names = []
    for r in runs:
        for c in r.checks:
            if c.name not in names:
                names.append(c.name)
    for name in names:
        cells = []
        fps = []
        for r in runs:
            c = next((c for c in r.checks if c.name == name), None)
            if c is None:
                cells.append("—")
                fps.append("<absent>")
            else:
                cells.append(f"{'PASS' if c.ok else 'FAIL'} `{c.fingerprint}`")
                fps.append(c.fingerprint)
        agree = "yes" if len(set(fps)) == 1 else "**NO**"
        lines.append(f"| `{name}` | " + " | ".join(cells) + f" | {agree} |")
    if diffs:
        lines += ["", "## Fingerprint divergence", ""]
        lines += [f"- {d}" for d in diffs]
    return "\n".join(lines) + "\n"


def self_test() -> int:
    """Prove the harness can distinguish agreement from divergence."""
    failures = []

    def check(label, cond):
        if not cond:
            failures.append(label)

    a = RunResult(1, "abc", [Check("x", True, "fp1"), Check("y", True, "fp2")])
    b = RunResult(2, "abc", [Check("x", True, "fp1"), Check("y", True, "fp2")])
    ok, diffs = compare([a, b])
    check("identical runs must agree", ok and not diffs)

    c = RunResult(2, "abc", [Check("x", True, "fp1"), Check("y", True, "DIFFERENT")])
    ok, diffs = compare([a, c])
    check("diverging fingerprints must be caught", not ok and len(diffs) == 1)

    d = RunResult(2, "abc", [Check("x", True, "fp1")])
    ok, diffs = compare([a, d])
    check("a missing check must be caught", not ok and diffs)

    ok, diffs = compare([a])
    check("a single run cannot prove determinism", not ok)

    # A run whose checks pass but whose fingerprints differ is still a failure:
    # that is the exact case a naive exit-code-only gate would wave through.
    e = RunResult(2, "abc", [Check("x", True, "OTHER"), Check("y", True, "fp2")])
    ok, _ = compare([a, e])
    check("passing-but-divergent must fail", not ok and a.ok and e.ok)

    p, f = parse_test_result("test result: ok. 1204 passed; 0 failed; 3 ignored;")
    check("parses a passing summary", (p, f) == (1204, 0))
    p, f = parse_test_result(
        "test result: ok. 10 passed; 0 failed;\ntest result: FAILED. 2 passed; 3 failed;"
    )
    check("sums multiple summaries", (p, f) == (12, 3))
    p, f = parse_test_result("no summary here")
    check("absent summary is not a pass", (p, f) == (0, 0))

    for label in failures:
        print(f"SELF-TEST FAIL: {label}")
    if failures:
        return 1
    print("self-test: all harness assertions passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--runs", type=int, default=2)
    ap.add_argument("--profile", default="selfdev")
    ap.add_argument("--skip-suites", action="store_true")
    ap.add_argument("--skip-package", action="store_true")
    ap.add_argument("--out", type=Path, default=None, help="write the manifest here")
    ap.add_argument("--json-out", type=Path, default=None)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument(
        "--allow-dirty",
        action="store_true",
        help="proceed with a dirty tree (the manifest records source identity either way)",
    )
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    if git_is_dirty() and not args.allow_dirty:
        print(
            "refusing to run: working tree is dirty, so the two runs would not "
            "provably share one source identity. Commit, or pass --allow-dirty.",
            file=sys.stderr,
        )
        return 2

    commit = git_commit()
    runs = []
    for i in range(1, args.runs + 1):
        print(f"=== F21 run {i}/{args.runs} @ {commit[:12]} ===", flush=True)
        run = do_run(i, args, commit)
        for c in run.checks:
            print(f"  [{'PASS' if c.ok else 'FAIL'}] {c.name}: {c.fingerprint} ({c.duration_s:.1f}s)", flush=True)
        runs.append(run)

    # A commit re-read after the runs guards against the tree moving mid-gate.
    final_commit = git_commit()
    if final_commit != commit:
        print(f"source identity changed mid-gate: {commit} -> {final_commit}", file=sys.stderr)
        return 2

    deterministic, diffs = compare(runs)
    report = render_report(runs, deterministic, diffs)
    print()
    print(report)

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(report)
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(
            json.dumps(
                {
                    "commit": commit,
                    "deterministic": deterministic,
                    "diffs": diffs,
                    "runs": [asdict(r) for r in runs],
                },
                indent=2,
            )
            + "\n"
        )

    all_ok = all(r.ok for r in runs) and deterministic
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())
