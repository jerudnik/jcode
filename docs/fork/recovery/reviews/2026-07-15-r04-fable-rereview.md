# R04 Fable bounded re-review of correction commit

Exact correction commit: `dc7d71df7be03c100338105a32e617f6d964f288` (`docs: Correct R04 lifecycle ledger.`)
Base commit reviewed against: `b4d39860abc5337c1937260af13bae45d2405d06` (`docs: Adjudicate R04 lifecycle seam.`)

Verdict: **PASS** for the bounded correction review.

Scope: I reviewed only the `dc7d71df7` correction over `b4d39860a`, focused on my prior C1/I1. I did not read any Sol rereview. Repository/source use was read-only; no daemon, network, credentials, source mutation, or test execution was used. This file is the only write.

## Remaining CRITICAL/IMPORTANT findings

None within the bounded C1/I1 correction scope.

## Verification against prior C1/I1

- **Guarded versus unsafe paths distinguished:** PASS. The amendment explicitly limits save-before-conditional-consume to the guarded crash-marker scan and separately marks `Session::reconcile_dead_owner` unsafe: ledger `:168-174`. This matches source at `session.rs:1070-1104`, `active_pids.rs:67-78`, guarded helper `active_pids.rs:167-178,251-284`, and crash scan `session/crash.rs:331-390`.
- **False invariants and census superseded:** PASS. The amendment supersedes the earlier universal durable-before-marker invariant, checkpoint 3 enumeration claim, incomplete terminal-writer table, save-before-consume failure-mode claim, and negative double-owner finding: ledger `:161-166`.
- **Effective R04 state blocked:** PASS. The amendment states effective R04 ledger state is `blocked pending a separate source fix and fixtures` and says `retain-fork` is not approval of the current unsafe terminal path: ledger `:153-159`. It also forbids citing strict pilot exclusion as R04 sign-off: ledger `:204-212`.
- **Disconnect cleanup added:** PASS. The corrected terminal-writer census includes `cleanup_client_connection`, `Agent::mark_closed`, and `Agent::mark_crashed`, marks the branch blocked, and calls out timeout separately: ledger `:176-187`. Source supports this chain at `client_disconnect_cleanup.rs:73-200` and `agent.rs:970-1012`.
- **Fix and fixtures adequate:** PASS. The required separate source fix and acceptance cases cover the missing surfaces: source refactor A, save-failure fixture B, replaced-marker fixture C, disconnect crash/reload fixture D, disconnect closed and lock-timeout fixtures E, and docs/sign-off follow-through F: ledger `:189-202`. They are separated enough to prevent hiding one failure class inside another.
- **No source fix smuggled into correction:** PASS. `git diff --name-only b4d39860a..dc7d71df7 -- crates src tests scripts Cargo.toml Cargo.lock` returned no paths. The correction is documentation/preservation only.
- **Sign-off preservation:** PASS. The Fable sign-off repository copy hashes to `1e4fdd9b35103e028b1ffdbb68a7bf22404d9d7b62c45e5425036b7387409103`, matching `/tmp/jcode-r04-fable-signoff.md`, and `cmp -s` passed. The Sol sign-off was hash/cmp-checked without reading content: both external and repository copy hash to `91378d6032426ba1d1a1cf13085f4caf56332fc27498742a7791da3e308dfe0d`, and `cmp -s` passed.

Minor note: the original metadata rows at ledger `:5-14` remain append-only historical metadata, so readers must use the corrective amendment's effective state at `:153-159`. Because the amendment explicitly supersedes contrary claims, I do not consider this an IMPORTANT finding.

## Commands run

```bash
pwd && git rev-parse HEAD && git show -s --format='%H %s' b4d39860a dc7d71df7 && git diff --name-status b4d39860a..dc7d71df7 | grep -vi 'sol'
git diff --stat b4d39860a..dc7d71df7 -- ':!**/*[Ss]ol*' ':!**/sol/**'
git diff --name-only b4d39860a..dc7d71df7 -- ':!**/*[Ss]ol*' ':!**/sol/**'
git diff --unified=80 b4d39860a..dc7d71df7 -- docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md
git show dc7d71df7:docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md | nl -ba | sed -n '1,260p'
git show dc7d71df7:docs/fork/recovery/reviews/2026-07-15-r04-fable-signoff.md | shasum -a 256
shasum -a 256 /tmp/jcode-r04-fable-signoff.md
git show dc7d71df7:docs/fork/recovery/reviews/2026-07-15-r04-fable-signoff.md | cmp -s /tmp/jcode-r04-fable-signoff.md -
# Hash/cmp only, no content read:
shasum -a 256 /tmp/jcode-r04-sol-signoff.md
git show dc7d71df7:docs/fork/recovery/reviews/2026-07-15-r04-sol-signoff.md | shasum -a 256
git show dc7d71df7:docs/fork/recovery/reviews/2026-07-15-r04-sol-signoff.md | cmp -s /tmp/jcode-r04-sol-signoff.md -
git diff --name-only b4d39860a..dc7d71df7 | grep -i 'sol.*rereview\|rereview.*sol' || true
git diff --name-only b4d39860a..dc7d71df7 -- crates src tests scripts Cargo.toml Cargo.lock 2>/dev/null || true
git show dc7d71df7:crates/jcode-base/src/session.rs | nl -ba | sed -n '1035,1104p'
git show dc7d71df7:crates/jcode-storage/src/active_pids.rs | nl -ba | sed -n '67,78p;167,178p;251,284p'
git show dc7d71df7:crates/jcode-app-core/src/server/client_disconnect_cleanup.rs | nl -ba | sed -n '73,200p'
git show dc7d71df7:crates/jcode-app-core/src/agent.rs | nl -ba | sed -n '970,1012p'
```

## Confidence and gaps

Confidence: **high** that the correction resolves my prior C1/I1 at the ledger level and correctly blocks effective R04 source sign-off pending a separate fix. Confidence is **medium-high** that the listed fixtures are sufficient, because they cover the observed control-flow classes but have not yet been implemented or executed.

Gaps: I did not read Sol rereview, run Cargo tests, execute a daemon, or validate any future fixed behavior. This PASS is only for the bounded documentation correction at `dc7d71df7`, not for the unfixed R04 source implementation.
