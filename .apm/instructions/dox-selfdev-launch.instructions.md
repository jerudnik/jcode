---
description: Self-development launch and automatic client handoff behavior.
applyTo: "src/cli/selfdev*.rs"
---

# Self-development launch

- An automatic client handoff resumes the selected build without rebuilding or republishing a different checkout. A resume ID alone is not an automatic handoff.
- Preserve stale-build detection on fresh launches and manual resumes, the explicit no-build option, and explicit build precedence. Verify both handoff and normal launch decisions when changing this policy.
