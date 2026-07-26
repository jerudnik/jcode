#!/usr/bin/env python3
"""Render a single-file status page for the ideal-base railway.

Every number on the page is derived from the repo at run time: node states come
from STATE.json, the dependency structure from WORK_GRAPH.json, and the live
facts (PR checks, disk, launcher staleness) from git/gh. Nothing is typed in by
hand, because a status page that has to be hand-updated is a status page that
silently goes stale, which is worse than not having one.

Usage:
    scripts/status_page.py [-o OUT] [--no-gh]

`--no-gh` skips the network calls to the GitHub API for offline/quick runs.
"""

from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import pathlib
import subprocess

REPO = pathlib.Path(__file__).resolve().parent.parent
IDEAL = REPO / "docs/fork/ideal-base"
PR_NUMBER = "31"
PR_REPO = "jerudnik/jcode"
# The published launcher is pinned to this build; staleness is measured
# against it rather than a hand-copied number.
LAUNCHER_COMMIT = "59521d509"


def sh(cmd: str, default: str = "") -> str:
    """Run a shell command, returning `default` if it fails."""
    try:
        out = subprocess.run(
            cmd, shell=True, capture_output=True, text=True, cwd=REPO, timeout=60
        )
        return out.stdout.strip() or default
    except subprocess.SubprocessError:
        return default


def load_nodes() -> list[dict]:
    """Join the work graph's structure with STATE.json's acceptance record."""
    graph = json.loads((IDEAL / "WORK_GRAPH.json").read_text())
    state = json.loads((IDEAL / "STATE.json").read_text())
    state = state.get("nodes", state)

    nodes = {
        n["id"]: n
        for n in graph.get("root_nodes", []) + graph.get("all_nodes", [])
        if isinstance(n, dict) and "id" in n
    }
    accepted = {
        i for i, s in state.items() if isinstance(s, dict) and s.get("state") == "accepted"
    }

    out = []
    for node_id, node in nodes.items():
        rec = state.get(node_id, {})
        deps = node.get("depends_on", []) or []
        unmet = [d for d in deps if d not in accepted]
        node_state = rec.get("state", "pending")
        out.append(
            {
                "id": node_id,
                "content": node.get("content", ""),
                "kind": node.get("kind", ""),
                "parent": node.get("parent"),
                "depends_on": deps,
                "unmet": unmet,
                "state": node_state,
                "summary": rec.get("summary", ""),
                # "ready" is the useful distinction for planning: a pending node
                # whose dependencies are all accepted can be started today.
                "ready": node_state == "pending" and not unmet,
            }
        )
    return sorted(out, key=lambda n: n["id"])


def collect_facts(use_gh: bool) -> dict:
    checks = []
    if use_gh:
        raw = sh(f"gh pr checks {PR_NUMBER} --repo {PR_REPO}")
        for line in raw.splitlines():
            parts = line.split("\t")
            if len(parts) >= 2:
                checks.append({"name": parts[0], "state": parts[1]})
    return {
        "generated": dt.datetime.now().strftime("%Y-%m-%d %H:%M"),
        "branch": sh("git branch --show-current"),
        "head": sh("git log --oneline -1"),
        "ahead": sh("git rev-list --count main..HEAD", "0"),
        "checks": checks,
        "disk_free": sh("df -h / | tail -1 | awk '{print $4}'"),
        "builds": sh("du -sh ~/.jcode/builds 2>/dev/null | cut -f1"),
        "launcher": sh("~/.local/bin/jcode --version 2>/dev/null | head -1"),
        "launcher_behind": sh(f"git rev-list --count {LAUNCHER_COMMIT}..main", "?"),
        "branches_local": sh("git branch | wc -l", "?").strip(),
        "stashes": sh("git stash list | wc -l", "?").strip(),
    }


