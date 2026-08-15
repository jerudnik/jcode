# actionlint gap: workflow parser vs. valid-YAML constructs

## The question

Does the repo's pinned actionlint correctly parse and validate every YAML construct
used in `.github/workflows/*.yml`? In particular, does it choke on or misinterpret
valid-YAML features it may not support: anchors/aliases, merge keys `<<:`, multiline
block scalars, custom tags, non-string keys, flow mappings, and comments in unusual
positions?

## What I checked

1. Read all 14 files under `.github/workflows/` (13 fork-owned workflows plus
   `freebsd-smoke.yml`; `build-matrix.json` is JSON data, not a workflow) and
   inventoried every YAML construct beyond the plain mapping/sequence/scalar base.
2. Ran the pinned actionlint three ways:
   - `nix run .#actionlint -- .github/workflows/` (the command prescribed for this node)
   - `nix run .#actionlint --` from the repo root (documented auto-discovery form)
   - `nix run .#actionlint -- <13-file CI list>` and `... -- .github/workflows/freebsd-smoke.yml`
3. Probed the pinned binary against synthetic minimal workflows (in `/tmp`, not the
   repo) for the exotic constructs the question names, to distinguish
   parse-fine / flagged / silently-skipped behavior.

## The conclusion

actionlint (flake-pinned v1.7.12) parses and validates **every YAML construct the repo
actually uses**, and exits 0 on all 14 workflow files. The repo only uses constructs in
actionlint's well-supported subset: plain block mappings/sequences/scalars, flow
sequences `[...]`, literal block scalars `|`, quoted scalars, bare booleans/integers, and
comments in standard positions. It uses **none** of the constructs actionlint mishandles,
so there is no live parser gap.

Of the exotic constructs the question lists, only two behave interestingly in this
actionlint build, and neither appears in the repo:

- **Merge keys `<<:`** — (b) explicitly rejected: `GitHub Actions does not support YAML
  merge key "<<"`, and the merge is not applied (downstream "missing run" errors).
- **Custom tags `!tag`** — (c) silently accepted: the tag is dropped and the underlying
  scalar is still validated, so actionlint does not flag something GitHub's parser may
  reject (unverified).

Everything else listed (anchors/aliases, flow mappings, literal block scalars, standard
comments) parses cleanly. Two operational caveats, both unrelated to YAML correctness:
the directory-argument form fails fast (actionlint 1.7.12 takes file paths or no args,
not a directory), and the CI list intentionally omits `freebsd-smoke.yml` (which
nevertheless passes).

## Evidence

### Constructs present (inventory)

- **Flow sequences `[...]`** (no flow mappings `{}`): `docs-impact.yml:5-6`
  (`branches: [main]`, `types: [opened, synchronize, reopened, ready_for_review]`),
  `governance-root.yml:9`, `main.yml:5`, `nix.yml:5` (`tags: ["v*"]`), `nix.yml:83`
  (`system: [x86_64-linux]`), `nix.yml:99` (`needs: [validate, build]`), `pr.yml:5`,
  `pr.yml:55` (`needs: [classify, checks]`).
- **Literal block scalars `|` only** — no folded `>`, no chomping (`-`/`+`), no
  indentation modifiers. 34 instances: `docs-impact.yml` 1, `fork-health.yml` 3,
  `governance-root.yml` 1, `nix-update.yml` 3, `nix.yml` 3, `pr.yml` 2, `release.yml` 9,
  `scheduled.yml` 8, `security.yml` 2. Used at `run:` (most), `with.extra_nix_config:`
  (`release.yml:46`, `scheduled.yml:86,132`), and `with.path:`
  (`scheduled.yml:71,116`).
- **Quoted scalars**: double — `nix.yml:5` `"v*"`, cron strings `fork-health.yml:11`
  `"37 9 * * *"`, `nix-update.yml:9`, `scheduled.yml:5`, and `fork-ci.yml:47`
  `cache-all-crates: "true"` (forced string). Single — `release.yml:6` `'v*'`,
  `release.yml:10,14,19`, `pr.yml:31-32`.
