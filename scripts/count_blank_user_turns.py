#!/usr/bin/env python3
# Verdict: KEEP-MANUAL
# Operator: the maintainer investigating hidden-continuation persistence runs
# this with a SINCE cutoff after a candidate fix. Personal session history and
# immutable old blanks make the all-time result unsuitable for a shared gate.
"""Count blank user turns across jcode session files.

The defect: a hidden continuation persists a user message whose content is
exactly one empty text block, which then ships to the provider as if the user
sent nothing.

Counting this correctly matters, and grep does not do it. A naive search for
`"text":""` also matches empty strings nested inside a tool_use *input*, which
are not message bodies. This parses instead.

Also reports the trailing-role split, because that is what the fix branches on:
a blank whose predecessor is an assistant message is the prefill-risk case.

SINCE is parsed into a datetime, never compared as a string. Lexicographic
order is not chronological order across ISO formats: '.' (0x2E) and '+' (0x2B)
both sort before 'Z' (0x5A), so `"...T20:39:00.123456Z" < "...T20:39:00Z"` is
True even though that instant is later. That silently drops sub-second
messages in the first second after a precise cutoff -- and it drops *new*
blanks, so it over-credits a fix. Caught by badger.

SINCE filters on each *message's own* timestamp, not the file's mtime. That
distinction is the whole point of the flag: a long-running session touched
today can contain blanks written weeks ago, so an mtime filter would credit
old damage to the current build. Neither fix rewrites history, so the total
over all time is fixed by construction and will never move; the number that
should move is blanks created after a fix landed, which is what SINCE reports.
"""

import json
import os
import sys
from datetime import datetime, timezone


def parse_since(text):
    """Parse a bare date or full ISO stamp into an aware UTC datetime.

    Rejects unparseable input loudly. Silently comparing nonsense is how the
    string-comparison bug this replaces stayed invisible.
    """
    candidate = text.strip().replace("Z", "+00:00")
    for form in (candidate, f"{candidate}T00:00:00+00:00"):
        try:
            parsed = datetime.fromisoformat(form)
        except ValueError:
            continue
        return parsed if parsed.tzinfo else parsed.replace(tzinfo=timezone.utc)
    raise SystemExit(f"unrecognised SINCE value: {text!r}")


def parse_stamp(text):
    """Parse a message timestamp, or None if it is absent/unparseable."""
    if not text:
        return None
    try:
        parsed = datetime.fromisoformat(text.strip().replace("Z", "+00:00"))
    except ValueError:
        return None
    return parsed if parsed.tzinfo else parsed.replace(tzinfo=timezone.utc)


def blanks_in(path, since=None):
    """Return (blanks, total_messages) for one session file.

    `since` is compared against each message's own timestamp, so a session
    that merely *contains* old blanks does not report them as new ones.
    """
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
            if since:
                stamp = parse_stamp(msg.get("timestamp"))
                # No parseable timestamp means it cannot be shown to be recent,
                # so it is excluded rather than assumed new. This under-counts
                # rather than over-credits a fix.
                if stamp is None or stamp < since:
                    continue
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
        print("  SINCE   optional date or ISO timestamp, e.g. 2026-07-01 or")
        print("          2026-08-01T20:00:00Z. Counts only blanks whose own message")
        print("          timestamp is at or after it. Omit to scan all of history.")
        print("          Parsed, not string-compared; unrecognised input is an error.")
        print()
        print("  The all-time total is immutable: no fix rewrites session files.")
        print("  To see whether a fix worked, pass the time it landed.")
        return 0
    since = parse_since(args[0]) if args else None

    total_blanks = safe = risky = 0
    scanned = 0
    offenders = []

    for name in sorted(os.listdir(sessions)):
        if not name.endswith(".json"):
            continue
        path = os.path.join(sessions, name)
        stamp = mtime_iso(path)
        found, n = blanks_in(path, since)
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

    print(f"sessions scanned: {scanned}")
    if since:
        print(f"blank user turns created >= {since.isoformat()}: {total_blanks}")
    else:
        print(f"blank user turns (all time, immutable): {total_blanks}")
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
