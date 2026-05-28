---
name: agent-toolbox
description: Global access and usage guide for the Nix agent toolbox from ~/infrastructure/nix-config/modules/flake/toolbox.nix.
allowed-tools: bash, webfetch, websearch, browser
---

# Agent Toolbox

Use when you need Nix-provided external CLIs that are not native Jcode tools.

## Access

Run a single tool from anywhere:

```bash
nix run $HOME/infrastructure/nix-config#<tool> -- <args>
```

Run several toolbox commands in one environment:

```bash
nix shell $HOME/infrastructure/nix-config#agents --command bash -c '<pipeline>'
```

Prefer built-in Jcode tools first for files, code search, edits, browser use, and web search. Use the toolbox when a specialized CLI is better.

## Tools by job

- Docs/reference: `ctx7`, `manix`, `cht-sh` (`cht.sh`), `tealdeer` (`tldr`), `nix-search-cli` (`nix-search`), `nix-index`.
- Web/research: `firecrawl`, `agent-browser`, `ddgr`, `parallel-cli` (large first-run closure; expect Nix fetch/build time).
- Code intel: `repomix`, `ripgrep-all` (`rga`), `gh`, `zat` (pass a supported source file, not `--help`).
- Data shaping: `yj`, `jo`, `stdbuf`.

## Good patterns

```bash
# Nix docs and package lookup
nix run $HOME/infrastructure/nix-config#manix -- services.nginx
nix run $HOME/infrastructure/nix-config#nix-search-cli -- ripgrep

# Quick CLI examples
nix run $HOME/infrastructure/nix-config#tealdeer -- tar
nix run $HOME/infrastructure/nix-config#cht-sh -- rust/iterator

# Web search or scraping when built-ins are insufficient
nix run $HOME/infrastructure/nix-config#ddgr -- 'site:nixos.org flakes'
nix run $HOME/infrastructure/nix-config#firecrawl -- --help

# Repo/package context for LLMs or archive/PDF search
nix run $HOME/infrastructure/nix-config#repomix -- --help
nix run $HOME/infrastructure/nix-config#ripgrep-all -- 'pattern' .
nix run $HOME/infrastructure/nix-config#zat -- src/main.rs

# JSON/YAML/TOML/HCL conversion and JSON construction
nix run $HOME/infrastructure/nix-config#yj -- -tj < file.toml
nix run $HOME/infrastructure/nix-config#jo -- key=value ok=true
```

## Guardrails

- Keep commands non-interactive and bounded.
- Never print secrets; `firecrawl` and `parallel-cli` may source API keys from local secrets wrappers.
- Use absolute flake path unless already in `~/infrastructure/nix-config`.
- Check `~/infrastructure/nix-config/modules/flake/toolbox.nix` if the inventory changes.
