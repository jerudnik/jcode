#!/usr/bin/env python3
"""Shared Rust production/test classifier for quality-gate budget scripts.

This is intentionally a lightweight classifier, not a full Rust parser. It keeps
budget scans trustworthy by recognizing direct `#[cfg(test)]` Rust items and by
counting braces only in code, not inside comments or string literals.
"""

from __future__ import annotations

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SCAN_ROOTS = (REPO_ROOT / "src", REPO_ROOT / "crates")
# Bounded direct item support only. Arbitrary macro invocations and
# statement/expression attributes remain conservatively counted because their
# boundaries are not safe to infer without broader Rust parsing.
ITEM_START_RE = re.compile(
    r"(?:pub(?:\s*\([^)]*\))?\s+)?"
    r"(?:(?:default|async|const|unsafe)\s+)*"
    r"(?:"
    r"macro_rules\s*!"
    r"|extern\b(?:\s+\"[^\"]+\")?"
    r"|(?:mod|fn|impl|struct|enum|trait|type|const|static|use|macro)\b"
    r")"
)


def is_test_rust_file(path: Path, repo_root: Path = REPO_ROOT) -> bool:
    rel = path.relative_to(repo_root).as_posix()
    if path.suffix != ".rs":
        return False
    parts = rel.split("/")
    if parts[0] == "tests" or any(
        part == "tests" or part.endswith("_tests") or part.endswith("_test") or part.startswith("tests_")
        for part in parts
    ):
        return True
    name = path.name
    return name == "tests.rs" or name.endswith("_tests.rs") or name.endswith("_test.rs") or name.startswith("tests_")


def production_rust_files(
    scan_roots: tuple[Path, ...] = DEFAULT_SCAN_ROOTS,
    repo_root: Path = REPO_ROOT,
) -> list[Path]:
    files: list[Path] = []
    for root in scan_roots:
        if not root.exists():
            continue
        for path in sorted(root.rglob("*.rs")):
            if path.suffix == ".rs" and not is_test_rust_file(path, repo_root):
                files.append(path)
    return files


def _blank_span(chars: list[str], start: int, end: int) -> None:
    for index in range(start, min(end, len(chars))):
        if chars[index] != "\n":
            chars[index] = " "


def _raw_string_end(source: str, start: int) -> int | None:
    index = start
    if source.startswith("br", index) or source.startswith("cr", index):
        index += 2
    elif source.startswith("r", index):
        index += 1
    else:
        return None

    hash_start = index
    while index < len(source) and source[index] == "#":
        index += 1
    if index >= len(source) or source[index] != '"':
        return None

    hashes = source[hash_start:index]
    terminator = '"' + hashes
    end = source.find(terminator, index + 1)
    if end == -1:
        return len(source)
    return end + len(terminator)


def _char_literal_end(source: str, start: int) -> int | None:
    index = start + 1
    if index >= len(source) or source[index] == "\n":
        return None

    escaped = False
    while index < len(source) and source[index] != "\n":
        char = source[index]
        index += 1
        if escaped:
            escaped = False
        elif char == "\\":
            escaped = True
        elif char == "'":
            content = source[start + 1 : index - 1]
            if len(content) > 8 or any(ch.isspace() or ch in ",<>:" for ch in content):
                return None
            return index
    return None


def _mask_rust_non_code(source: str) -> str:
    """Return source with comments and literals replaced by spaces.

    Newlines and byte offsets are preserved so later ranges can be applied back
    to the original source. Rust nested block comments and raw strings are
    handled because those are common places for unmatched braces in tests.
    """

    chars = list(source)
    index = 0
    while index < len(source):
        raw_end = _raw_string_end(source, index)
        if raw_end is not None:
            _blank_span(chars, index, raw_end)
            index = raw_end
            continue

        if source.startswith("//", index):
            end = source.find("\n", index)
            if end == -1:
                end = len(source)
            _blank_span(chars, index, end)
            index = end
            continue

        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < len(source) and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            _blank_span(chars, index, end)
            index = end
            continue

        if source[index] == "'":
            char_end = _char_literal_end(source, index)
            if char_end is not None:
                _blank_span(chars, index, char_end)
                index = char_end
                continue

        if source[index] in {'"'} or (
            source[index] in {"b", "c"} and index + 1 < len(source) and source[index + 1] in {'"', "'"}
        ):
            quote_start = index
            if source[index] in {"b", "c"}:
                index += 1
            quote = source[index]
            if quote == "'":
                char_end = _char_literal_end(source, index)
                if char_end is None:
                    index = quote_start + 1
                    continue
                _blank_span(chars, quote_start, char_end)
                index = char_end
                continue
            index += 1
            escaped = False
            while index < len(source):
                char = source[index]
                index += 1
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == quote:
                    break
            _blank_span(chars, quote_start, index)
            continue

        index += 1

    return "".join(chars)


