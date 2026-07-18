# Seam records

> **Historical seam index.** Early table cells are superseded by the Phase 6
> rollup and final overlay amendments later in this file. Do not use this index to
> dispatch new work.

Each reviewed responsibility receives one directory named `<ID>-<slug>`.

- Full review: `opus-review.md`, `grok-review.md`, and Terra-authored `ledger.md`.
- Light review: one `ledger.md` using the lightweight section of the template.
- Terra is the sole committer on a full-review seam branch. The coordinator integrates the commit into the recovery branch after the ledger gate.
- Reviewers write only their assigned review file and do not edit the responsibility index or progress log.
- Source code is read-only during seam research.

## Integrated ledgers

| Responsibility | Review | Disposition | Pilot status |
|---|---|---|---|
| [R00 integration provenance](R00-integration-provenance/ledger.md) | light overlay | `retain-fork` | binding on every seam |
| [R01 runtime build identity](R01-runtime-build-identity/ledger.md) | full | `retain-fork` | strict projection prerequisite integrated and independently re-reviewed; pilot gate pending G2 |
| [R02 config/provider routing](R02-config-provider-routing/ledger.md) | full | `compose` | strict tier/auth prerequisite integrated and independently re-reviewed; pilot gate pending G2 |
| [R03A wire compatibility](R03A-wire-compatibility/ledger.md) | full | `retain-fork` | strict fail-closed compatibility prerequisite integrated and independently re-reviewed; pilot gate pending G2 |
| [R03B transport/client attach lifecycle](R03B-transport-client-attach-lifecycle/ledger.md) | light | `retain-fork` | isolated Unix socket 20/20 and live-attachment 1/1 fixtures pass; WebSocket/mobile deferred |
| [R04 session/process/background lifecycle](R04-session-process-background-lifecycle/ledger.md) | full | `retain-fork` | narrow marker-durability prerequisite independently verified and integrated; lifecycle widening remains gated |
| [R05A plan/DAG/control log](R05A-plan-dag-control-log/ledger.md) | light | `retain-fork` | no-swarm pilot defer; graph/control authority preserved |
| [R05B worker dispatch/reclaim](R05B-worker-dispatch-reclaim/ledger.md) | full | `retain-fork` | blocked for swarm widening on spawn authority and history-preservation defects |
| [R06A durable session evidence](R06A-durable-session-evidence/ledger.md) | light | `retain-fork` | storage fixture approved; live emission remains R12 |
| [R07A tool/MCP lifecycle](R07A-tool-mcp-lifecycle/ledger.md) | light | `retain-fork` | no-tool pilot defer; real MCP/tool execution escalates |
| [R07C telemetry consent](R07C-telemetry-consent/ledger.md) | light | `retain-fork` | reporting-disabled fixture posture approved |
| [R08A operator input/command semantics](R08A-operator-input-command-semantics/ledger.md) | light | `defer` | no-UI pilot only; onboarding/import consent is unapproved and full-review gated |
| [R09 quality gates](R09-quality-gates/ledger.md) | light overlay | `retain-fork` | binding red-debt policy |
| [R10 packaging/release/update](R10-packaging-release-update-distribution/ledger.md) | light | `compose` | no-network identity smoke only; release/install/update paths unapproved |
| [R11 documentation governance](R11-documentation-governance/ledger.md) | light overlay | `retain-fork` | binding evidence policy |
| [R12 agent turn evidence](R12-agent-turn-evidence/ledger.md) | full | `retain-fork` | narrow no-tool/non-retry terminal-evidence prerequisite independently verified and integrated; cancellation/retry/compaction and tool widening remain gated |
| [R13 compaction/context budget](R13-compaction-context-budget/ledger.md) | light | `retain-fork` | one-turn pilot avoidance approved |

Full-seam Sol and Fable sign-offs, failed sign-offs, append-only corrections, bounded re-reviews, and the lightweight-ledger Opus review are preserved under [`../reviews/`](../reviews/). The remaining-light-ledger review is [`2026-07-15-remaining-light-ledgers-opus-review.md`](../reviews/2026-07-15-remaining-light-ledgers-opus-review.md), SHA-256 `b537bc5674fdb9385e60c2dd18a44db5e61ba4f57146cd57fbf91f7a58a8a55d`. The independently passing R04 implementation reviews are [`2026-07-15-r04-marker-fix-opus-review.md`](../reviews/2026-07-15-r04-marker-fix-opus-review.md) and [`2026-07-15-r04-marker-fix-fable-review.md`](../reviews/2026-07-15-r04-marker-fix-fable-review.md). The R12 implementation review history preserves the initial [`Opus PASS`](../reviews/2026-07-15-r12-evidence-fix-opus-review.md), initial [`Fable FAIL`](../reviews/2026-07-15-r12-evidence-fix-fable-review.md), and both passing bounded correction re-reviews ([`Opus`](../reviews/2026-07-15-r12-evidence-fix-opus-rereview.md), [`Fable`](../reviews/2026-07-15-r12-evidence-fix-fable-rereview.md)).

