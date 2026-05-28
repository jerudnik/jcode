# Fork Patch Records

This directory holds explicit maintenance packets for justified fork-only customizations. An empty directory, or this README with no patch records, means there are no known fork-only runtime patches justified yet.

Create one Markdown file per patch. Keep records small and auditable, and list every touched path so upstream-sync reviews can include those files explicitly.

## Maintenance packet template

```markdown
# <short patch title>

- id: fork-patch-YYYYMMDD-slug
- owner: @owner-or-team
- status: proposed | active | adapting | retired
- upstream attachment: upstream issue/PR/commit/path, or `none`
- touched paths:
  - path/from/repo/root

## Why

What local need this patch preserves.

## Behavior contract

Observable behavior that must survive upstream syncs.

## Assumptions

What must remain true for this patch to be valid.

## Validation

Commands or manual checks that prove the behavior contract.

## Drop/adapt criteria

When to remove, upstream, or redesign this patch.
```
