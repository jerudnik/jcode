# R02 Configuration, auth readiness, provider/model entitlement, and routing: authoritative ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `7ff4fc6be8dcf0410f2f61994752fdf5ee93e6e4`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `full` |
| Research budget | `8 decisive checkpoints; 8 consumed` |
| Authority today | `split` |
| Recommended disposition | `compose` |
| Pilot entry verdict | `blocked` |
| Confidence | `medium-high` for fork provenance/route behavior and the stale-tier defect; `medium` for the final catalog contract; `medium-low` for server usage admission |
| Last updated | `2026-07-15 UTC` |

## Preserved independent reviews

| Review | External artifact | SHA-256 | Repository copy | Copy result |
|---|---|---|---|---|
| Opus | `/tmp/jcode-r02-opus-review.md` | `122b647b2cbd39d64c43d4dc07a89edf83d926a03ab175318945c9b32d51be8f` | [`opus-review.md`](./opus-review.md) | byte-identical (`cmp -s`) |
| Grok | `/tmp/jcode-r02-grok-review.md` | `8a6c18e5f99862e92155160da44ae9c2e0ef3f0b00320a573b546df2fa7d4312` | [`grok-review.md`](./grok-review.md) | byte-identical (`cmp -s`) |

The two reviews are preserved verbatim. Their reviewed `8848f2d54...` head was the behavior baseline plus Phase 1 documentation. This ledger was adjudicated from the fixed fork baseline and authored at requested seam head `f5a8999d81311d237d1c106a9d980fd86fa34b6e`. The source tree was read-only. No network, credentials, payment, stash replay, publication, or destructive operation was used.

## Scope and invariants

- **Owns:** layered configuration provenance; credential *references* and readiness; account/provider/model selection; configured sidecar route selection; route result; local subscription-tier model admission; `/v1/me` tier freshness and offline fallback; and usage data classification.
- **Excludes:** R12 agent-turn execution and request/result persistence, R03A wire verdicts, R06 durable evidence, and R13 context/compaction policy. It neither establishes product pricing nor invents a server-side billing policy.
- **Must preserve:** fork config provenance and policy-over-durable layering; explicit provider/model/profile route identity over ambient state; credentials never emitted as observables; an exact R02-to-R12 provider outcome; and server authority for actual router/budget admission.
- **Binding overlays:** R00 fixes refs and forbids stash replay or implicit upstream authority. R09 forbids `--update` and makes R02 own R02-path debt. R11 requires append-only, hashed external evidence and leaves the user-controlled prompt edit untouched.

## Divergence at a glance

| Concern | Fork | Upstream | Consequence |
|---|---|---|---|
| Config provenance and credential refresh | `ConfigProvenance`, policy-over-durable merge, and explicit config-file-only auth refresh | No equivalent provenance system found | **Fork authoritative. Retain.** |
| Selected route identity | Resolves provider/profile specs and persists provider key plus route API method; sidecars fork the configured spec | Narrower configured sidecar handling | **Fork authoritative. Retain.** |
| OpenRouter catalog fallback | Provider-key-aware resolution and primary credential source | Suppresses fallbacks when OpenRouter definitively lacks a model | **Two-sided. Compose both predicates.** |
| Tier catalog | Two tiers: Plus and Flagship. Unknown wire tier parses to `None` then defaults to Plus | Five tiers and changed pricing/budget/model-floor constants | Upstream is a **candidate schema**, not product authority. Do not import its business constants blindly. |
| Live tier refresh | Persists only `Some(parsed_tier)` | Expanded known set, but review reports the same unknown-tier non-clear shape | A successful unknown live response can preserve cached Flagship. **Pilot blocker.** |
| Usage and budget | Parsed and displayed; no local usage admission consumer | No authority established by this review | Router admission is **server-authoritative**. Do not claim local over-budget admission. |

## Eight-checkpoint evidence ledger

All commands below were read-only and run against the fixed refs or current checkout. A failed early combined `git rev-parse` invocation was corrected before the eight counts below, and is not evidence.

