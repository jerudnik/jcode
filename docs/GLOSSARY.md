# Glossary

Definitions for terms that already exist in this repository's history and
docs. When you meet an opaque term in a commit or plan, look here. When an
ordinary word works, use it and do not add entries.

| Term | Plain meaning |
|---|---|
| ideal-base | A finished 2026 code-cleanup project. Historical; no ongoing obligations. |
| durable rail | A long-lived branch. There is exactly one: `main`. |
| fork-point | Immutable git tag marking where this repo diverged from upstream. |
| maintenance window | The time span in which a PR was merged. A log note, nothing scheduled. |
| signoff | A reviewer's approval, usually with a test log attached. |
| closeout | Finishing and archiving a piece of work. |
| barrier (B0..B3) | A point in the modernization plan where all prior tasks must finish before later ones start. |
| H-node (H10, H30, H31) | A step in the modernization plan that stops and asks the user for permission (to publish, merge, or release). |
| Modernization-Node trailer | A `Key: value` line in a commit message tying the commit to a task-graph node, so a restarted run knows what is done. |
| advertised surface | The public API a crate exports. |
| typed artifact | The structured report (findings, evidence, confidence, ...) a swarm worker must file when finishing a task. |
| ratchet | A CI check that forbids a metric from getting worse. |
| DOX contract | The generated `AGENTS.md` files. Edit `.apm/instructions/*` instead; `apm compile` regenerates them. |
| APM | The tool that compiles `.apm/instructions/*.instructions.md` into `AGENTS.md`. |

Retired vocabulary: prefer the plain column above in all new writing. Do not
coin successors. See `.jcode/skills/plain-language/SKILL.md`.
