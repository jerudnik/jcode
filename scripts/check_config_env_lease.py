#!/usr/bin/env python3
"""Fail when a test mutates a config-fingerprint env var without the env lease.

`jcode_base::config` keeps a process-global config cache whose reload decision
is driven by a fingerprint over the config file's metadata *and* the values of
the `CONFIG_ENV_KEYS` environment variables. Tests run concurrently in one
process, so a test that mutates one of those keys without holding the exclusive
test environment lease (`crate::storage::lock_test_env()`) changes the
fingerprint underneath every other test.

That produces a rare, order-dependent, machine-dependent flake. The failure
surfaces in an unrelated test (typically a cache-generation assertion such as
`global_config_cache_reloads_after_manual_file_edit`), names no cause, and does
not reproduce when the suspected test is run alone. It cost a full debugging
session to trace one instance back to
`memory_embedding_backend_normalizes_env_reintroduced_bad_value`.

This gate makes the invariant structural instead of tribal: mutating a
fingerprint key inside a `#[test]` requires an environment lease acquired
directly or through a helper.

Exit status is non-zero when a violation is found, so it can run as a blocking
preflight/CI gate. It is a pure text scan and costs no compilation.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CONFIG_RS = REPO_ROOT / "crates" / "jcode-base" / "src" / "config.rs"
SCAN_ROOTS = (REPO_ROOT / "src", REPO_ROOT / "crates")

# The canonical lease, plus any local helper that resolves to it. Helper names
# are discovered per-file rather than hardcoded: a helper counts as a lease if
# its body acquires `lock_test_env` (directly or transitively) or serializes on
# a static mutex, which is the other in-tree way tests exclude each other.
ROOT_LEASE_MARKERS = ("lock_test_env",)
STATIC_MUTEX = re.compile(r"static\s+\w+\s*:\s*(?:std::sync::)?Mutex")

TEST_ATTR = re.compile(r"#\[(?:tokio::)?test\b")
FN_NAME = re.compile(r"\bfn\s+(\w+)")
# `set_var("KEY", ...)` / `remove_var("KEY")`
LITERAL_MUTATION = re.compile(r"(?:set_var|remove_var)\(\s*\"([A-Za-z_0-9]+)\"")
# `set_var(key, ...)` where `key` is a local bound to a string literal.
VARIABLE_MUTATION = re.compile(r"(?:set_var|remove_var)\(\s*(\w+)\s*[,)]")
LOCAL_STRING_BINDING = re.compile(
    r"let\s+(\w+)\s*(?::\s*&\s*str\s*)?=\s*\"([A-Za-z_0-9]+)\"\s*;"
)


def config_env_keys() -> set[str]:
    """Parse `CONFIG_ENV_KEYS` out of config.rs so the gate cannot drift."""
    source = CONFIG_RS.read_text(encoding="utf-8")
    match = re.search(r"const CONFIG_ENV_KEYS[^\[]*\[(.*?)\n\];", source, re.S)
    if not match:
        sys.exit(f"error: could not locate CONFIG_ENV_KEYS in {CONFIG_RS}")
    keys = set(re.findall(r'"([^"]+)"', match.group(1)))
    if not keys:
        sys.exit(f"error: parsed an empty CONFIG_ENV_KEYS from {CONFIG_RS}")
    return keys


ANY_FN = re.compile(r"\bfn\s+(\w+)\s*(?:<[^>]*>)?\s*\(")
IMPL_BLOCK = re.compile(r"\bimpl(?:\s*<[^>]*>)?\s+(?:[\w:]+\s+for\s+)?([A-Z]\w*)")
# Names too generic to identify a helper by mention alone.
GENERIC_NAMES = {"new", "default", "drop", "run", "build", "setup", "init", "with"}
# A helper must *look* like a test-scoping helper. Without this, transitive
# text matching drags in half the workspace (`Config`, `env`, `set_var`, ...)
# and the gate silently passes everything.
HELPER_NAME_HINT = re.compile(
    r"(?:lock|guard|lease|scope|sandbox|isolat|with_|temp|env)", re.I
)


def collect_lease_helpers(sources: dict[Path, str]) -> set[str]:
    """Names whose mention in a test body proves environment exclusion.

    Seeded with `lock_test_env`, then closed over helpers that both (a) reach a
    known lease in their body and (b) have a name that reads like a scoping
    helper. Methods register their enclosing *type* (so `EnvGuard::new()`
    registers `EnvGuard`, not the useless `new`). Functions that serialize on a
    private static `Mutex` also count, that being the other in-tree exclusion
    mechanism.

    The name hint keeps the closure tight on purpose. An over-broad helper set
    makes this gate vacuous, which is worse than no gate at all.
    """
    bodies: dict[str, str] = {}

    for source in sources.values():
        impls = [
            (m.start(), _matching_brace(source, source.find("{", m.end())), m.group(1))
            for m in IMPL_BLOCK.finditer(source)
            if source.find("{", m.end()) != -1
        ]
        for match in ANY_FN.finditer(source):
            open_brace = source.find("{", match.end() - 1)
            if open_brace == -1:
                continue
            end = _matching_brace(source, open_brace)
            if end is None:
                continue
            name = match.group(1)
            for start, impl_end, type_name in impls:
                if impl_end and start < match.start() < impl_end:
                    name = type_name
                    break
            if name in GENERIC_NAMES or not HELPER_NAME_HINT.search(name):
                continue
            bodies[name] = bodies.get(name, "") + strip_comments(source[open_brace:end])

    helpers = set(ROOT_LEASE_MARKERS)
    for name, body in bodies.items():
        if STATIC_MUTEX.search(body) and ".lock()" in body:
            helpers.add(name)

    changed = True
    while changed:
        changed = False
        for name, body in bodies.items():
            if name in helpers:
                continue
            if any(
                re.search(r"\b" + re.escape(helper) + r"\b", body) for helper in helpers
            ):
                helpers.add(name)
                changed = True
    return helpers


def iter_test_bodies(source: str):
    """Yield `(fn_name, body, line_number)` for each `#[test]` function.

    The body is delimited by brace matching from the function's opening `{`, so
    a test never absorbs code from whatever follows it. Matching skips braces
    inside string literals, char literals, and comments.
    """
    for attr in TEST_ATTR.finditer(source):
        name_match = FN_NAME.search(source, attr.end())
        if not name_match:
            continue
        open_brace = source.find("{", name_match.end())
        if open_brace == -1:
            continue
        end = _matching_brace(source, open_brace)
        if end is None:
            continue
        line = source.count("\n", 0, attr.start()) + 1
        yield name_match.group(1), source[open_brace:end], line


def _matching_brace(source: str, start: int) -> int | None:
    """Index just past the `}` matching the `{` at `start`, or None."""
    depth = 0
    index = start
    length = len(source)
    while index < length:
        char = source[index]
        if char == "/" and index + 1 < length:
            following = source[index + 1]
            if following == "/":
                newline = source.find("\n", index)
                index = length if newline == -1 else newline
                continue
            if following == "*":
                close = source.find("*/", index + 2)
                index = length if close == -1 else close + 2
                continue
        elif char == '"':
            index = _skip_string(source, index)
            continue
        elif char == "'":
            index = _skip_char_literal(source, index)
            continue
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index + 1
        index += 1
    return None


def _skip_string(source: str, index: int) -> int:
    """Index just past the string literal starting at `index`."""
    # Raw strings: r"..", r#".."#, br#".."#
    prefix = source.rfind("r", max(0, index - 2), index)
    if prefix != -1 and set(source[prefix + 1 : index]) <= {"#"}:
        hashes = index - prefix - 1
        terminator = '"' + "#" * hashes
        close = source.find(terminator, index + 1)
        return len(source) if close == -1 else close + len(terminator)
    index += 1
    while index < len(source):
        if source[index] == "\\":
            index += 2
            continue
        if source[index] == '"':
            return index + 1
        index += 1
    return index


def _skip_char_literal(source: str, index: int) -> int:
    """Index past a char literal, or past the quote if it is a lifetime tick."""
    rest = source[index : index + 4]
    if re.match(r"^'\\.'", rest) or re.match(r"^'[^\\']'", rest):
        return index + len(re.match(r"^'(?:\\.|[^\\'])'", rest).group(0))
    return index + 1  # lifetime (`'a`), not a literal


LINE_COMMENT = re.compile(r"//[^\n]*")
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)


def strip_comments(body: str) -> str:
    """Remove comments so prose mentioning a lease cannot mask a violation."""
    return LINE_COMMENT.sub("", BLOCK_COMMENT.sub("", body))


def mutated_keys(body: str) -> set[str]:
    keys = set(LITERAL_MUTATION.findall(body))
    bindings = dict(LOCAL_STRING_BINDING.findall(body))
    for variable in VARIABLE_MUTATION.findall(body):
        if variable in bindings:
            keys.add(bindings[variable])
    return keys


def main() -> int:
    keys = config_env_keys()
    sources: dict[Path, str] = {}
    for root in SCAN_ROOTS:
        if not root.exists():
            continue
        for path in sorted(root.rglob("*.rs")):
            sources[path] = path.read_text(encoding="utf-8", errors="replace")

    helpers = collect_lease_helpers(sources)
    helper_pattern = re.compile(
        r"\b(?:" + "|".join(sorted(map(re.escape, helpers), key=len, reverse=True)) + r")\b"
    )

    violations: list[tuple[Path, int, str, list[str]]] = []
    for path, source in sources.items():
        if "set_var" not in source and "remove_var" not in source:
            continue
        for name, body, line in iter_test_bodies(source):
            body = strip_comments(body)
            if helper_pattern.search(body):
                continue
            # A test that declares and locks its own static mutex serializes
            # itself against its crate's other tests inline.
            if STATIC_MUTEX.search(body) and ".lock()" in body:
                continue
            hits = sorted(mutated_keys(body) & keys)
            if hits:
                violations.append((path.relative_to(REPO_ROOT), line, name, hits))

    if violations:
        print("Config-fingerprint env vars mutated without the test env lease:\n")
        for path, line, name, hits in violations:
            print(f"  {path}:{line}: {name}")
            print(f"      mutates: {', '.join(hits)}")
        print(
            "\nThese variables feed the global config cache fingerprint. Mutating\n"
            "them without `let _guard = crate::storage::lock_test_env();` races\n"
            "every concurrently running test and shows up as an unrelated,\n"
            "order-dependent cache-generation flake.\n\n"
            "Fix: acquire the lease in the listed test (or use a scoped helper\n"
            "that does). Do not silence this by renaming the variable."
        )
        return 1

    print(
        f"config env lease: ok (no unleased mutations of {len(keys)} "
        "config-fingerprint env vars in tests)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