The 2026-07-15 combined prerequisite evidence, exact integrated commit categories, independent review hashes, immutable no-reload build hash, R09 expected exits, infrastructure failures, and three SHA-256 manifests are indexed in [`../evidence/README.md`](../evidence/README.md). The previously named strict prerequisite-node count is zero, but pilot authorization remains OPEN pending independent G2 adjudication.

## 2026-07-15 G2 status amendment

The `pilot gate pending G2` cells above are superseded for only the strict composite fixture boundary. Independent Opus review returned **PASS** at fixed commit `16e52bf4bcdffb0e8aea46266488960673e8ee5f`; the preserved review is [`../reviews/2026-07-15-g2-pilot-gate-opus.md`](../reviews/2026-07-15-g2-pilot-gate-opus.md), SHA-256 `abb7b2694abccb0c32385fc552dcc29bf0eba854d439c5c43dc82ba4f3991e4f`. All wider R01/R02/R03A/R12 residuals and the R04/R05B/R07/R08/R10 exclusions remain live stop conditions.

## 2026-07-16 Phase 6 active-status rollup

The original integrated-ledger table is preserved as a Phase 2/3 snapshot. Its
`pending G2`, `lifecycle widening remains gated`, `R05B blocked`, and dangerous
R08A consent status cells are superseded for current recovery status by this
rollup. Adjacent live/external claim limits remain binding.

| Responsibility | Current recovery status |
|---|---|
| R00 | Preservation and provenance checks pass at the coordinator audit; overlay retirement awaits final joint sign-off. |
| R01 | Canonical runtime identity retained; prerequisite and cross-seam fixtures pass; documented residuals remain deferred. |
| R02 | Composition retained; W4 route closure and W5 credential-boundary observation complete; upstream commercial tier truth remains a non-adoption. |
| R03A | Compatibility authority retained; Phase 5 made no protocol diff and `PROTOCOL_VERSION` remains `1`. |
| R03B | Unix attach evidence retained; WebSocket/mobile remains deferred with its full-review trigger. |
| R04 | W3 deterministic lifecycle widening and carried follow-ups are integrated and pass 14 exact fixtures; live lifecycle exercise remains gated. |
| R05A | Observe-only control-log authority retained; both W2 entry fixtures reproduced. |
| R05B | W2 offline spawn/reclaim safety is integrated and independently approved; live swarm use remains separately gated. |
| R06A | Durable evidence authority retained; schema/version widening remains gated. |
| R07A | Observe-only fail-closed posture retained; tool/MCP exercise remains deferred. |
| R07C | Reporting-disabled fixture posture retained; telemetry test-isolation QoL remains deferred. |
| R08A | Broader disposition remains `defer`; W5 closes the named expiry-as-consent defect with explicit-consent fixtures. |
| R09 | Trusted greens and expected-red debt reproduce without baseline movement; overlay retirement awaits final joint sign-off. |
| R10 | W6 acquisition/release ordering is integrated and independently approved; real release/install/update remains unauthorized. |
| R11 | Append-only evidence and hash checks pass; overlay retirement awaits final joint sign-off. |
| R12 | W1 cancellation/retry/error-class evidence is integrated; all 11 `r12_` fixtures pass. |
| R13 | Writer census remains authoritative; Phase 5 added no provider-session assignment. |

The final coordinator package is
[`../evidence/2026-07-16-phase6-final-audit/`](../evidence/2026-07-16-phase6-final-audit/),
with `SHA256SUMS` SHA-256
`9af58f1563f266066edd6da9208983da62eeb0b1997ec78f9c26318221dcd2a3`.

### Spot-check metadata correction

The hash immediately above identifies the exact candidate reviewed by Opus.
After its sole LOW finding, metadata now states 62 real checks and 76 TSV
physical lines. The current package `SHA256SUMS` SHA-256 is
`ca8ff5b9f3b6c09dc0ff05de9b3c1c426fc2373706eeeca26cad87126f2e14d8`.
No seam status or test result changed.

## 2026-07-16 final overlay-status amendment

Independent architecture review and joint Sol/Fable sign-off are complete at
fixed signed head `17586246a`, all PASS with zero unresolved IMPORTANT or
CRITICAL findings. The final Sol and Fable report SHA-256 values are
`228f5937dd7eafa6570ed857b3a8db43a1ed43c0a3c9ad6dcaf6e2d29ef8ebe4` and
`7da9ca6810bde9db1035b68e1d2a46f3c0966c6610db7c19553acc96cacc13d3`.

R00, R09, and R11 are retired as special recovery overlays by their append-only
ledger amendments. All 17 seam dispositions remain authoritative historical and
maintenance records; retirement does not delete a ledger or relax its durable
policy. R08A remains the one broader defer. Phase 6 recovery is complete.
