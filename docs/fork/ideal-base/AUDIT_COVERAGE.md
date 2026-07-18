# Source-audit coverage map

Recorded: 2026-07-18

The closure audit identified 25 remaining work items. The executable graph splits
several items into design, implementation, and independent verification nodes.
This map proves that the decomposition preserves every audited item.

| Audit ID | Audited work item                                                         | Graph nodes   | Class                        |
| -------- | ------------------------------------------------------------------------- | ------------- | ---------------------------- |
| A01      | One work-aware server activity and bounded shutdown authority             | F01, F02, F03 | Mandatory deterministic      |
| A02      | Atomic, serialized, recoverable background-task status persistence        | F04, F05      | Mandatory deterministic      |
| A03      | Explicit parent ownership and bounded reap for pooled MCP children        | F06           | Mandatory deterministic      |
| A04      | Dead and hung MCP detection, eviction, cooldown, and reconnect            | F07, F08      | Mandatory deterministic      |
| A05      | Stale selfdev pending-activation reconciliation                           | F09, F11      | Mandatory deterministic      |
| A06      | Durable disconnect-cleanup intent and startup reconciliation              | F10, F11      | Mandatory deterministic      |
| A07      | Global MCP-child and background-task resource bounds                      | F12, F13, F14 | Mandatory deterministic      |
| A08      | Real-process lifecycle promotion and repeated residue-free execution      | F14, F16      | Mandatory deterministic      |
| A09      | Blocking intended Linux test rail                                         | F17           | Mandatory deterministic      |
| A10      | Blocking intended macOS test rail                                         | F17           | Mandatory deterministic      |
| A11      | Executed deterministic TUI tests rather than compile-only coverage        | F17           | Mandatory deterministic      |
| A12      | Real Nix package build and packaged-binary launch in pull requests        | F18           | Mandatory deterministic      |
| A13      | Installed mobile assets served outside a source checkout                  | F19           | Mandatory deterministic      |
| A14      | Hermetic installer/updater acquisition, rollback, and exactly-once reload | F20, F21      | Mandatory deterministic      |
| A15      | Ignored-test classification and deterministic CI hermeticity              | F15, F16, F21 | Mandatory deterministic      |
| A16      | Expiring security advisory policy and strict Homebrew host identity       | F22           | Mandatory deterministic      |
| A17      | Zero-growth critical-path quality budgets with downward targets           | F23           | Mandatory deterministic      |
| A18      | Pinned compatibility inputs, reproducibility scope, provenance, and SBOM  | F24           | Mandatory deterministic      |
| A19      | Socket sidecar, malformed swarm state, and control-log retention hygiene  | F25, F27      | Mandatory deterministic      |
| A20      | Dead PID sweep, process-aware telemetry liveness, and duplicate cleanup   | F26, F27      | Mandatory deterministic      |
| A21      | `aarch64-linux` build/smoke or explicit support downgrade                 | G01           | Authorization/platform gated |
| A22      | Minimal provider-doctor and fresh full catalog validation                 | G02           | Authorization/network gated  |
| A23      | Mobile/iOS simulator or device attach validation                          | G03           | Authorization/platform gated |
| A24      | Windows and FreeBSD scheduled compile/install smoke                       | G04           | Platform gated               |
| A25      | Disposable draft-release acquisition and live updater smoke               | G05           | Authorization/network gated  |

`WORK_GRAPH.json` contains the same mapping in machine-readable form. The railway
validator rejects missing audit IDs, unknown node references, or executable F/G
nodes that are not covered by at least one audit item.