| # | Finding | Evidence and reproduction | Confidence | Decides |
|---:|---|---|---|---|
| 1 | Comparison refs are reproducible | `git rev-parse --verify 7ff4fc6be^{commit}`; same for `802f690982` and `631935dd1`; `git merge-base 7ff4fc6be 802f690982` returned `631935dd1...` | H | Fixed comparison authority |
| 2 | Fork’s two-tier parser under-entitles recognized newer tiers when no stronger cache exists | Fork `subscription_catalog.rs:15-58,163-177`: only Plus/Flagship parse; `effective_tier()` falls back to Plus. `cargo test -p jcode-base --lib subscription_catalog::tests` passed with no secrets. | H | Fork tier catalog cannot be accepted unchanged |
| 3 | Upstream carries a five-tier ordering and pricing/budget/model-floor policy | `git show 802f690982:crates/jcode-base/src/subscription_catalog.rs` inspected at lines 10-100 and 360-390: Pro/Max/Ultra plus price/budget and `model_entitlements_match_paid_tiers` | H for code difference, M for product truth | Catalog requires an explicit policy decision, not mechanical adoption |
| 4 | Successful `/v1/me` unknown tier does not clear stale cached entitlement | Fork `subscription_api.rs:76-87` calls `store_cached_tier` only under `if let Some(tier)`; test at `:118-129` accepts `mystery` as `None`. Thus cached Flagship can survive a successful unknown response. | H | Must fail closed or clear cache before pilot |
| 5 | Local tier checks have more than one observable consumer and one authoritative admission path is not yet proved | `rg -n -e 'tier\\.allows' -e '\\.min_tier' -e 'effective_tier\\(' crates --glob '*.rs'` found catalog helper, `provider/models.rs:191-192`, display filtering, and TUI rendering. | M | Require a single invariant test covering setter and display/route filtering; do not merely assert uniqueness |
| 6 | `catalog_routes.rs` is a genuine two-sided fallback collision | Base-to-fork diff adds `resolve_current_model_spec` and `ApiKeyCredentialSource::primary_only`; base-to-upstream diff guards fallback with `standard_catalog_lists_model(...) != Some(false)` for Claude/OpenAI routes. | H | Compose, not pick one side |
| 7 | Fork provenance and credential-source behavior have no upstream substitute | `ConfigProvenance`, `build_layer_metadata`, `merge_policy_over_durable`, `provenance_for`, and `CONFIG_LAYER` occur in fork config paths; provider-env documents process-env-first and config-file-only auth-change loading. | H | Retain fork and expose/contain ambient env influence |
| 8 | Usage fields do not locally gate model admission | `rg -n -e 'used_usd' -e 'budget_usd' crates --glob '*.rs'` locates parsing/display/constants; `provider/models.rs:150-210` gates curated membership and effective tier only. | M | Server/router is authoritative for usage/budget admission |

### Negative findings and bounded gaps

- No upstream patch-ID equivalence was claimed. R00 treats `b3ed82a6b` as an ancestry gap, so file similarity or upstream provenance is not adoption evidence.
- No test asserts `cached Flagship + successful unknown /v1/me -> no Flagship admission`; the present test instead accepts an unknown tier parse. This is the blocking missing test.
- No live `/v1/me`, real credential, router request, payment, or server budget denial was exercised. Those are intentionally outside this seam review.
- No product source confirmed the upstream `Pro`, `Max`, `Ultra` names, prices, budgets, or the changed `gpt-5.6-sol` floor. Upstream is not authority for those facts.
- The exhaustive semantics of the large sidecar body and every possible tier consumer were not re-proved here. Sidecars remain out of the pilot unless their configured route observables are explicitly enabled.
- Hot-path stashes remain preserved evidence only: `stash@{0}` account failover, `stash@{1}` TUI/config callers, `stash@{2}` config warn-once plus sidecar dedup, and `stash@{3}` pre-sync work. None was applied, replayed, or converted into open R02 work.

