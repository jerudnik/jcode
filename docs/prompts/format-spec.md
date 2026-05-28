# Prompt Format Spec

Canonical future layout: `prompts/<name>/PROMPT.md`.

Required frontmatter:
- `name`: lowercase kebab-case identifier matching the parent directory.
- `description`: one-line summary of the prompt purpose.

Optional extension fields must use the `x-` prefix.

Required body: Markdown content with at least one heading. Content must avoid host-specific user paths and deployment policy.

Validation: `python3 scripts/validate_agent_content.py` once prompt files exist.
