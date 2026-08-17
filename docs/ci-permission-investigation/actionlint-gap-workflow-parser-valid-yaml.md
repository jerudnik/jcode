# actionlint gap: workflow parser vs. valid-YAML constructs

## The question

Does actionlint's workflow parser correctly handle every YAML construct used in
this repo's `.github/workflows/*.yml`, including valid-YAML features that
actionlint may choke on or misinterpret: anchors/aliases, merge keys `<<:`,
multiline block scalars, custom tags, non-string keys, flow mappings, and
comments in unusual positions?

## What I checked

1. Inventoried every YAML construct in all 13 `.github/workflows/*.yml` files
   (the 12 linted by CI plus the exempt `freebsd-smoke.yml`) and noted anything
   beyond plain mappings/sequences/scalars. `build-matrix.json` is JSON, not a
   workflow, so out of scope.
2. Ran actionlint exactly the way CI does: `nix run .#actionlint --` over the
   same 12-file list CI invokes in `.github/workflows/nix.yml:48-60`, plus the
   exempt workflow. Repeated with `-verbose` to capture per-file parse-error
   counts.
3. Cross-checked upstream `nixpkgs#actionlint` 1.7.12 against the flake-pinned
   `.#actionlint` 1.7.12 to isolate parser behavior from patching.
4. Probed the pinned binary against synthetic minimal workflows in `/tmp` for
   each exotic construct named in the question, distinguishing parse-fine,
   flagged, and silently-skipped behavior.

Environment: Determinate Nix 3.20.0 on darwin/arm64. Both binaries report
"1.7.12, built with go1.x". `shellcheck` is not installed in this sandbox, so
every actionlint invocation exits 3 on a `run:` literal block scalar it cannot
shellcheck; that exit code is unrelated to YAML parsing.

## The conclusion

actionlint (flake-pinned v1.7.12 and upstream 1.7.12 alike) parses and
validates every YAML construct the repo actually uses. Per-file verbose
output: `Found 0 parse errors in 0-1 ms` for every workflow. The repo only
uses constructs in actionlint's well-supported subset (plain block
mappings/sequences/scalars, flow sequences `[...]`, literal block scalars `|`,
quoted scalars, bare booleans/integers, and standard comments), and uses none
of the constructs actionlint mishandles. There is no live parser gap.

Of the exotic constructs the question names, actionlint 1.7.12 behaves as:

- **(a) parses fine:** anchors (`&x`), aliases (`*x`), flow mappings
  `{k: v, k2: v2}`, literal block scalars `|`, flow sequences `[...]`, all
  comment positions used in this repo, quoted scalars, bare booleans, both
  `on:` (bare, which YAML 1.2 treats as string) and `"on":` (quoted).
- **(b) flagged:** YAML merge keys `<<:` — explicit diagnostic:
  `GitHub Actions does not support YAML merge key "<<"`. Undefined anchors
  produce `could not parse as YAML: yaml: unknown anchor 'X' referenced`.
- **(c) silently skipped:** custom tags (`!tag`, `!!str`) — the tag is
  dropped, the underlying scalar is validated as if it had no tag, so
  actionlint does not catch a tag-only validation gap GitHub's parser may
  raise. Non-string mapping keys are accepted by the YAML layer and only
  flagged semantically downstream (a step entry that is an integer instead of
  a mapping raises `step must run script with "run" section or run action
  with "uses" section`).

None of (b) or (c) appears in the repo today, so the silent-skip and
false-positive categories are not actively exercised.

## Evidence

### Constructs used in production workflows

- **Flow sequences `[...]`** (no flow mappings `{}`): `docs-impact.yml:5-6`
  (`branches: [main]`, `types: [opened, synchronize, reopened, ready_for_review]`),
  `governance-root.yml:9`, `main.yml:5`, `nix.yml:5` (`tags: ["v*"]`),
  `nix.yml:83` (`system: [x86_64-linux]`), `nix.yml:99` (`needs: [validate, build]`),
  `pr.yml:5`, `pr.yml:55` (`needs: [classify, checks]`).
- **Literal block scalars `|`** (no folded `>`, no chomping `-`/`+`, no indent
  indicators). Used at `run:` (most), `with.extra_nix_config:`
  (`release.yml:46`, `scheduled.yml:86, 132`), and `with.path:`
  (`scheduled.yml:71, 116`).
- **Quoted scalars**: double `"v*"` (`nix.yml:5`), cron strings
  (`fork-health.yml:11`, `nix-update.yml:9`, `scheduled.yml:5`), and forced
  strings `cache-all-crates: "true"` (`fork-ci.yml:47`). Single `'v*'`
  (`release.yml:6`) and several descriptions.
