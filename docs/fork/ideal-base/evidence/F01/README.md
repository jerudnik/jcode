# F01 evidence: shutdown coordinator and activity-lease design

Recorded: 2026-07-18 at commit `c96c4b57de57438d63e23796e6b038027265fca4`
(branch `main`, dirty worktree; only pre-existing drift plus this evidence
directory). Design-only node: no source code was modified.

Contents:

- `design.md`: the design record. It contains
  1. the source ownership census of every current exit authority and work
     class, with file:line references verified against the checkout above;
  2. the exit-reason taxonomy;
  3. the pure lifecycle state model (`LeaseTable` + `ExitDecision` +
     `ShutdownCoordinator` phases) covering every normal exit and every
     active work class, with the full reason x work-class behavior matrix;
  4. the testability contract F02 must implement and F03 must verify.

Verification method: every cited symbol was located with ripgrep/sed against
the working tree at the commit above and cross-checked against
`evidence/W0.2/source_census.md`. Gates for this node:

- "Pure lifecycle state model covers every normal exit and active work class":
  satisfied by the matrix in `design.md` section 4.1 (every row is a normal exit
  reason, every column a censused work class, every cell a defined behavior).
- "Independent architecture critique finds no owner/lease gap": deferred to the
  independent fable+opus review declared by the node; this artifact is the
  input to that critique.
