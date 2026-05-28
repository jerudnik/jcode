# Context Pipeline Evaluation Harness

This document describes the deterministic experiment harness introduced for TASK-79. It is intended to answer a practical question before we implement context-management features in the runtime agent:

> Which candidate techniques are cheap enough, safe enough, and effective enough to deserve native JCODE implementation?

The harness is deliberately modest. It does **not** claim publication-grade evaluation. It creates a repeatable baseline using synthetic scenarios plus optional local session/log samples, applies rough prototype transforms, and writes an evaluation matrix.

## Candidate techniques covered

`./scripts/context_pipeline_eval.py` currently prototypes:

- `baseline`: no transform.
- `stable_tiering`: wraps context blocks in status/trust-tagged XML-like blocks.
- `boundary_gate`: replaces oversized, binary, or minified inputs with placeholders.
- `tool_budget`: head/tail budgets large tool outputs and records restore handles.
- `duplicate_prune`: replaces older duplicate tool output with placeholders while keeping the latest.
- `trust_quarantine`: quarantines failed, speculative, or unverified context.
- `rust_skeleton`: rough Rust source skeletonization for oversized non-target files.
- `combined_p0`: combines the low-risk/high-ROI P0 candidates.

These are intentionally rough prototypes. They measure likely impact before runtime integration.

## Local quick start

From the repository root:

```bash
python3 scripts/context_pipeline_eval.py run-local
```

To include local JCODE log snippets from `~/.jcode/logs`:

```bash
python3 scripts/context_pipeline_eval.py run-local --include-local-sessions
```

Outputs are written under:

```text
target/context-eval/<timestamp>/
  scenarios/context_pipeline_baseline.json
  runs/<technique>.context.json
  matrix.json
  matrix.csv
```

The terminal table includes:

- estimated tokens saved,
- noise-reduction ratio,
- protected-term retention ratio,
- transform latency,
- a simple heuristic practical score.

## Remote run on `serious-callers-only`

The remote path uses non-interactive SSH and `rsync`. It avoids assuming a specific VM provider by accepting a host-side VM start command.

Basic remote run:

```bash
python3 scripts/context_pipeline_eval.py run-remote \
  --host serious-callers-only
```

With a host-side VM/provision command:

```bash
JCODE_CONTEXT_EVAL_VM_START_CMD='your-idempotent-vm-start-command-here' \
python3 scripts/context_pipeline_eval.py run-remote \
  --host serious-callers-only \
  --remote-dir /tmp/jcode-context-eval
```

Environment variables:

| Variable | Default | Purpose |
| --- | --- | --- |
| `JCODE_CONTEXT_EVAL_HOST` | `serious-callers-only` | SSH host. |
| `JCODE_CONTEXT_EVAL_REMOTE_DIR` | `/tmp/jcode-context-eval` | Remote staging directory. |
| `JCODE_CONTEXT_EVAL_VM_START_CMD` | empty | Optional idempotent command run on the SSH host before syncing/running. |

Safety boundaries:

- SSH uses `BatchMode=yes`, so the pipeline fails instead of prompting.
- Repository sync excludes `.git/` and `target/`.
- Remote staging defaults to `/tmp/jcode-context-eval`.
- The script does not mutate runtime JCODE state.
- Local session inclusion is opt-in with `--include-local-sessions`.

## How this maps to a VM-backed jcode experiment

The current script is the deterministic core. A full VM-backed experiment can wrap it in the following phases:

1. Start or reset an isolated VM on `serious-callers-only` using `JCODE_CONTEXT_EVAL_VM_START_CMD`.
2. Sync this checkout into the VM or into a host-side isolated worktree.
3. Build or reuse `jcode` inside the isolated environment.
4. Generate scenarios from synthetic fixtures, local session history, and public datasets.
5. Run prototype context transforms over the scenarios.
6. Optionally replay selected scenarios through a test jcode instance once runtime hooks exist.
7. Fetch `matrix.csv`, `matrix.json`, and transformed context artifacts.

Until runtime hooks exist, the harness evaluates the context payload transformations directly.

## Interpreting the matrix

Use the matrix as a triage tool, not a final proof.

Good candidates should show:

- high protected-term retention,
- meaningful token/noise reduction,
- very low transform latency,
- clear restore handles or placeholders for omitted content,
- simple implementation path and low persistent-state burden.

A technique should be deferred if it:

- saves tokens by dropping protected facts,
- requires heavy dependencies or model calls in the hot path,
- needs ambiguous persistent state migrations,
- produces unstable output ordering,
- cannot be snapshot or property tested.

## Suggested decision thresholds

Initial pragmatic thresholds:

| Criterion | P0 target |
| --- | --- |
| Protected-term retention | `>= 0.95` |
| Typical transform latency | `< 20ms` per provider context render fixture |
| Boundary check latency | `< 5ms` per file/tool output in microbenchmarks |
| Noise reduction on noisy scenarios | `>= 0.30` |
| Restore handle coverage for placeholders | `100%` |
| Snapshot determinism | stable across repeated runs |

## Next improvements

- Add real transcript parsers once stable session schemas are confirmed.
- Add public SWE-bench/Terminal-Bench style fixture ingestion.
- Add runtime jcode provider-payload capture mode for true replay experiments.
- Add implementation-burden fields to `matrix.csv` once prototype-to-runtime diffs are estimated.
- Add a small HTML/Markdown report generator for reviewer-friendly summaries.