- **Bare booleans/numbers** throughout (`true`/`false`, integer
  `timeout-minutes`, `fetch-depth: 0`, `retention-days: 14`).
- **Comments** in standard positions: file-top blocks (`fork-health.yml:3-7`,
  `governance-root.yml:3-6`, `nix-update.yml:3-5`, `nix.yml:44-46`), between job
  keys (`fork-ci.yml:19-22, 29-35`), and trailing after a `uses:` scalar
  (`fork-ci.yml:27`, `nix.yml:38, 41, 85, 88, 102, 105, 113`, `release.yml:40, 44`).
  Trailing comments are common — actionlint preserves them and continues
  validating.

### Constructs absent

Anchors `&` / aliases `*`, merge keys `<<`, custom tags `!tag` / `!!str`,
folded `>` scalars, chomping/indent modifiers, document markers (`---`/`...`),
non-string mapping keys, and inline flow mappings `{...}`. The only `!`
occurrences are expression negation `${{ !inputs.x }}` in `ci.yml` and
`security.yml`; those are expression syntax, not YAML tags.

### actionlint run results

```
nix run .#actionlint -- -no-color -shellcheck= -verbose \
  .github/workflows/{ci,docs-impact,fork-ci,fork-health,governance-root,main,nix,nix-update,pr,release,scheduled,security}.yml

verbose: Found 0 parse errors in 0-1 ms for .github/workflows/<each>.yml
```

Same `-verbose -shellcheck=` invocation against `freebsd-smoke.yml`:
`Found 0 parse errors in 0 ms`. Both binaries (flake `.#actionlint` 1.7.12 and
upstream `nixpkgs#actionlint` 1.7.12) parse all 13 workflows with zero
findings when shellcheck is bypassed. The flake and upstream binaries
disagree only on the patched constructs documented in
`actionlint-gap-dollar-actionlint-compat.md` (none relevant here).

### Synthetic probes against the pinned binary

Each probe is a minimal synthetic workflow written to `/tmp` and linted with
the flake binary:

- **Anchor + alias** in a valid step context: exit 0; alias resolves and the
  resolved structure is validated.
- **Inline flow mapping** at a step value: `env: { foo: bar, baz: qux }` —
  exit 3 only due to shellcheck on `run:`; YAML parse is clean.
- **`on:` bare vs `"on":` quoted**: both exit 0; actionlint treats both as
  the trigger key. YAML 1.2 (used by `yaml/go-yaml`, the library actionlint
  switched to in 1.7.8) parses `on` as a string.
- **YAML merge key `<<:`**: exit 1 with two diagnostics:
  `GitHub Actions does not support YAML merge key "<<"` and
  `step must run script with "run" section or run action with "uses" section`
  — the merge is not applied, so downstream keys are absent.
- **Custom tag `!!str`** on a step `run:` value: `Found 0 parse errors`;
  actionlint drops the tag and validates the underlying scalar. Untested
  against GitHub; not exercised by the repo.
- **Non-string key** (`env: 1: foo`): actionlint parses without crashing and
  the subsequent semantic check fires `step must run script with "run"
  section or run action with "uses" section` — that is a semantic flag, not a
  YAML grammar failure.
- **Trailing comment** after a `uses:` scalar: accepted; this is the
  comment style used in `fork-ci.yml:27`, `nix.yml:38, 41, ...`,
  `release.yml:40, 44`.

actionlint's parser is `yaml/go-yaml` v4 (replaced `go-yaml/v3` in 1.7.8 per
`/nix/store/.../CHANGELOG.md` line 197 and `parse.go` comment "Note: Unknown
anchors are detected by go-yaml parser so we don't need to detect them by
ourselves"). The library is YAML 1.2, which is why `on:` is a string and
anchors produce the documented behavior in `docs/checks.md:3050-3177`.

## Remaining unknowns

- Whether GitHub.com's parser rejects unknown custom tags where this
  actionlint silently accepts them: not verified against GitHub, and the
  repo uses no tags, so this is a hypothetical gap.
- `nixpkgs#actionlint` upstream was tested only at 1.7.12 (the flake pin);
  the comment at `scripts/check_workflow_permissions.py:23-25` notes upstream
  table gaps "through current main (as of 2026-08-10)" but I did not build
  upstream `main` to confirm.
- Comments inside `nixpkgs#just` / `.#packages` style paths or inside `run:`
  block scalars are scalar/script text, not YAML comments — they are not
  parse-position-sensitive.
- YAML 1.1 vs 1.2 scalar coercion (`yes`/`no`/`on`/`off` as bare booleans)
  is not exercised by these workflows: every boolean is `true`/`false` and
  every key is a string.