- **Bare booleans/numbers**: `true`/`false` throughout (`cancel-in-progress`,
  `required`, `skipPush`, `useDaemon`, `fail-fast`, `continue-on-error`,
  `if-no-files-found`); integers (`timeout-minutes`, `fetch-depth: 0`,
  `retention-days: 14`).
- **Comments in standard positions**: full-line blocks at top level
  (`governance-root.yml:3-6`, `fork-health.yml:3-7`, `nix-update.yml:3-5`), between job
  keys (`fork-ci.yml:19-22`), between step items (`fork-ci.yml:29-35`), and trailing
  after a `uses:` scalar (`fork-ci.yml:27`, `nix.yml:38,41,85,88,102,105,113`,
  `release.yml:40,44`). Note `#` inside `nixpkgs#just` / `.#packages` and inside `run:`
  block scalars is scalar/script text, not a YAML comment.

### Constructs absent

Anchors `&`, aliases `*`, merge keys `<<`, custom tags `!tag`, folded `>` scalars,
chomping/indent modifiers, document markers (`---`/`...`), non-string mapping keys, and
flow mappings `{}`. (The only `!` occurrences are expression negation `${{ !inputs.x }}`
in `ci.yml:31,39,48,56` and `security.yml:40`; not tags.)

### actionlint run results (flake pin, v1.7.12, shellcheck 0.11.0 + pyflakes 3.4.0 on PATH via the nix wrapper)

- `nix run .#actionlint -- .github/workflows/` → **exit 3**, `could not read
  ".github/workflows/": read .github/workflows/: is a directory`. actionlint 1.7.12's
  help documents file paths or no-arg auto-discovery, not a directory argument; the
  prescribed command is unsupported usage, not a parser failure.
- `nix run .#actionlint --` (repo root) → **exit 0** (auto-discovers and lints all
  `.github/workflows/*.yml`, including `freebsd-smoke.yml`).
- `nix run .#actionlint -- <13-file CI list>` → **exit 0**.
- `nix run .#actionlint -- .github/workflows/freebsd-smoke.yml` → **exit 0**.
- `nix run .#actionlint -- -version` → `1.7.12`.

Because the nix wrapper injects `shellcheck` 0.11.0, the `run:` literal block scalars
had their shell bodies linted too, and everything still passed.

### Synthetic probes of the exotic constructs (in `/tmp`)

- anchor + alias → **exit 0** (alias resolved and the resolved structure validated).
- flow mapping `strategy.matrix: { os: [ubuntu-latest] }` → **exit 0**.
- merge key `defaults: { <<: *d }` → **exit 1**: `GitHub Actions does not support YAML
  merge key "<<" [syntax-check]`, plus the merge was not applied (the section reported
  missing `run` / empty).
- custom tag `runs-on: !custom ubuntu-latest` → **exit 0** (tag silently dropped, value
  `ubuntu-latest` still validated).
- integer key `- 123: 456` → **exit 1** (`step must run script with "run" section or run
  action with "uses" section`): parsed without a YAML crash and flagged semantically.

## Remaining unknowns

- Whether GitHub.com's own YAML parser rejects unknown custom tags (`!tag`) where this
  actionlint accepts them; unverified against GitHub, and moot since no repo workflow
  uses tags.
- YAML 1.1 vs 1.2 scalar coercion (`on`/`yes`/`no` as bare values) is not exercised by
  these files; every boolean value is `true`/`false`, so no coercion ambiguity exists
  here to test.
- The reason CI pins an explicit 13-file list rather than using no-arg auto-discovery:
  plausibly to keep `freebsd-smoke.yml` (called out in `nix.yml:46` as "the sole
  upstream exemption") out of the enforced set; not a parser limitation.
