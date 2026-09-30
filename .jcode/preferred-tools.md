# Preferred tools

- Start with `agentgrep` for repository text, paths, outlines, and relationships. Use Serena for language-aware references or safe symbol edits. Narrow before reading whole files.
- Batch independent reads and checks. Prefer `apply_patch`, `edit`, `multiedit`, or Serena edits to shell rewrites. Preserve unrelated work.
- Run Rust through `scripts/dev_cargo.sh`; `scripts/test_fast.sh` for the fast suite; `scripts/preflight.sh` for fork guardrails.
- In self-development, prefer `selfdev build`, `selfdev build-reload`, and `selfdev test`; use `debug_socket` for runtime and TUI verification.
- Prefer Nix and `scripts/remote_build.sh` for reproducible or heavy work. Read script help and flake outputs instead of memorizing flags.
- Use `swarm` for coordinated parallel work, `subagent` for one isolated result. `.jcode/swarm-prompt.md` owns routing.
- Keep shell execution non-interactive. Never expose credentials, tokens, or secret-bearing environment output.
