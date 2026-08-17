---
title: "Investigate: swarm DM delivery to a mid-turn agent"
status: open
priority: medium
owner: unassigned
opened: 2026-08-17
---

# Investigate: swarm DM delivery to a mid-turn agent

**Mode: investigation and written plan only. Do not change code, do not send
messages to the live sessions, do not mutate swarm state.** The deliverable is
a plan document. Implementation is a separate, later session.

## Why you are here

Two jcode sessions were working the same repo (`$WORKTREE_PRIMARY`,
GitHub `jerudnik/jcode`) and needed to coordinate to avoid a collision. One
(`piglet`) was about to open a repo-wide governance window; the other
(`retriever`, `session_hibiscus_1786957248681_6b38b29947ce2b74`) was mid-task
and, as it turned out, checked out on the first session's open PR branch with
uncommitted work in it. A DM was the right mechanism and the coordination
ultimately succeeded. It succeeded because **the human noticed and manually
prompted the recipient**, which is the part we want to remove.

Two distinct defects showed up. Treat them separately; they may have nothing to
do with each other.

### Defect A: the message arrived but did not become actionable

`piglet` sent a DM at `2026-08-17T09:16:29Z` with `delivery: notify` to
`retriever`, which at that moment was `running`, ~14 minutes into a turn,
`Activity: thinking`. The tool returned success.

The recipient did not act on it. The human reported seeing the message text
appear in the recipient's *thinking* history, but no response or behavior
change followed. The human then sent a manual message telling it to go read
the DM, and only then did it respond. Its own words afterward: *"Just read
your DM (via your context — my user flagged it)."*

So the payload reached the recipient's context but did not produce either an
actionable interrupt or a queued item the model handled at the next turn
boundary. Establish what `notify` is actually specified to do to a recipient in
each state (`ready`/idle, `running` mid-turn, `stopped`), and where the
observed behavior departs from it. Do the same for `interrupt` and `wake`, so
the comparison is a matrix rather than one anecdote.

The sender chose `notify` deliberately, to avoid derailing a long turn. If the
honest answer is "`notify` cannot be reliable for a mid-turn recipient and
`interrupt` is the only delivery with an at-least-once guarantee", that is a
fine finding, but then the tool description is misleading and the fix may be
documentation plus a warning rather than mechanism.

### Defect B: the recipient's copy was truncated

The recipient reported: *"my copy is truncated after the #152 paragraph."* The
body sent was roughly 4.7k characters of Markdown. The recipient acted on the
first section only and had to ask for the rest to be resent.

Verified truncation sites, both found by reading the source, neither yet
confirmed as the one that fired:

- `crates/jcode-swarm-core/src/lib.rs:503` `normalize_completion_report()`,
  bounded by `MAX_SWARM_COMPLETION_REPORT_CHARS`, appends the marker
  `"\n\n[Report truncated by jcode before delivery.]"`. Named for *completion
  reports* — determine whether DM bodies traverse it at all.
- `crates/jcode-protocol/src/comm_format.rs:517` `MAX_REPORT_CHARS: usize = 4000`,
  same shape, separate constant.

Note the arithmetic problem with both candidates: the recipient says it was cut
after a passage that sits roughly 1.2k–2.0k characters into the body, which is
well under 4000. So either the cut happened somewhere neither of these
explains, or the recipient's description is imprecise. **Resolve this against
the control log rather than by reasoning about it.** If a third truncation path
exists, that is the actual finding.

Also determine whether the truncation marker was present in the delivered copy.
The recipient described the message as truncated but did not quote a marker. A
silent truncation and a marked one are very different defects: a marked one
lets the recipient ask for the rest, which is what eventually happened here,
possibly by luck.

Related, and worth checking while you are in this code: `tldr` is mandatory
over `SWARM_TLDR_REQUIRED_OVER_CHARS` (240) and capped at
`MAX_SWARM_TLDR_CHARS` (200), and the tool description says recipients "see the
tldr collapsed with an expand control instead of the full body". An expand
control is a human affordance. Establish what a non-interactive agent recipient
actually receives, and whether "expand" is reachable for it at all. If it is
not, then long DMs are structurally lossy for agent recipients and the 240/200
design is optimized for the wrong consumer.

