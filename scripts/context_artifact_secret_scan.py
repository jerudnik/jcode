#!/usr/bin/env python3
"""Scan context-eval artifacts for likely secrets before sharing.

Stdlib-only scanner for generated context experiment artifacts and reports. It
recursively scans text-like files for high-confidence API keys, tokens, private
key blocks, and configured/sentinel secret values. Findings are written as JSON
plus a concise human summary. The process exits non-zero when high-severity
findings remain after allowlist and minimum-length filtering.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

MAX_FILE_BYTES = 8 * 1024 * 1024
DEFAULT_OUT_DIRNAME = "secret_scan"
TEXT_EXTENSIONS = {
    ".csv",
    ".json",
    ".jsonl",
    ".log",
    ".md",
    ".out",
    ".report",
    ".txt",
    ".yaml",
    ".yml",
}
DEFAULT_EXCLUDE_GLOBS = [
    "**/.git/**",
    "**/target/debug/**",
    "**/target/release/**",
    "**/node_modules/**",
    "**/__pycache__/**",
    "secret_scan/findings.json",
    "secret_scan/summary.txt",
    "**/secret_scan/findings.json",
    "**/secret_scan/summary.txt",
]
BENIGN_SUBSTRINGS = [
    "example",
    "sample",
    "dummy",
    "placeholder",
    "redacted",
    "your_api_key",
    "your-api-key",
    "changeme",
    "not-a-secret",
    "no-secret",
    "test-only",
    "fake",
]
SENTINEL_SECRET_TERMS = [
    "PAYMENT_SECRET_DO_NOT_USE",
    "SENTINEL_SECRET",
    "SECRET_DO_NOT_SHARE",
    "LEAKED_SECRET_SENTINEL",
]


@dataclass(frozen=True)
class SecretPattern:
    name: str
    regex: re.Pattern[str]
    severity: str = "high"
    min_secret_len: int = 20
    secret_group: int | str = 0
    description: str = ""


@dataclass
class Finding:
    path: str
    line: int
    column: int
    detector: str
    severity: str
    match_hash: str
    redacted_match: str
    context: str
    description: str


SECRET_PATTERNS = [
    SecretPattern(
        "private_key_block",
        re.compile(r"-----BEGIN (?:RSA |DSA |EC |OPENSSH |PGP )?PRIVATE KEY-----.*?-----END (?:RSA |DSA |EC |OPENSSH |PGP )?PRIVATE KEY-----", re.DOTALL),
        min_secret_len=80,
        description="PEM/OpenSSH/PGP private key block",
    ),
    SecretPattern(
        "openai_api_key",
        re.compile(r"\b(sk-(?:proj-)?[A-Za-z0-9_-]{32,})\b"),
        secret_group=1,
        description="OpenAI-style API key",
    ),
    SecretPattern(
        "anthropic_api_key",
        re.compile(r"\b(sk-ant-[A-Za-z0-9_-]{40,})\b"),
        secret_group=1,
        description="Anthropic-style API key",
    ),
    SecretPattern(
        "github_token",
        re.compile(r"\b((?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{36,})\b"),
        secret_group=1,
        description="GitHub token",
    ),
    SecretPattern(
        "slack_token",
        re.compile(r"\b(xox[baprs]-[A-Za-z0-9-]{20,})\b"),
        secret_group=1,
        description="Slack token",
    ),
    SecretPattern(
        "aws_access_key_id",
        re.compile(r"\b((?:AKIA|ASIA)[A-Z0-9]{16})\b"),
        secret_group=1,
        description="AWS access key id",
    ),
    SecretPattern(
        "jwt",
        re.compile(r"\b(eyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,})\b"),
        secret_group=1,
        min_secret_len=48,
        description="JWT-like bearer token",
    ),
    SecretPattern(
        "secret_assignment",
        re.compile(r"(?i)\b(?:api[_-]?key|access[_-]?token|auth[_-]?token|bearer|client[_-]?secret|secret|token|password)\b\s*[:=]\s*[\"']?([A-Za-z0-9_./+=:@~-]{24,})"),
        secret_group=1,
        description="High-entropy secret-looking assignment",
    ),
    SecretPattern(
        "sentinel_secret",
        re.compile(r"\b(" + "|".join(re.escape(term) for term in SENTINEL_SECRET_TERMS) + r")(?:[=:][^\s\"']{8,})?\b"),
        severity="high",
        min_secret_len=10,
        secret_group=0,
        description="Known sentinel secret marker",
    ),
]


def sha256_short(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8", "replace")).hexdigest()[:16]


def redact(value: str) -> str:
    compact = value.replace("\n", "\\n")
    if len(compact) <= 12:
        return "<redacted>"
    return f"{compact[:4]}…{compact[-4:]}"


def line_col(text: str, offset: int) -> tuple[int, int]:
    line = text.count("\n", 0, offset) + 1
    last_newline = text.rfind("\n", 0, offset)
    column = offset + 1 if last_newline == -1 else offset - last_newline
    return line, column


def context_line(text: str, offset: int) -> str:
    start = text.rfind("\n", 0, offset) + 1
    end = text.find("\n", offset)
    if end == -1:
        end = len(text)
    line = text[start:end].strip()
    return line[:240]


def looks_text(path: Path) -> bool:
    if path.suffix.lower() in TEXT_EXTENSIONS:
        return True
    return path.suffix == "" or ".context" in path.name or "report" in path.name.lower()


def is_excluded(path: Path, root: Path, exclude_globs: list[str]) -> bool:
    try:
        rel = path.relative_to(root).as_posix()
    except ValueError:
        rel = path.as_posix()
    return any(fnmatch.fnmatch(rel, pattern) or fnmatch.fnmatch(path.as_posix(), pattern) for pattern in exclude_globs)


def iter_files(roots: Iterable[Path], exclude_globs: list[str]) -> Iterable[Path]:
    for root in roots:
        if root.is_file():
            if not is_excluded(root, root.parent, exclude_globs):
                yield root
            continue
        for path in sorted(root.rglob("*")):
            if path.is_file() and not is_excluded(path, root, exclude_globs):
                yield path


def read_text_if_scannable(path: Path, max_file_bytes: int) -> str | None:
    try:
        size = path.stat().st_size
    except OSError:
        return None
    if size > max_file_bytes or not looks_text(path):
        return None
    try:
        data = path.read_bytes()
    except OSError:
        return None
    if b"\0" in data[:4096]:
        return None
    return data.decode("utf-8", "replace")


def entropyish(value: str) -> bool:
    if len(value) < 20:
        return False
    classes = sum(bool(re.search(pattern, value)) for pattern in (r"[a-z]", r"[A-Z]", r"[0-9]", r"[_./+=:@~-]"))
    unique_ratio = len(set(value)) / max(1, len(value))
    return classes >= 2 and unique_ratio >= 0.25


def load_allowlist(path: Path | None) -> set[str]:
    if path is None:
        return set()
    raw = json.loads(path.read_text())
    if not isinstance(raw, list):
        raise SystemExit("allowlist must be a JSON list of strings or sha256 prefixes")
    return {str(item).lower() for item in raw}


def is_allowed(secret: str, detector: str, allowlist: set[str]) -> bool:
    lower = secret.lower()
    if detector not in {"private_key_block", "sentinel_secret"} and any(marker in lower for marker in BENIGN_SUBSTRINGS):
        return True
    digest = hashlib.sha256(secret.encode("utf-8", "replace")).hexdigest().lower()
    candidates = {lower, digest, digest[:16], f"{detector}:{digest}", f"{detector}:{digest[:16]}"}
    return bool(candidates & allowlist)


def scan_text(path: Path, root: Path, text: str, allowlist: set[str], min_len: int) -> list[Finding]:
    findings: list[Finding] = []
    rel = path.relative_to(root).as_posix() if path.is_relative_to(root) else str(path)
    seen: set[tuple[str, int, str]] = set()
    for pattern in SECRET_PATTERNS:
        for match in pattern.regex.finditer(text):
            secret = match.group(pattern.secret_group) if pattern.secret_group else match.group(0)
            if len(secret) < max(min_len, pattern.min_secret_len):
                continue
            if pattern.name == "secret_assignment" and not entropyish(secret):
                continue
            if is_allowed(secret, pattern.name, allowlist):
                continue
            line, column = line_col(text, match.start())
            key = (pattern.name, line, sha256_short(secret))
            if key in seen:
                continue
            seen.add(key)
            findings.append(
                Finding(
                    path=rel,
                    line=line,
                    column=column,
                    detector=pattern.name,
                    severity=pattern.severity,
                    match_hash=sha256_short(secret),
                    redacted_match=redact(secret),
                    context=context_line(text, match.start()).replace(secret, redact(secret)),
                    description=pattern.description,
                )
            )
    return findings


def scan(args: argparse.Namespace) -> int:
    roots = [Path(item).resolve() for item in args.paths]
    out = Path(args.out).resolve() if args.out else roots[0] / DEFAULT_OUT_DIRNAME
    allowlist = load_allowlist(Path(args.allowlist).resolve() if args.allowlist else None)
    exclude_globs = DEFAULT_EXCLUDE_GLOBS + list(args.exclude or [])

    all_findings: list[Finding] = []
    scanned = 0
    skipped = 0
    for path in iter_files(roots, exclude_globs):
        text = read_text_if_scannable(path, args.max_file_bytes)
        if text is None:
            skipped += 1
            continue
        scanned += 1
        root = next((candidate for candidate in roots if path == candidate or candidate in path.parents), path.parent)
        all_findings.extend(scan_text(path, root, text, allowlist, args.min_length))

    high_count = sum(1 for finding in all_findings if finding.severity == "high")
    payload = {
        "schema_version": 1,
        "roots": [str(root) for root in roots],
        "scanned_files": scanned,
        "skipped_files": skipped,
        "finding_count": len(all_findings),
        "high_severity_count": high_count,
        "allowlist_count": len(allowlist),
        "findings": [finding.__dict__ for finding in all_findings],
    }
    out.mkdir(parents=True, exist_ok=True)
    (out / "findings.json").write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")

    by_detector: dict[str, int] = {}
    for finding in all_findings:
        by_detector[finding.detector] = by_detector.get(finding.detector, 0) + 1
    lines = [
        "Context artifact secret scan",
        f"Roots: {', '.join(str(root) for root in roots)}",
        f"Scanned files: {scanned}",
        f"Skipped files: {skipped}",
        f"Findings: {len(all_findings)} (high severity: {high_count})",
    ]
    if by_detector:
        lines.append("By detector:")
        lines.extend(f"- {name}: {count}" for name, count in sorted(by_detector.items()))
        lines.append("Findings:")
        lines.extend(
            f"- {finding.severity} {finding.detector} {finding.path}:{finding.line}:{finding.column} hash={finding.match_hash} value={finding.redacted_match}"
            for finding in all_findings[: args.summary_limit]
        )
        if len(all_findings) > args.summary_limit:
            lines.append(f"- ... {len(all_findings) - args.summary_limit} more findings omitted from summary")
    else:
        lines.append("No likely secrets found.")
    summary = "\n".join(lines) + "\n"
    (out / "summary.txt").write_text(summary)
    print(summary, end="")
    print(f"Wrote JSON findings to {out / 'findings.json'}")
    print(f"Wrote summary to {out / 'summary.txt'}")
    return 1 if high_count else 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="+", help="Artifact directories or report files to scan recursively")
    parser.add_argument("--out", default=None, help="Output directory for findings.json and summary.txt")
    parser.add_argument("--allowlist", default=None, help="JSON list of allowed literal values or sha256/sha256-16 hashes")
    parser.add_argument("--exclude", action="append", default=None, help="Additional glob to exclude; may be repeated")
    parser.add_argument("--min-length", type=int, default=16, help="Global minimum matched secret length")
    parser.add_argument("--max-file-bytes", type=int, default=MAX_FILE_BYTES, help="Skip files larger than this byte count")
    parser.add_argument("--summary-limit", type=int, default=25, help="Maximum findings listed in the human summary")
    parser.set_defaults(func=scan)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
