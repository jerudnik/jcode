# Independent Sol re-review: R04 correction commit

Verdict: **PASS**

Reviewed SHA: `dc7d71df7be03c100338105a32e617f6d964f288`

Base SHA: `b4d39860abc5337c1937260af13bae45d2405d06`

Scope: bounded review of only correction commit `dc7d71df7` over `b4d39860a`. I used my preserved failed sign-off and only the Fable CRITICAL/IMPORTANT finding blocks as the source of already-named issues. I did not broaden to unrelated R04 ledger claims. Repo/source remained read-only.

## Remaining IMPORTANT findings

None.

## Remaining CRITICAL findings

None.

## Verification notes

- **Append-only correction:** `git diff --name-only b4d39860a dc7d71df7` shows only three added/changed docs paths: the Sol failed sign-off copy, the Fable failed sign-off copy, and the R04 ledger. The ledger diff is an appended amendment beginning at `docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md:137`; it says it supersedes only contrary claims and does not rewrite commit `b4d39860a` at lines `139-140`.
- **Failed sign-offs preserved byte-identically:** the amendment records Sol SHA-256 `91378d6032426ba1d1a1cf13085f4caf56332fc27498742a7791da3e308dfe0d` and Fable SHA-256 `1e4fdd9b35103e028b1ffdbb68a7bf22404d9d7b62c45e5425036b7387409103` at ledger lines `144-149`. I verified those hashes and `cmp -s` against `/tmp/jcode-r04-sol-signoff.md` and `/tmp/jcode-r04-fable-signoff.md`.
- **Universal save-before-consume overclaim superseded:** the amendment marks the prior invariant and checked-failure-mode claim as superseded at lines `161-166`. It narrows the guarded claim to `session/crash.rs` only at line `172`, marks `Session::reconcile_dead_owner` unsafe at line `173`, and this matches source: `Session::mark_crashed` unregisters before `reconcile_dead_owner` saves at `crates/jcode-base/src/session.rs:1040-1104`, while `unregister_active_pid` unconditionally removes markers at `crates/jcode-storage/src/active_pids.rs:67-78`.
- **Complete-census overclaim corrected:** the amendment explicitly supersedes the complete-census claim and omission of disconnect cleanup at lines `161-166`, then includes `Client disconnect cleanup` in the corrected census at line `182`.
- **Client disconnect cleanup accurately enumerated:** line `174` describes `cleanup_client_connection`, `Agent::mark_closed`, `Agent::mark_crashed`, swarm membership cleanup, signal cleanup, and the timeout branch. This matches `client_disconnect_cleanup.rs:73-130,171-251` and `agent.rs:970-1012`.
- **Effective state blocked:** lines `155-159` state effective R04 ledger state is `blocked pending a separate source fix and fixtures`, and line `226` records Sol `FAIL`, Fable `FAIL`, Terra correction `blocked`, with fresh review required.
- **Separate fix/fixtures adequate:** lines `189-202` require a separate source `fix` commit plus separate save-failure, replaced-marker, disconnect crash/reload, disconnect closed, lock-timeout, and docs follow-up cases with stop/rollback conditions. The fixtures are specific enough to cover both named sign-off issues without conflating docs correction with source repair.
- **Strict-pilot scope preserved without approval claim:** lines `204-212` preserve the strict no-tool/no-lifecycle pilot as a scope fact only, explicitly saying it is not evidence the source defect is harmless, not a basis to waive it, and not an R04 sign-off.

## Commands run

```bash
pwd
git rev-parse HEAD
git status --short
git log --oneline --decorate -3
git rev-parse dc7d71df7 b4d39860a
git diff --stat b4d39860a dc7d71df7
git diff --name-only b4d39860a dc7d71df7
git show --name-only --format='%H%n%s%n%b' dc7d71df7
git diff --unified=80 b4d39860a dc7d71df7 -- docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md
nl -ba docs/fork/recovery/reviews/2026-07-15-r04-sol-signoff.md | sed -n '1,80p'
awk '... IMPORTANT/CRITICAL finding blocks only ...' docs/fork/recovery/reviews/2026-07-15-r04-fable-signoff.md
shasum -a 256 docs/fork/recovery/reviews/2026-07-15-r04-sol-signoff.md docs/fork/recovery/reviews/2026-07-15-r04-fable-signoff.md
shasum -a 256 /tmp/jcode-r04-sol-signoff.md /tmp/jcode-r04-fable-signoff.md
cmp -s /tmp/jcode-r04-sol-signoff.md docs/fork/recovery/reviews/2026-07-15-r04-sol-signoff.md
cmp -s /tmp/jcode-r04-fable-signoff.md docs/fork/recovery/reviews/2026-07-15-r04-fable-signoff.md
nl -ba docs/fork/recovery/seams/R04-session-process-background-lifecycle/ledger.md | sed -n '132,240p'
nl -ba crates/jcode-base/src/session.rs | sed -n '1035,1104p'
nl -ba crates/jcode-storage/src/active_pids.rs | sed -n '60,80p;167,178p;251,284p'
nl -ba crates/jcode-base/src/session/crash.rs | sed -n '356,370p'
nl -ba crates/jcode-app-core/src/server/client_disconnect_cleanup.rs | sed -n '19,35p;73,130p;171,251p'
nl -ba crates/jcode-app-core/src/agent.rs | sed -n '970,1012p'
```

## Confidence and gaps

Confidence: **high** for this bounded PASS. The amendment directly addresses both named failed-signoff issues and explicitly blocks R04 until a separate source fix and fixtures land.

Gaps: I did not rerun tests or inspect unrelated R04 claims by instruction. I did not read the full Fable sign-off beyond its CRITICAL/IMPORTANT issue blocks. No live daemon, network, credentials, source mutation, or repo mutation was used.
