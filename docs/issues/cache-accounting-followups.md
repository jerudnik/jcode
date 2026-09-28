---
title: "Cache accounting: restored-history cost, estimate warnings and unknown ratios"
status: open
priority: medium
owner: unassigned
opened: 2026-09-28
related:
  - crates/jcode-tui/src/tui/app/misc_ui.rs
  - crates/jcode-tui/src/tui/app.rs
  - crates/jcode-tui/src/tui/app/state_ui.rs
  - crates/jcode-compaction-core/src/lib.rs
---

# Cache accounting: restored-history cost, estimate warnings and unknown ratios

The per-request cache accounting port (upstream 8343408df, 8ef938219,
c0be37ce1, db75c8013) landed with the contract's core invariants covered:
resolved `prompt_tokens` per request, `cache_prompt_tokens` totals with
zip semantics, snapshot-difference cost accrual, and a compaction feed that
no longer drops fully cached Anthropic prompts. The independent review
(gpt-6-astra, 2026-09-28) left four items that are real but outside that
port's scope. None is a regression from before the port.

## 1. Restored history is priced from raw aggregates with the current model

`App::seed_cost_from_history_totals` (`misc_ui.rs`) prices
`TokenUsageTotals.input_tokens / cache_read / cache_creation` with the
pricing of the provider and model active *now*. For a mixed history (one
OpenAI request `input 10000, read 6000, write 2000` plus one Anthropic
request `input 1000, read 7000, write 2000`) the aggregates are
`input 11000, read 13000, write 4000`. Subset pricing computes fresh input
as `11000 - 13000 - 4000 = 0` instead of the true `2000 + 1000 = 3000`, and
the original providers' rates and TTL premiums are lost. The stored
`cache_prompt_tokens = 20000` is ignored because a single aggregate cannot
be partitioned per provider.

Fix direction: persist a per-request cost (or per-provider sub-totals) at
record time, or keep per-message `StoredTokenUsage` plus provider identity
and reprice by walking the history on restore. Do not add
`cache_prompt_tokens` into the existing formula; it does not partition.

## 2. Estimated TTLs still produce definitive expiry warnings

`cache_ttl_is_estimate` is consulted only by `/cache stats`. The KV-cache
warnings in `app.rs` (around `Prompt cache went cold` and the
`/cache to extend` hint) treat an OpenAI estimate as a hard expiry, and the
`/cache` extend hint is Anthropic-only advice shown on OpenAI routes.
Upstream b3a005e41 and f9185a14e cover this; they were not in the reviewed
port set.

## 3. Write-only telemetry shows a 0% session read ratio

When a request reports `cache_creation` but no `cache_read`
(`last_cache_read_tokens = None`), `record_completed_stream_cache_usage`
adds 0 to the session read total while the denominator is known, so
`/cache stats` prints `0%` for the session even though the last read is
unknown. The per-request `last_ratio()` correctly reports `None` and the
miss alarm is correctly suppressed (`app.rs`, explicit-read guard). Decide
whether the session ratio should also read `unknown` when any contributing
request lacked a read count.

## 4. Compaction keeps a name heuristic for third-party providers

`effective_context_tokens_from_usage` in `jcode-compaction-core` resolves
OpenAI and Anthropic by name and falls back to
`cache_creation > 0 || cache_read > input` for everyone else. OpenRouter
and Gemini report reads as subsets, and an OpenRouter write count for an
OpenAI-backed model would flip them to split accounting and over-count
the budget. Custom `openai-compatible` profile names that do not contain
`openai` take the same path. The contract asked for the fallback to label
itself as legacy/unknown; today it is stored as a resolved `prompt_tokens`.

## Evidence

- Review report: swarm session `session_lizard_1790602372078_3716387460964f05`,
  findings 3, 5, 6 and 8.
- Contract: `~/.jcode/recovery/2026-09-28/artifact-cache-contract.md`
  sections 3 (item 3), 5 and 7 (C6).