## Adjudication

| Disagreement | Opus position | Grok position | Terra resolution | Deciding evidence |
|---|---|---|---|---|
| Overall disposition | Retain fork broadly, adopt upstream tier ladder under product and enforcement prerequisites, compose routes | Compose and block pilot | **Compose, blocked.** Retain fork route/provenance; compose route fallback; defer catalog policy import until product truth and safe freshness are implemented. | Checkpoints 2-8 |
| Five-tier catalog | Adopt upstream after product confirmation | Reconcile after policy and fixtures | Treat upstream as a bounded candidate, not pricing/business truth. Accept no price, budget, or model floor until a product-owned fixture/decision supplies it. | Checkpoint 3 |
| Unknown live tier | Identified Plus under-entitlement from parse failure | Identified stale cached Flagship over-entitlement | Both are real. A live refresh must never retain a stronger cached tier on unknown input. The safe behavior is explicit denial or cache clear to a documented lowest safe tier. | Checkpoints 2 and 4 |
| `catalog_routes` fallback | Compose both sides | Retain fork identity, adopt suppression | Compose provider/profile-aware resolution and primary credential source with upstream’s definitive-catalog suppression. | Checkpoint 6 |
| Usage admission | Did not make it a local policy | Server-authoritative unless a local policy is added | **Server-authoritative today.** R02 reports usage but cannot locally approve budgeted requests. | Checkpoint 8 |
| Hot-path stashes | Preserved evidence, not work | Preserved evidence, not work | Retain as evidence only. A current hot-path regression is a new bounded issue, never a reason to replay a stash. | R00/R02 review evidence and stash list |

**Terra reproduction:** checkpoints 1-8 above were rerun locally. The decisive result is checkpoint 4: a successful `/v1/me` response with an unrecognized tier takes the `None` branch and leaves a cached Flagship value untouched. This is not corrected by merely adding known tiers, because future unknown values retain the same unsafe behavior.

## Authority today and pilot entry verdict

| Decision surface | Authority today | Rule |
|---|---|---|
| File-layer provenance, credential reference refresh, profile/provider/model resolution, route identity, configured sidecar routing | Fork | Retain the fork implementation and tests. Do not replace it with upstream’s narrower behavior. |
| `/v1/me` reported account tier | Server response, subject to explicit local freshness handling | A known, fixture-accepted response may update cache. An unknown, malformed, or contradictory successful response must not retain a stronger cached entitlement. |
| Tier names, prices, budgets, and curated-model floors | `unclear` pending product-owned fixture/decision | Upstream code is evidence only. No automatic import. |
| OpenRouter availability fallback | Split | Compose fork route identity/credential-source behavior with upstream definitive-catalog suppression. |
| Usage/budget admission | Server/router | Local code may display usage and local tier policy, but only router success/error is an over-budget admission verdict. |

**Pilot entry verdict: BLOCKED.** The seam may enter the Phase 3 fixture pilot only after all of the following are green:

1. A deterministic, no-secret `/v1/me` fixture explicitly classifies each accepted tier and raw unknown/absent/malformed tier. With cached Flagship, an unknown live tier must clear/downgrade safely or explicitly deny Flagship-only admission. A stale cached value may not win.
2. A product-owned decision fixes the accepted tier set, labels, prices/budgets if displayed, and curated model minimum tiers. This decision may adopt some upstream values, but the recorded fixture is the authority, not the upstream commit.
3. Fixture tests prove local admission consistently for the selected route and its displayed models. The `set_model` and picker/filter outcomes must agree for every accepted tier.
4. Provider-affecting process-env overrides are either absent from the fixture or recorded as an explicit ambient source. Existing file provenance alone cannot claim env provenance.
5. Fixture-backed `/v1/me` success, not key presence, establishes auth readiness. Credential observables are boolean/reference/source category only, never values.
6. R12 and R13 dependencies below are accepted, while R09’s trusted green gates remain green and its red debt remains visible without `--update`.

