# G03 verification: packaged browser control surface

Date: 2026-08-02
Commit under test: `2ee70cb0d`
Nix output: `/nix/store/n91lbii58pdp69b9f7glcr8ar33fjx1x-jcode-0.46.0`
Runner: `scripts/verify_g03_browser_surface.sh <out-path>`

## What G03 asked for, and where each part is actually proven

G03 names two different things, and they are not proven in the same place.
Saying "G03 passed" without that split would overstate the result.

**The gateway protocol** (pairing, subscribe, history, send/cancel,
disconnect/reconnect/resync, stale-ack isolation) is already covered end to end
over a real loopback socket by
`gateway_e2e_pair_ws_history_send_cancel_reconnect_and_stale_ack_isolation`
in `crates/jcode-base/src/gateway_tests.rs`, alongside ~20 focused tests for
pairing, token auth, re-pairing, bearer/query extraction and access policy. That
test binds a real `TcpListener`, POSTs a pairing code, opens a WebSocket, and
drives a runtime peer. Re-driving the same protocol from a shell script would be
a worse copy of a better test, so this run does not do that.

**The packaging seam is what was unproven**, and it is what this run adds: an
installed binary must serve *its own* packaged assets out of the FHS `share/`
layout without depending on the caller's working directory. A cargo test
structurally cannot show this, because it always runs inside a source checkout
where the developer CWD fallback would mask a broken install.

## Result

All checks PASS against the Nix-built binary:

- packaged layout: executable present, `share/jcode/web/jcode-mobile` present,
  and `index.html` / `app.js` / `style.css` all shipped (a package that serves
  an index but drops `app.js` is still a broken install)
- served from a temp dir that is not a checkout: server starts and answers
- **control**: a decoy `web/jcode-mobile/index.html` planted in CWD is *not*
  served
- served bytes are byte-identical to the packaged files for all three assets
  (checked with `cmp`, not just a 200)
- path traversal to `/etc/passwd` refused (404)

## Why the control is trustworthy

A green control is worthless until it is shown capable of going red. The decoy
check was falsified directly: pointing `JCODE_MOBILE_WEB_ROOT` at the decoy root
makes the same binary serve `DECOY-SHOULD-NEVER-BE-SERVED`. So the passing run
reflects resolution order actually preferring packaged assets, not the decoy
being unreachable or the assertion being inert.

The byte-comparison matters for the same reason. "Not the decoy" alone would
also be satisfied by an error page, so the positive side (`served == packaged`)
is asserted independently.

## Infrastructure note (not a product defect)

The Nix build first failed twice with `Nix daemon disconnected unexpectedly`,
then a third attempt hung for ~84 minutes with **zero `rustc` processes on
either the local machine or the remote builder** and the remote daemon at 0%
CPU. That is a stall, not slowness. Rather than retry a fourth time, the build
was re-run with `--builders ''` to bypass the `ssh-ng://` remote builder, and it
completed in **under 90 seconds**. The fault is in the remote builder path, not
in the package or the flake. Recorded here because "the build is just slow"
would have been the wrong diagnosis and would have cost hours.

## Scope

Local and deterministic: a Nix build plus a loopback server. No credentials, no
external writes, no Apple or PWA publication path. This is a browser control
surface, not a PWA, per the node's own wording.
