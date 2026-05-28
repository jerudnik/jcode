# Agent Content Validation Strategy

This plan covers portable agent content in this repository without adding deployment policy. The current checkout contains `.jcode/skills/*/SKILL.md` and format specs for skills, permissions, prompts, and specs. Top-level portable content roots beyond `.jcode/skills` are not present yet, so the validator checks current skill content now and defines the same host-independent shape for future content roots.

## Goals

- Validate content shape, frontmatter, naming, and portability locally and in CI.
- Keep checks independent of nix-config, home-manager, launchd, secrets, runtime state, and host paths.
- Produce actionable contributor-facing errors.
- Allow vendor-neutral extension metadata without accepting arbitrary deployment policy.

## Validation scope by content type

| Content type | Current layout | Future layout | Canonical file | Required frontmatter | Required body |
| --- | --- | --- | --- | --- | --- |
| Skill | `.jcode/skills/<name>/SKILL.md` | `skills/<name>/SKILL.md` | `SKILL.md` | `name`, `description`, `allowed-tools` | At least one Markdown heading. |
| Permission | not present | `permissions/<name>/PERMISSION.md` | `PERMISSION.md` | `name`, `description` | At least one Markdown heading. |
| Prompt | not present | `prompts/<name>/PROMPT.md` | `PROMPT.md` | `name`, `description` | At least one Markdown heading. |
| Spec | not present | `specs/<name>/SPEC.md` | `SPEC.md` | `name`, `description` | At least one Markdown heading. |

## Frontmatter and naming rules

Implemented by `scripts/validate_agent_content.py`:

1. Files must begin with a simple YAML frontmatter block delimited by `---`.
2. Required fields must be present and scalar.
3. `name` must match `^[a-z0-9][a-z0-9-]*$`.
4. The parent directory name must match `name`.
5. Unsupported fields fail unless they start with `x-`, which is the vendor-neutral extension namespace.
6. The body must contain a Markdown heading.
7. Bodies must not contain host-specific user paths such as `/Users/...`, `C:\Users\...`, or the current repo root.

The parser intentionally supports only scalar `key: value` frontmatter today. That keeps the check dependency-free and makes failures easy to understand. If a future content type needs lists or nested fields, extend the validator and format docs together.

## Local and CI check plan

| Check | Local command | CI/flake placement | Failure message requirement |
| --- | --- | --- | --- |
| Agent content schema/layout | `python3 scripts/validate_agent_content.py` | CI job step; future `nix flake check` app/check when flake outputs exist | Print exact file and rule that failed. |
| Path portability | included in `scripts/validate_agent_content.py` | Same as schema check | Identify host-specific path line. |
| Generated-output purity | future catalog generation should diff generated output after running generator | CI guardrail or flake check once generator exists | Tell contributor which generator to run. |
| Documentation links | future markdown link checker when docs/spec trees exist | CI guardrail, not deployment policy | Report broken source path and link target. |
| Repository guardrails | existing scripts such as `scripts/check_dependency_boundaries.py` where relevant | CI guardrail | Keep messages actionable and repo-relative. |

## Current validation command

```bash
python3 scripts/validate_agent_content.py
```

Current expected output in this checkout:

```text
agent content validation passed (permission=0, prompt=0, skill=2, spec=0)
```

## Boundaries and non-goals

- Do not validate installation locations, activation behavior, launchd services, secrets, or machine-specific runtime state here.
- Do not require Nix, cargo, network access, or host-specific paths for the basic schema check.
- Do not infer deployment policy from content metadata. If consumers need policy, add it in the consuming repo or behind an explicitly portable metadata design task.

## Future extensions

- Add first-class format specs under `docs/skills/`, `docs/permissions/`, `docs/prompts/`, and `docs/specs/` when those content roots are introduced.
- Add a changed-content mode for CI to validate only touched content files on fast paths.
- Add generated catalog purity checks once a catalog generator exists.
- Promote the Python validator into a flake check only after the flake surface exists.
