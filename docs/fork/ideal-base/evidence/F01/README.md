# F01 evidence: shutdown coordinator and activity-lease design

Recorded: 2026-07-18. Revision 1 at commit
`c96c4b57de57438d63e23796e6b038027265fca4`; revision 2 (current) verified
against `398b51c07d1f0545bfdccd6a33e6ea9fd76b6574` (branch `main`).
Design-only node: no source code was modified.

History: revision 1 FAILED its independent architecture critique
(`../../reviews/F01-architecture-critique.md`, reviewer OpenAI `gpt-5.6-sol`
per decision D009, recorded in D011). Revision 2 resolved those findings but
FAILED the independent re-review (`../../reviews/F01-architecture-re-review.md`,
commit `09f367098`, same reviewer route) on two narrower blockers. Revision 3
(current) resolves B-R1/B-R2/I-R1/I-R2/M-R1/M-R2; the full mapping for both
rounds is `revision_response.md`.

Contents:

- `design.md`: the design record (revision 3). It contains
  1. the source ownership census of every current exit authority and work
     class, with file:line references verified against the checkout above;
  2. the exit-reason taxonomy;
  3. the single authorities: the `jcode-core` activity-lease inversion seam,
     the serialized `ShutdownCoordinator` executor with its total reason
     lattice and coordinator-owned watchdog, the common provider-turn guard
     boundary, the real-API cleanup list, and the complete residue contract;
  4. the pure lifecycle state model with quiescence-epoch idle semantics and
     the full reason x work-class coverage matrix;
  5. the testability contract F02 must implement and F03 must verify.
- `revision_response.md`: finding-by-finding response to the FAIL review,
  citing the seven preserved worker artifacts under
  `../F01-R/worker-artifacts/`.

Verification method: every cited symbol was located with ripgrep/sed against
the working tree at the commit above. Gates for this node:

- "Pure lifecycle state model covers every normal exit and active work class":
  satisfied by the matrix in `design.md` section 4.1.
- "Independent architecture critique finds no owner/lease gap": revisions 1
  and 2 failed this gate; revision 3 awaits the second independent re-review
  (F01-V round 2), which per D009/D011 runs on the strongest available
  non-Anthropic route and must name the actual model used.