## Deterministic no-secret fixture contract and observables

The fixture must use temporary config/home paths, symbolic credentials such as `fixture-key`, and an in-process or local mocked `/v1/me` response. It must not call the network, read user config, log a credential, create payment state, or start the user daemon.

| Fixture test | Required observable and assertion |
|---|---|
| Config layering | Durable, policy, default, and explicitly allowed env values produce `Config::provenance_for` results for selected provider/model keys. Policy wins durable; an ambient env override is absent or recorded as ambient rather than misreported as file provenance. |
| Auth readiness and source | Only `configured`/`missing`, source category (`process-env`, `saved-file`, or explicit auth-change file-only), resolved router base presence, and fixture `/v1/me` status are exposed. No key text or header value is captured. |
| Route identity | Declared provider/model spec, resolved model spec, account label/source, `provider.name()`, `provider.model()`, `session.provider_key`, `session.route_api_method`, route API method, and credential-source category agree before and after restore. |
| Catalog fallback | A prefixed/profile model preserves fork provider identity and source selection. A model definitively absent from OpenRouter has no fallback route. Unknown availability retains the intended fallback behavior. |
| Tier freshness | For every accepted known tier, raw tier, parsed tier, freshness source (`live`, `cache`, `default`, or `unknown-denied`), effective tier, selected-model admission, and displayed-model membership agree. No-cache offline defaults are explicit. |
| Stale-cache regression | Seed cached Flagship, return a successful unknown `/v1/me` fixture, then assert cached Flagship cannot admit a Flagship-only model. This is mandatory and currently fails by absence. |
| Usage classification | Fixture `used_usd`, `budget_usd`, and reset time are displayed/classified, but no local code reports an over-budget allow. A separate mocked router denial is recorded as a **server** verdict, not local tier admission. |
| R12 handoff | The request/result record contains exactly the route identity selected above: account/provider/model/entitlement/route/API method. R02 does not create a competing identity. |

## Recommendation

- **Disposition:** `compose`.
- **Why:** The fork is superior and authoritative for explainable configuration, credential-reference refresh, provider/profile identity, and sidecar route preservation. Upstream supplies a useful candidate for richer tier parsing and an independent OpenRouter fallback-suppression rule. Neither upstream pricing nor its model-floor choices are adopted without product evidence. The shared stale-unknown cache defect blocks either catalog from a pilot.
- **Cross-seam dependencies:** R00 preservation/fixed refs and stop budgets; R09 gates and R02 debt visibility; R11 append-only evidence; R12 exact request/result route identity; R13 all provider-session invalidation writers and a no-compaction/joint-invalidation proof; R03A carries R01/R02 identity only after both authorities approve. R06A must round-trip the fixture evidence before the Phase 3 pilot.
- **Upstream opportunity:** a narrowly composed `catalog_routes` patch and, only after product decision, a catalog schema/policy slice. Upstream tier constants are not a wholesale sync target.
- **Quality-of-life ideas:** sidecar warning/log dedup belongs in a separate evidence-backed lane. The preserved stashes are not authorization to implement it.

## R09 debt ownership

R09 remains binding. These current red entries are assigned to R02 by behavior/path and must stay visible until separately remediated. This documentation-only commit changes none of them.

| Gate | R02-owned current entry | Required handling |
|---|---|---|
| Panic | `crates/jcode-base/src/config.rs` +1; `config/config_file.rs` +2 | Any R02 implementation touching these paths must lower or keep counts and run the ratchet. No `--update`. |
| Swallowed error | `config.rs` 2 -> 5; `config/config_file.rs` 3 -> 8; `provider/openrouter.rs` 6 -> 7; `provider_catalog.rs` 7 -> 8; `sidecar.rs` 2 -> 3 | Treat as R02 behavior debt, preserve attribution, and split real remediation from sync/fix work. |
| Production size | `provider/catalog_routes.rs` 1296 -> 1378 LOC; `provider/mod.rs` 2700 -> 2797; `sidecar.rs` 1299 -> 2235 | No growth without an explicit bounded refactor. |
| Test size | `config_tests.rs` 1504 LOC; `provider/tests/model_resolution.rs` 2233 -> 2414 | New fixture coverage must be isolated or reduce existing structure. |

