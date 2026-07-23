# CI proof — real Nix build + launch on the pull_request event

**PR:** https://github.com/jerudnik/jcode/pull/20
**Workflow run:** https://github.com/jerudnik/jcode/actions/runs/30044431553
**Event:** `pull_request` · **Head SHA:** `01fcf0bba` · **Conclusion:** success

## Jobs (as designed)

```
event=pull_request conclusion=success
  select build matrix: success      # emitted x86_64-linux only for the PR
  fast validation:     success       # actionlint + dry-run + flake check --no-build + flake.lock check
  build (x86_64-linux): success      # the only build job on a PR (no aarch64-darwin)
```

The `select build matrix` job restricted the PR to `x86_64-linux`; no
`aarch64-darwin` build job was scheduled, confirming the PR-only Linux matrix.

## Acceptance gate #1 — relevant path change runs a real `nix build` on the PR

The `build (x86_64-linux)` job's "Build package" step on the pull_request event:

```
build (x86_64-linux)  Build package  Run nix build .#packages.x86_64-linux.jcode --print-build-logs
build (x86_64-linux)  Build package  env: CACHIX_CAN_PUSH: false
build (x86_64-linux)  Build package  jcode-deps>    Compiling jcode v0.46.0 (/build/source)
build (x86_64-linux)  Build package  jcode>    Compiling jcode v0.46.0 (/build/source)
```

`CACHIX_CAN_PUSH: false` confirms the PR reads the public Cachix cache and never
pushes (constraint preserved).

## Acceptance gate #2 — `result/bin/jcode --version` succeeds on the PR

```
build (x86_64-linux)  Smoke test binary (launch gate)  Run ./result/bin/jcode --version
build (x86_64-linux)  Smoke test binary (launch gate)  env: CACHIX_CAN_PUSH: false
build (x86_64-linux)  Smoke test binary (launch gate)  jcode v0.46.0 (b148ebe)
```

Both gates hold on the `pull_request` event. `doCheck` was not enabled; the
package build proves packaging + launch, exactly as F18 requires.
