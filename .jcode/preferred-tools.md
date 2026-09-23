# Preferred tools

- Use `agentgrep` for repository text, paths, outlines, and relationships; use Serena for language-aware references or safe symbol edits. Symbol lookup and references are trustworthy; diagnostics describe the checkout this Serena was started in, which is your worktree only when your session started there. Narrow before full-file reads.
- Batch independent reads and checks. Use `apply_patch`, `edit`, `multiedit`, or Serena edits; preserve unrelated work.
- Run Rust via `scripts/dev_cargo.sh`, the fast suite via `scripts/test_fast.sh`, and fork guardrails via `scripts/preflight.sh`.
- In self-development, use `selfdev build`, `selfdev build-reload`, `selfdev test`, and `debug_socket` for runtime or TUI checks.
- Use Nix and `scripts/remote_build.sh` for reproducible or heavy work. Read help and flake outputs instead of guessing flags.
- Use `swarm` for coordinated parallel work and `subagent` for one isolated result; `.jcode/swarm-prompt.md` owns routing.
- Keep shell execution non-interactive; never expose credentials, tokens, or secret-bearing environment output.
