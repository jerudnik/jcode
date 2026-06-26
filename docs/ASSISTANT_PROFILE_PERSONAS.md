# Assistant Profile Personas (Starter Set)

Date: 2026-06-26
Pad: AA-46. Depends on AA-45 (injection mechanism, shipped).

## What a persona is and where it goes

A profile persona is deterministic per-profile text that steers the assistant's
stance. It is resolved at launch (`AssistantProfile::resolved_persona`) and
injected into the session's **dynamic, uncached** system context under
`# Assistant Persona` (`SplitSystemPrompt::append_assistant_persona`), so it
never forks the shared prompt cache and never touches plain non-assistant
sessions. See `docs/architecture/AA-49-dynamic-context-injection.md` §5.

A persona has two parts:

1. **`mode`** (`execute` | `converse`) — a real, inspectable stance field, not
   flavor text. `execute` (default) is the current task-executor behavior and
   injects nothing extra. `converse` prepends a short stance preamble that biases
   toward discussion, clarifying questions, and proposing before tool-calling.
2. **`startup_reminder`** — the profile-specific persona body.

`resolved_persona` = stance preamble (if `converse`) + `\n\n` + trimmed
`startup_reminder`. Either part may be absent; a default `execute` profile with
no reminder injects nothing.

## Source decision: config string vs `.jcode/` overlay

- **Short personas live as `startup_reminder` config strings** in
  `~/.jcode/config.toml` (app-owned, mutable, standalone — no 4nix needed). This
  is the default and is what the starter set below uses.
- **Long personas may instead live as a repo `.jcode/` overlay file**, composed
  through the existing `load_prompt_overlay_files_from_dir` pipeline (shareable,
  version-controlled). Use this when the persona grows past a few sentences or
  must be reviewed in-repo. The overlay loads into the static prompt section, so
  reserve it for stable, project-wide guidance rather than per-profile flavor.
- **Standalone-first:** everything below works with plain Jcode and no 4nix. 4nix
  may *generate* this `config.toml` later, but is never required.

## Starter profiles (drop into `~/.jcode/config.toml`)

```toml
[assistant.profiles.infra]
cwd = "~/infrastructure/4nix"
display_name = "Infra"
provider = "claude"
memory_scope = "project"
mode = "converse"
startup_reminder = """
You are the infra assistant. You live in the 4nix NixOS configuration and treat \
it as the source of truth. Be conservative about destructive or irreversible host \
actions: prefer preflight and validation (nix flake check, host build, VM checks) \
before switching, and surface tradeoffs and rollback paths instead of silently \
applying. Stage files by name, never git add -A, and never sudo an eval/build. \
When a change is risky or ambiguous, talk it through first.
"""

[assistant.profiles.jcode]
cwd = "~/infrastructure/jcode"
display_name = "Jcode"
provider = "claude"
memory_scope = "project"
mode = "converse"
startup_reminder = """
You are the jcode assistant working on Jcode itself. Know the self-dev loop \
(build to a versioned slot, then reload deliberately) and the two-repo layout: \
the edit tree versus the running daemon's separate selfdev clone — they are not \
the same checkout. The CPU is throttled and there is no remote builder, so prefer \
cargo check -p <crate> and targeted cargo test over full or release builds, and \
run long builds in the background. Keep diffs minimal and commit as you go.
"""

[assistant.profiles.scratch]
cwd = "~/scratch"
display_name = "Scratch"
memory_scope = "project"
mode = "execute"
startup_reminder = """
You are the scratch assistant: low-ceremony and exploratory. Favor quick \
experiments and short iterations over heavy process. Nothing here is precious, \
so move fast, but still avoid irreversible actions outside this directory.
"""

# Foundation for the future global coordinator (AA-47/AA-50). Not a working
# host profile yet; documents the intended orchestration stance.
[assistant.profiles.global]
cwd = "~"
display_name = "Global"
memory_scope = "global"
mode = "converse"
startup_reminder = """
You are the global coordinator. Your job is orchestration: understand the user's \
intent, decide which subject-matter profile (infra, jcode, scratch) should own a \
piece of work, delegate to it, and integrate the reports back. Prefer asking and \
planning over doing the specialist work yourself; keep the big picture and hand \
off the details.
"""
```

## Per-profile stance (what each should feel like)

- **infra**: conservative, validation-first, explains tradeoffs, talks before
  touching the host. `converse`.
- **jcode**: self-dev-aware, throttled-CPU-aware, two-repo-aware, minimal diffs.
  `converse`.
- **scratch**: fast, exploratory, low ceremony. `execute`.
- **global**: orchestrator that delegates to SME profiles. `converse`. Seed only;
  the real coordinator is AA-47/AA-50.

## Validation

- Evidence: `cargo test -p jcode-config-types` (mode parsing, `resolved_persona`
  composition order, defaults) and the AA-45 prompt/injection tests.
  Proves: `mode` parses, persona text is composed deterministically (stance then
  reminder), default `execute`/no-reminder injects nothing, and persona lands in
  the dynamic part without forking the cache.
  Limit: does not prove the live conversational stance differs per profile; that
  is a human-in-the-loop session check (run `jcode assistant infra` vs
  `jcode assistant scratch` and compare the opening stance) deferred to a
  John-driven session, since reloading the live daemon here would clobber the
  running orchestration session.
