# W2 post-integration validation evidence

This directory preserves validation of merge commit `cc1f93847f2bab2bb27a5af0ed741e518d94457a` on `recovery/2026-07-15`.

The focused W2 matrix passed thirteen fixture commands and the affected
three-package check. The mandatory R09 matrix then matched every pre-encoded
exit without `--update`: classifier, dependency, wildcard, warning, shell
syntax, and diff check green; panic, swallowed-error, production-size, and
test-size debt visibly red.

The first combined attempt is intentionally preserved. It completed all 14
Rust checks and then stopped because the dev shell did not expose `python3`.
The Python gates were rerun with the cached arm64 pinned interpreter already
used by preserved R09 evidence. No network, provider, credential, daemon,
reload, publication, release, install, updater, live swarm, or baseline
mutation occurred.

`post-integration-results.tar.gz` contains individual command logs and exit
files. `results.txt` is the concise exit/count summary. `scope-state.txt`
records integration and preservation invariants.
