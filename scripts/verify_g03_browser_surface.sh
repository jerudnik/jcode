#!/usr/bin/env bash
# G03: verify the packaged browser control surface served by the Nix binary.
#
# Scope note, stated up front so this script is not read as proving more than
# it does. The gateway *protocol* behaviors named in G03 (pairing, subscribe,
# history, send/cancel, disconnect/reconnect/resync, stale-ack isolation) are
# already covered end to end over a real loopback socket by
# `gateway_e2e_pair_ws_history_send_cancel_reconnect_and_stale_ack_isolation`
# in crates/jcode-base/src/gateway_tests.rs. Re-driving that protocol from a
# shell would be a worse copy of a test that already exists.
#
# What is NOT covered there, and what this script adds, is the packaging seam:
# an installed binary must serve *its own* packaged assets, from the FHS
# `share/` layout, without depending on the caller's working directory. That
# seam is invisible to a cargo test because a cargo test always runs inside a
# source checkout, where the developer fallback would mask a broken install.
#
# The decisive check here is a CONTROL, not a fetch. We plant a *different*
# web/jcode-mobile in the working directory and require the packaged binary to
# keep serving the packaged bytes. A fetch alone would pass just as happily if
# resolution had silently fallen through to CWD, which is the exact failure the
# resolver's ordering exists to prevent.
set -euo pipefail

OUT="${1:-}"
if [[ -z "$OUT" ]]; then
  echo "usage: $0 <nix-out-path>" >&2
  exit 2
fi

BIN="$OUT/bin/jcode"
PACKAGED="$OUT/share/jcode/web/jcode-mobile"
fail=0
note() { printf '%-58s %s\n' "$1" "$2"; }
check() {
  if [[ "$2" == "$3" ]]; then note "$1" "OK"; else
    note "$1" "FAIL (want=$3 got=$2)"; fail=1
  fi
}

echo "== packaged layout =="
[[ -x "$BIN" ]] && note "binary is executable" "OK" || { note "binary is executable" "FAIL"; fail=1; }
[[ -d "$PACKAGED" ]] && note "share/jcode/web/jcode-mobile present" "OK" \
  || { note "share/jcode/web/jcode-mobile present" "FAIL"; fail=1; }

# Every asset the repo ships must be in the package. A package that serves an
# index.html but silently drops app.js is still a broken install.
for asset in index.html app.js style.css; do
  [[ -f "$PACKAGED/$asset" ]] && note "packaged asset $asset" "OK" \
    || { note "packaged asset $asset" "FAIL"; fail=1; }
done

echo
echo "== serve from a NON-checkout cwd, with a decoy in place =="
WORK="$(mktemp -d)"
trap 'kill "${SRV:-}" 2>/dev/null || true; rm -rf "$WORK"' EXIT
# The decoy: a plausible-looking web root in CWD. If asset resolution ever
# prefers CWD, the server returns DECOY and this run fails loudly.
mkdir -p "$WORK/web/jcode-mobile"
printf 'DECOY-SHOULD-NEVER-BE-SERVED' > "$WORK/web/jcode-mobile/index.html"

cd "$WORK"
PORT=$(( 20000 + RANDOM % 20000 ))
JCODE_HOME="$WORK/home" "$BIN" mobile-server serve-internal --port "$PORT" --bind 127.0.0.1 \
  > "$WORK/serve.log" 2>&1 &
SRV=$!

for _ in $(seq 1 60); do
  curl -fsS "http://127.0.0.1:$PORT/" -o /dev/null 2>/dev/null && break
  sleep 0.5
done

if ! kill -0 "$SRV" 2>/dev/null; then
  note "server started" "FAIL (exited)"; sed 's/^/    /' "$WORK/serve.log"; exit 1
fi
note "server started" "OK"

served="$(curl -fsS "http://127.0.0.1:$PORT/")"
if [[ "$served" == *DECOY* ]]; then
  note "CONTROL: cwd decoy not served" "FAIL (served the decoy)"; fail=1
else
  note "CONTROL: cwd decoy not served" "OK"
fi

# Positive side of the control: the bytes served must be byte-identical to the
# packaged file. "Not the decoy" alone would also be satisfied by an error page.
if [[ "$served" == "$(cat "$PACKAGED/index.html")" ]]; then
  note "served bytes == packaged index.html" "OK"
else
  note "served bytes == packaged index.html" "FAIL"; fail=1
fi

for asset in app.js style.css; do
  code="$(curl -s -o "$WORK/got.$asset" -w '%{http_code}' "http://127.0.0.1:$PORT/$asset")"
  check "GET /$asset -> 200" "$code" "200"
  if cmp -s "$WORK/got.$asset" "$PACKAGED/$asset"; then
    note "  bytes match packaged $asset" "OK"
  else
    note "  bytes match packaged $asset" "FAIL"; fail=1
  fi
done

# A path-traversal probe. This is a static file server reachable from a phone
# on the LAN; escaping the web root would be the whole ballgame.
trav="$(curl -s -o /dev/null -w '%{http_code}' --path-as-is "http://127.0.0.1:$PORT/../../../../etc/passwd")"
if [[ "$trav" == "200" ]]; then
  note "path traversal refused" "FAIL (200)"; fail=1
else
  note "path traversal refused" "OK ($trav)"
fi

echo
if [[ "$fail" == "0" ]]; then echo "G03 packaging seam: PASS"; else echo "G03 packaging seam: FAIL"; fi
exit "$fail"
