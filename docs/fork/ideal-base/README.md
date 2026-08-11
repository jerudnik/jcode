# Ideal-base retired reference

Recorded: 2026-08-08

This directory is the archived record of the completed ideal-base program.
Use the files below when you need the accepted result, support limits, checksums,
or private recovery references.

## Find the result quickly

- [`ACCEPTANCE_STANDARD.md`](ACCEPTANCE_STANDARD.md): accepted result and support limits.
- [`BASELINE.md`](BASELINE.md): protected starting boundary and preserved inputs.
- [`DECISIONS.md`](DECISIONS.md): final decisions and reopen triggers.
- [`STATE.json`](STATE.json): frozen disposition snapshot.
- [`WORK_GRAPH.json`](WORK_GRAPH.json): frozen graph record.
- [`evidence/`](evidence/): archived node evidence and checksum manifests.
- [`../recovery/`](../recovery/): private recovery references preserved in place.

## Archive boundary

This tree is historical. It does not contain current dashboards, execution tools,
or live workflow instructions. Anything rebuildable from the active repository
has been left out of the archive where possible.

The `evidence/` index and node evidence are retained for audit trail. Missing
rebuildable fixtures, including old governance fixture files, stay retired rather
than being restored into this tree; regenerate them into `target/` or another
temporary path when an active check needs one.
