# Telemetry

**This fork sends no telemetry unless you turn it on and point it somewhere.**

Two independent conditions must both hold before a single payload leaves the
machine:

1. Telemetry is **enabled** (off by default).
2. A telemetry **endpoint is configured** (no default).

Either one missing means nothing is sent. There is no build, flag, or code path
in which the default configuration transmits data.

## How this differs from upstream

Upstream `1jehuang/jcode` ships telemetry **opt-out**: it is on unless you set
`JCODE_NO_TELEMETRY`, set `DO_NOT_TRACK`, or create a `~/.jcode/no_telemetry`
marker. It posts to a Cloudflare Worker operated by the upstream maintainer,
hardcoded as a constant.

This fork changes both halves:

| | upstream | this fork |
|---|---|---|
| default state | enabled | disabled |
| to change it | `JCODE_NO_TELEMETRY=1` to disable | `JCODE_TELEMETRY=1` to enable |
| destination | hardcoded upstream Worker | none; `JCODE_TELEMETRY_ENDPOINT` or nothing |

The reasoning is narrow. A fork's users obtain a fork's binary. Sending their
install id, OS/arch, provider/model, and session-cadence counters to a server
the fork's maintainer does not operate, by default, is not a decision this fork
gets to make on their behalf. Opt-out is defensible for a project whose users
chose that project; it is not transitive across a fork.

## Enabling it

Both must be set.

```sh
export JCODE_TELEMETRY=1
export JCODE_TELEMETRY_ENDPOINT=https://collector.example/v1/event
```

Instead of the env var, the enable can be persisted with a marker file:

```sh
touch "${JCODE_HOME:-$HOME/.jcode}/telemetry_opt_in"
```

`JCODE_TELEMETRY` accepts `1`, `true`, `yes`, `on` (case-insensitive, trimmed).
Anything else, including `0` and the empty string, reads as *not* opted in, so
`JCODE_TELEMETRY=0` means what it looks like rather than "the variable is set,
therefore yes".

The endpoint receives the payload described in `jcode-telemetry-core`. Run your
own collector; the fork ships no default and no fallback.

## Disabling it

Every upstream opt-out mechanism still works and still takes precedence, so a
machine already configured to opt out keeps that behavior with no action:

- `JCODE_NO_TELEMETRY=1`
- `DO_NOT_TRACK=1` (the [consortium standard](https://consoledonottrack.com/))
- a `no_telemetry` file in the jcode dir

**An explicit disable always beats an explicit enable.** If a shell profile or
shared script sets `JCODE_TELEMETRY=1` on a machine where you opted out, you
stay opted out.

## Content sharing is separate

Sharing prompt and transcript *content* is a distinct, more sensitive consent
gated by its own marker file. It was already off by default upstream and remains
off here. Enabling anonymous telemetry does not enable content sharing.

## Invariants under test

In `crates/jcode-telemetry-core/src/tests.rs`:

- `telemetry_is_off_by_default` — the headline behavior. Fails if `is_enabled()`
  regresses to upstream's `true`.
- `opt_in_env_var_requires_an_affirmative_value` — `JCODE_TELEMETRY=0` is a no.
- `explicit_opt_out_overrides_opt_in` — precedence, across all three disable
  mechanisms against both enable mechanisms.
- `telemetry_endpoint_has_no_default` — no baked-in destination; blank is not
  "configured".
- `upstream_telemetry_host_is_absent_from_source` — a source-level guard so the
  upstream host cannot reappear as a fallback on a path behavioral tests miss.

Each was verified to fail when the behavior it guards is reverted.
