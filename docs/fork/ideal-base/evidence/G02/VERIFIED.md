# G02: authorized provider-doctor across live credentials, network, and spend

Result: **gate met, and the sweep surfaced a real provider defect.**
Coverage is fresh, not inherited: every number below comes from a run against
`9d1470f86829740e74c9af1624cf196b8b3aba85` performed for this ledger. Ten
providers were exercised at the catalog tier, two providers at the spending
full tier, and one provider through the local Bifrost gateway.

Authorization for live credentials, network, and spend was granted explicitly
for this node.

## The binary under test is the tree under test

The published binary on `PATH` was **25 commits stale** (`82277c6df`). Evidence
produced from it would describe a tree nobody is shipping, so the current tree
was built first and every command below ran `./target/selfdev/jcode`:

    selfdev build -> exit 0, 77s, revision 9d1470f86

## What each tier actually costs

Read from the tier definitions rather than assumed:

| Tier | Credential | Network | Spend |
|---|---|---|---|
| `offline` | none | none | none |
| `catalog` | required | live `GET /models` | ~none |
| `full` | required | chat + streaming + tools | **spends balance** |

## Catalog tier, ten providers

    claude       passed=True   11 live model(s) returned
    openai       passed=True    9 live model(s) available
    cursor       passed=True   28 live model(s) available
    antigravity  passed=True   24 live model(s) returned
    deepseek     passed=True    2 live model(s) returned
    minimax      passed=True    8 live model(s) returned
    zai          passed=True    8 live model(s) returned
    gemini       passed=False  not signed in
    copilot      passed=False  not signed in
    perplexity   passed=False  HTTP 404 on model catalog

Seven pass. The three failures are of **two different kinds**, and conflating them
would hide the one that matters.

### Not signed in (2): named next action, not a defect

    gemini   auth_credential_loaded: load Gemini OAuth tokens.
             Run `jcode login --provider gemini` to sign in.
    copilot  auth_credential_loaded: load GitHub Copilot token.
             Run `jcode login --provider copilot` to sign in.

Both fail at the *first* checkpoint, before any network call. This is absence of
a credential on this workstation, and the doctor names the exact remedy. Recorded
as `authorization_blocked` in substance with the next action above.

### Perplexity: a real defect in this repository

    perplexity  model_catalog_live_endpoint: Perplexity live model catalog
                failed (HTTP 404 Not Found)

The catalog URL is built as `{api_base}/models`
(`crates/jcode-provider-openrouter-runtime/src/lib.rs:543`), and Perplexity's
profile sets `api_base = "https://api.perplexity.ai"` with no `/v1`.

Confirmed against the live API rather than inferred from the profile:

    GET  https://api.perplexity.ai/models            -> 404
    GET  https://api.perplexity.ai/v1/models         -> 200 (37 models)

Vendor documentation (`docs.perplexity.ai/api-reference/models-get`) agrees the
endpoint is `/v1/models`.

**The obvious fix is wrong, and it was checked before being proposed.** The same
`api_base` also builds the chat URL. With an identical payload:

    POST https://api.perplexity.ai/chat/completions      -> 200
    POST https://api.perplexity.ai/v1/chat/completions   -> 404

The routes are **inverted**: chat is correct at the current base, models is not.
Appending `/v1` to `api_base` would fix the catalog and break chat. The fix must
be a per-provider models-path override.

Blast radius, re-derived from the catalog rather than recalled: of **36**
OpenAI-compatible profiles, **6** have an `api_base` not ending in `/v1` (zai,
gemini-api, deepseek, fpt, perplexity, deepinfra). Only perplexity failed here,
and zai and deepseek passed in this same sweep, so this is genuinely
per-provider and not a general rule about missing `/v1`.

Not fixed in this node: G02 owns only `docs/fork/ideal-base/evidence/G02/**`.
Filed as a finding.

## Full tier (spends balance)

    claude              tier_passed=True   13/13 pass   model claude-haiku-4-5-20251001
                        spend: 4 billable calls, 2188 tokens
    openai-compatible   tier_passed=True   12/12 pass   model bridge/gpt-5.5 (via gateway)
    via Bifrost         spend: 3 billable calls, 1018 tokens

Both cover non-streaming chat, streaming chat, tool-call parsing, the tool
execution loop, tool-result follow-up, and a real jcode tool smoke.

