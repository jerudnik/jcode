# Preferred tools

- Start with `agentgrep` for repository text, paths, outlines, and relationships. Use Serena when language-aware references or safe symbol edits matter. Narrow before reading whole files.
- Batch independent reads, searches, and checks. Prefer `apply_patch`, `edit`, `multiedit`, or Serena edits to shell rewrites. Preserve unrelated work.
- Run Rust through `scripts/dev_cargo.sh`. Use `scripts/test_fast.sh` for the fast suite and `scripts/preflight.sh` for fork guardrails.
- In self-development, prefer `selfdev build`, `selfdev build-reload`, and `selfdev test`; use `debug_socket` for runtime and TUI verification.
- Prefer Nix and `scripts/remote_build.sh` for reproducible or heavy work. Inspect script help and current flake outputs instead of memorizing flags or checks.
- Use `swarm` for coordinated parallel work and review, and `subagent` for one isolated result. Run `swarm list_models` before selecting a non-default route; `.jcode/swarm-prompt.md` owns routing.
- Keep shell execution non-interactive. Never expose credentials, auth files, tokens, or secret-bearing environment output.