## Bounded implementation slices

No implementation slice is authorized by this docs-only ledger. If the coordinator opens one, it must remain within the following boundaries and stop rather than broaden.

| Slice | Class | Change | Acceptance | Rollback or stop condition |
|---|---|---|---|---|
| 1 | `fix` | Make successful unknown/malformed live tier clear cached entitlement or fail closed; add stale-Flagship regression fixture. | The stale-cache fixture cannot admit Flagship-only models, and known tiers retain documented behavior. | Stop if safe fallback needs a product policy not recorded in the fixture, changes server protocol, needs credentials/network, or alters unrelated account persistence. Revert the isolated commit. |
| 2 | `sync` | Add only product-approved accepted tier schema, labels, display constants, and curated floors, potentially drawing on upstream. | One fixture table covers every accepted wire tier and `set_model` plus display filtering agree. | Stop if pricing/budget/floor product truth is absent, upstream and fixture differ, or more than the catalog and targeted tests must change. Do not import upstream wholesale. |
| 3 | `refactor` | Compose `catalog_routes` provider/profile-aware resolution and primary-only credentials with definitive OpenRouter fallback suppression; factor shared admission assertion only if needed to test consistency. | Route matrix proves provider identity survives and definitive absence suppresses fallback without changing unknown-availability behavior. | Stop on any route identity drift, credential-value exposure, changed auth precedence, or a conflict requiring broad provider rewrite. Revert the isolated commit. |
| 4 | `docs` | Publish the no-secret fixture matrix, source classifications, route/tier observables, server-authoritative usage statement, and R12/R13 handoff evidence. | Exact fixture output has no secret, every observable above is present, and hashes/links reproduce. | Stop if documentation would claim router success without a fixture result or requires changing R12/R13-owned records. Amend rather than overwrite. |

## Validation and sign-off

- **Commands run:** fixed-ref verification and merge base; fixed-ref `subscription_catalog` and `subscription_api` inspection; catalog-route two-sided diffs; source grep for tier consumers/provenance/credentials/usage; `nix develop --command cargo test -p jcode-base --lib subscription_catalog::tests`; `nix develop --command cargo test -p jcode-base --lib subscription_api::tests`; R09 ratchets observed red without `--update`; review SHA-256 and `cmp -s` preservation.
- **Verbatim-review whitespace:** default `git diff --check` reports exactly five trailing two-space Markdown hard breaks in copied Grok review lines 3-7. They are source bytes required by the verbatim-copy rule, not ledger whitespace. `git -c core.whitespace=-blank-at-eol diff --cached --check` is clean; hash and `cmp -s` prove the copy was not normalized.
- **Targeted test result:** both no-secret test commands exited `0`. The test run validates existing parser/catalog/API tests, not the absent stale-Flagship unknown-live-tier regression.
- **Failure modes checked:** unknown-to-Plus under-entitlement; unknown-live stale-Flagship over-entitlement; accidental whole-upstream pricing import; fallback suppression lost by choosing one side; ambient env replacing declared route; local usage mistaken for server admission; stash replay; and hidden R09 rebaseline.
- **Remaining risks:** product catalog truth, server response semantics, R12 request/result correlation, R13 invalidation writers, and sidecar behavior when explicitly exercised.
- **Opus review:** `pass as evidence`, SHA-256 recorded above. Its conditional adopt-upstream recommendation is narrowed by the product-authority and stale-cache blocker.
- **Grok review:** `pass as evidence`, SHA-256 recorded above. Its stale-cache blocker and server-authoritative usage classification are accepted.
- **Terra adjudication:** `pass with pilot blocked`.