`offline/claude` also passes (4 pass, 9 skip, zero spend), which confirms the
harness itself is sane before any credential is involved.

## Gateway leg, and the control that makes it mean something

The local Bifrost gateway (`x-bf-vk` virtual keys) was exercised two ways.

Direct chat completions, five distinct upstream providers, all live:

    bridge/gpt-5.5              OK  'ok'            312 tokens
    4nix/Qwen3.6-MoE            OK  (reasoning)      31 tokens
    deepseek/deepseek-v4-flash  OK  'ok'             97 tokens
    zai/glm-5.2                 OK  'ok'            115 tokens
    minimax/MiniMax-M3          OK  (reasoning)     197 tokens

Then jcode itself, not curl, through the same gateway: `provider-doctor
openai-compatible --tier full -m bridge/gpt-5.5` returned `tier_passed=True`
with 34 live models and 3 billable calls.

**The first control failed to discriminate, and that was my error, not the
gateway's.** Re-running without my environment override produced an *identical*
34-model pass. That contradicted the claim that the override caused the result,
so the pass was not accepted until the cause was found. A persisted
`openai-compatible.env` in the application support directory already pins
`JCODE_OPENAI_COMPAT_API_BASE` to the gateway. The doctor was reaching the
gateway by saved configuration all along.

A control that *does* discriminate was then run, with the same command, same credential, and
base pointed at a closed port:

    JCODE_OPENAI_COMPAT_API_BASE=http://127.0.0.1:9/v1
      model_catalog_live_endpoint: FAIL
      "error sending request for url (http://127.0.0.1:9/v1/models):
       tcp connect error: Connection refused (os error 61)"
      tier_passed: false

Only the base URL changed between the pass and this failure, so the checkpoint
is genuinely observing the endpoint and is not vacuous. Independently, a run
with a bogus key returned the gateway's own error text
(`virtual key is required ... x-bf-vk`, `"provider":"4nix"`), which is proof of
*which* server answered, not merely that some server did.

Two upstream misreads were corrected the same way. Port 8000 answered
`/v1/models` with 200 but the listening process is `omlx-server`, a local MLX
server, not Bifrost. And the first virtual key tried returned HTTP 200 with
**zero** models and `Provider 'openai' is not allowed for this virtual key`;
`nix-config/modules/ai/shared/virtual-keys.nix` shows MCP applications are
declared `providerPolicy = "none"`, so that key is provider-less **by design**.
The inference key (`providerPolicy = "curated"`) returns 34 models. The secrets
directory is not listable by this user, and that boundary was not routed around.

## What this does not buy

- **Two providers are untested here**, not passing: gemini and copilot have no
  credential on this workstation. This ledger does not claim they work.
- **Catalog tier is not a chat test.** Twenty-eight Cursor models listed means
  the catalog endpoint answered; it does not mean any of them completes a turn.
  Only claude and the gateway route were exercised at the spending tier.
- **One model per full-tier provider.** `bridge/gpt-5.5` and
  `claude-haiku-4-5` passed; the other 33 gateway routes and 10 Claude models
  were not individually exercised.
- **Perplexity chat is unverified through jcode.** The 200 above came from
  curl. The catalog defect blocks the doctor before it reaches chat.
- **Gateway reachability is site-local.** It depends on this workstation's
  network and its saved `openai-compatible.env`; it says nothing about a fresh
  end-user install.
- **Spend was small and shallow**: 7 billable calls, 3206 tokens total. This is
  a smoke, not a load or rate-limit test.

## Findings

1. **Perplexity `/models` 404**: real, reproduced against the live API, root
   cause identified, and the naive fix disproved before proposal. Needs a
   per-provider models-path override. Not fixed here (out of owned paths).
2. **gemini / copilot not signed in**: no defect. The next action is
   `jcode login --provider {gemini,copilot}`.

## Reproduce

    ./target/selfdev/jcode provider-doctor <PROVIDER> --tier catalog --json
    ./target/selfdev/jcode provider-doctor claude --tier full --json
    ./target/selfdev/jcode provider-doctor openai-compatible --tier full \
        -m bridge/gpt-5.5 --json

Raw JSON reports were kept out of the repository because they embed endpoint and
account detail; the ledger above is the redacted record. No key material appears
in this file: keys are described only by prefix and length.
