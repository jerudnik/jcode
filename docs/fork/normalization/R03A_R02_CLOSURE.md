# R03A and R02 W7 candidate closure adjudication

Date: 2026-07-16

## Decision

The two dormant candidates from the original W7 proposal are **closed as
unwarranted at the normalized fork head**. They are not deferred under a vague
"W7 later" label.

| Candidate | Decision | Why |
|---|---|---|
| R03A centralized advertising-subscribe/verdict consumption | Close | The compatibility calculation and verdict construction are already centralized. The two remaining consumers serve intentionally different transport phases, so combining them would hide rather than remove an authority boundary. |
| R02 `sidecar.rs` / `provider/mod.rs` file splitting | Close as a W7 requirement; retain measurable size debt in the normal quality register | File size is real debt, but there is no new correctness trigger or independently justified extraction boundary. A broad split would churn high-risk routing/auth/provider code solely to satisfy shape. |

This closes Completion Standard D3's requirement that both candidates be either
implemented with evidence or explicitly rejected with rationale.

## R03A: no further verdict-consumption centralization

### Current authority

The current implementation already has one pure daemon-side compatibility and
event-construction authority:

- `crates/jcode-app-core/src/server/handshake.rs`,
  `evaluate_subscribe_handshake`, calls the protocol authority
  `HandshakeCompatibility::evaluate` exactly once and constructs the optional
  typed `HandshakeVerdict` event.
- `evaluate_and_notify` is the post-initialization adapter. It logs the same
  evaluation and sends the typed event through the established client event
  channel.
- `preflight_initial_incompatible_advertised_subscribe` in
  `server/client_lifecycle.rs` is the pre-session fail-closed adapter. It must
  write directly to the transport before full session initialization, then
  refuse the incompatible client.

The duplication that remains is not duplicate verdict semantics. It is two
transport adapters around the same evaluated value:

1. direct writer before session/event-channel establishment, for fail-closed
   initial subscribe rejection; and
2. event-channel notification after the normal client lifecycle exists.

Collapsing them would require either initializing more session state before an
incompatible client is rejected, or teaching the handshake module about both
transport lifetimes. Either change widens authority and regression surface
without changing behavior.

### Existing evidence

- The R03A ledger assigns stable wire/verdict/action semantics to the fork and
  identity meaning to R01.
- Focused handshake tests cover legacy clients, advertised incompatible clients,
  matching clients, direct preflight rejection, and end-to-end compatible and
  incompatible verdict events.
- The W7 review explicitly found no new R03A correctness trigger and recommended
  keeping this candidate dormant rather than coupling it to the R12 work.

### Re-open triggers

Re-open as a new, independently reviewed design task only if one of these occurs:

- a third transport adapter begins evaluating or constructing subscribe verdicts;
- compatibility logic appears outside `HandshakeCompatibility::evaluate` or
  `evaluate_subscribe_handshake`;
- direct preflight and event-channel paths produce observably different verdict
  contents for the same identity inputs; or
- a lifecycle redesign creates one transport-neutral emission interface that is
  already valid both before and after session initialization.

## R02: no blanket split of `sidecar.rs` or `provider/mod.rs`

### Current state

At the measured normalized product head:

- `crates/jcode-base/src/provider/mod.rs` is 2,797 LOC and already delegates to
  29 provider submodules. Its remaining body coordinates shared provider state,
  route memoization, failover, auth refresh, model switching, and the `Provider`
  implementation.
- `crates/jcode-base/src/sidecar.rs` is 2,235 LOC. It combines configured-route
  gating, backend selection, OpenAI and Anthropic request/response handling,
  memory-sidecar operations, and colocated fixtures.

The size debt is genuine, but LOC alone does not identify a safe semantic seam.
Both files sit on fail-closed configuration, entitlement, credential, routing,
and provider-selection boundaries. Recovery deliberately pinned those behaviors
before optional cleanup. Splitting now would create module visibility churn and
move tightly coupled private types without fixing a demonstrated defect.

### Debt disposition

The candidate is closed **as a W7 refactor**, not erased. Exact current size
state, affected files, ownership, and ratchet triggers are migrated to
[`QUALITY_DEBT.md`](QUALITY_DEBT.md). The frozen size baselines remain unchanged.
`provider/mod.rs` and `sidecar.rs` retain the existing no-growth obligation.

This is consistent with the W7 review's instruction that R09 forbids blanket
cleanup and that neither file-splitting candidate had a new correctness trigger.

### Re-open triggers

A bounded extraction may be proposed when all of the following are true:

1. an adjacent correctness or feature change already needs to modify the target
   responsibility;
2. the extracted responsibility has a named authority and a narrow API;
3. focused tests pin the before/after behavior and failure semantics;
4. the patch reduces the tracked production file without growing another
   untracked oversized file; and
5. independent review confirms the change is not an auth, entitlement, route,
   or failover widening.

Good candidate examples, when triggered by real work, include a self-contained
configured-route gate, one protocol request/response codec, or a provider-state
memoization unit. A whole-file mechanical shuffle is not an accepted trigger.

## Closure status

- R03A verdict centralization: **closed, no implementation**.
- R02 broad file splitting: **closed, no implementation**.
- R02/R09 size debt: **owned in the normal quality register**.
- Original W7: **fully adjudicated; no remaining "W7 later" item**.
