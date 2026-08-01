#!/usr/bin/env python3
"""Count blank user turns across jcode session files.

The defect: a hidden continuation persists a user message whose content is
exactly one empty text block, which then ships to the provider as if the user
sent nothing.

Counting this correctly matters, and grep does not do it. A naive search for
`"text":""` also matches empty strings nested inside a tool_use *input*, which
are not message bodies. This parses instead.

Also reports the trailing-role split, because that is what the fix branches on:
a blank whose predecessor is an assistant message is the prefill-risk case.
"""

import json
import os
import sys
from datetime import datetime, timezone


def blanks_in(path):
    """Return (blanks, total_messages) for one session file."""
    try:
        with open(path) as fh:
            doc = json.load(fh)
    except (json.JSONDecodeError, OSError):
        return None, 0

    messages = doc.get("messages") if isinstance(doc, dict) else doc
    if not isinstance(messages, list):
        return None, 0

    found = []
    for i, msg in enumerate(messages):
        if msg.get("role") != "user":
            continue
        content = msg.get("content")
        # Exactly one block, and that block is empty text. Not a substring match.
        if (
            isinstance(content, list)
            and len(content) == 1
            and content[0].get("type") == "text"
            and content[0].get("text", "") == ""
        ):
            prev_role = messages[i - 1].get("role") if i > 0 else None
            found.append((i, prev_role))
    return found, len(messages)


def mtime_iso(path):
    return datetime.fromtimestamp(os.path.getmtime(path), timezone.utc).isoformat(
        timespec="seconds"
    )


def main():
    sessions = os.path.expanduser("~/.jcode/sessions")
    args = sys.argv[1:]
    if args and args[0] in ("-h", "--help"):
        print(__doc__.strip())
        print()
        print("usage: count_blank_user_turns.py [SINCE]")
        print()
        print("  SINCE   optional ISO date/prefix (e.g. 2026-07-01); only sessions")
        print("          modified at or after it are scanned. Omit to scan all.")
        return 0
    since = args[0] if args else None

    total_blanks = safe = risky = 0
    scanned = 0
    offenders = []

    for name in sorted(os.listdir(sessions)):
        if not name.endswith(".json"):
            continue
        path = os.path.join(sessions, name)
        stamp = mtime_iso(path)
        if since and stamp < since:
            continue
        found, n = blanks_in(path)
        if found is None:
            continue
        scanned += 1
        if not found:
            continue
        s = sum(1 for _, role in found if role == "user")
        r = len(found) - s
        total_blanks += len(found)
        safe += s
        risky += r
        offenders.append((name, stamp, len(found), s, r, [i for i, _ in found]))

    print(f"sessions scanned: {scanned}" + (f"  (modified >= {since})" if since else ""))
    print(f"blank user turns: {total_blanks}")
    print(f"  prev=user      (safe to drop)   : {safe}")
    print(f"  prev=assistant (prefill risk)   : {risky}")
    if offenders:
        print()
        for name, stamp, n, s, r, idx in offenders:
            short = name.replace("session_", "").replace(".json", "")
            print(f"  {short[:44]:44s} {stamp}  n={n} safe={s} risky={r}")
            shown = idx if len(idx) <= 24 else idx[:24]
            more = "" if len(idx) <= 24 else f" ... (+{len(idx) - 24} more)"
            print(f"    indices: {shown}{more}")
    return 1 if total_blanks else 0


if __name__ == "__main__":
    sys.exit(main())