- **Sol sign-off:** `pass` as an integration-ready adjudication document with the pilot blockers preserved; see [`2026-07-15-r01-r02-sol-signoff.md`](../../reviews/2026-07-15-r01-r02-sol-signoff.md), SHA-256 `84943fd4bc97c1a69ee8e63b7f2df1e05b27f447132618eeb73f06d800a6acdb`.
- **Fable sign-off:** `pass` as an integration-ready adjudication document with the pilot blockers preserved; see [`2026-07-15-r01-r02-fable-signoff.md`](../../reviews/2026-07-15-r01-r02-fable-signoff.md), SHA-256 `942d282e3245e7493e7dd2c0d816e72982d1d1fc0ebdaa2dfa920fb60c7bf32b`.

## 2026-07-15 R02 stale-tier pilot follow-up

Implementation commit: `4f104a609 fix: fail closed stale jcode subscription tier`.

This follow-up resolves the pilot blocker without importing upstream business constants. The product-owned entitlement contract remains intentionally narrow:

- Accepted live `/v1/me` tier wire values: `plus`, `flagship`.
- Accepted labels and displayed plan constants remain the fork-owned `Plus` and `Flagship` fixture values already present in `subscription_catalog.rs`.
- Upstream `Pro`, `Max`, `Ultra`, pricing, budgets, and model-floor changes remain evidence-only and are not mechanically adopted.
- `JCODE_TIER` is no longer an ambient process-env entitlement source for local admission. Tier cache reads use the saved jcode subscription env file only, so a shell-level `JCODE_TIER=flagship` cannot unlock or resurrect Flagship admission.

Fail-closed behavior added:

- A successful live `/v1/me` response with an accepted active tier stores that tier as live truth.
- Unknown, absent, blank, non-string, malformed JSON, or inactive/contradictory live tier truth clears cached tier state and records `unknown-denied` behavior.
- After any denied live truth, the effective local admission tier is the lowest safe tier (`Plus`), and Flagship-only models such as `claude-fable-5` and `gpt-5.6-sol` are denied.
- Jcode auth readiness now becomes `RequestValid` only after the local `/v1/me` fixture returns accepted entitlement; key presence alone remains `CredentialPresent`.

Deterministic no-secret fixture coverage added:

- In-process localhost HTTP `/v1/me` fixtures for both accepted tiers.
- Unknown (`pro`), absent tier, non-string tier, inactive Flagship contradiction, and malformed JSON stale-Flagship regressions.
- Ambient process env override regression: `JCODE_TIER=flagship` does not grant entitlement and cannot survive denied live truth.
- Admission/display coherence: `set_model` guard and picker/display filtering agree for every accepted tier, and canonical route identity ignores provider suffixes for curated model matching.

Validation run before this ledger append:

```text
scripts/dev_cargo.sh fmt --all
scripts/dev_cargo.sh test -p jcode-base subscription_ -- --nocapture
scripts/dev_cargo.sh test -p jcode-base test_subscription_ -- --nocapture
```

Result: `32` subscription-filtered tests passed and `4` provider subscription guard tests passed. The first attempted focused test command used multiple cargo test filters and failed before compiling; it was corrected with the valid filters above.

## 2026-07-15 reviewer-B amendment: cache-clear failure remains fail-closed

Implementation commit: `760dcae11 fix: deny stale tier when cache clear fails`.

Independent reviewer B found that the malformed JSON path used a best-effort `store_cached_tier(None)` and could ignore a durable clear failure. That would let an old Flagship cache remain admitted after a malformed live response if the config write/remove failed.

The amendment makes denial a two-step fail-closed operation:

- Before a denied live tier attempts to clear durable cache, the process sets an in-memory `unknown-denied` marker.
- If durable clearing fails, the error is surfaced, but the marker continues to override any stale cached Flagship for local admission in the running process.
- A later accepted live tier clears the marker after storing accepted tier truth.
- Malformed JSON now uses the same mandatory denied-tier path instead of best-effort clearing, and the validation failure is recorded before the clear attempt.

