#!/usr/bin/env python3
"""S01 transcript normalizer.

Contract: docs/fork/ideal-base/evidence/S01/NORMALIZER_SPEC.md (frozen before
first use). Erases exactly N1-N7 and nothing else.

Usage:
    normalize.py <transcript>            -> prints normalized text to stdout
    normalize.py --hash <transcript>     -> prints "<sha256>  <n_lines>"

Refuses input below MIN_LINES: an empty capture hashes stably and would read as
perfect determinism.
"""
import hashlib
import os
import re
import sys

MIN_LINES = 20

HOME = os.path.expanduser("~")

# N1 wall-clock timestamps
RE_TS_BRACKET = re.compile(r"\[\d{2}:\d{2}:\d{2}\]")
RE_TS_ISO = re.compile(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?")

# N2 elapsed durations
RE_DUR_IN = re.compile(r"\bin \d+(?:\.\d+)?(?:s|ms|m)\b")
RE_DUR_TOOK = re.compile(r"\btook \d+(?:\.\d+)?(?:s|ms|m)\b")
RE_DUR_MS = re.compile(r"\bfinished in \d+(?:\.\d+)?(?:s|ms)\b")

# N3 process ids
RE_PID_KV = re.compile(r"\bpid[= ]\d+\b", re.I)
RE_PID_PAREN = re.compile(r"\(pid[= ]\d+\)", re.I)

# N4 temp dirs
RE_TMP_VAR = re.compile(r"/var/folders/[A-Za-z0-9_/+-]+")
RE_TMP_T = re.compile(r"/tmp/[A-Za-z0-9._-]*[A-Za-z0-9]{6,}[A-Za-z0-9._-]*")

# N5 nix store hashes
RE_NIX = re.compile(r"/nix/store/[a-z0-9]{32}-")

# N7 cargo target fingerprints
RE_FP = re.compile(r"(target/[a-z0-9_-]+/deps/[A-Za-z0-9_]+)-[0-9a-f]{16}")


def normalize(text: str) -> str:
    # N6 first: HOME may contain segments the later patterns would mangle.
    text = text.replace(HOME, "<HOME>")
    text = RE_NIX.sub("/nix/store/<HASH>-", text)          # N5
    text = RE_TMP_VAR.sub("<TMP>", text)                   # N4
    text = RE_TMP_T.sub("<TMP>", text)                     # N4
    text = RE_FP.sub(r"\1-<FP>", text)                     # N7
    text = RE_TS_BRACKET.sub("[TS]", text)                 # N1
    text = RE_TS_ISO.sub("[TS]", text)                     # N1
    text = RE_DUR_MS.sub("finished in <DUR>", text)        # N2
    text = RE_DUR_IN.sub("in <DUR>", text)                 # N2
    text = RE_DUR_TOOK.sub("took <DUR>", text)             # N2
    text = RE_PID_PAREN.sub("(pid <PID>)", text)           # N3
    text = RE_PID_KV.sub("pid <PID>", text)                # N3
    return text


def main() -> int:
    args = sys.argv[1:]
    want_hash = False
    if args and args[0] == "--hash":
        want_hash = True
        args = args[1:]
    if len(args) != 1:
        print(__doc__, file=sys.stderr)
        return 2
    path = args[0]
    if not os.path.isfile(path):
        print(f"REFUSED: no such transcript: {path}", file=sys.stderr)
        return 3
    raw = open(path, "r", errors="replace").read()
    n_lines = raw.count("\n")
    # D4 guard: assert non-emptiness BEFORE hashing.
    if n_lines < MIN_LINES:
        print(
            f"REFUSED: transcript has {n_lines} lines, below floor {MIN_LINES}. "
            "An empty or truncated capture hashes stably and would read as "
            "perfect determinism.",
            file=sys.stderr,
        )
        return 4
    out = normalize(raw)
    if want_hash:
        h = hashlib.sha256(out.encode()).hexdigest()
        print(f"{h}  {n_lines}")
    else:
        sys.stdout.write(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
