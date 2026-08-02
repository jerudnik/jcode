# F30-FIX-1: close the distribution-policy allowlist opt-in hole

Reviewed commit `5fdd4457b`, published in merge `b88250783` (PR #90).

## The gap, as F27 reproduced it

`tests/test_nix_distribution_policy.py` scanned only the 8-file
`ACTIVE_DISTRIBUTION_DOCS` tuple. 31 other active install/update/distribution
documents were unguarded, so a retired-channel claim added to any of them
passed the gate. The policy said "no non-Nix distribution channels"; the test
enforced that on eight files it had been told about.

This is the program's defect class exactly: a gate that reports a true-sounding
pass because the thing it did not look at cannot fail it.

## The fix: opt-out, not opt-in

`tracked_active_documents()` now walks the tree and returns every active `.md`
plus everything under `.apm/instructions`, subtracting only explicit
`SKIPPED_DIRECTORIES`, `UNSCANNED_PREFIXES`, `PROHIBITION_DOCS`, and
APM-generated agent contracts (detected by reading the generated marker out of
the file head, not by matching a path).

The walk uses `rglob` rather than shelling to `git ls-files` deliberately: this
suite also runs inside the hermetic `nix-distribution-policy` derivation, whose
sandbox has neither a `.git` directory nor a `git` binary. A `git`-based
implementation would have passed locally and been vacuous in the sandbox, which
would have reintroduced the same defect one layer down.

## Verification

```text
nix shell nixpkgs#python313 --command python3 -m unittest tests.test_nix_distribution_policy
Ran 13 tests in 6.444s
OK
```

Control observed failing before the fix: an isolated `.apm/instructions/**`
plant is caught by the opt-out walk and was invisible to the 8-file allowlist.

## Not verified

- The exemption for generated agent contracts is a real hole by construction:
  text inside an APM-generated `AGENTS.md`/`CLAUDE.md` is not scanned. It is
  bounded by the generated-marker check, so a hand-written file cannot claim
  the exemption, and the owning `.apm/instructions/**` primitive *is* scanned.
