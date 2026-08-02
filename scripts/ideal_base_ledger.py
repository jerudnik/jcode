#!/usr/bin/env python3
"""Ideal-base node ledger: one rerunnable number per disposition.

Prints the open-node count that the tail program is driving to zero, plus the
supporting counts. Exists because "how many nodes are left" was being tracked
in conversation rather than read out of the files that decide it.

    python3 scripts/ideal_base_ledger.py            # summary
    python3 scripts/ideal_base_ledger.py --open     # list open node ids
"""

from __future__ import annotations

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
BASE = ROOT / "docs/fork/ideal-base"

# A node is off the board when it is published-complete or formally retired.
# `implemented` is deliberately NOT here: work sitting on an unmerged branch is
# still open, which is the distinction the railway gate enforces by refusing a
# reviewed_commit that is not an ancestor of main.
CLOSED = {"accepted", "authorization_blocked", "superseded", "rejected"}


def main() -> int:
    graph = json.loads((BASE / "WORK_GRAPH.json").read_text())
    state = json.loads((BASE / "STATE.json").read_text())["nodes"]

    ids = [n["id"] for n in graph["all_nodes"]]
    by_disposition: dict[str, list[str]] = {}
    for node_id in ids:
        disposition = state.get(node_id, {}).get("state", "missing")
        by_disposition.setdefault(disposition, []).append(node_id)

    open_ids = sorted(
        node_id for node_id in ids if state.get(node_id, {}).get("state") not in CLOSED
    )

    if "--open" in sys.argv:
        for node_id in open_ids:
            print(f"{node_id}\t{state.get(node_id, {}).get('state', 'missing')}")
        return 0

    print(f"nodes       {len(ids)}")
    for disposition in sorted(by_disposition):
        print(f"  {disposition:<22}{len(by_disposition[disposition])}")
    print(f"OPEN        {len(open_ids)}")
    print(f"CLOSED      {len(ids) - len(open_ids)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
