# G02-FIX-1: catalog path override for OpenAI-compatible providers

Perplexity's model catalog and its chat endpoint live at **different** routes.
Every other profile in the catalog serves both from one `api_base`. This node
adds a sparse catalog-path override table so the inversion can be *described*
rather than worked around, and proves with controls that the obvious
alternative (appending `/v1` to `api_base`) is wrong.

## The defect, re-derived with a real key

Earlier probes were unauthenticated, then used a bad bearer. Both are weaker
than they look: an unauthenticated 404 can be a missing-credential shape rather
than a routing fact. This run used a **real** `PERPLEXITY_API_KEY` (53 chars),
which upgrades the catalog evidence from "401, so the route exists" to "200,
here is the catalog".

Run 2026-08-05T08:24Z:

| route | status | body (truncated) |
|---|---|---|
| `GET https://api.perplexity.ai/models` | **404** | *(empty)* |
| `GET https://api.perplexity.ai/v1/models` | **200** | `{"object":"list","data":[{"id":…` |
| `POST https://api.perplexity.ai/chat/completions` | **400** | `{"error":{"message":"max_tokens must be at least 16",…` |
| `POST https://api.perplexity.ai/v1/chat/completions` | **404** | *(empty)* |

The 400 is a *semantic* validation error, which proves the route exists and
parsed the request body. No generation was issued: the probe deliberately sends
`max_tokens: 5` so the request is rejected on validation, satisfying the gate
without spending a Perplexity model call.

Reproduce:

```sh
curl -s -o /dev/null -w '%{http_code}\n' https://api.perplexity.ai/models \
  -H "Authorization: Bearer $PERPLEXITY_API_KEY"        # 404
curl -s -o /dev/null -w '%{http_code}\n' https://api.perplexity.ai/v1/models \
  -H "Authorization: Bearer $PERPLEXITY_API_KEY"        # 200
```

**The routes are inverted**: chat at the bare base, catalog under `/v1`. So a
single `api_base` cannot express both, and editing `api_base` trades one 404
for another.

## Scope, measured

Re-derived from `catalog.rs` rather than inherited from the node text:

- **36** `OpenAiCompatibleProfile` literals, all in `catalog.rs`. A naive grep
  finds 41 hits across three files; the extra 5 are the `pub struct` definition
  and `ResolvedOpenAiCompatibleProfile` occurrences.
- **6** profiles whose `api_base` does not end in `/v1`: `zai`, `gemini-api`,
  `deepseek`, `fpt`, `perplexity`, `deepinfra`. Only perplexity is inverted;
  the other five answer 200/400/401 with a bearer, never 404.
- **3** independent `{api_base}/models` construction sites.
- `api_base` is **not unique**: `openai-api` and `openai-compatible` both use
  `https://api.openai.com/v1` (35 distinct bases across 36 profiles).

## The fix, and the design that was abandoned first

The first implementation added `models_path: Option<&'static str>` as a field on
`OpenAiCompatibleProfile`. **That approach was abandoned**, and the reasoning is
recorded here because the failure was structural, not incidental.

The struct has no `Default` impl and no constructor macro, so one exception
forced an edit to **all 36** literals. Those 36 `models_path: None` lines pushed
`catalog.rs` from 1172 to 1213 LOC, past the 1200-line cap in
`scripts/check_code_size_budget.py`. The budget was **not** re-baselined. Fixing
that by splitting the file then moved `EXPECTED_FILE_COUNTS` 19 to 20, which
invalidated the `--expect-digest` pin in `fork-ci.yml`, which in turn is a
protected governance path requiring a transaction-bound maintenance procedure.
Three gates in three CI rounds, each fix creating the next collision.

The gates were right and the design was wrong. `catalog.rs` has **28 lines of
headroom** (1172 of 1200), and the minimal field version needs 36, so it could
never have fit. That is a fact about the file, not about how carefully the field
was written.

The shipped design is a **sparse override table** in `lib.rs`:

```rust
const CATALOG_PATH_OVERRIDES: [(&str, &str); 1] = [("https://api.perplexity.ai", "/v1/models")];

pub fn openai_compatible_models_url(api_base: &str) -> String
```

One exception costs one line. `catalog.rs` is **untouched** (`git diff` on it is
empty), which keeps the size ratchet, the file-count registry, the digest pin,
and Governance Root entirely out of scope. The abandoned work is preserved at
tag `g02f1-field-attempt` rather than deleted.

The lookup is keyed on **`api_base`**, not on the profile, because two of the
three call sites only ever hold a bare base string. `fetch_models_from_api`
takes an `api_base: String` and has five callers, none of which has the profile
in scope; keying on the base reaches it without widening that signature through
all five.

Because the key is not unique, a dedicated test requires profiles sharing an
`api_base` to agree on their override, so a future entry for one of a shared
pair cannot silently reroute its twin.