Additional regression coverage:

- `denied_live_tier_overrides_stale_cache_when_durable_clear_fails` forces a tier-cache write failure, verifies the old Flagship cache remains on disk, and still proves `effective_tier()` is safe Plus and Flagship-only admission is denied until a later accepted live tier clears the marker.
- Focused validation after this amendment: `scripts/dev_cargo.sh fmt --all`; `scripts/dev_cargo.sh test -p jcode-base subscription_ -- --nocapture`; `scripts/dev_cargo.sh test -p jcode-base test_subscription_ -- --nocapture`. Result: `33` subscription-filtered tests passed and `4` provider subscription guard tests passed.

## 2026-07-15 reviewer-correction slice: authoritative denial status and display truth

Source commit: `114daee99 fix: deny authoritative subscription auth failures`.
Test commit: `6606194ef test: cover authoritative subscription denials`.

This bounded correction preserves the prior implementation, reviews, and reviewer disagreement while addressing the Fable correction items:

- `/v1/me` HTTP `401 Unauthorized` and `403 Forbidden` are now treated as authoritative denied live entitlement truth. The client records jcode validation failure, enters the same fail-closed denial/cache-clear path used for unknown live tier truth, and still returns the HTTP error to the caller.
- Transient request failures and non-authoritative HTTP failures such as `5xx` remain outside the denial path; they continue to return the fetch error without overwriting cached entitlement truth.
- The in-process `LIVE_TIER_DENIED` marker now preserves the last authoritative denied result after successful cache clear as well as after cache-clear failure. A later accepted live tier is the operation that clears the marker. This keeps local admission deterministic without broadening into multi-account state.
- The TUI live account tier label now derives from `tier_truth()` rather than raw `parsed_tier()`, so a response such as `tier=flagship` with inactive status displays `unknown-denied (inactive subscription status)` rather than `Flagship`.

Additional coverage added:

- Local no-secret fixtures for stale cached Flagship followed by `/v1/me` `401` and `403`, proving cache clear, Plus effective tier, Flagship-only model denial, validation failure, returned HTTP error, and auth-readiness downgrade from `RequestValid` to `CredentialPresent`.
- A pure TUI helper test proving the display label uses `tier_truth` for inactive Flagship and still labels accepted active Flagship as `Flagship`.
- Explicit freshness assertions for accepted cached tier (`Cache`) and for an unaccepted persisted tier value (`UnknownDenied`, reason `cached tier is not accepted`).

Validation limitation for this correction slice: per coordinator instruction, no Cargo, Nix, build, or test commands were run. Static-only validation performed before commit: `git diff --check` on the source/test/docs diffs plus manual diff review.

## 2026-07-15 independent correction review preservation

The initial independent reviews and their disagreement are preserved verbatim rather than replaced:

- Initial Opus review: `PASS` with a required follow-up caveat. Artifact [`2026-07-15-r02-tier-initial-opus.md`](../../reviews/2026-07-15-r02-tier-initial-opus.md), SHA-256 `13e239bc56f3ae36149e49166f6969a620a933dd1f9cf7c24ce41fea008424ea`.
- Initial Fable review: `FAIL`, identifying durable cache-clear failure, authoritative `401`/`403` denial, denied-truth display, and freshness coverage gaps. Artifact [`2026-07-15-r02-tier-initial-fable.md`](../../reviews/2026-07-15-r02-tier-initial-fable.md), SHA-256 `2784e51bffae80d441a24820dbe4a2bc90a27ea9efa510833dd954dd51f28239`.

After commits `760dcae11`, `5ba8eed30`, `114daee99`, `6606194ef`, and `91475c598`, both bounded correction re-reviews passed:

