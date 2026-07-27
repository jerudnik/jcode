# F29 — ambient filesystem roots routed through jcode-storage

## Outcome

Merged as PR #34 (`81adbf4e5`, squashed onto `main`). The inventory's 49 direct
`dirs::` root sites are down to **22**, every one allowlisted with a stated
reason and pinned `file:line` in a shrink-only ratchet.

| | before | after |
|---|---|---|
| direct `dirs::` root sites outside jcode-storage | 49 | 22 |
| swallowed-error budget | 3059 | 3034 |
| panic-prone budget | 59 | 56 |

## Class A: nine real defects

Seven were found by the original inventory:

| Site | Defect |
|---|---|
| `memory_log::log_dir` | wrote logs into the real `~/.jcode/logs` |
| copilot `machine_id` | read and created in the real home |
| `ui_changelog` | read and rewrote the real "last seen changelog" marker |
| openrouter `cache_path_for_namespace` | re-implemented the `JCODE_HOME` rule by hand, missing the harness redirect |
| openrouter `endpoints_cache_path` | no `JCODE_HOME` arm at all |
| `mobile_server::jcode_home` | full reimplementation of `jcode_dir()` minus the redirect |
| `browser::jcode_dir` fallback | re-derived the home, so the one case it existed for is where it un-isolated |

Two more surfaced only when the allowlist's placeholder reasons were audited
(see below), and were fixed in the same class:

| Site | Defect |
|---|---|
| `surface_workspace::surface_workspaces_dir` | hand-rolled `var_os("JCODE_HOME")` else `home_dir().join(".jcode")`: skipped both the harness redirect and the blank-value guard |
| `doctor::cache_path` | `dirs::home_dir().join(".jcode")`, ignoring `JCODE_HOME` outright |

Each has a regression test proven to fail on revert.

## Two defects in the surrounding tooling

**The shared quality gates counted comments and string literals as production
code.** `rust_production_filter.py` returned the *original* source, so the panic
and swallowed-error budgets matched their own patterns inside comments and
strings. Two consequences: documenting *why* a pattern is avoided cost budget,
and 3 of the 59 "panic-prone" sites were **JavaScript in a Rust string literal**
(`tool/computer/win.rs` calls `ObjC.unwrap(...)`). The masking pass already
existed for brace counting and simply was not applied to the returned lines.

**`preflight.sh` skipped the unused-dependency gate** when cargo-machete was
absent, which it is in this devShell, so a local run reported all-green while CI
failed on an orphaned `dirs` dep in `jcode-setup-hints`. Now falls back to
`nix shell nixpkgs#cargo-machete`.

## This gate was asserting something false

The gate printed `all allowlisted with a stated reason` while **20 of its 24
entries said `TODO: state why this cannot use jcode-storage`**. `allowlist_entries()`
strips `#.*` to compare sites, so the reason text was never read by anything.
`--update` seeds that TODO deliberately, and nothing forced it to be replaced.

This mattered beyond tidiness: writing the 18 real reasons is what surfaced the
two Class A defects above. Both were sitting behind a placeholder, indistinguishable
from the genuinely-intentional sites. The gate now fails on any placeholder, and
that check is proven non-vacuous in both directions (`gate-after.txt`).

Of the 22 remaining sites, the honest reasons fall into four groups:

- **`~` expansion of user-typed paths** (hooks, CLI args, login prompt, diff
  prompt): the user means their real shell home, not the sandbox.
- **External applications' real-home locations** (Firefox native-messaging
  manifests, `~/.codex/auth.json`, Cursor's config, `~/.cargo/bin`,
  `~/.config/kitty`): another program owns the path.
- **Display-only abbreviation** of the real home to `~` in rendered output:
  read-only, never resolves a root for I/O.
- **Deliberate real-home anchoring** (macOS menu bar singleton): the lock must
  share one inode across processes with different `JCODE_HOME` values, and the
  sandbox check compares `JCODE_HOME` *against* the real `~/.jcode`, so
  resolving both through storage would make the comparison always true.

## Additional defects fixed under this node

- **XDG vars bypassed the home redirect.** `linux_config_paths` honored
  `XDG_CONFIG_HOME` ahead of the redirect, so Linux CI
  (`XDG_CONFIG_HOME=/home/runner/.config`) read the real config. Overrides aimed
  inside the real home are now filtered while the home is redirected.
- **A blank `JCODE_HOME` resolved to a *relative* path**, landing under the
  current working directory. The repo had accumulated an untracked directory
  named after a literal tab containing real telemetry ids and sessions; a
  shipped binary would do the same wherever the user was standing. Fixed at one
  reader (`jcode_home_override`) across all four roots after the first fix
  addressed only the root that was noticed.
- **macOS CI flake**, two independent env races: a module guarding
  `JCODE_HOME`/`HOME` with a *private* mutex while every other module took the
  real test-env lease, and an isolation helper returning a tuple whose lease was
  destructured into locals, which drop in *reverse* declaration order. Measured
  under identical conditions, 120 focused runs each: **3 failures before, 0
  after**. macOS CI green.

## Gate status

```
ambient roots: ok (22 site(s), all allowlisted with a stated reason)
```

Full preflight passes with nothing skipped (12/12). `jcode-base` 226 and `jcode`
1209 library tests green. All three CI workflows green on `main`.

Raw output: `gate-after.txt`.
