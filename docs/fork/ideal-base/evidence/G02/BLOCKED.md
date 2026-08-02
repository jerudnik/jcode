# G02: authorization blocked

**State: `authorization_blocked`. Not incomplete, and not verified.**

## What the node requires

> Run authorized provider-doctor minimal tier and fresh full pre-release catalog
> tier.

Both tiers make authenticated network calls against real provider endpoints
using live credentials, and the catalog tier can incur spend. That is an
external side effect with real-world cost, which is outside what an agent may
self-authorize.

## Why this is blocked rather than pending

Nothing in the repository blocks this work. There is no code to write, no test
to fix, and no local investigation left. Only authorization is missing.
Recording it as `pending` would imply there is local work outstanding, which
would be false.

## Named next action

With explicit user authorization:

1. Run the **minimal tier** first, so the cheapest check is the one that can
   surface a credential or endpoint problem before anything expensive runs.
2. Then run the **full pre-release catalog tier** from a clean credential state.
3. Attach both transcripts under `docs/fork/ideal-base/evidence/G02/`.

## Not verified

Everything this node asserts is unverified. No provider-doctor tier has been run
for this program. This file records a decision about *who may authorize the
run*, not a result.
