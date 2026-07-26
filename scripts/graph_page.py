#!/usr/bin/env python3
"""Render the ideal-base work graph as an interactive SVG dependency map.

The status page answers "how many?"; this answers "what depends on what, and
where is the frontier?". Nodes are laid out in workstream columns (W0..W5,
which is the program's own spine) and coloured by state, with dependency edges
drawn between them.

Layout is computed here rather than delegated to Graphviz so the output is a
dependency-free single file that opens anywhere. The graph is small (52 nodes,
6 columns) and its column assignment is given by the data, so a full layout
engine would buy nothing.

Usage:
    scripts/graph_page.py [-o OUT]
"""

from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import pathlib

REPO = pathlib.Path(__file__).resolve().parent.parent
IDEAL = REPO / "docs/fork/ideal-base"

# Geometry. Columns are workstreams; rows are nodes within a workstream.
COL_W = 196
ROW_H = 46
BOX_W = 150
BOX_H = 32
# Left pad must clear half a box, or the first column's labels clip off-canvas.
PAD_X = BOX_W // 2 + 26
PAD_Y = 128

STATE_STYLE = {
    "accepted": ("#1a3a24", "#3fb950", "#7ee787"),
    "ready": ("#132c4d", "#58a6ff", "#a5d6ff"),
    "blocked": ("#21262d", "#484f58", "#8b949e"),
}


def load() -> tuple[list[dict], dict[str, dict]]:
    graph = json.loads((IDEAL / "WORK_GRAPH.json").read_text())
    state = json.loads((IDEAL / "STATE.json").read_text())
    state = state.get("nodes", state)

    roots = graph.get("root_nodes", [])
    children = graph.get("all_nodes", [])
    accepted = {
        i for i, s in state.items() if isinstance(s, dict) and s.get("state") == "accepted"
    }

    nodes = []
    for n in roots + children:
        if not isinstance(n, dict) or "id" not in n:
            continue
        node_id = n["id"]
        deps = n.get("depends_on", []) or []
        unmet = [d for d in deps if d not in accepted]
        st = state.get(node_id, {}).get("state", "pending")
        if st == "accepted":
            bucket = "accepted"
        elif unmet:
            bucket = "blocked"
        else:
            bucket = "ready"
        nodes.append(
            {
                "id": node_id,
                "content": n.get("content", ""),
                "kind": n.get("kind", ""),
                "parent": n.get("parent"),
                "is_root": n in roots,
                "depends_on": deps,
                "unmet": unmet,
                "bucket": bucket,
                "summary": state.get(node_id, {}).get("summary", ""),
            }
        )
    return nodes, {n["id"]: n for n in nodes}


def layout(nodes: list[dict]) -> dict[str, tuple[float, float]]:
    """Assign each node a point. Column = workstream, row = order within it."""
    streams = ["W0", "W1", "W2", "W3", "W4", "W5"]
    pos: dict[str, tuple[float, float]] = {}

    for col, stream in enumerate(streams):
        x = PAD_X + col * COL_W
        # The workstream root sits above its children as a column header.
        pos[stream] = (x, PAD_Y - 62)
        kids = [n for n in nodes if n.get("parent") == stream]
        for row, kid in enumerate(kids):
            pos[kid["id"]] = (x, PAD_Y + row * ROW_H)
    return pos


def edge_path(x1: float, y1: float, x2: float, y2: float) -> str:
    """Cubic bezier between node edges, bowed horizontally between columns."""
    if abs(x1 - x2) < 1:  # same column: bow out to the left
        bow = 34
        return f"M{x1},{y1} C{x1 - bow},{y1 + 6} {x2 - bow},{y2 - 6} {x2},{y2}"
    mx = (x1 + x2) / 2
    return f"M{x1},{y1} C{mx},{y1} {mx},{y2} {x2},{y2}"