def _parse_outer_attribute(code: str, start: int) -> tuple[int, int, str] | None:
    if start >= len(code) or code[start] != "#":
        return None
    index = start + 1
    while index < len(code) and code[index].isspace():
        index += 1
    if index >= len(code) or code[index] != "[":
        return None

    depth = 0
    inner_start = index + 1
    while index < len(code):
        if code[index] == "[":
            depth += 1
        elif code[index] == "]":
            depth -= 1
            if depth == 0:
                return start, index + 1, code[inner_start:index]
        index += 1
    return None


def _parse_inner_attribute(code: str, start: int) -> tuple[int, int, str] | None:
    if start >= len(code) or code[start] != "#":
        return None
    index = start + 1
    while index < len(code) and code[index].isspace():
        index += 1
    if index >= len(code) or code[index] != "!":
        return None
    index += 1
    while index < len(code) and code[index].isspace():
        index += 1
    if index >= len(code) or code[index] != "[":
        return None

    depth = 0
    inner_start = index + 1
    while index < len(code):
        if code[index] == "[":
            depth += 1
        elif code[index] == "]":
            depth -= 1
            if depth == 0:
                return start, index + 1, code[inner_start:index]
        index += 1
    return None


def _skip_ws_and_outer_attrs(code: str, start: int) -> int:
    index = start
    while index < len(code):
        while index < len(code) and code[index].isspace():
            index += 1
        attr = _parse_outer_attribute(code, index)
        if attr is None:
            return index
        index = attr[1]
    return index


def _split_top_level_args(args: str) -> list[str]:
    parts: list[str] = []
    depth = 0
    start = 0
    for index, char in enumerate(args):
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
        elif char == "," and depth == 0:
            parts.append(args[start:index])
            start = index + 1
    parts.append(args[start:])
    return [part for part in parts if part]


def _cfg_expr_requires_test(expr: str) -> bool:
    if expr == "test":
        return True
    if expr.startswith("not(") and expr.endswith(")"):
        return False
    if expr.startswith("all(") and expr.endswith(")"):
        return any(_cfg_expr_requires_test(arg) for arg in _split_top_level_args(expr[4:-1]))
    if expr.startswith("any(") and expr.endswith(")"):
        args = _split_top_level_args(expr[4:-1])
        return bool(args) and all(_cfg_expr_requires_test(arg) for arg in args)
    return False


def _is_cfg_test_attr(inner: str) -> bool:
    normalized = re.sub(r"\s+", "", inner)
    if not normalized.startswith("cfg(") or not normalized.endswith(")"):
        return False
    return _cfg_expr_requires_test(normalized[4:-1])


def _item_end(code: str, start: int) -> int | None:
    index = start
    paren_depth = 0
    bracket_depth = 0
    while index < len(code):
        char = code[index]
        if char == ";" and paren_depth == 0 and bracket_depth == 0:
            return index + 1
        if char == "(":
            paren_depth += 1
        elif char == ")" and paren_depth > 0:
            paren_depth -= 1
        elif char == "[":
            bracket_depth += 1
        elif char == "]" and bracket_depth > 0:
            bracket_depth -= 1
        elif char == "{" and paren_depth == 0 and bracket_depth == 0:
            depth = 1
            index += 1
            while index < len(code):
                if code[index] == "{":
                    depth += 1
                elif code[index] == "}":
                    depth -= 1
                    if depth == 0:
                        return index + 1
                index += 1
            return len(code)
        index += 1
    return None


def _file_requires_test(masked_code: str) -> bool:
    index = 0
    while index < len(masked_code):
        while index < len(masked_code) and masked_code[index].isspace():
            index += 1
        attr = _parse_inner_attribute(masked_code, index)
        if attr is None:
            return False
        if _is_cfg_test_attr(attr[2]):
            return True
        index = attr[1]
    return False


def _cfg_test_item_ranges(masked_code: str) -> list[tuple[int, int]]:
    ranges: list[tuple[int, int]] = []
    index = 0
    while index < len(masked_code):
        attr_start = masked_code.find("#", index)
        if attr_start == -1:
            break
        attr = _parse_outer_attribute(masked_code, attr_start)
        if attr is None:
            index = attr_start + 1
            continue

        start, attr_end, inner = attr
        if not _is_cfg_test_attr(inner):
            index = attr_end
            continue

        item_start = _skip_ws_and_outer_attrs(masked_code, attr_end)
        match = ITEM_START_RE.match(masked_code, item_start)
        if match is None:
            index = attr_end
            continue

        end = _item_end(masked_code, match.end())
        if end is not None:
            ranges.append((start, end))
            index = end
        else:
            index = attr_end
    return ranges


def production_lines_from_text(source: str) -> list[str]:
    masked_code = _mask_rust_non_code(source)
    production_chars = list(source)
    if _file_requires_test(masked_code):
        _blank_span(production_chars, 0, len(production_chars))
        return "".join(production_chars).splitlines()
    for start, end in _cfg_test_item_ranges(masked_code):
        _blank_span(production_chars, start, end)
    return "".join(production_chars).splitlines()


def production_lines(path: Path) -> list[str]:
    return production_lines_from_text(path.read_text(encoding="utf-8", errors="ignore"))
