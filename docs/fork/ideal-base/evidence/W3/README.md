# W3 synthesis: deterministic validation and packaging made authoritative

All nine W3 children accepted:

| Node | What closed | Evidence |
|---|---|---|
| F15 | CI hermeticity audit: 58 ignores classified, 0 unclassified; rails mapped; flakiness root-caused | evidence/F15 |
| F16 | Real-process cancellation/reload/re-exec promoted to hermetic, with runtime binary resolution | evidence/F16 |
| F17 | Linux/macOS/app-core/TUI rails promoted advisory to blocking; `jcode-tui` runs, not compile-only | evidence/F17 |
| F18 | PRs build and launch the real Nix package (`result/bin/jcode --version`) | evidence/F18 |
| F19 | Mobile static assets packaged executable-adjacent; CWD cannot mask them | evidence/F19 |
| F20a | Nix-native, update-inert: store-path detection makes the packaged binary self-declare managed | evidence/F20a |
| F20b | Self-dev reload collapsed to one atomic fixed path, no channel drift or downgrade | evidence/F20b |
| F20c | Distribution surface retired: update-core, version store, channel matrix, installers deleted | evidence/F20c |
| F21 | Full deterministic CI/package/updater integration gate, green twice from clean state | evidence/F21 |

## Why the wave is closable

The W3 purpose was to make deterministic validation and packaging
*authoritative* rather than advisory. That is now true in both directions:

- **Authoritative:** F17 made the test rails blocking, so a red rail stops a
  push instead of annotating it. F18 makes every PR build the real package, so
  packaging cannot rot between releases.
- **Deterministic:** F21 ran the whole gate twice from clean state at one
  commit, with 12/12 checks agreeing including the nix store path
  (`fa0mbkdylvqnr3r66dx9if4m743y01d9`); the second run reproduced the
  derivation without rebuilding.

## Non-vacuity

The closing gate was proven capable of failing, three independent ways
(F21): comparator sabotage; real checks run against a source binary presented
as the package (4 of 6 correctly refused); and residue sabotage. Determinism
is judged on normalized per-check fingerprints rather than raw logs, and the
suite fingerprints pin pass counts so silent test-count drift cannot slip
through.

## Review cycles

F16 took a FAIL then fix cycle (binary resolution and panic-safe teardown).
F17 was ACCEPT-WITH-CAVEATS after root-causing a real concurrency bug: the
mermaid deferred-worker `ACTIVE_DIAGRAMS` pollution race, fixed with a runtime
synchronous-render mode and validated over 95 serial rounds. The remaining
nodes passed first round.

## Real defects found en route

The wave found and fixed defects rather than routing around them:

- F17: mermaid deferred-worker `ACTIVE_DIAGRAMS` pollution race.
- F20c: five real defects root-caused, four of the ambient-state class.
- F21: a Linux-only F28 regression where test fixtures released their env
  lease before restoring `JCODE_HOME` (two independent instances), now gated
  structurally by `check_env_lease_drop_order.py`.

The recurring theme is ambient state leaking into tests. That class is
carried forward, not assumed closed: F28 (TUI hermeticity, to restore
parallelism) and F29 (route every ambient filesystem root through the
isolated `jcode-storage` helpers) are both accepted under W4.

## Carried forward

- F28 hardens lock discipline so the serialized (`--test-threads=1`) rails can
  return to parallelism.
- F29 eliminates the remaining ambient filesystem roots.
- `doCheck` stays false and cache push stays off (F18/F19); enabling either is
  a separate decision.
