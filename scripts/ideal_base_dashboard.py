#!/usr/bin/env python3
"""Render a self-contained HTML dashboard of ideal-base program progress.

Every figure is derived from WORK_GRAPH.json and STATE.json at run time; nothing
is transcribed by hand. Evidence paths are stat'd on disk so the page can state
whether accepted work is actually backed by the artifacts it claims, rather than
trusting the state records. Regenerate with:

    python3 scripts/ideal_base_dashboard.py

Writes docs/fork/ideal-base/dashboard.html (git-ignored; a rendered view, not a
source of truth). Pass --check to verify integrity without writing.
"""
from __future__ import annotations

import argparse
import html
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
CONTROL = REPO_ROOT / "docs/fork/ideal-base"
GRAPH_PATH = CONTROL / "WORK_GRAPH.json"
STATE_PATH = CONTROL / "STATE.json"
OUT_PATH = CONTROL / "dashboard.html"

STATE_ORDER = ["accepted", "in_progress", "assigned", "pending", "blocked", "failed"]
STATE_COLOR = {
    "accepted": "#2ea043",
    "in_progress": "#d29922",
    "assigned": "#58a6ff",
    "pending": "#6e7681",
    "blocked": "#f85149",
    "failed": "#f85149",
}


def load() -> tuple[dict, dict]:
    return json.loads(GRAPH_PATH.read_text()), json.loads(STATE_PATH.read_text())


def git(*args: str) -> str:
    try:
        out = subprocess.run(
            ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True, timeout=15
        )
        return out.stdout.strip() if out.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


def collect(graph: dict, state: dict) -> dict:
    """Build the full model, including on-disk evidence verification."""
    nodes = state["nodes"]
    roots = graph["root_nodes"]
    expansions = graph["expansions"]

    evidence_present = 0
    evidence_missing: list[tuple[str, str]] = []
    for nid, rec in nodes.items():
        for path in rec.get("evidence", []):
            if (REPO_ROOT / path).exists():
                evidence_present += 1
            else:
                evidence_missing.append((nid, path))

    waves = []
    for root in roots:
        rid = root["id"]
        children = expansions.get(rid, [])
        child_rows = []
        for child in children:
            rec = nodes.get(child["id"], {})
            child_rows.append(
                {
                    "id": child["id"],
                    "content": child.get("content", ""),
                    "kind": child.get("kind", ""),
                    "state": rec.get("state", "unknown"),
                    "summary": rec.get("summary", ""),
                    "updated_at": rec.get("updated_at", ""),
                    "evidence": rec.get("evidence", []),
                    "gates": child.get("acceptance_gates", []),
                    "depends_on": child.get("depends_on", []),
                }
            )
        rec = nodes.get(rid, {})
        done = sum(1 for c in child_rows if c["state"] == "accepted")
        waves.append(
            {
                "id": rid,
                "content": root.get("content", ""),
                "kind": root.get("kind", ""),
                "state": rec.get("state", "unknown"),
                "summary": rec.get("summary", ""),
                "updated_at": rec.get("updated_at", ""),
                "depends_on": root.get("depends_on", []),
                "children": child_rows,
                "done": done,
                "total": len(child_rows),
            }
        )

    counts = {s: 0 for s in STATE_ORDER}
    for rec in nodes.values():
        counts[rec["state"]] = counts.get(rec["state"], 0) + 1

    return {
        "waves": waves,
        "counts": counts,
        "total_nodes": len(nodes),
        "evidence_present": evidence_present,
        "evidence_missing": evidence_missing,
        "program": graph.get("program", "ideal-base"),
        "program_state": state.get("program_state", ""),
        "graph_mode": graph.get("graph_mode", ""),
        "last_checkpoint": state.get("last_checkpoint", ""),
        "head": git("rev-parse", "--short", "HEAD"),
        "branch": git("rev-parse", "--abbrev-ref", "HEAD"),
        "generated": datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%SZ"),
    }


def esc(text: object) -> str:
    return html.escape(str(text), quote=True)


