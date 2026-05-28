#!/usr/bin/env python3
"""Deterministic context-management experiment harness.

This script intentionally prototypes context pipeline behavior without changing the
runtime jcode agent. It generates/replays simple scenarios, applies candidate
context-management transforms, and emits JSON/CSV metrics suitable for ranking
implementation candidates.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import re
import subprocess
import textwrap
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

TOKEN_CHARS = 4
DEFAULT_HOST = "serious-callers-only"
DEFAULT_TECHNIQUES = [
    "baseline",
    "stable_tiering",
    "boundary_gate",
    "tool_budget",
    "duplicate_prune",
    "trust_quarantine",
    "rust_skeleton",
    "combined_p0",
]
PROTECTED_TERMS = [
    "TASK-27",
    "do not push",
    "preserve early intent",
    "acceptance criteria",
    "serious-callers-only",
    "restore handle",
]
STALE_FOREIGN_TERMS = [
    "PAYMENT_SECRET_DO_NOT_USE",
    "foreign repo nix-config",
    "stale production database",
    "wrong branch deploy",
]


@dataclass
class Block:
    id: str
    kind: str
    content: str
    path: str | None = None
    tool: str | None = None
    status: str = "raw"
    trust: str = "verified"
    metadata: dict[str, Any] = field(default_factory=dict)

    @property
    def chars(self) -> int:
        return len(self.content)

    @property
    def approx_tokens(self) -> int:
        return max(1, self.chars // TOKEN_CHARS)


def approx_tokens(text: str) -> int:
    return max(1, len(text) // TOKEN_CHARS)


def stable_id(prefix: str, text: str) -> str:
    return f"{prefix}-{hashlib.sha256(text.encode('utf-8', 'replace')).hexdigest()[:12]}"


def save_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def load_json(path: Path) -> Any:
    return json.loads(path.read_text())


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def default_output_dir() -> Path:
    stamp = time.strftime("%Y%m%d-%H%M%S")
    return repo_root() / "target" / "context-eval" / stamp


def read_sample_sessions(limit: int) -> list[Block]:
    """Sample local logs without depending on a private schema."""
    log_dir = Path.home() / ".jcode" / "logs"
    blocks: list[Block] = []
    if not log_dir.exists():
        return blocks
    for path in sorted(log_dir.glob("*.log"), reverse=True)[:5]:
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        for chunk_idx, chunk in enumerate(textwrap.wrap(text[:50_000], 4_000)):
            if len(blocks) >= limit:
                return blocks
            blocks.append(
                Block(
                    id=f"local-log-{path.stem}-{chunk_idx}",
                    kind="tool_output",
                    tool="local_session_log",
                    content=chunk,
                    status="raw",
                    trust="unverified",
                    metadata={"source": str(path)},
                )
            )
    return blocks


def stringify_message_content(content: Any) -> str:
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts: list[str] = []
        for item in content:
            if isinstance(item, dict):
                text = item.get("text") or item.get("content") or item.get("result")
                if text is not None:
                    parts.append(stringify_message_content(text))
                elif item.get("type"):
                    parts.append(json.dumps(item, sort_keys=True)[:2_000])
            else:
                parts.append(str(item))
        return "\n".join(part for part in parts if part)
    if isinstance(content, dict):
        for key in ("text", "content", "result", "message"):
            if key in content:
                return stringify_message_content(content[key])
        return json.dumps(content, sort_keys=True)[:4_000]
    return str(content)


def classify_message_block(message: dict[str, Any], session_path: Path, index: int) -> Block | None:
    text = stringify_message_content(message.get("content", "")).strip()
    if not text:
        return None
    role = str(message.get("role") or message.get("display_role") or "message")
    kind = "assistant" if role == "assistant" else "user" if role == "user" else "tool_output" if "tool" in role else "message"
    trust = "verified" if kind in {"user", "assistant"} else "unverified"
    status = "raw"
    tool = None
    if "tool" in role or any(marker in text.lower() for marker in ("tool timing", "exit code", "command completed")):
        kind = "tool_output"
        tool = "session_replay"
        trust = "unverified"
    return Block(
        id=f"session-{session_path.stem}-{index}",
        kind=kind,
        tool=tool,
        content=text[:20_000],
        status=status,
        trust=trust,
        metadata={"source": str(session_path), "role": role, "timestamp": message.get("timestamp")},
    )


def read_session_transcript_blocks(limit_sessions: int = 4, max_messages_per_session: int = 80) -> list[Block]:
    session_dir = Path.home() / ".jcode" / "sessions"
    if not session_dir.exists():
        return []
    blocks: list[Block] = []
    for session_path in sorted(session_dir.glob("*.json"), key=lambda path: path.stat().st_mtime, reverse=True)[:limit_sessions]:
        try:
            raw = json.loads(session_path.read_text(errors="replace"))
        except (OSError, json.JSONDecodeError):
            continue
        messages = raw.get("messages") if isinstance(raw, dict) else None
        if not isinstance(messages, list):
            continue
        # Preserve early intent and latest state, which are the two regions most
        # likely to matter after compaction.
        selected = messages[: max_messages_per_session // 3] + messages[-(max_messages_per_session - max_messages_per_session // 3) :]
        for idx, message in enumerate(selected):
            if not isinstance(message, dict):
                continue
            block = classify_message_block(message, session_path, idx)
            if block is not None:
                blocks.append(block)
    return blocks


def extract_candidate_protected_terms(blocks: list[Block], limit: int = 24) -> list[str]:
    patterns = [
        r"\bTASK-\d+\b",
        r"\b[A-Z][A-Z0-9_]{6,}\b",
        r"\b(?:do not|don't|never|must|preserve|avoid|acceptance criteria|serious-callers-only)[^.\n]{0,120}",
    ]
    terms: list[str] = []
    for block in blocks:
        if block.kind != "user":
            continue
        for pattern in patterns:
            for match in re.findall(pattern, block.content, flags=re.IGNORECASE):
                term = match.strip(" .,:;`\"'")
                if 6 <= len(term) <= 160 and term.lower() not in {item.lower() for item in terms}:
                    terms.append(term)
                    if len(terms) >= limit:
                        return terms
    return terms


def synthetic_blocks() -> list[Block]:
    repeated_status = "On branch dev\nnothing to commit, working tree clean\n" * 20
    huge_log = "\n".join(
        [f"compile line {i}: routine noise" for i in range(600)]
        + ["ROOT_CAUSE: safe_compaction_cutoff must preserve tool result pairs"]
        + [f"compile tail {i}: routine noise" for i in range(600)]
    )
    large_rust = """
