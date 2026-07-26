#!/usr/bin/env python3
"""Find jcode-tui tests that touch process-global render state without the lock.

The TUI test crate shares layout snapshots, Mermaid diagram registries, frame
histories, and side-panel caches across the whole process. A test that mutates
any of them without holding `lock_test_render_state()` is only safe while the
suite runs single-threaded, which is why the fork-ci rails currently pass
`--test-threads=1`. This scan is the measurable gate for lifting that cap: it
must reach zero before parallelism can be restored.

Detecting the lock textually is not enough, because most tests acquire it
indirectly through a helper (`with_serialized_mermaid_state`, a `_lock()`
fixture, and so on). So helper functions are resolved transitively: any
function whose body acquires the lock, or calls something that does, counts as
locking, and a test calling it is considered covered.

Usage:
    check_tui_render_lock.py            # report, exit 1 if any are unlocked
    check_tui_render_lock.py --list     # one `file::test` per line
    check_tui_render_lock.py --baseline N   # fail only if count exceeds N
"""

from __future__ import annotations

import argparse
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "crates/jcode-tui/src"

LOCK_FN = "lock_test_render_state"

# Calls that reach state which is genuinely shared across test threads.
#
# Membership here was determined by reading each backing store, not by guessing
# from the name, because the two categories look identical at the call site:
#
#   shared      ACTIVE_DIAGRAMS, STREAMING_PREVIEW_DIAGRAM, IMAGE_STATE live in
#               the jcode-tui-mermaid crate, so jcode-tui's `cfg(test)` does not
#               apply to them and they stay `LazyLock<Mutex<..>>` during tests.
#               WIDGETS_STATE and SLOW_FRAME_HISTORY are plain statics with no
#               test-only variant.
#
#   not shared  The side-panel markdown/render/debug caches are swapped to
#               `thread_local!` under `#[cfg(test)]` (ui_pinned.rs), so tests
#               touching only those cannot interfere across threads and are
#               deliberately excluded. Flagging them would be a false positive
#               that trains people to ignore this scan.
MUTATORS = re.compile(
    r"\b("
    r"render_frame|render_app|draw_frame|"
    r"register_active_diagram|clear_active_diagrams|restore_active_diagrams|"
    r"set_streaming_preview_diagram|clear_streaming_preview_diagram|"
    r"set_video_export_mode|clear_image_state|register_inline_image|"
    r"record_slow_frame|record_flicker_frame|"
    r"set_widget_placement|clear_widget_placements_for_tests|"
    r"clear_test_render_state_for_tests"
    r")\b"
)

FN_DEF = re.compile(
    r"(?:#\[(?P<attr>[^\]]*)\]\s*)*"
    r"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(?P<name>\w+)"
)
TEST_ATTR = re.compile(r"#\[(?:tokio::)?test\b[^\]]*\]")


def split_functions(src: str) -> list[tuple[str, str, bool]]:
    """Return (name, body, is_test) for each function, via brace matching."""
    out = []
    for m in re.finditer(r"\bfn\s+(\w+)", src):
        name = m.group(1)
        brace = src.find("{", m.end())
        if brace == -1:
            continue
        depth, i = 0, brace
        while i < len(src):
            if src[i] == "{":
                depth += 1
            elif src[i] == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        body = src[brace:i]
        # A test is marked by an attribute in the ~200 chars preceding it.
        head = src[max(0, m.start() - 200):m.start()]
        out.append((name, body, bool(TEST_ATTR.search(head))))
    return out


def build_index() -> tuple[dict, dict, list]:
    """Index every function in the crate by name."""
    bodies: dict[str, list[str]] = defaultdict(list)
    tests = []
    for path in sorted(ROOT.rglob("*.rs")):
        src = path.read_text(encoding="utf-8", errors="replace")
        for name, body, is_test in split_functions(src):
            bodies[name].append(body)
            if is_test:
                tests.append((path.relative_to(ROOT), name, body))
    return bodies, {}, tests


CALL = re.compile(r"\b(\w+)\s*\(")


def calls_in(body: str) -> set[str]:
    """Identifiers invoked as functions within a body."""
    return {m.group(1) for m in CALL.finditer(body)}


def locking_functions(bodies: dict[str, list[str]]) -> set[str]:
    """Names of functions that acquire the render lock, directly or transitively.

    Computed as a reverse-reachability closure over the call graph rather than a
    repeated rescan: the crate has thousands of functions, and re-matching every
    body against every known locking name is quadratic enough to look hung.
    """
    callers: dict[str, set[str]] = defaultdict(set)
    direct = {LOCK_FN}
    for name, bs in bodies.items():
        for body in bs:
            if LOCK_FN in body:
                direct.add(name)
            for callee in calls_in(body):
                callers[callee].add(name)

    locking, stack = set(direct), list(direct)
    while stack:  # propagate "locks" up to every caller
        for caller in callers.get(stack.pop(), ()):
            if caller not in locking:
                locking.add(caller)
                stack.append(caller)
    return locking


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--list", action="store_true", help="machine-readable list")
    ap.add_argument("--baseline", type=int, default=0,
                    help="max tolerated unlocked tests")
    args = ap.parse_args()

    bodies, _, tests = build_index()
    locking = locking_functions(bodies)

    unlocked = defaultdict(list)
    locked = 0
    for path, name, body in tests:
        if not MUTATORS.search(body):
            continue
        if LOCK_FN in body or (calls_in(body) & locking):
            locked += 1
        else:
            unlocked[str(path)].append(name)

    total = sum(len(v) for v in unlocked.values())

    if args.list:
        for f, names in sorted(unlocked.items()):
            for n in sorted(names):
                print(f"{f}::{n}")
        return 1 if total > args.baseline else 0

    print(f"render-state tests holding the lock: {locked}")
    print(f"render-state tests WITHOUT the lock: {total}")
    if unlocked:
        print()
        for f, names in sorted(unlocked.items(), key=lambda kv: -len(kv[1])):
            print(f"  {len(names):>3}  {f}")
            for n in sorted(names):
                print(f"         {n}")

    if total > args.baseline:
        print(f"\nFAIL: {total} unlocked (baseline {args.baseline}).")
        return 1
    print(f"\nOK: within baseline ({args.baseline}).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
