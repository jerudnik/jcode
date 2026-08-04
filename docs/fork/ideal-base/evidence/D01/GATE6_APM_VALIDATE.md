# D01 gate 6: `apm compile --validate` and generated-surface drift

Gate 6 reads: *"Generated instructions match their .apm primitives; apm compile
--validate and drift check pass."*

Across three earlier sessions this gate was recorded as un-runnable because "apm
is not installed". That was true of the machine, not of the world. The tool is
packaged in nixpkgs and needed no installation at all:

```
nix shell nixpkgs#apm-cli --command apm --version
Agent Package Manager (APM) CLI version 0.21.0
```

`0.21.0` is exactly the version stamped into every generated surface
(`<!-- APM Version: 0.21.0 -->`), so this is the same compiler that produced the
committed artifacts, not a lookalike.

**A version trap worth recording.** My first install was `uv tool install
apm-cli`, which resolved to **0.27.0**. That copy would have compiled with a
different generator than the one that stamped the tree, so any drift it reported
would have been unattributable: a real primitive edit and a generator upgrade
produce the same red diff. It was uninstalled and removed from `PATH` before any
measurement was taken. Pinned tooling from the repo's own channel beats whatever
a package manager resolves today.

## Result: the gate has two halves and they disagree

| Half | Verdict |
|---|---|
| `apm compile --validate` | **vacuous**: passes on garbage |
| generated-surface drift | **live**: edits propagate and are detectable |

### Half 1 is vacuous, proven by mutation

`--validate` exits 0 and prints `All primitives validated successfully!` while a
primitive is destroyed. Four escalating mutations of
`.apm/instructions/dox-apm.instructions.md`, each applied to the real file and
each reverted from a `cp` backup:

| # | Mutation | Exit | Reported |
|---|---|---|---|
| A | frontmatter opener corrupted (`---` -> `--- BROKEN`) | 0 | Validated 6 primitives |
| B | YAML frontmatter block deleted entirely | 0 | Validated 6 primitives |
| C | file truncated to zero bytes | 0 | Validated 6 primitives |
| D | file overwritten with `zzz not yaml at all\x00` | 0 | Validated 6 primitives |
| E | restored | 0 | Validated 6 primitives |

The count stays at 6 whether the file is a valid instruction or a NUL byte, so
the number counts *files discovered*, not *files validated*. Mutation A was
verified to have actually landed on disk (`head -3` showed `--- BROKEN`) before
its exit 0 was accepted, because an edit that silently no-ops would produce the
same passing result for an entirely different reason.

This is the defect class this program already has a name for: **a guard that
answers "fine" regardless of input.** Same shape as `check_warning_budget.sh`
reporting `OK: current=0` while its own `rg` was missing from `PATH`, and the
same shape as the sentinel bug in `compiled_body`. A green tick from this
subcommand is not evidence and must not be cited as any.

### Half 2 is live, proven in both directions

Appending one sentinel line to a primitive and recompiling:

```
sentinel occurrences in generated .apm/AGENTS.md: 1
body sha (line 3 excluded)  before=8b1a79267921c073  after=418de34d607cff8d
restore -> 8b1a79267921c073   (matches before: YES)
```

The edit reaches the generated surface, changes its hash, and reverting restores
the original hash exactly. Content drift between a primitive and its generated
artifact *is* mechanically detectable. That is the half worth wiring into CI.

### Drift status of the working tree: none

Recompiling and diffing all six surfaces against pre-compile backups:

| File | Body (line 3 excluded) |
|---|---|
| `AGENTS.md` | identical (`96c4be3952e93415`) |
| `CLAUDE.md` | identical (`1979dab95da6b61e`) |
| `docs/AGENTS.md` | identical (`0f8c17d796fd2fc2`) |
| `.apm/AGENTS.md` | identical, byte-for-byte |
| `.apm/CLAUDE.md` | identical, byte-for-byte |
| `crates/jcode-desktop/AGENTS.md` | identical, byte-for-byte |

Three files differ on **line 3 only**, the `<!-- Build ID: ... -->` stamp. That
stamp is content-derived and stable, not a nonce: recompiling twice with zero
source edits reproduces `e9949292e458` both times. So the correct drift
comparison excludes line 3, and under it the tree has **zero drift**.

`apm compile` also generates `GEMINI.md`, which did not previously exist locally.
Like the other five surfaces it is untracked and gitignored, and `git status`
was empty after every compile in this session.

## Correction to an earlier claim of mine

I previously reported "two generated surfaces were hand-edited to match the
primitive byte-for-byte rather than regenerated", and later described this as
outstanding debt. **Both statements were wrong**, and the second was wrong in a
way that made the repository look less healthy than it is. All six surfaces are
untracked and gitignored (`.gitignore:25`); commit `240d74f2a` *removed* them
from tracking. They are build artifacts regenerated from primitives on demand,
so a hand edit cannot persist into the repository and there is nothing to drift.
Retracted.

## The drift half is already enforced, and it agrees with the real compiler

Before writing a new checker I looked for an existing one, and the repository
already has it. `scripts/check_agent_instructions.py` maps each generated surface
to its primitive and recomputes the expected body. Planting a primitive edit
*without* recompiling, so the generated file is genuinely stale:

```
checker exit=1
agent-instructions: .apm/AGENTS.md is stale; run apm compile
restore -> exit=0
```

It fails, names the offending file, and prints the exact remedy.

The check that matters is **agreement with the real compiler**, because this
script reimplements APM's output rather than invoking it, and a reimplementation
that has quietly diverged would be worse than nothing. Running `apm compile`
0.21.0 and then the checker gives `ok (projected=7433/8192, compiled=7181/8192)`,
exit 0: the checker accepts genuine APM output as canonical. It is wired into CI
at `fork-ci.yml:254` and is itself a governance-protected path
(`governance-root.yml:41`).

So gate 6's useful half is enforced today, and needed no new code. Adding a
second drift checker would have duplicated a working one.

## What remains

Wiring `apm compile --validate` into CI, as the gate's wording implies, would add
a check that **cannot fail**, which is worse than no check because it reads as
coverage. It should not be added. The gate's substance is met by
`check_agent_instructions.py`; the `--validate` clause is satisfied only in the
trivial sense that the command exits 0, and this document records why that fact
carries no weight.

## Reproduce

```
nix shell nixpkgs#apm-cli --command apm --version          # must print 0.21.0
nix shell nixpkgs#apm-cli --command apm compile --validate # exits 0 (see caveat)
: > .apm/instructions/dox-apm.instructions.md              # destroy a primitive
nix shell nixpkgs#apm-cli --command apm compile --validate # STILL exits 0
git checkout -- .apm/instructions/dox-apm.instructions.md
```