def render(m: dict) -> str:
    counts = m["counts"]
    total = m["total_nodes"]
    accepted = counts.get("accepted", 0)
    pct = (accepted / total * 100) if total else 0.0

    seg = "".join(
        f'<div class="seg" style="width:{counts[s]/total*100:.4f}%;background:{STATE_COLOR[s]}" '
        f'title="{s}: {counts[s]}"></div>'
        for s in STATE_ORDER
        if counts.get(s)
    )

    legend = "".join(
        f'<span class="lg"><i style="background:{STATE_COLOR[s]}"></i>{s.replace("_", " ")} '
        f'<b>{counts[s]}</b></span>'
        for s in STATE_ORDER
        if counts.get(s)
    )

    ev_ok = not m["evidence_missing"]
    ev_class = "ok" if ev_ok else "bad"
    ev_text = (
        f'{m["evidence_present"]} evidence paths verified on disk'
        if ev_ok
        else f'{len(m["evidence_missing"])} evidence paths MISSING'
    )

    waves_html = []
    for w in m["waves"]:
        wpct = (w["done"] / w["total"] * 100) if w["total"] else 0.0
        rows = []
        for c in w["children"]:
            ev = "".join(
                f'<code class="ev{"" if (REPO_ROOT / p).exists() else " missing"}">{esc(p.split("/")[-1])}</code>'
                for p in c["evidence"]
            )
            gates = "".join(f"<li><code>{esc(g)}</code></li>" for g in c["gates"])
            rows.append(
                f"""<tr class="s-{esc(c['state'])}">
<td class="nid">{esc(c['id'])}</td>
<td><span class="pill k-{esc(c['kind'])}">{esc(c['kind'])}</span></td>
<td class="ct">{esc(c['content'])}
{f'<div class="sum">{esc(c["summary"])}</div>' if c['summary'] else ''}
{f'<details class="gates"><summary>{len(c["gates"])} acceptance gates</summary><ul>{gates}</ul></details>' if gates else ''}
</td>
<td class="evc">{ev or '<span class="dim">-</span>'}</td>
<td><span class="badge" style="background:{STATE_COLOR.get(c['state'], '#6e7681')}">{esc(c['state'].replace('_', ' '))}</span></td>
</tr>"""
            )
        dep = (
            f'<span class="dep">after {esc(", ".join(w["depends_on"]))}</span>'
            if w["depends_on"]
            else ""
        )
        body = (
            f'<table><thead><tr><th>node</th><th>kind</th><th>work</th>'
            f'<th>evidence</th><th>state</th></tr></thead><tbody>{"".join(rows)}</tbody></table>'
            if rows
            else '<p class="dim pad">No child nodes expanded yet.</p>'
        )
        waves_html.append(
            f"""<section class="wave">
<header>
  <div class="wh">
    <h2>{esc(w['id'])} <span class="badge" style="background:{STATE_COLOR.get(w['state'], '#6e7681')}">{esc(w['state'].replace('_', ' '))}</span> {dep}</div>
    <div class="wcount">{w['done']}/{w['total']}</div>
  </div>
  <p class="wc">{esc(w['content'])}</p>
  {f'<p class="sum">{esc(w["summary"])}</p>' if w['summary'] else ''}
  <div class="bar sm"><div class="fill" style="width:{wpct:.2f}%"></div></div>
</header>
{body}
</section>"""
        )

    return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>ideal-base program map</title>
<style>
:root {{ color-scheme: dark; }}
* {{ box-sizing: border-box; }}
body {{ margin:0; background:#0d1117; color:#e6edf3;
  font:14px/1.55 -apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif; }}
