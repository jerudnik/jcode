# Investigation: nix-native update + reload

Three-pass, read-only investigation into whether jcode's hand-rolled "version
store + channel symlink" reload subsystem can be collapsed to a nix-native
single atomic path, given that this hard fork has a single nix/NixOS maintainer
who works almost entirely inside the self-dev hot-reload loop.

## Files
- `investigator-A.md`, `investigator-B.md` — two independent code studies.
- `synthesis.md` — a third pass that re-derived every load-bearing claim from
  source, adjudicated A vs B, and produced the migration plan.

## Verdict (see synthesis.md for the file:line evidence)
Collapse is safe and half-built already. The one thing that must not be gotten
wrong is **atomicity of the reload target**: the version store's irreducible job
is atomic stage->fsync->smoke->rename (`jcode-build-support/src/lib.rs`, guarded
by a source-truncation regression test), which a nix `result`/store path
preserves for free but a bare `target/selfdev/jcode` does not. The
`JCODE_NIX_MANAGED` scaffolding (`paths.rs`) already implements the single-path
model for the non-self-dev case and is dead only because nothing sets the var.

## How this maps to the work graph
F20 ("harden the release acquisition matrix") targeted a subsystem this fork is
retiring, so it was replaced (WORK_GRAPH.json) by a serialized chain under W3:
- **F20a** — nix-native + update-inert (set `JCODE_NIX_MANAGED` in the package;
  gate `should_auto_update` on it; honest `jcode update` guidance). Safe,
  reversible, high value.
- **F20b** — collapse self-dev reload to one atomic fixed path; stable/comfort
  fallback via nix generations (home-manager for keeps, `nix profile` for
  test-drives). The keystone surgery.
- **F20c** — delete the release-acquisition subsystem + version store + channels.

F21 (the deterministic integration gate) now depends on F20c.
