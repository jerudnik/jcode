# Ideal-base evidence namespace

Store bounded accepted evidence under one directory per work-graph node:

```text
evidence/<node-id>/
```

Each directory should contain a concise `README.md` stating the reviewed commit,
commands, outcomes, residue checks, and any external evidence references. Add
`SHA256SUMS` when retaining multiple logs or fixtures. Do not copy large
rebuildable outputs or modify frozen normalization/recovery evidence.