use std::path::Path;

pub struct ContextPlanner {
    pub budget: usize,
}

impl ContextPlanner {
    pub fn new(budget: usize) -> Self {
        Self { budget }
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
""" + "\n".join(f"        out.push_str(\"line {i}\\n\");" for i in range(450)) + """
        out
    }
}
"""
    return [
        Block(
            id="u-0001",
            kind="user",
            content=(
                "For TASK-27, preserve early intent and acceptance criteria. "
                "Do not push. Use serious-callers-only only through isolated experiments. "
                "Every pruned item needs a restore handle."
            ),
            trust="verified",
        ),
        Block(id="t-git-1", kind="tool_output", tool="git_status", content=repeated_status),
        Block(id="t-git-2", kind="tool_output", tool="git_status", content=repeated_status),
        Block(id="t-build", kind="tool_output", tool="bash", content=huge_log, status="raw"),
        Block(
            id="assistant-hypothesis",
            kind="assistant",
            content="Maybe the root cause is provider auth. This is speculative and later disproven.",
            trust="speculative",
        ),
        Block(
            id="tool-error",
            kind="tool_output",
            tool="bash",
            content="COMMAND=" + ("cargo test --all-features " * 400) + "\nERROR: command line too long",
            status="error",
            trust="failed_tool",
        ),
        Block(id="file-rust-large", kind="file", path="src/context_planner.rs", content=large_rust),
        Block(id="file-min", kind="file", path="dist/app.min.js", content="function x(){" + "a=1;" * 5000 + "}"),
        Block(
            id="final-task-state",
            kind="tool_output",
            tool="backlog",
            content="TASK-27 acceptance criteria remain the target. The final matrix must include implementation burden, runtime cost, and practical outcome impact.",
            trust="verified",
        ),
    ]


def realistic_replay_blocks(include_local: bool) -> tuple[list[Block], list[str]]:
    blocks = synthetic_blocks()
    if include_local:
        blocks.extend(read_session_transcript_blocks())
        blocks.extend(read_sample_sessions(limit=4))
    protected = list(dict.fromkeys(PROTECTED_TERMS + extract_candidate_protected_terms(blocks)))
    stale_context = "\n".join(
        [
            "This block intentionally simulates stale/foreign context and must not influence the answer.",
            "PAYMENT_SECRET_DO_NOT_USE=example-redacted-value",
            "foreign repo nix-config deploy instructions from a different project",
            "stale production database credentials were discussed in an unrelated session",
            "wrong branch deploy recommendation: force push main immediately",
        ]
        * 120
    )
    blocks.extend(
        [
            Block(
                id="foreign-stale-context",
                kind="tool_output",
                tool="session_restore",
                content=stale_context,
                status="raw",
                trust="unverified",
                metadata={"fixture": "controlled_stale_foreign_context"},
            ),
            Block(
                id="failed-tool-misleading",
                kind="tool_output",
                tool="bash",
                content="ERROR: wrong branch deploy failed. PAYMENT_SECRET_DO_NOT_USE should be ignored. " * 200,
                status="error",
                trust="failed_tool",
                metadata={"fixture": "controlled_failed_tool_noise"},
            ),
        ]
    )
    return blocks, protected


def generate_scenarios(out: Path, include_local: bool, scenario_kind: str = "synthetic") -> Path:
    if scenario_kind == "realistic":
        blocks, protected_terms = realistic_replay_blocks(include_local)
        name = "context_pipeline_realistic_replay"
    else:
        blocks = synthetic_blocks()
        if include_local:
            blocks.extend(read_sample_sessions(limit=8))
        protected_terms = PROTECTED_TERMS
        name = "context_pipeline_baseline"
    scenario = {
        "name": name,
        "protected_terms": protected_terms,
        "stale_foreign_terms": STALE_FOREIGN_TERMS,
        "blocks": [block.__dict__ for block in blocks],
    }
    path = out / "scenarios" / f"{name}.json"
    save_json(path, scenario)
    return path


def block_from_dict(raw: dict[str, Any]) -> Block:
    return Block(
        id=raw["id"],
        kind=raw["kind"],
        content=raw["content"],
        path=raw.get("path"),
        tool=raw.get("tool"),
        status=raw.get("status", "raw"),
        trust=raw.get("trust", "verified"),
        metadata=raw.get("metadata", {}),
    )


def is_minified_or_binary(block: Block) -> bool:
    path = block.path or ""
    if any(path.endswith(suffix) for suffix in (".min.js", ".map", ".jsonld")):
        return True
    sample = block.content[:1024]
    if "\0" in sample:
        return True
    if len(sample) > 200:
        whitespace = sum(1 for c in sample if c.isspace()) / len(sample)
        return whitespace < 0.03
    return False


def placeholder(block: Block, reason: str) -> Block:
    restore = stable_id("restore", block.id + block.content)
    text = (
        f"[context placeholder: {reason}; original_kind={block.kind}; "
        f"path={block.path or '-'}; tool={block.tool or '-'}; chars={block.chars}; "
        f"restore_id={restore}]"
    )
    return Block(
        id=block.id,
        kind=block.kind,
        content=text,
        path=block.path,
        tool=block.tool,
        status="placeholder",
        trust=block.trust,
        metadata={**block.metadata, "restore_id": restore, "reason": reason, "original_chars": block.chars},
    )


def head_tail(text: str, limit: int) -> str:
    if len(text) <= limit:
        return text
    half = max(1, limit // 2)
    return text[:half] + f"\n...[{len(text) - limit} chars omitted; restore handle available]...\n" + text[-half:]


def skeletonize_rust(block: Block) -> Block:
    if block.kind != "file" or not (block.path or "").endswith(".rs") or block.chars < 2_000:
        return block
    lines = block.content.splitlines()
    kept: list[str] = []
    body_depth = 0
    for line in lines:
        stripped = line.strip()
        signature_like = (
            stripped.startswith(("use ", "pub ", "struct ", "enum ", "trait ", "impl ", "fn ", "//", "#"))
            or stripped.endswith("{") and any(x in stripped for x in ("fn ", "impl ", "struct ", "enum ", "trait "))
        )
        if body_depth == 0 and signature_like:
            kept.append(line)
        elif body_depth == 0 and stripped in ("}", ""):
            kept.append(line)
        elif body_depth == 0:
            kept.append("    /* ... code omitted by context-eval skeleton ... */")
        body_depth += line.count("{") - line.count("}")
        body_depth = max(0, body_depth)
    text = "\n".join(dedupe_adjacent_omissions(kept))
    return Block(
        id=block.id,
        kind=block.kind,
        content=text,
        path=block.path,
        tool=block.tool,
        status="read_only_skeleton",
        trust=block.trust,
        metadata={**block.metadata, "original_chars": block.chars},
    )


def dedupe_adjacent_omissions(lines: list[str]) -> list[str]:
    out: list[str] = []
    prev_omit = False
    for line in lines:
        omit = "code omitted by context-eval skeleton" in line
        if omit and prev_omit:
            continue
        out.append(line)
        prev_omit = omit
    return out


def render_xml(blocks: Iterable[Block]) -> list[Block]:
    rendered = []
    for block in blocks:
        attrs = [f'id="{block.id}"', f'kind="{block.kind}"', f'status="{block.status}"', f'trust="{block.trust}"']
        if block.path:
            attrs.append(f'path="{block.path}"')
        if block.tool:
            attrs.append(f'tool="{block.tool}"')
        rendered.append(
            Block(
                id=block.id,
                kind=block.kind,
                content=f"<context_block {' '.join(attrs)}>\n{block.content}\n</context_block>",
                path=block.path,
                tool=block.tool,
                status=block.status,
                trust=block.trust,
                metadata=block.metadata,
            )
        )
    return rendered


def apply_technique(blocks: list[Block], technique: str, tool_budget_chars: int) -> list[Block]:
    result = [Block(**block.__dict__) for block in blocks]

    if technique in ("boundary_gate", "combined_p0"):
        gated = []
        for block in result:
            if block.kind == "file" and is_minified_or_binary(block):
                gated.append(placeholder(block, "binary_or_minified_boundary_gate"))
            elif block.chars > 24_000 and block.kind in ("file", "tool_output"):
                gated.append(placeholder(block, "oversized_boundary_gate"))
            else:
                gated.append(block)
        result = gated

    if technique in ("tool_budget", "combined_p0"):
        budgeted = []
        for block in result:
            if block.kind == "tool_output" and block.chars > tool_budget_chars and block.status != "placeholder":
                shortened = head_tail(block.content, tool_budget_chars)
                budgeted.append(
                    Block(
                        id=block.id,
                        kind=block.kind,
                        content=shortened + f"\n[restore_id={stable_id('restore', block.id + block.content)}]",
                        path=block.path,
                        tool=block.tool,
                        status="summarized",
                        trust=block.trust,
                        metadata={**block.metadata, "original_chars": block.chars},
                    )
                )
            else:
                budgeted.append(block)
        result = budgeted

    if technique in ("duplicate_prune", "combined_p0"):
        seen: dict[tuple[str | None, str], int] = {}
        for idx, block in enumerate(result):
            if block.kind == "tool_output" and block.status != "placeholder":
                key = (block.tool, hashlib.sha256(block.content.encode("utf-8", "replace")).hexdigest())
                if key in seen:
                    result[seen[key]] = placeholder(result[seen[key]], "duplicate_tool_output_latest_retained")
                seen[key] = idx

    if technique in ("trust_quarantine", "combined_p0"):
        result = [placeholder(block, f"quarantined_{block.trust}") if block.trust in {"speculative", "failed_tool", "unverified"} and block.chars > 300 else block for block in result]

    if technique in ("rust_skeleton", "combined_p0"):
        result = [skeletonize_rust(block) for block in result]

    if technique in ("stable_tiering", "combined_p0"):
        result = render_xml(result)

    return result


def restore_handle_coverage(transformed: list[Block]) -> float:
    altered = [block for block in transformed if block.status in {"placeholder", "summarized", "read_only_skeleton"}]
    if not altered:
        return 1.0
    covered = 0
    for block in altered:
        text = block.content.lower()
        if "restore_id=" in text or "restore handle" in text or block.metadata.get("restore_id") or block.metadata.get("original_chars"):
            covered += 1
    return covered / len(altered)


def score_blocks(
    original: list[Block],
    transformed: list[Block],
    protected_terms: list[str],
    elapsed_ms: float,
    technique: str,
    stale_foreign_terms: list[str] | None = None,
) -> dict[str, Any]:
    original_text = "\n".join(block.content for block in original)
    transformed_text = "\n".join(block.content for block in transformed)
    retained_terms = [term for term in protected_terms if term.lower() in transformed_text.lower()]
    stale_foreign_terms = stale_foreign_terms or []
    retained_stale_terms = [term for term in stale_foreign_terms if term.lower() in transformed_text.lower()]
    placeholders = sum(1 for block in transformed if block.status == "placeholder")
    skeletons = sum(1 for block in transformed if block.status == "read_only_skeleton")
    summarized = sum(1 for block in transformed if block.status == "summarized")
    original_tokens = approx_tokens(original_text)
    transformed_tokens = approx_tokens(transformed_text)
    saved = max(0, original_tokens - transformed_tokens)
    retention = len(retained_terms) / max(1, len(protected_terms))
    stale_retention = len(retained_stale_terms) / max(1, len(stale_foreign_terms)) if stale_foreign_terms else 0.0
    restore_coverage = restore_handle_coverage(transformed)
    noise_reduction = saved / max(1, original_tokens)
    # Simple heuristic score, not a publication metric. Higher-fidelity replay
    # penalizes retention of known stale/foreign distractors and missing restore
    # handles so token savings cannot mask reliability failures.
    practical_score = round(
        (
            retention * 0.45
            + noise_reduction * 0.25
            + min(1.0, placeholders / 4) * 0.10
            + restore_coverage * 0.10
            + (1.0 - stale_retention) * 0.10
        )
        * 100,
        2,
    )
    return {
        "technique": technique,
        "original_tokens_est": original_tokens,
        "transformed_tokens_est": transformed_tokens,
        "tokens_saved_est": saved,
        "noise_reduction_ratio": round(noise_reduction, 4),
        "protected_retention_ratio": round(retention, 4),
        "stale_foreign_retention_ratio": round(stale_retention, 4),
        "restore_handle_coverage_ratio": round(restore_coverage, 4),
        "retained_terms": retained_terms,
        "retained_stale_foreign_terms": retained_stale_terms,
        "placeholders": placeholders,
        "skeletons": skeletons,
        "summarized_blocks": summarized,
        "latency_ms": round(elapsed_ms, 3),
        "practical_score": practical_score,
    }


def run_experiment(scenario_path: Path, out: Path, techniques: list[str], tool_budget_chars: int) -> list[dict[str, Any]]:
    raw = load_json(scenario_path)
    original = [block_from_dict(item) for item in raw["blocks"]]
    protected_terms = raw.get("protected_terms", PROTECTED_TERMS)
    stale_foreign_terms = raw.get("stale_foreign_terms", [])
    matrix = []
    for technique in techniques:
        started = time.perf_counter()
        transformed = apply_technique(original, technique, tool_budget_chars)
        elapsed_ms = (time.perf_counter() - started) * 1000
        metrics = score_blocks(original, transformed, protected_terms, elapsed_ms, technique, stale_foreign_terms)
        matrix.append(metrics)
        save_json(out / "runs" / f"{technique}.context.json", [block.__dict__ for block in transformed])
    save_json(out / "matrix.json", matrix)
    with (out / "matrix.csv").open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(matrix[0].keys()))
        writer.writeheader()
        writer.writerows(matrix)
    return matrix


def print_matrix(matrix: list[dict[str, Any]]) -> None:
    cols = [
        "technique",
        "tokens_saved_est",
        "noise_reduction_ratio",
        "protected_retention_ratio",
        "stale_foreign_retention_ratio",
        "restore_handle_coverage_ratio",
        "latency_ms",
        "practical_score",
    ]
    widths = {col: max(len(col), *(len(str(row[col])) for row in matrix)) for col in cols}
    print("  ".join(col.ljust(widths[col]) for col in cols))
    print("  ".join("-" * widths[col] for col in cols))
    for row in sorted(matrix, key=lambda item: item["practical_score"], reverse=True):
        print("  ".join(str(row[col]).ljust(widths[col]) for col in cols))


def run_local(args: argparse.Namespace) -> None:
    out = Path(args.out or default_output_dir()).resolve()
    out.mkdir(parents=True, exist_ok=True)
    scenario = Path(args.scenario).resolve() if args.scenario else generate_scenarios(out, args.include_local_sessions, args.scenario_kind)
    matrix = run_experiment(scenario, out, args.technique, args.tool_budget_chars)
    print_matrix(matrix)
    print(f"\nWrote context evaluation artifacts to {out}")


def run_remote(args: argparse.Namespace) -> None:
    host = args.host
    remote_dir = args.remote_dir.rstrip("/")
    local_root = repo_root()
    remote_repo = f"{remote_dir}/jcode-context-eval"
    ssh = ["ssh", "-o", "BatchMode=yes", host]

    def remote(command: str) -> None:
        print(f"[remote {host}] {command}")
        subprocess.run(ssh + [command], check=True)

    if args.vm_start_cmd:
        remote(args.vm_start_cmd)

    remote(f"mkdir -p {remote_dir}")
    rsync = [
        "rsync",
        "-az",
        "--delete",
        "--exclude", "target/",
        "--exclude", ".git/",
        f"{local_root}/",
        f"{host}:{remote_repo}/",
    ]
    print("[local] " + " ".join(rsync))
    subprocess.run(rsync, check=True)

    remote_cmd = (
        f"cd {remote_repo} && "
        f"python3 scripts/context_pipeline_eval.py run-local "
        f"--out {remote_repo}/target/context-eval/remote "
        f"--scenario-kind {args.scenario_kind} "
        f"{'--include-local-sessions' if args.include_local_sessions else ''}"
    )
    remote(remote_cmd)

    out = Path(args.out or (local_root / "target" / "context-eval" / f"remote-{host}"))
    out.mkdir(parents=True, exist_ok=True)
    subprocess.run(["rsync", "-az", f"{host}:{remote_repo}/target/context-eval/remote/", f"{out}/"], check=True)
    print(f"Fetched remote results into {out}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    gen = sub.add_parser("generate-scenarios", help="write deterministic synthetic/local replay scenarios")
    gen.add_argument("--out", default=None)
    gen.add_argument("--include-local-sessions", action="store_true")
    gen.add_argument("--scenario-kind", choices=("synthetic", "realistic"), default="synthetic")
    gen.set_defaults(func=lambda args: print(generate_scenarios(Path(args.out or default_output_dir()), args.include_local_sessions, args.scenario_kind)))

    run = sub.add_parser("run-local", help="run local deterministic context-pipeline experiments")
    run.add_argument("--scenario", default=None)
    run.add_argument("--out", default=None)
    run.add_argument("--include-local-sessions", action="store_true")
    run.add_argument("--scenario-kind", choices=("synthetic", "realistic"), default="synthetic")
    run.add_argument("--tool-budget-chars", type=int, default=4_000)
    run.add_argument("--technique", action="append", choices=DEFAULT_TECHNIQUES + ["duplicate_prune"], default=None)
    run.set_defaults(func=run_local)

    remote = sub.add_parser("run-remote", help="run over SSH on serious-callers-only or compatible host")
    remote.add_argument("--host", default=os.environ.get("JCODE_CONTEXT_EVAL_HOST", DEFAULT_HOST))
    remote.add_argument("--remote-dir", default=os.environ.get("JCODE_CONTEXT_EVAL_REMOTE_DIR", "/tmp/jcode-context-eval"))
    remote.add_argument("--vm-start-cmd", default=os.environ.get("JCODE_CONTEXT_EVAL_VM_START_CMD", ""))
    remote.add_argument("--out", default=None)
    remote.add_argument("--include-local-sessions", action="store_true")
    remote.add_argument("--scenario-kind", choices=("synthetic", "realistic"), default="synthetic")
    remote.set_defaults(func=run_remote)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if getattr(args, "technique", None) is None:
        args.technique = DEFAULT_TECHNIQUES
    args.func(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
