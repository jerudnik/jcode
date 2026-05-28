# Skill Format Spec

Canonical layout: `.jcode/skills/<name>/SKILL.md` today, with future portable layout `skills/<name>/SKILL.md`.

Required frontmatter:
- `name`: lowercase kebab-case identifier matching the parent directory.
- `description`: one-line summary of when to use the skill.
- `allowed-tools`: comma-separated tool names allowed by the skill.

Optional extension fields must use the `x-` prefix.

Required body: Markdown content with at least one heading. Content must avoid host-specific user paths and deployment policy.

Validation: `python3 scripts/validate_agent_content.py`.