Call sites now routed through the resolver:

| site | before |
|---|---|
| `jcode-provider-openrouter-runtime/src/lib.rs` | `format!("{}/models", api_base)` |
| `jcode-provider-doctor/src/live_provider_probes.rs` | `format!("{}/models", resolved.api_base.trim_end_matches('/'))` |
| `jcode-base/src/usage/api_keys.rs` | `format!("{}/models", base)` |

The doctor reaches the resolver through `jcode_base::provider_catalog`, not
directly: that crate carries `jcode-provider-metadata` only as a
**dev-dependency**, so a direct call compiles under `cargo test` and fails the
library build.

## Controls

Every control was planted from a `cp` backup, **confirmed on disk** before its
exit code was read, then restored and verified byte-identical with `diff -q`.
Baseline: `cargo test -p jcode-provider-metadata` = **20 passed, 0 failed**.

| # | mutation | result | fails at |
|---|---|---|---|
| 1 | override table emptied (`; 0] = []`) | 3 failed | `perplexity_catalog_url_carries_the_v1_prefix` (lib.rs:740) |
| 2 | the **wrong fix**: `api_base` += `/v1`, override dropped | 3 failed | `perplexity_chat_base_must_not_carry_a_v1_prefix` (lib.rs:756) |

The two failure sets differ in exactly one member:

```
<     tests::perplexity_catalog_url_carries_the_v1_prefix
>     tests::perplexity_chat_base_must_not_carry_a_v1_prefix
```

**Control 2 is the load-bearing one.** Under the `api_base` edit, gate 1's test
still reports `... ok`, because `.../v1` + `/models` also yields
`/v1/models`. Gate 1 alone therefore cannot tell the correct fix from the
incorrect one; only the chat-base assertion distinguishes them. That is
precisely why the node demanded a gate written to fail on the `api_base` edit.

One control run was **voided and repeated**: the mutation script died with a
Python `SyntaxError` before writing, so the tree was unmutated and its exit 0
measured nothing. A green control run is not evidence unless the mutation is
confirmed present on disk first.

## Gate status

| gate | satisfied by |
|---|---|
| 1. control proves defect before fix | control 1 |
| 2. all three sites agree, fails if one reverts | `all_catalog_url_sites_route_through_the_resolver` (lib.rs:839) |
| 3. chat URL unchanged, fails on `api_base` edit | control 2 |
| 4. other 5 non-`/v1` profiles inert | `only_perplexity_overrides_its_catalog_path`, which names all five explicitly |
| 5. live re-probe with key, status + body | table above, run with a real key |

## Blast radius, settled mechanically

Two runs of the four-crate suite failed on tests unrelated to URLs
(`live_map_prunes_only_after_terminal_persistence`, then
`standard_openrouter_catalog_refresh_fires_when_named_profile_owns_slot`, both
of which document themselves as flaky under parallel execution). Clean main
passed 14 runs, including 6 in the **same working directory** with the changes
stashed, which rules out a directory or leftover-state confound.

Run counts alone could not settle 2-of-12 against 0-of-14, so the question was
answered mechanically instead. A temporary probe compared, for all 36 profiles,
the new resolver against the exact expression each call site used before:

```
differs=[("perplexity", "https://api.perplexity.ai/models",
                        "https://api.perplexity.ai/v1/models")]
```

Exactly one profile differs. The change is a provable no-op for the other 35, so
it cannot reach the openrouter or background tests. The probe was removed
afterward and all four files verified byte-identical by md5.

One further run exited **75**, which was neither pass nor fail: the remote build
host was unreachable. Those runs are excluded rather than counted as failures.

## Verification

- `cargo check --workspace --all-targets` exit 0. This is what caught the three
  call-site errors: the earlier "compiles and passes 20 tests" was scoped to the
  metadata crate alone, so the call sites had **never been compiled**. Two had
  wrong import paths and one passed a `String` where `&str` was required.
- `cargo clippy` on the four affected crates, `--all-targets --all-features`,
  exit 0.
- Four-crate test suite: **1229 passed**.
- All **15** guardrail steps from `fork-ci.yml`'s Quality Guardrails job, run
  locally, all exit 0. `check_warning_budget.sh` needs `nix develop` (outside it
  it exits 127 and correctly refuses to report a count it could not measure);
  inside, `current=0 baseline=0`.

## Known limits

- Gate 2's cross-crate agreement is asserted against the **source text** of the
  three sites, not by executing them. A unit test in this crate cannot call into
  three downstream crates, and the sites are network calls. So the test catches a
  site reverting to a bare `format!`, but would not catch a site rewritten to
  build the URL by some third mechanism.
- The `/v1/models` route was confirmed to return **200** with a real key, but the
  fetch path itself was not driven end to end against the live provider; no
  Perplexity model call was made.