.wrap {{ max-width:1180px; margin:0 auto; padding:32px 24px 72px; }}
h1 {{ font-size:24px; margin:0 0 4px; letter-spacing:-.01em; }}
.sub {{ color:#8b949e; margin:0 0 24px; font-size:13px; }}
.sub code {{ color:#a5d6ff; }}
.cards {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(150px,1fr)); gap:12px; margin-bottom:20px; }}
.card {{ background:#161b22; border:1px solid #30363d; border-radius:8px; padding:14px 16px; }}
.card .n {{ font-size:26px; font-weight:600; line-height:1.1; }}
.card .l {{ color:#8b949e; font-size:12px; margin-top:2px; }}
.bar {{ height:12px; background:#21262d; border-radius:6px; overflow:hidden; display:flex; margin:6px 0 10px; }}
.bar.sm {{ height:5px; margin:8px 0 0; }}
.bar .fill {{ background:#2ea043; height:100%; }}
.seg {{ height:100%; }}
.lg {{ color:#8b949e; font-size:12px; margin-right:14px; display:inline-flex; align-items:center; }}
.lg i {{ width:9px; height:9px; border-radius:2px; display:inline-block; margin-right:5px; }}
.lg b {{ color:#e6edf3; margin-left:4px; font-weight:600; }}
.note {{ border-radius:8px; padding:10px 14px; margin:16px 0 26px; font-size:13px; border:1px solid; }}
.note.ok {{ background:#0f2417; border-color:#238636; color:#7ee787; }}
.note.bad {{ background:#2d1417; border-color:#f85149; color:#ffa198; }}
.wave {{ background:#161b22; border:1px solid #30363d; border-radius:10px; margin-bottom:18px; overflow:hidden; }}
.wave header {{ padding:16px 18px 14px; border-bottom:1px solid #30363d; }}
.wh {{ display:flex; align-items:center; justify-content:space-between; gap:12px; }}
.wave h2 {{ font-size:16px; margin:0; display:flex; align-items:center; gap:9px; }}
.wcount {{ color:#8b949e; font-size:13px; font-variant-numeric:tabular-nums; }}
.wc {{ margin:8px 0 0; color:#c9d1d9; font-size:13px; }}
.sum {{ color:#8b949e; font-size:12.5px; margin:6px 0 0; font-style:italic; }}
.dep {{ color:#8b949e; font-size:11.5px; font-style:italic; font-weight:400; }}
table {{ width:100%; border-collapse:collapse; font-size:13px; }}
th {{ text-align:left; color:#8b949e; font-weight:500; font-size:11px; text-transform:uppercase;
  letter-spacing:.04em; padding:9px 14px; border-bottom:1px solid #30363d; }}
td {{ padding:10px 14px; border-bottom:1px solid #21262d; vertical-align:top; }}
tr:last-child td {{ border-bottom:none; }}
tr.s-accepted {{ background:rgba(46,160,67,.04); }}
tr.s-in_progress {{ background:rgba(210,153,34,.07); }}
.nid {{ font-family:ui-monospace,SFMono-Regular,Menlo,monospace; color:#a5d6ff; white-space:nowrap; font-size:12.5px; }}
.ct {{ max-width:520px; }}
.pill {{ font-size:11px; padding:2px 7px; border-radius:10px; background:#21262d; color:#8b949e; white-space:nowrap; }}
.k-implement {{ background:#1f2d3d; color:#79c0ff; }}
.k-verify {{ background:#2d2413; color:#e3b341; }}
.k-explore {{ background:#1c2b22; color:#7ee787; }}
.k-fix {{ background:#3d1f24; color:#ff9492; }}
.k-synthesize {{ background:#2b2139; color:#d2a8ff; }}
.badge {{ font-size:11px; padding:2px 8px; border-radius:10px; color:#0d1117; font-weight:600; white-space:nowrap; }}
.ev {{ display:inline-block; font-size:11px; background:#21262d; color:#7ee787; padding:1px 6px;
  border-radius:4px; margin:0 4px 4px 0; font-family:ui-monospace,Menlo,monospace; }}
.ev.missing {{ background:#3d1f24; color:#ff9492; text-decoration:line-through; }}
.evc {{ max-width:220px; }}
.dim {{ color:#6e7681; }}
.pad {{ padding:14px 18px; }}
.gates {{ margin-top:7px; }}
.gates summary {{ color:#8b949e; font-size:11.5px; cursor:pointer; }}
.gates ul {{ margin:6px 0 0; padding-left:18px; }}
.gates code {{ font-size:11px; color:#a5d6ff; }}
footer {{ color:#6e7681; font-size:12px; margin-top:28px; text-align:center; }}
</style></head><body><div class="wrap">

<h1>ideal-base program map</h1>
<p class="sub">
  program <code>{esc(m['program'])}</code> &middot; state <code>{esc(m['program_state'])}</code> &middot;
  mode <code>{esc(m['graph_mode'])}</code> &middot; branch <code>{esc(m['branch'])}</code> at
  <code>{esc(m['head'])}</code> &middot; generated {esc(m['generated'])}
</p>

<div class="cards">
  <div class="card"><div class="n">{pct:.0f}%</div><div class="l">nodes accepted</div></div>
  <div class="card"><div class="n">{accepted}<span class="dim" style="font-size:16px">/{total}</span></div><div class="l">accepted / total</div></div>
  <div class="card"><div class="n">{len(m['waves'])}</div><div class="l">root waves</div></div>
  <div class="card"><div class="n">{m['evidence_present']}</div><div class="l">evidence paths on disk</div></div>
</div>

<div class="bar">{seg}</div>
<div>{legend}</div>

<div class="note {ev_class}">
  <b>Integrity check:</b> {esc(ev_text)}.
  Every node, state, and gate on this page is read from
  <code>WORK_GRAPH.json</code> and <code>STATE.json</code> at generation time;
  evidence paths are stat'd against the working tree.
</div>

{''.join(waves_html)}

<footer>
  Generated by <code>scripts/ideal_base_dashboard.py</code> &middot;
  last checkpoint {esc(m['last_checkpoint'])} &middot; rendered view, not a source of truth
</footer>
</div></body></html>"""


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true",
                    help="verify integrity without writing the file")
    ap.add_argument("--out", type=Path, default=OUT_PATH)
    args = ap.parse_args(argv)

    graph, state = load()
    model = collect(graph, state)

    counted = sum(model["counts"].values())
    if counted != model["total_nodes"]:
        print(f"FAIL: state histogram {counted} != {model['total_nodes']} nodes",
              file=sys.stderr)
        return 1
    if model["evidence_missing"]:
        for nid, path in model["evidence_missing"]:
            print(f"FAIL: {nid} claims missing evidence {path}", file=sys.stderr)
        return 1

    accepted = model["counts"].get("accepted", 0)
    print(f"ideal-base dashboard: {accepted}/{model['total_nodes']} accepted, "
          f"{len(model['waves'])} waves, {model['evidence_present']} evidence paths verified")

    if args.check:
        return 0

    args.out.write_text(render(model))
    print(f"wrote {args.out.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