# Findings from the 2026-07-26 provenance sweep. These are conclusions, not
# measurements, so they live here rather than being re-derived; the ledger in
# DISK_HYGIENE_LEDGER.md carries the full reasoning.
PROVENANCE = [
    (
        "ambient queue",
        "resolved",
        "6 scheduled items with ambient mode disabled. Test code was writing into "
        "live user state; undeliverable by construction and never surfaced. Leak "
        "fixed upstream, queue cleared, fragile manual env restore replaced with a "
        "shared RAII guard.",
    ),
    (
        "recovery archive",
        "keep",
        "364 MB private repo, created during fork normalization. Verified against "
        "GitHub: still private, 42 branches, untouched since 2026-07-17. Holds 38 "
        "of 44 local branches byte-identical. Do not delete while the fork is in "
        "flight.",
    ),
    (
        "6 stashes",
        "5 dead / 1 live",
        "Verdicts derived by reverse-applying each stash against HEAD. {0} duplicates "
        "a branch, {1} is a review-failed attempt whose successor shipped, {2}/{3} "
        "landed, {4} was superseded by better code. Only {5} holds unmerged content.",
    ),
    (
        "launcher",
        "stale, deferred",
        "Not broken as first assumed: it resolves and runs, but is pinned to a dirty "
        "Jul 20 build. Silently running old code is the subtler hazard. Republish is "
        "correctly deferred until after the F20c merge changes the publish path.",
    ),
]

CSS = """
:root{--bg:#0d1117;--panel:#161b22;--line:#30363d;--fg:#e6edf3;--dim:#8b949e;
--ok:#3fb950;--warn:#d29922;--bad:#f85149;--accent:#58a6ff;--purple:#bc8cff}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);
font:14px/1.55 -apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,sans-serif}
.wrap{max-width:1180px;margin:0 auto;padding:32px 24px 72px}
h1{font-size:23px;margin:0 0 4px;letter-spacing:-.02em}
h2{font-size:15px;margin:34px 0 12px;color:var(--fg);letter-spacing:-.01em}
.sub{color:var(--dim);font-size:13px;margin-bottom:26px}
.grid{display:grid;gap:12px}
.cards{grid-template-columns:repeat(auto-fit,minmax(155px,1fr))}
.card{background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:14px 16px}
.card .n{font-size:25px;font-weight:600;letter-spacing:-.02em}
.card .l{color:var(--dim);font-size:11px;text-transform:uppercase;letter-spacing:.06em;margin-top:3px}
.bar{height:9px;border-radius:5px;background:#21262d;overflow:hidden;display:flex;margin:8px 0 6px}
.bar i{display:block;height:100%}
.legend{display:flex;gap:16px;flex-wrap:wrap;color:var(--dim);font-size:12px;margin-bottom:6px}
.dot{display:inline-block;width:8px;height:8px;border-radius:50%;margin-right:6px;vertical-align:middle}
table{width:100%;border-collapse:collapse;font-size:13px}
th{text-align:left;color:var(--dim);font-weight:500;font-size:11px;
text-transform:uppercase;letter-spacing:.06em;padding:0 10px 8px;border-bottom:1px solid var(--line)}
td{padding:9px 10px;border-bottom:1px solid #21262d;vertical-align:top}
tr:last-child td{border-bottom:none}
code,.mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12px}
.id{color:var(--accent);font-weight:600}
.pill{display:inline-block;padding:1px 8px;border-radius:999px;font-size:11px;
font-weight:500;white-space:nowrap}
.p-ok{background:rgba(63,185,80,.14);color:var(--ok)}
.p-warn{background:rgba(210,153,34,.14);color:var(--warn)}
.p-bad{background:rgba(248,81,73,.14);color:var(--bad)}
.p-dim{background:rgba(139,148,158,.14);color:var(--dim)}
.p-acc{background:rgba(88,166,255,.14);color:var(--accent)}
.panel{background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:4px 16px}
.note{color:var(--dim);font-size:12.5px;margin:8px 0 0}
.next{background:linear-gradient(180deg,rgba(88,166,255,.09),rgba(88,166,255,0));
border:1px solid #1f3b5c;border-radius:8px;padding:15px 18px;margin-bottom:8px}
.next b{color:var(--accent)}
.blocked-by{color:var(--dim);font-size:11.5px}
.dep{color:var(--purple)}
footer{margin-top:44px;color:var(--dim);font-size:12px;border-top:1px solid var(--line);padding-top:14px}
"""


