# Ideal-base evidence namespace

This namespace is retained as the retired ideal-base evidence index. Store any
future addenda under one directory per work-graph node:

```text
evidence/<node-id>/
```

Each directory should contain a concise `README.md` stating the reviewed commit,
commands, outcomes, residue checks, and any external evidence references. Add
`SHA256SUMS` when retaining multiple logs or fixtures. Do not copy large
rebuildable outputs, do not restore deleted rebuildable fixtures, and do not
modify frozen normalization/recovery evidence.