def render(nodes: list[dict], by_id: dict[str, dict]) -> str:
    pos = layout(nodes)
    counts = {b: sum(1 for n in nodes if n["bucket"] == b and not n["is_root"]) for b in STATE_STYLE}

    max_row = max(
        (len([n for n in nodes if n.get("parent") == w]) for w in ["W0", "W1", "W2", "W3", "W4", "W5"]),
        default=0,
    )
    width = PAD_X * 2 + 5 * COL_W + BOX_W // 2
    height = PAD_Y + max_row * ROW_H + 24

    # --- edges -------------------------------------------------------------
    edges = []
    for n in nodes:
        if n["is_root"]:
            continue
        tid = n["id"]
        if tid not in pos:
            continue
        tx, ty = pos[tid]
        for dep in n["depends_on"]:
            if dep not in pos or by_id.get(dep, {}).get("is_root"):
                continue
            sx, sy = pos[dep]
            # anchor on box edges
            x1, y1 = sx + BOX_W / 2, sy + BOX_H / 2
            x2, y2 = tx - BOX_W / 2, ty + BOX_H / 2
            if abs(sx - tx) < 1:
                x1, x2 = sx - BOX_W / 2, tx - BOX_W / 2
            met = by_id[dep]["bucket"] == "accepted"
            cls = "e-met" if met else "e-unmet"
            edges.append(
                f'<path class="edge {cls}" data-from="{dep}" data-to="{tid}" '
                f'd="{edge_path(x1, y1, x2, y2)}"/>'
            )

    # --- column headers ----------------------------------------------------
    heads = []
    stream_titles = {
        "W0": "bootstrap",
        "W1": "runtime ownership",
        "W2": "recovery + bounds",
        "W3": "validation + packaging",
        "W4": "security + quality",
        "W5": "external gates + signoff",
    }
    for w in ["W0", "W1", "W2", "W3", "W4", "W5"]:
        x, y = pos[w]
        kids = [n for n in nodes if n.get("parent") == w]
        done = sum(1 for k in kids if k["bucket"] == "accepted")
        frac = done / len(kids) if kids else 0
        colour = "#3fb950" if frac == 1 else ("#58a6ff" if frac else "#484f58")
        heads.append(
            f'<g class="head"><text x="{x}" y="{y}" class="w-id" fill="{colour}">{w}</text>'
            f'<text x="{x}" y="{y + 15}" class="w-t">{html.escape(stream_titles[w])}</text>'
            f'<text x="{x}" y="{y + 31}" class="w-c" fill="{colour}">{done}/{len(kids)}</text>'
            f'<rect x="{x - BOX_W / 2}" y="{y + 38}" width="{BOX_W}" height="4" rx="2" fill="#21262d"/>'
            f'<rect x="{x - BOX_W / 2}" y="{y + 38}" width="{BOX_W * frac:.1f}" height="4" rx="2" fill="{colour}"/>'
            "</g>"
        )

    # --- node boxes --------------------------------------------------------
    boxes = []
    for n in nodes:
        if n["is_root"] or n["id"] not in pos:
            continue
        x, y = pos[n["id"]]
        fill, stroke, text = STATE_STYLE[n["bucket"]]
        deps = ", ".join(n["depends_on"]) or "none"
        unmet = ", ".join(n["unmet"])
        tip = f"{n['id']} · {n['kind']}\n{n['content'][:150]}\n\ndepends on: {deps}"
        if unmet:
            tip += f"\nstill waiting on: {unmet}"
        boxes.append(
            f'<g class="node n-{n["bucket"]}" data-id="{n["id"]}" '
            f'data-deps="{html.escape(" ".join(n["depends_on"]))}">'
            f'<title>{html.escape(tip)}</title>'
            f'<rect x="{x - BOX_W / 2}" y="{y}" width="{BOX_W}" height="{BOX_H}" rx="6" '
            f'fill="{fill}" stroke="{stroke}" stroke-width="1.2"/>'
            f'<text x="{x - BOX_W / 2 + 10}" y="{y + 20}" class="n-id" fill="{text}">{n["id"]}</text>'
            f'<text x="{x + BOX_W / 2 - 10}" y="{y + 20}" class="n-k" fill="{stroke}" '
            f'text-anchor="end">{n["kind"][:9]}</text>'
            "</g>"
        )

    svg = (
        f'<svg id="g" viewBox="0 0 {width} {height}" width="{width}" height="{height}" '
        f'xmlns="http://www.w3.org/2000/svg">'
        f'<g id="edges">{"".join(edges)}</g>'
        f'<g id="heads">{"".join(heads)}</g>'
        f'<g id="nodes">{"".join(boxes)}</g></svg>'
    )

    frontier = [n for n in nodes if n["bucket"] == "ready" and not n["is_root"]]
    frontier_ids = ", ".join(sorted(n["id"] for n in frontier))
    gen = dt.datetime.now().strftime("%Y-%m-%d %H:%M")

    return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>jcode ideal-base — dependency graph</title><style>
