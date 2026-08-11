# Terminal-Bench 2.0 with jcode

This document describes the cleanest currently-working path for running jcode on Terminal-Bench 2.0 through Harbor.

## Local performance trend benchmarks

These benchmarks are for local trend data, scheduled observation, and release notes. They are intentionally nonblocking until enough same-host history exists, because startup, compile, terminal, and memory numbers vary heavily by machine load, terminal stack, filesystem cache, and installed helper tools.

Keep each run's raw output machine-readable and small. Store JSON artifacts as build logs or local files, not as required CI checks.

| Area | Stable command | Primary trend metric | Suggested nonblocking regression range |
| --- | --- | --- | --- |
| Startup | `python3 scripts/bench_startup.py ./target/release/jcode --runs 5 --json-out /var/tmp/jcode-startup-bench.json` | `metrics.cold_total.median_ms`, plus `metrics.server_ready.median_ms` | Warn when same-host median grows by more than 25% or 100 ms, whichever is larger, for two consecutive scheduled runs. |
| Warm compile | `scripts/bench_compile.sh check --runs 5 --edit src/main.rs --json` | `median_seconds` | Warn when same-host median grows by more than 20% or 5 s, whichever is larger, for two consecutive scheduled runs. |
| TUI visible/input delay | `python3 scripts/bench_startup_visible_ready.py --tools jcode --runs 10 --json-out /var/tmp/jcode-visible-ready-bench.json` | `tools.jcode.first_visible_summary.median_ms` and `tools.jcode.input_ready_summary.median_ms` | Warn when same-host median grows by more than 25% or 250 ms, whichever is larger. Keep failures local if terminal emulation differs between hosts. |
| Memory | `python3 scripts/bench_memory_cli.py --tools jcode_memory_off jcode_memory_on --sessions 1 --json-out /var/tmp/jcode-memory-bench.json` | `results[].pss_mb` | Warn when same-host PSS grows by more than 20% or 100 MB, whichever is larger, after confirming no model cache or auth-state change. |

Use the lower-cost commands for scheduled collection and rerun manually with higher `--runs` when a trend looks suspicious. Do not make any of these blocking gates until the range has been checked against several runs on the same worker class.

## What is in the repo

- `scripts/jcode_harbor_agent.py`
  - Harbor custom agent adapter for jcode
- `scripts/run_terminal_bench_harbor.sh`
  - helper that wires Harbor to the adapter and a Linux-compatible jcode binary
- `scripts/run_terminal_bench_campaign.py`
  - sequential campaign runner that preserves small batches in a stitchable layout
- `scripts/build_linux_compat.sh`
  - builds a Linux jcode artifact against an older glibc baseline for TB-style containers

## Why the compat binary matters

Many Terminal-Bench task containers use an older glibc than a locally-built host binary. The Harbor adapter should use a Linux binary produced by:

```bash
scripts/build_linux_compat.sh /tmp/jcode-compat-dist
```

The helper script will build it for you automatically if it is missing.

## Auth and model assumptions

The current adapter is designed for:

- OpenAI OAuth auth file at `~/.jcode/openai-auth.json`
- `gpt-5.4`
- high reasoning effort
- priority service tier

Those defaults can be overridden with environment variables.

## Sequential campaign mode

If you want to run only a few tasks at a time but keep a coherent artifact set, use the campaign runner.

Example:

```bash
python scripts/run_terminal_bench_campaign.py \
  --campaign-dir ~/tb2-jcode-campaign \
  --task regex-log \
  --task largest-eigenval \
  --task cancel-async-tasks
```

What it does:

- runs tasks sequentially with `--n-concurrent 1`
- preserves Harbor jobs under `campaign-dir/harbor-jobs/`
- writes a pinned `campaign.json`
- refuses to mix runs if key settings drift
- appends per-task outcomes to `results.jsonl`

This is the recommended path when you want to batch tasks gradually and stitch them together later.

## Quick start

Assuming Terminal-Bench is already available at `/tmp/terminal-bench-2`:

```bash
scripts/run_terminal_bench_harbor.sh \
  --include-task-name regex-log \
  --n-tasks 1 \
  --n-concurrent 1 \
  --jobs-dir /tmp/jcode-tb2 \
  --job-name regex-log-pilot \
  --yes
```

Or point Harbor directly at the remote dataset:

```bash
scripts/run_terminal_bench_harbor.sh \
  --dataset terminal-bench@2.0 \
  --include-task-name regex-log \
  --n-tasks 1 \
  --n-concurrent 1 \
  --jobs-dir /tmp/jcode-tb2 \
  --job-name regex-log-pilot \
  --yes
```

## Useful environment variables

- `JCODE_HARBOR_BINARY`
  - path to the Linux-compatible jcode binary to upload into the task container
- `JCODE_HARBOR_BINARY_DIR`
  - output directory used when auto-building the compat binary
- `JCODE_HARBOR_OPENAI_AUTH`
  - path to the OpenAI OAuth file
- `JCODE_HARBOR_CA_BUNDLE`
  - optional host CA bundle path to upload into the task container
- `JCODE_TB_MODEL`
  - Harbor model string, default `openai/gpt-5.4`
- `JCODE_TB_PATH`
  - default local Terminal-Bench path, default `/tmp/terminal-bench-2`
- `JCODE_OPENAI_REASONING_EFFORT`
  - default `high`
- `JCODE_OPENAI_SERVICE_TIER`
  - default `priority`

## Notes on fairness and state isolation

The adapter gives each trial a fresh in-container jcode home directory under `/tmp/jcode-home`, so memories and auth state are isolated per trial container.

## Current validation status

This path has already been validated with real Harbor task runs using:

- `regex-log`
- `largest-eigenval`
- `cancel-async-tasks`

All three passed in-container with verifier reward `1.0` during the initial pilot.
