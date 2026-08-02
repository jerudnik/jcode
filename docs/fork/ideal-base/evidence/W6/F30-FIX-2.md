# F30-FIX-2: widen the distribution-policy forbidden-token list

Reviewed commit `829a1df02`, published in merge `b88250783` (PR #90).

## The gap

The forbidden-token set was plain substrings and missed the AUR, curl-pipe, and
PowerShell-pipe install idioms entirely. A retired-channel claim phrased as
`yay -S jcode-git` or `curl ... | sh` passed the gate.

## The fix

`FORBIDDEN_ACTIVE_DOC_PATTERNS` adds seven regexes. They are regexes and not
substrings for a measured reason: the plain-substring forms collide with
ordinary prose and markdown tables. `| sh` matches a table cell, and `jcode-bin`
matches `<jcode-binary>`. Each pattern is therefore word-boundary anchored:

```text
AUR yay install          \byay\s+-S\b
AUR paru install         \bparu\s+-S\b
AUR -git package         \bjcode(?:-desktop)?-git\b
AUR jcode-bin package    \bjcode-bin\b
curl pipe to shell       \bcurl\b[^\n|]*\|\s*(?:sudo\s+)?(?:sh|bash|zsh)\b
wget pipe to shell       \bwget\b[^\n|]*\|\s*(?:sudo\s+)?(?:sh|bash|zsh)\b
PowerShell pipe to iex   \b(?:iwr|irm|Invoke-WebRequest|Invoke-RestMethod)\b[^\n|]*\|\s*(?:iex|Invoke-Expression)\b
```

Naive patterns here would have produced the opposite failure of F30-FIX-1: a
gate that fails on true-sounding *prose*, gets muted by an exemption, and then
guards nothing.

## Verification

```text
nix shell nixpkgs#python313 --command python3 -m unittest tests.test_nix_distribution_policy
Ran 13 tests in 6.444s
OK
```

`test_active_docs_do_not_document_retired_install_channels` covers this; the
patterns run against the full opt-out document set from F30-FIX-1, so the two
fixes compose rather than each covering half the surface.
