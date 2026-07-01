# Cloudflare Experiment Strategy After Upstream Sync

Date: 2026-06-16
Historical base: former `nix-flake` branch after syncing upstream through v0.28.0. Current branch model uses `distro/nix` for packaging and `main` for custom work.
Historical source branch reviewed: former `origin/nix-flake-dev`.

## Summary

Do not merge the former `nix-flake-dev` work wholesale. It is based on the old pre-sync fork base and is now hundreds of upstream commits behind the stable custom fork branch. The Cloudflare work contains useful experiments, but the latest upstream memory/retrieval stack changes the baseline: hybrid dense+BM25 recall, listwise LLM reranking, ONNX/cross-encoder work, and cadence gating already improve the latency/quality tradeoff locally.

The prior host experiments found Cloudflare service latency creates a hard ceiling for latency-sensitive work. Treat Cloudflare as a durability, sharing, observability, and batch/offline substrate, not as the hot path for interactive memory recall or prompt-turn decisions unless measurements prove otherwise.

## Salvage first

1. **Local-first artifact spill and retrieval**
   - Relevant commits: `fc053a26`, `db07c267`, later artifact-spill eval commits.
   - Candidate files from dev branch: `remote/artifact_store.rs`, `tool/experimental/artifact_get.rs`, `tool/experimental/artifact_spill.rs`, R2 artifact worker/docs/tests.
   - Rationale: large artifacts are latency-tolerant enough when accessed by handle, and this can reduce transcript bloat.
   - Rule: local-first must remain the default. Remote spill should be opt-in or only for artifacts above a size/value threshold.

2. **Evaluation metrics and replay harnesses**
   - Relevant commits: `f7edf423`, `80e0c009`, `85f2b870`, `0305d956`.
   - Rationale: needed to prove whether Cloudflare artifact/cache paths beat current local behavior.
   - Keep code quality at upstream level: deterministic tests, no global metrics pollution, no always-on telemetry.

3. **Cloudflare cache worker as optional latency-tolerant cache**
   - Relevant commits: `f1e0cd4a`, `418e56ea`, `723de77f`, `d61287c7`, `4442b5aa`.
   - Rationale: useful for repeated web/tool fetches if cache hit rate and freshness are measurable.
   - Constraint: never block interactive turns on a cold remote cache path if local/direct fetch is comparable or faster.

## Defer or re-prove

1. **Latency-sensitive memory recall/rerank outsourcing**
   - Defer until it beats current upstream local hybrid recall plus cadence-gated reranking.
   - Required proof: p50/p95 wall-clock latency, answer quality, token cost, and failure fallback behavior against current `memory_recall_bench` baselines.

2. **Durable lesson library / lesson shadow telemetry**
   - Interesting but speculative.
   - Re-prove after artifact and eval harnesses are on current upstream.
   - Keep shadow telemetry off by default and make cross-repo discovery explicit.

3. **Queue/Vectorize/D1 control-plane guardrails**
   - Keep as design/reference material until there is a concrete production feature needing them.
   - Avoid adding operational surface area before product value is proven.

## Discard or avoid for now

- Any Cloudflare feature that adds turn-time network round trips without a local fallback.
- Any branch code that lowers upstream code quality: broad globals, hidden telemetry, network-dependent unit tests, hard-coded account/resource names, or unclear ownership boundaries.
- Blind rebase of all 71 former `nix-flake-dev` commits.

## Porting order

1. Create a fresh branch from current `main`.
2. Cherry-pick docs/eval scaffolding first, not runtime hooks.
3. Port artifact store/spill/get behind explicit config and local-first defaults.
4. Add benchmarks comparing:
   - current upstream local memory path,
   - local artifact spill,
   - R2-backed artifact spill,
   - direct Cloudflare cache hit/miss.
5. Only then port web cache / durable lessons if the numbers justify it.

## Minimum validation for any salvaged Cloudflare chunk

- Unit tests with mocked Cloudflare responses.
- One live smoke mode gated by explicit env vars/secrets, skipped by default.
- p50/p95 latency and failure-rate report against current upstream baseline.
- Fallback proof: remote failure must not break the local interactive path.
- `cargo fmt`/focused tests/checks on touched crates.

## Decision rule

A Cloudflare-backed feature should survive only if it is one of:

- **Latency-tolerant and value-positive**, such as artifact durability, large payload offload, async eval storage, or cross-device sharing.
- **Latency-sensitive but empirically faster/better** than current local upstream behavior at p95, including network failures and cold-starts.

Otherwise, keep it as research documentation, not product code.
