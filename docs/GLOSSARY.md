# Glossary

Definitions for terms that already exist in this repository's history and
docs. When you meet an opaque term in a commit or plan, look here. When an
ordinary word works, use it and do not add entries.

| Term | Plain meaning |
|---|---|
| durable rail | A long-lived branch. There is exactly one: `main`. |
| fork-point | Immutable git tag marking where this repo diverged from upstream. |
| maintenance window | The short, recorded span in which branch protection is relaxed to land a change to a protected path, then restored byte for byte. A real procedure with a real cost, not a log note. |
| signoff | A reviewer's approval, usually with a test log attached. |
| closeout | Finishing and archiving a piece of work. |
| advertised surface | The public API a crate exports. |
| typed artifact | The structured report (findings, evidence, confidence, ...) a swarm worker must file when finishing a task. |
| ratchet | A CI check that forbids a metric from getting worse. |
| DOX contract | The generated `AGENTS.md` files. Edit `.apm/instructions/*` instead; `apm compile` regenerates them. |
| APM | The tool that compiles `.apm/instructions/*.instructions.md` into `AGENTS.md`. |

Retired vocabulary: prefer the plain column above in all new writing. Do not
coin successors. See the shared `plain-language` skill deployed to `~/.agents/skills`.
