---
name: plain-language
description: "Use when writing anything a human will read in this repository: commit messages, docs, plans, PR text, reports, comments, or instruction files. Also use when reviewing or rewriting existing text that is hard to understand, and when naming new things."
allowed-tools: bash, read, edit, grep, agentgrep, batch
---

# Plain language

Agent-written text in this repository has drifted into private jargon. The fix
is mechanical: use the reader's words, not new ones.

## The one rule

Before writing a term, ask: does a person seeing this repo for the first time
know what it means? If not, either use the ordinary word or define the term
once, in one place, and link to it. `docs/GLOSSARY.md` holds the definitions
that already exist. Do not add to it when an ordinary word works.

## Do not coin

A new name is a cost, not a contribution. Signs you are coining:

- A noun phrase nobody outside this session has used ("advertised-surface
  disposition", "maintenance window", "durable rail").
- A capitalized Process Word for an ordinary act ("Authorization", "Signoff",
  "Closeout", "Barrier") when "ask the user", "review", "finish", "wait"
  would do.
- Renaming an existing concept to sound more rigorous ("evidence defect" for
  "wrong test log", "prewarm harness state" for "run the tests once first").

If a concept genuinely recurs and has no ordinary name, define it once in the
owning doc and reuse it verbatim. Never let two names exist for one thing.

## Rewrite rules

Apply these to your own drafts before saving:

1. Verb first. "Record PR 138 maintenance window" -> "note when PR 138 merged".
2. Concrete object. "Synthesize the disposition" -> "write the summary of
   which APIs to keep".
3. Delete hedges that carry no information: "as appropriate", "where
   applicable", "in principle", "modulo", "it is worth noting".
4. Delete process theater: text whose only content is that process was
   followed ("this commit records the recording of..."). If a commit only
   marks that a step happened, say the step: "merge PR 138".
5. One qualification maximum per sentence. If you need more, the sentence is
   hiding a decision you have not made. Make it.
6. Prefer a number, path, or command over an adjective. "Substantially
   smaller" -> "410 lines removed".

## Commit messages

- Conventional prefix, then what changed, in words a stranger knows:
  `fix(tui): stop cursor jump on paste`, not
  `fix(ideal-base): repair blocked signoff evidence and formatting`.
- The body says what changed and why. It does not narrate the process that
  produced the change.
- No project code-names in subjects. Name the code that changed.

## Plans and reports

- Lead with the outcome in one sentence a manager could repeat.
- State facts as facts. "The build takes 21 minutes", not "the build has been
  observed to take on the order of 21 minutes under typical conditions".
- Uncertainty is fine, stated once, with what would resolve it: "I did not
  test the Windows path; running X would confirm."
- A report that a reader cannot act on is not finished. End with the decision
  needed or the next command to run.

## Test yourself

Read your text aloud as if explaining to a colleague at a whiteboard. Every
phrase you would not say out loud, rewrite as what you would say.