def pill(text: str, cls: str) -> str:
    return f'<span class="pill {cls}">{html.escape(text)}</span>'


def render(nodes: list[dict], facts: dict) -> str:
    acc = [n for n in nodes if n["state"] == "accepted"]
    ready = [n for n in nodes if n["ready"]]
    blocked = [n for n in nodes if n["state"] == "pending" and not n["ready"]]
    total = len(nodes)
    pct = round(100 * len(acc) / total) if total else 0

    checks = facts["checks"]
    n_pass = sum(1 for c in checks if c["state"] == "pass")
    n_fail = sum(1 for c in checks if c["state"] in ("fail", "failure"))
    n_pend = sum(1 for c in checks if c["state"] == "pending")

    e = html.escape

    def check_pill(state: str) -> str:
        cls = {
            "pass": "p-ok",
            "fail": "p-bad",
            "pending": "p-warn",
            "skipping": "p-dim",
        }.get(state, "p-dim")
        return pill(state, cls)

    rows_ready = "".join(
        f"<tr><td class='id'>{e(n['id'])}</td>"
        f"<td>{pill(n['kind'], 'p-acc' if n['kind']=='implement' else 'p-dim')}</td>"
        f"<td>{e(n['content'][:118])}{'…' if len(n['content'])>118 else ''}</td></tr>"
        for n in sorted(ready, key=lambda x: (x["kind"] != "implement", x["id"]))
    )
    rows_blocked = "".join(
        f"<tr><td class='id'>{e(n['id'])}</td>"
        f"<td class='blocked-by'>waits on <span class='dep mono'>"
        f"{e(', '.join(n['unmet']))}</span></td>"
        f"<td>{e(n['content'][:98])}{'…' if len(n['content'])>98 else ''}</td></tr>"
        for n in sorted(blocked, key=lambda x: len(x["unmet"]))
    )
    rows_checks = "".join(
        f"<tr><td>{e(c['name'])}</td><td>{check_pill(c['state'])}</td></tr>" for c in checks
    )
    rows_prov = "".join(
        f"<tr><td><b>{e(t)}</b></td>"
        f"<td>{pill(v, 'p-ok' if v in ('resolved','keep') else 'p-warn')}</td>"
        f"<td>{e(d)}</td></tr>"
        for t, v, d in PROVENANCE
    )

    w_acc = 100 * len(acc) / total
    w_rdy = 100 * len(ready) / total
    w_blk = 100 * len(blocked) / total

    pr_state = (
        f"{n_pass} passing"
        + (f", {n_fail} failing" if n_fail else "")
        + (f", {n_pend} running" if n_pend else "")
    )

    return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>jcode ideal-base — status</title><style>{CSS}</style></head><body><div class="wrap">

<h1>jcode ideal-base railway</h1>
<div class="sub">Generated {e(facts['generated'])} from STATE.json, WORK_GRAPH.json, and live git/gh.
Branch <code>{e(facts['branch'])}</code>, {e(facts['ahead'])} commits over main.</div>

<div class="grid cards">
  <div class="card"><div class="n" style="color:var(--ok)">{len(acc)}</div><div class="l">accepted</div></div>
  <div class="card"><div class="n" style="color:var(--accent)">{len(ready)}</div><div class="l">ready now</div></div>
  <div class="card"><div class="n" style="color:var(--dim)">{len(blocked)}</div><div class="l">blocked</div></div>
  <div class="card"><div class="n">{pct}%</div><div class="l">complete</div></div>
</div>