### Lead C: possible state bleed between sessions

Lower confidence, include only if the evidence supports it. In `swarm list`
output taken at `09:15:25Z`, the `Report:` field shown for `retriever` was
verbatim text that `piglet` had produced in its own conversation ("Both done,
and the failed job is the expected one. **The failure is `Governance Root` on
#152, by design.**"). Also `retriever` was listed as the swarm coordinator
while `piglet`, which had been operating as the primary session, was listed as
an ordinary member.

This may be benign: a shared report slot, a display fallback, or an artifact of
how the human seeded the second session. But if report fields genuinely
cross-populate between sessions, that is a correctness bug in the registry with
privacy implications, and it also means agents can silently read each other's
state in ways neither expects. Confirm or dismiss it explicitly.

## Where to look

- `crates/jcode-swarm-core/src/lib.rs` (835 lines) — tldr validation, report
  normalization, truncation, name handling.
- `crates/jcode-swarm-core/src/control_log.rs` — the control log, which is the
  authoritative record of what was actually enqueued and delivered. Find its
  on-disk location. The swarm keyed at `$WORKTREE_PRIMARY/.git` was at
  control log offset `5786201` as of `09:15:25Z`.
- `crates/jcode-protocol/src/comm_format.rs` — the second truncation site.
- The `swarm` tool's `dm`/`message`/`broadcast` handlers and whatever maps
  `delivery` onto notify/interrupt/wake behavior. Follow it through to the
  point where a recipient's turn loop consumes a pending item.
- The two sessions' own transcripts. `piglet` is the session that wrote this
  brief; `retriever` is `session_hibiscus_1786957248681_6b38b29947ce2b74`.
  Both DMs (`09:16:29Z`, `09:23:39Z`) and the relayed reply (`09:20:58Z`) are
  in the record.

Prefer the control log and the transcripts over inference from source. The
question "what did the recipient actually receive" is answerable from
artifacts, and reading the sending code will tell you what *should* have
happened, which is the thing under suspicion.

## Constraints

- **Plan only.** No code edits, no config changes, no commits.
- **Do not DM, interrupt, wake, stop, or otherwise touch `piglet` or
  `retriever`.** Both are live. `piglet` holds two open PRs and a pending
  governance transaction that temporarily drops a required status check from
  `main`; `retriever` has committed to staying off git remote operations until
  told otherwise. Interfering could land a change with the rail down.
- If you need to observe delivery behavior empirically, **spawn your own
  throwaway pair of agents in a scratch directory** and instrument that. Do not
  experiment on the live pair.
- The repo has an active second writer. Do not create branches, do not push,
  and do not run anything that writes to `$WORKTREE_PRIMARY`.
  `retriever` works in a separate worktree at `$WORKTREE_GUARDRAIL`;
  leave that alone too.
- `gh` in that checkout had no default repo and silently resolved to
  `upstream` (`1jehuang/jcode`) rather than the fork. It has since been pinned,
  but pass `--repo jerudnik/jcode` explicitly for anything that matters.

## Deliverable

A written plan, in this directory, containing:

1. **What actually happened**, per defect, grounded in log or transcript
   evidence, with the specific code path that produced it. Where you could not
   determine something, say so; do not fill the gap with a plausible mechanism.
2. **The delivery-semantics matrix**: for each of `notify`/`interrupt`/`wake`
   crossed with each recipient state, what is specified, what is implemented,
   and what is observed. Mark cells you did not verify.
3. **Options, with tradeoffs.** Expect tension between "coordination messages
   must not be lost" and "a long turn must not be derailed by every ping". A
   plausible shape is a delivery that is guaranteed to be *seen at the next
   turn boundary* even if not immediate, plus a bounded queue, plus never
   silently dropping or truncating. Do not commit to that shape if the evidence
   points elsewhere.
4. **A recommendation**, with your confidence and what would change your mind.
5. **What you would *not* do**, and why. "Leave `notify` as is and fix the
   documentation" is a legitimate recommendation if the mechanism is sound.

## Standard of proof

This repo has a recent, well-documented weakness for checks and signals that
cannot fail being mistaken for ones that pass: a scheduled workflow reported
success for 19 consecutive days while its script exited 2; a guard harness once
counted crashes as proof; a measurement in this very investigation's parent
session returned exit 127 (`command not found`) for four guards and was briefly
read as four genuine failures. Three instances in one day.

Your subject matter is the same class of defect. A notification mechanism that
delivers into a context nobody reads is indistinguishable from one that drops
the message. So: whatever you propose, state how it would be *demonstrated
failing* — a test that goes red when delivery is lost — not merely observed
working once. A fix whose success criterion is "we sent one and it seemed to
arrive" reproduces the bug at the level of the test suite.

If your investigation contradicts the framing above, the framing is wrong, not
the evidence. Say so plainly and stop rather than making the evidence fit.

---

## Addendum: further evidence gathered after the brief was written

Three things emerged while the sending session continued working. All of it
sharpens Defect B in particular, and one item probably *is* the mechanism.

### B's likely mechanism is collapse-at-240, not truncation-at-4000

The `swarm dm` tool rejected a 247-character body with this error:

    'tldr' is required for this DM because the body is 247 chars (over 240).
    Add a one-line 'tldr' (under 200 chars) summarizing it; recipients see the
    tldr collapsed with an expand control.

Read that last clause literally: for any body over 240 characters, what the
recipient is shown is **the tldr, collapsed, with an expand control**. An expand
control is a mouse affordance. If a non-interactive agent recipient cannot
actuate it, then every DM over 240 characters is delivered to an agent as a
≤200-character summary plus, apparently, some rendered prefix of the body.

That fits the observed symptom far better than either truncation constant. The
recipient described a 4.7k-character message as "truncated after the #152
paragraph", a cut roughly 1.2–2.0k characters in, which matched neither
`MAX_REPORT_CHARS` (4000) nor `MAX_SWARM_COMPLETION_REPORT_CHARS`. A
prefix-plus-collapse render explains it without needing a third truncation site.

**Revised priority: establish what an agent recipient receives for a body over
240 characters, and whether "expand" is reachable for it at all.** If it is not,
the 240/200 design is optimized for a human reading a UI, and agent-to-agent
DMs are lossy by construction above 240 characters. That is a design question,
not a bug, and it is the highest-value thing in this investigation.

Both truncation sites named earlier remain worth confirming, but they are now
secondary.

### The same payload failed to arrive twice

The four findings were resent verbatim at `09:23:39Z`. At `09:31:14Z`, roughly
eight minutes later, the recipient sent a substantial message that closed with:
*"The four truncated items from your earlier DM are still outstanding on my
end."* So the resend did not land either, or landed and was not surfaced.

Two consecutive failures on the same channel with `delivery: notify` moves this
from anecdote to reproducible. Both sends returned success to the caller.

### A comparative data point was deliberately generated

To give this investigation a controlled comparison, the sender then delivered
the same information a third time, changing two variables on purpose:

1. **Content moved out of the channel.** The full reply was written to
   `/tmp/piglet-retriever-findings.md` and the DM reduced to a pointer. This is
   the artifact-based dataflow the swarm tool description itself recommends over
   chat, and it is the workaround any fix should make unnecessary.
2. **`delivery: interrupt`, body held under 240 characters** so that the
   collapse mechanism is excluded and the trial isolates delivery semantics.

Sent at `09:34:49Z`, with an explicit request to acknowledge receipt. Compare
the recipient's handling of that against the two `notify` sends at `09:16:29Z`
and `09:23:39Z`. Whether the ack arrives, and how promptly relative to the
recipient's turn boundary, is the cleanest signal available on Defect A. The
control log should show all three sends; the difference in what the recipient
did with each is the finding.

Note the asymmetry this implies for the plan: if `interrupt` is the only
delivery with an at-least-once property, then the mode a considerate sender
reaches for, `notify`, is the unreliable one. That is a footgun shaped exactly
like the rest of this repo's incidents, where the careful choice is the one that
silently does nothing.