- Opus re-review: `PASS`. Artifact [`2026-07-15-r02-tier-correction-rereview-opus.md`](../../reviews/2026-07-15-r02-tier-correction-rereview-opus.md), SHA-256 `2e5e3c0e0acc63fd22bade8015fdafb003c7fcfb1d0884088345ed92b25388a2`.
- Fable re-review: `PASS`. Artifact [`2026-07-15-r02-tier-correction-rereview-fable.md`](../../reviews/2026-07-15-r02-tier-correction-rereview-fable.md), SHA-256 `1a5ac839a8ea5a83fda1323427e6688210f7921a30f32dcfbbd8d3d6a513dcf3`.

Coordinator post-commit validation on the clean final branch passed `28/28` focused fixtures: subscription API `8/8`, catalog/freshness `14/14`, provider admission/display `5/5`, and TUI denied-label `1/1`. The affected TUI check and no-reload selfdev-profile TUI build passed. R09 remained unchanged without `--update`: the 17 classifier tests, warning budget, and wildcard budget passed; panic, swallowed-error, production-size, and test-size ratchets remained visibly red.

Both final reviewers retain non-blocking residuals: the process-local denial marker cannot survive a restart after a durable clear failure, and some secondary cached-tier status surfaces can show stale presentation even while admission remains fail-closed. These remain visible follow-ups and do not authorize the pilot by themselves.

## Coordinator combined-validation amendment (2026-07-15)

At coordinator HEAD `6c6a4f2c8c78a7f9a08e39a4356e2ab401370de3`, the integrated R02 chain is `3063fe0fa` through `cb924b3ae`. Exact source, test, documentation, correction-review, validation-manifest, and intentionally absent category linkage is preserved in [`../../evidence/README.md`](../../evidence/README.md).

The independent correction reviews reproduce byte-exact: Opus PASS SHA-256 `2e5e3c0e0acc63fd22bade8015fdafb003c7fcfb1d0884088345ed92b25388a2`; Fable PASS SHA-256 `1a5ac839a8ea5a83fda1323427e6688210f7921a30f32dcfbbd8d3d6a513dcf3`.

The sequential combined manifest SHA-256 is `41ece4820891461de774dbc5ab06d8e8a66c00630be62274d00dc1f5a9952291`. R02-relevant results are 35/35 selected subscription catalog/API tests, 4/4 selected provider-admission tests, and 1/1 denied-tier TUI label test. Cargo also emitted zero-test lines for unrelated binaries; those lines are preserved but are not counted as passing evidence.

The previously named R02 strict prerequisite is closed as a source-fix node. This does not decide product policy beyond the approved fixtures and does not authorize a pilot. G2 remains the independent pilot gate and may inject new blockers.

## 2026-07-16 fork product-governance amendment

The operator has now supplied the product decision that W4 required. The fork
will **not** adopt upstream's expanding jcode subscription ladder as product
authority. In particular, this recovery will not import `Pro`, `Max`, `Ultra`,
upstream price or budget constants, or upstream model-floor changes. Coupling
an ever-changing model list to upstream commercial tiers is rejected as the
fork's product architecture.

The existing `Plus` and `Flagship` parser, fixtures, and fail-closed denial
behavior remain temporarily for compatibility and safety. They are not an
endorsement of an upstream subscription product and are not the intended
long-term fork entitlement model. Unknown live tiers continue to deny stale
elevation rather than silently granting it.

Future model availability and any commercial or account entitlement policy
will be a fork-owned seam designed after the basic independent product is
working. That later design should separate dynamic model/provider capability
from any commercial plan vocabulary instead of mechanically inheriting
upstream's tier-to-model matrix. No such redesign is authorized in this
recovery amendment.

W4's catalog-schema sync slice is therefore closed as a documented
**non-adoption**, not an implementation task. Its route-composition half must
first reconcile current HEAD because the definitive-absence suppression may
already coexist with fork provider/profile identity. If current behavior
already satisfies the route matrix, W4 closes with targeted tests and docs,
not a synthetic source diff.

The related future cross-surface naming decision is pinned separately in
[`docs/proposals/observability-field-naming.md`](../../../../proposals/observability-field-naming.md).
That proposal does not widen R02, R03A, W2, or any durable schema now.
