# Observability field naming across fork-owned surfaces

Status: Proposal seed

## Why this is pinned

As the fork adopts more runtime, swarm, evidence, replay, and protocol surface,
the same concept can appear in logs, member status, event history, persisted
records, and client responses. Those fields should be understandable at a
glance and should not acquire different names merely because they crossed a
storage or transport boundary.

W2 exposed the issue while distinguishing a requested spawn mode from the mode
that actually ran. Recovery deliberately did not solve the naming system by
widening the public response or durable replay schema. This proposal records
the future governance task without making that change now.

## Direction

Before the next public or durable schema expansion, define one fork-wide naming
convention for:

- caller intent, inherited configuration, policy resolution, and observed
  execution outcome;
- stable reason codes versus optional human-readable detail;
- lifecycle state, terminal outcome, retry/fallback provenance, and error
  classification;
- identical semantics carried through logs, status snapshots, events, replay,
  and wire responses.

Candidate semantic axes to evaluate:

| Concept | Candidate prefix or suffix | Meaning to keep distinct |
|---|---|---|
| Caller intent | `requested_*` | What the caller explicitly asked for |
| Inherited input | `configured_*` | What configuration supplied when intent was absent |
| Policy decision | `resolved_*` | What policy selected before execution |
| Runtime observation | `observed_*` or `actual_*` | What the system verifiably executed |
| Stable fallback cause | `fallback_reason_code` | Closed, machine-readable value |
| Safe explanation | `fallback_detail` | Optional bounded text with secret-safe rules |
| Failure category | `error_class` | Closed enum or allowlist, never raw provider text |

The final vocabulary is intentionally undecided. In particular, the fork should
choose one of `observed` and `actual`, define whether `effective` is needed, and
avoid retaining two words for the same semantic stage.

## Required properties

- Names make the intent-to-outcome sequence obvious without reading the
  implementation.
- The same semantic value keeps the same name across fork-owned surfaces.
- Stable values use closed enums or allowlists where practical.
- Human-readable detail is separate, bounded, and secret-safe.
- Public wire and durable replay additions receive their owning compatibility
  and migration review before implementation.
- UI wording may be friendlier, but it must map unambiguously to the canonical
  field vocabulary.

## Non-goals for this recovery slice

- No W2 protocol or replay widening.
- No broad log, event, status, or UI rename.
- No new schema version.
- No claim that the candidate terms above are already authoritative.

## Trigger for resolution

Resolve this proposal before the next feature needs to expose the same new
state through more than one of: structured logs, status snapshots, event
history, durable replay, or public wire responses.