:root{{--bg:#0d1117;--panel:#161b22;--line:#30363d;--fg:#e6edf3;--dim:#8b949e;--accent:#58a6ff}}
*{{box-sizing:border-box}}
body{{margin:0;background:var(--bg);color:var(--fg);
font:14px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,sans-serif}}
.wrap{{max-width:1400px;margin:0 auto;padding:28px 22px 60px}}
h1{{font-size:22px;margin:0 0 4px;letter-spacing:-.02em}}
.sub{{color:var(--dim);font-size:13px;margin-bottom:18px}}
.legend{{display:flex;gap:20px;flex-wrap:wrap;align-items:center;
background:var(--panel);border:1px solid var(--line);border-radius:8px;
padding:11px 16px;margin-bottom:16px;font-size:12.5px}}
.sw{{display:inline-block;width:11px;height:11px;border-radius:3px;
margin-right:7px;vertical-align:-1px}}
.hint{{color:var(--dim);margin-left:auto;font-size:12px}}
.scroll{{background:var(--panel);border:1px solid var(--line);border-radius:8px;
padding:8px;overflow-x:auto}}
svg{{display:block}}
.n-id{{font:600 12.5px ui-monospace,SFMono-Regular,Menlo,monospace}}
.n-k{{font:10px -apple-system,sans-serif;opacity:.72}}
.w-id{{font:600 13px ui-monospace,Menlo,monospace;text-anchor:middle}}
.w-t{{font:11px -apple-system,sans-serif;fill:#8b949e;text-anchor:middle}}
.w-c{{font:600 11px ui-monospace,Menlo,monospace;text-anchor:middle}}
.edge{{fill:none;stroke-width:1.3}}
.e-met{{stroke:#2ea04355}}
.e-unmet{{stroke:#8b949e40;stroke-dasharray:4 3}}
.node{{cursor:pointer}}
.node rect{{transition:filter .12s}}
.node:hover rect{{filter:brightness(1.45)}}
svg.sel .node{{opacity:.2}}
svg.sel .node.on{{opacity:1}}
svg.sel .edge{{opacity:.05}}
svg.sel .edge.on{{opacity:1;stroke-width:2.1;stroke:#58a6ff}}
.note{{color:var(--dim);font-size:12.5px;margin:14px 0 0;max-width:900px}}
code{{font-family:ui-monospace,Menlo,monospace;font-size:12px;color:#a5d6ff}}
</style></head><body><div class="wrap">

<h1>ideal-base dependency graph</h1>
<div class="sub">{len([n for n in nodes if not n["is_root"]])} nodes across 6 workstreams ·
generated {gen} from WORK_GRAPH.json + STATE.json</div>

<div class="legend">
  <span><span class="sw" style="background:#1a3a24;border:1px solid #3fb950"></span>
    accepted ({counts['accepted']})</span>
  <span><span class="sw" style="background:#132c4d;border:1px solid #58a6ff"></span>
    unblocked ({counts['ready']})</span>
  <span><span class="sw" style="background:#21262d;border:1px solid #484f58"></span>
    blocked ({counts['blocked']})</span>
  <span style="color:#8b949e">— solid edge = dependency met · dashed = still waiting</span>
  <span class="hint">click a node to isolate its dependencies · click empty space to reset</span>
</div>

<div class="scroll">{svg}</div>

<p class="note"><b>Reading it:</b> columns are the program's own workstreams, left to right.
W0–W2 are closed. W3 is one node from done (<code>F21</code>). The entire remaining frontier is
<code>{html.escape(frontier_ids)}</code>. Everything in W4/W5 that is grey is waiting on a named
predecessor, not on a decision — the dashed edges show exactly which.</p>

<script>
const svg = document.getElementById('g');
const nodes = [...svg.querySelectorAll('.node')];
const edges = [...svg.querySelectorAll('.edge')];
const deps = new Map(nodes.map(n => [n.dataset.id, (n.dataset.deps||'').split(' ').filter(Boolean)]));

// Walk the dependency closure in both directions so a click shows the whole
// chain a node sits in: everything it waits on, and everything waiting on it.
function closure(start, forward) {{
  const seen = new Set([start]);
  const stack = [start];
  while (stack.length) {{
    const cur = stack.pop();
    const next = forward
      ? (deps.get(cur) || [])
      : [...deps].filter(([, ds]) => ds.includes(cur)).map(([id]) => id);
    for (const nx of next) if (!seen.has(nx)) {{ seen.add(nx); stack.push(nx); }}
  }}
  return seen;
}}

function select(id) {{
  const set = new Set([...closure(id, true), ...closure(id, false)]);
  svg.classList.add('sel');
  nodes.forEach(n => n.classList.toggle('on', set.has(n.dataset.id)));
  edges.forEach(e => e.classList.toggle('on',
    set.has(e.dataset.from) && set.has(e.dataset.to)));
}}

nodes.forEach(n => n.addEventListener('click', ev => {{
  ev.stopPropagation();
  if (n.classList.contains('on') && svg.classList.contains('sel')) reset();
  else select(n.dataset.id);
}}));

function reset() {{
  svg.classList.remove('sel');
  nodes.forEach(n => n.classList.remove('on'));
  edges.forEach(e => e.classList.remove('on'));
}}
document.body.addEventListener('click', reset);
</script>
</div></body></html>"""


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("-o", "--out", default="/tmp/jcode-graph.html")
    args = ap.parse_args()
    nodes, by_id = load()
    out = pathlib.Path(args.out)
    out.write_text(render(nodes, by_id))
    print(f"wrote {out} ({out.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
