# Config layering: declarative policy, durable taste, ephemeral session state

Status: Proposal foothold — spec for the overlay feature (Nix cutover blocker)

## Problem

Three forces collide on `~/.jcode/config.toml`:

1. **Declarative management wants to own it.** The Home Manager module writes
   `config.toml` as a read-only nix-store symlink when `settings`/`configFile`
   is set. But jcode mutates its own config at runtime: `Config::save()` has
   ~14 call sites (default model switch, copilot premium, hotkey import,
   trusted sources, …). Under a read-only symlink every one of those paths
   fails. Full declarative ownership is therefore wrong, not just
   inconvenient.
2. **Runtime writes race each other.** Every save is load → patch → save of
   the whole file with no locking. Two concurrent sessions (multi-surface
   operation is normal now) can interleave and last-writer-wins the entire
   file. Mostly benign today, but it is a real race, and it grows worse as
   more surfaces (web/gateway clients) gain config mutation.
3. **Not all keys have the same lifecycle.** Observed key classes in a real
   operator config (~120 accumulated keys):
   - **Policy**: providers, endpoints, memory backend, websearch engine,
     feature flags — fleet-wide invariants that should follow the machine,
     not the whim of a session. Naturally nix-owned.
   - **Durable taste**: keybindings, display tuning, notifications, launch
     hotkeys — mutated intentionally and occasionally, from any surface.
   - **Ephemeral**: model/effort/tier switches mid-session — today these
     write the *global* file, which is both the race amplifier and a
     semantics bug (switching model in one TUI should not retarget every
     future session).

## Design

### Layer model

```
effective config = defaults
                 ⊕ policy layer      (read-only, declaratively managed)
                 ⊕ durable layer     (mutable config.toml, jcode-owned)
                 ⊕ session overrides (in-memory / session state, never persisted globally)
```

`⊕` = per-key merge, later layer wins, with one exception: keys *declared* in
the policy layer are **pinned** — the durable layer cannot override them, and
runtime attempts to save a pinned key are rejected with a visible
warning naming the policy source (WarnOnce, same pattern as WI-4 unknown-key
warnings).

### File layout

- `$JCODE_HOME/config.nix.toml` — policy layer. Read-only symlink is fine;
  jcode never writes it. Name is convention only; discovery is: if the file
  exists, it is loaded. (Alternative spelling `config.d/*.toml` rejected for
  now: one policy source is enough, and glob-merge ordering is a foot-gun.)
- `$JCODE_HOME/config.toml` — durable layer, exactly as today. Stays mutable,
  stays jcode-owned. The HM module stops writing it.
- No new file for session overrides — they live where session state already
  lives (server-side session records), which also makes them visible to
  remote surfaces via the existing protocol (`set_model`, `set_reasoning_effort`
  etc. already exist as session-scoped requests; the bug is that some of their
  TUI paths *also* call `Config::save`).

### Merge + provenance semantics

- Merge at deserialization time in `Config::load()`: parse policy file (if
  present), parse durable file, deep-merge tables per-key.
- Track provenance per top-level key (enum: Default | Policy | Durable) —
  cheap, one map, populated during merge.
- `Config::save()` writes **only durable-owned keys**: serialize the effective
  config minus policy-pinned keys minus values still at default. This fixes
  symlink-clobber and shrinks the write surface at once.
- Saving a policy-pinned key: skip the key, warn once per process
  ("config: `providers.omlx` is managed by config.nix.toml; runtime change
  ignored on save"). In-memory mutation still applies for the current process
  where that is the existing behavior (do not break live semantics silently).

### Concurrency fix (rides along)

`Config::save()` gains advisory file locking (`flock` on a sidecar
`config.toml.lock`) around the load-patch-save cycle, and call sites move to
key-scoped patch helpers (the existing `set_default_model`-style helpers
already reload-patch-save; the change is making that the *only* write path
and locking it). This closes the last-writer-wins race between sessions.

### Home Manager module change (fork repo, `nix/modules/home-manager.nix`)

- `programs.jcode.settings` → renders to `config.nix.toml` (policy layer)
  instead of `config.toml`.
- New escape hatch `programs.jcode.manageConfigToml = true` for users who
  genuinely want full ownership (accepts the write-path breakage; assertion
  message documents it).
- `configFile` keeps meaning "the whole config, pre-authored" and also moves
  to the policy path unless `manageConfigToml` is set.

### Migration

- On first run with a policy file present, jcode logs the merged provenance
  summary once (info-level): how many keys from policy vs durable.
- The nix-config side ships the policy file with exactly the keys currently
  in the repo module (providers, websearch, memory, features, ambient,
  compaction, tools, autoreview/autojudge, display.auto_server_reload,
  provider.stream_idle_timeout_secs) — everything else in the operator's
  live config.toml is untouched durable state. Cutover becomes additive.

## Interaction with the web/remote surface work

Remote config mutation (set model / toggle features from a phone or web
cockpit — lanes 2 and 6 of the web-exploration reports) is only safe once
ephemeral session overrides are first-class: the remote client mutates
session state through existing requests, never the durable file. The policy
layer additionally gives remote surfaces a trustworthy read-only "what is
managed here" view. The provenance map should therefore be exposed on the
protocol (small addition to the state/config introspection response) so
surfaces can render pinned keys as locked.

## Related: stream-idle policy is a config-shape symptom

`provider.stream_idle_timeout_secs` is one global scalar, but the correct
value differs by *role*: interactive foreground sessions want fast dead-model
detection (~3 min); background swarm workers on flaky networks want patience
(~10 min). This proposal's layer model does not solve that (it is one key),
but the fix belongs in the same family: role-aware defaults (foreground vs
subagent) with the global key as fallback. Tracked as a separate foothold —
see `stream-idle-roles` (proposals/) once seeded.

## Acceptance criteria

- [ ] `Config::load()` merges `config.nix.toml` under `config.toml`, with
      per-key provenance.
- [ ] `Config::save()` never writes policy-pinned or default-valued keys, and
      takes an advisory lock.
- [ ] Runtime save of a pinned key warns once, names the policy file.
- [ ] HM module writes the policy path; `config.toml` untouched by default.
- [ ] Existing behavior unchanged when no policy file exists (bit-for-bit
      save output on a config with no policy layer).
- [ ] Unit tests: merge precedence, pinned-key rejection, lock contention
      (two writers), provenance map correctness.
- [ ] Docs: `docs/NIX.md` + HM module option docs updated.