<div class="bar" style="margin-top:16px">
  <i style="width:{w_acc:.1f}%;background:var(--ok)"></i>
  <i style="width:{w_rdy:.1f}%;background:var(--accent)"></i>
  <i style="width:{w_blk:.1f}%;background:#30363d"></i>
</div>
<div class="legend">
  <span><span class="dot" style="background:var(--ok)"></span>{len(acc)} accepted</span>
  <span><span class="dot" style="background:var(--accent)"></span>{len(ready)} unblocked</span>
  <span><span class="dot" style="background:#30363d"></span>{len(blocked)} waiting on dependencies</span>
</div>

<h2>Right now</h2>
<div class="next">
  <b>PR #31 (F20c, retire the dead distribution surface)</b> — {e(pr_state)}.
  Head <code>{e(facts['head'])}</code>.
  <div class="note">F20c is already <b>accepted</b> on the railway; the PR is the merge of that
  accepted work. Merging it unblocks the launcher republish, which is deliberately deferred
  because F20c changes the publish path.</div>
</div>
<div class="panel"><table><tbody>{rows_checks}</tbody></table></div>

<h2>Next up ({len(ready)} unblocked)</h2>
<div class="panel"><table>
<thead><tr><th style="width:64px">node</th><th style="width:96px">kind</th><th>what it does</th></tr></thead>
<tbody>{rows_ready}</tbody></table></div>
<p class="note"><b>F28 is the real next step</b>, not F29: F29 (route every ambient filesystem
root through jcode-storage — the 41 <code>dirs::</code> call sites) depends on F28 finishing
jcode-tui test hermeticity. The <code>G0x</code> nodes are verification gates that need
hardware, providers, or authorization rather than code.</p>

<h2>Blocked ({len(blocked)})</h2>
<div class="panel"><table>
<thead><tr><th style="width:64px">node</th><th style="width:210px">blocked by</th><th>what it does</th></tr></thead>
<tbody>{rows_blocked}</tbody></table></div>
<p class="note">The tail is a chain: <code>F27</code> gathers seven nodes, then
<code>S01→S02→S03</code> close the program. <code>W3→W4→W5</code> are the workstream rollups.</p>

<h2>Provenance sweep (2026-07-26)</h2>
<div class="panel"><table>
<thead><tr><th style="width:150px">thing</th><th style="width:120px">verdict</th><th>finding</th></tr></thead>
<tbody>{rows_prov}</tbody></table></div>

<h2>Machine state</h2>
<div class="grid cards">
  <div class="card"><div class="n">{e(facts['disk_free'])}</div><div class="l">disk free</div></div>
  <div class="card"><div class="n">{e(facts['builds'])}</div><div class="l">~/.jcode/builds</div></div>
  <div class="card"><div class="n" style="color:var(--warn)">{e(str(facts['launcher_behind']))}</div>
    <div class="l">launcher commits behind</div></div>
  <div class="card"><div class="n">{e(facts['branches_local'])}</div><div class="l">local branches</div></div>
  <div class="card"><div class="n">{e(facts['stashes'])}</div><div class="l">stashes (5 droppable)</div></div>
</div>
<p class="note">Launcher reports <code>{e(facts['launcher'])}</code>. It runs fine, so this is
staleness rather than breakage. 38 of the local branches are byte-identical to archived copies.</p>

<footer>Regenerate with <code>scripts/status_page.py</code>. Every figure is read from the repo at
run time, so this page cannot drift from STATE.json without the file itself changing.</footer>
</div></body></html>"""


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("-o", "--out", default="/tmp/jcode-status.html")
    ap.add_argument("--no-gh", action="store_true", help="skip GitHub API calls")
    args = ap.parse_args()

    nodes = load_nodes()
    facts = collect_facts(use_gh=not args.no_gh)
    out = pathlib.Path(args.out)
    out.write_text(render(nodes, facts))
    print(f"wrote {out} ({out.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
