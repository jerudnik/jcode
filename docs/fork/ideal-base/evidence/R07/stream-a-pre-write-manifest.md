# R07 Stream A pre-write archive manifest

Captured: `2026-07-28T22:27:03Z`
Preparation branch: `automation/r07-impl-archive` at `886f1a383d8eb5e7cd7d5bbb64f122a61e3cfbde`
Proposed manifest: `archive-manifest.proposed.json` (`33` heads + `6` tags = `39` refs)

## Safety result

- **No push or other remote write was executed.** Remote interaction was limited to `git ls-remote` against the explicit canonical URL.
- Local result: **39/39 manifest source objects exist and are commits**.
- Stash-tag result: **6/6 local lightweight tags resolve exactly to the manifest objects**.
- Time-sensitive result: **6/6 ref-unreachable reviewed commits are still present and reflog-protected**.
- Privacy is **not proved by `ls-remote`**. Per design §10 and barrier 0, the coordinator must independently prove this repository is private before authorizing any write.

## Remote identity and current advertised state

Design repository: `jerudnik/jcode-recovery-archive`
Canonical URL queried: `https://github.com/jerudnik/jcode-recovery-archive.git`
Configured fetch URL(s): `https://github.com/jerudnik/jcode-recovery-archive.git`
Configured push URL(s): `https://github.com/jerudnik/jcode-recovery-archive.git`

Identity result: **PASS for existence and canonical owner/repository URL**. `git ls-remote` succeeded against the exact design-named URL. The canonical full HTTPS URL is used directly; configured shorthand rewrite rules do not match it.

HEAD advertisement:

```text
ref: refs/heads/main	HEAD
152ececcc57c153731685ff398352a4494bd679b	HEAD
```

Advertised ordinary refs: **44**. Tags: **0**. Managed pre-write namespaces:

- `refs/heads/archive/reviewed/*`: **0 refs**
- `refs/tags/archive/stash-*`: **0 refs**

Exact `git ls-remote --refs https://github.com/jerudnik/jcode-recovery-archive.git` output:

```text
e02f40c91e3759a048a4b3a0df109b023d92a7ee	refs/heads/agent/hotpath-stabilization
6fc5623a540e6675841481c3faf90b3e8e2fcfbc	refs/heads/agent/marker-hardening
59c6a0ba0923b5ca9661611015abf8025bb72be2	refs/heads/archive/detached/jcode-up-2026-07-17
4a3f2e19dcb69d25ff4ba85ccbf4a5249be5121f	refs/heads/archive/local/ci-cleanup-drop-windows-cicd-2026-07-27
696dda16a7f3f5f0bf9fc1aaed5f2f96bd6d93ff	refs/heads/archive/local/f17-local-dirty-backup-2026-07-27
2ccf43fd79c7802b4b6605998e0717f45ef54583	refs/heads/backup/pre-stabilization-2026-07-14
e601b95b299c5a0941864f6480beaf61ba3eefe1	refs/heads/distro/nix
47f848494f51c4d3ef85a0ae7a287e0c5252b2ed	refs/heads/feat/nix-managed-mode
e0a8de8e8a34c8f31f3aeb0188661bf9d49c3752	refs/heads/fix/mcp-selfspawn-supervision-hardening
7195b6a313a3ed57be7810588ff6f14cc97670b9	refs/heads/follow-upstream
152ececcc57c153731685ff398352a4494bd679b	refs/heads/main
42aa9cc64183741efb000a6d58c2c920de77e146	refs/heads/normalize/integration
18f9fa1b88b39fe46760992d060524bef4552b6e	refs/heads/orch/f5-name-resolution
5e802effebed623c845b29150605800882ecaae1	refs/heads/orch/failure-scoreboard
9bd1c4fc0ca8f3515b2ffd723b0a97990d5a3978	refs/heads/orch/w1-control-log
ed88e1bde7a7b24d3fc659c84554ab2db8785b58	refs/heads/orch/w3-lifecycle
8a81c60b25b2da911d4493b14d91b48002468549	refs/heads/recovery/2026-07-15
6cc72ef780af5c3cdc5a8ac04622a6950b733705	refs/heads/recovery/close-w4-r02-compose-2026-07-16
2ab8135b2fa38c042ed4a82b815c0799a21ac1f6	refs/heads/recovery/docs-fork-governance-2026-07-16
c53022f4d4135b43fc86337c9c689a9e73c27807	refs/heads/recovery/fix-gate-parser-2026-07-15
a888ba86ac243858334ff097c77603301be376f4	refs/heads/recovery/fix-r01-r03a-identity-20260715
cdb115a9d2efee1ff9c7bc61fcf0fc1bf21bb163	refs/heads/recovery/fix-r02-tier-20260715
566d7930606f96add92aed65564c95b539a03df0	refs/heads/recovery/fix-r04-lifecycle-widening-2026-07-16
0f8bd8d9f5556accfebf522577d40930ac9eac47	refs/heads/recovery/fix-r04-marker-20260715
7be320f4942522f1e992bd920d40f403cc263ec3	refs/heads/recovery/fix-r05b-spawn-reclaim-2026-07-15
f0e77020c20920a8d2e3225f976e5b7a4a1e1512	refs/heads/recovery/fix-r12-evidence-20260715
63309f670ee27e4479ebea3a0867456f36f87e4e	refs/heads/recovery/fix-r12-terminal-evidence-2026-07-15
52aed00e95887f8c694dd3249927fbaeed1a04ba	refs/heads/recovery/fix-w5-onboarding-consent-2026-07-16
19d90af988a52ad31294beceb89c8ffe51920e2c	refs/heads/recovery/fix-w6-r10-acquisition-2026-07-16
57b31756fc435c8a4cbcc0dbe288d06a87165db4	refs/heads/recovery/light-control-20260715
d1388625f4d6dcfa2ee70ee7847e681fe944e458	refs/heads/recovery/light-ledgers-20260715
914916719bb1af321a3ceb257d9eaf4008ac88f6	refs/heads/recovery/light-pilot-20260715
6ca1fcf2ec2366c7abc99664a485c40d60cec80e	refs/heads/recovery/orchestrator-s4-20260715
6ca1fcf2ec2366c7abc99664a485c40d60cec80e	refs/heads/recovery/orchestrator-s5-20260715
6ca1fcf2ec2366c7abc99664a485c40d60cec80e	refs/heads/recovery/orchestrator-s6-20260715
d5898df4c03297ccc277f354b068655df4587810	refs/heads/recovery/pilot-prereq-ledgers-20260715
7c858453768dcd7d9a22bb4641ae0c3203c77cbf	refs/heads/recovery/seam-r01-20260715
3217dbcbf22ea6ef13525e7f3f1571b0a49132d6	refs/heads/recovery/seam-r02-20260715
73e4e9e62b33177a4a96f3eca86c8c9eaf1a2ef0	refs/heads/recovery/seam-r03a-20260715
557cf7ddbcfedd64fab04a842d68b7a31c6f7387	refs/heads/recovery/seam-r04-20260715
9385e9a46afe598fb47a00c1d5433923b8f26df0	refs/heads/recovery/seam-r05b-20260715
3831f4afbc1bdb7e505e66733cd747df0409c3b1	refs/heads/recovery/seam-r12-20260715
0ad2278ab913eb1c8cbd31bd5fdda54e7ef0b0a7	refs/heads/sync/upstream-v0.46
631935dd1d3b2e31e167e2b12ad463e54bcf4b8d	refs/heads/vendor/upstream
```

The 44-ref baseline matches design §10. This is a timestamped pre-write snapshot, not a concurrency lock; integration must repeat the read-only identity/namespace checks immediately before an authorized push.

## Local source-object verification

| Target ref | Manifest object | Type | Result | Local reachability/detail |
|---|---|---|---|---|
| `refs/heads/archive/reviewed/W0` | `b238d7034fdef981a2430224e71b9e6daed2cf23` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/W1` | `4df63d04ef645ea07abafb46ff90c2cc908e1334` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/W2` | `61ba2e2656e258b636b1137f114b91031d30aa0a` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/W0.1` | `d5d0adaaf1120bd412246dce428b0d00dbe8238e` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/W0.2` | `fb00ab840df36bf71762f08f4f8339d38a001123` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/W0.3` | `b238d7034fdef981a2430224e71b9e6daed2cf23` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F01` | `a70db370025c53b449cca4138be7cfd5e55c5f17` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F02` | `2b560788231e10741267d3dbfe74dc48368225a8` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F03` | `d8c223d29e35ee8b3b37070686fb1be19cf8b799` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F04` | `9c4c99897b88456257525d359f11fa357669c134` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F05` | `9f4d34d11e9c54e8023538d6bb4ceff0780f0dfe` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F06` | `84dc0aa2b5989a14a3ca7d4d215636fb4ecf0c1b` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F07` | `58a80640134e72b5183da2420cdff3280209cbd5` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F08` | `003419d9b71e3e95e48ea79487dfa96f3eb1e9e5` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F09` | `d5d388028ed56518e8fec4a81c87a0b47a9fad70` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F10` | `4b66de27c0a2c3430b7046c95d03a6e03e6dd43c` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F11` | `77582db053e54096d004277b9bbe97c63bbb757b` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F12` | `a1c9075afa7fad83a2ef1877e2fcb31ffc7adc7a` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F13` | `080bfc9d735d130086197b8c9195361cc3f905ec` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F14` | `e47efacdb6b5c02cfb6b43278c8413d975f3408c` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F15` | `bb53da1476b83c3a35a9e8893f706eb0030d3123` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F16` | `8971ed1dbe1ee8d44d7cedc370c3a61cd2fac178` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F17` | `cdb2ee303fbece2cf4ab6d0558f137055055c204` | commit | PASS | ref-unreachable; reflog-protected |
| `refs/heads/archive/reviewed/F18` | `ca5f38bde426036bb1ad69119775f2289f6e9c2c` | commit | PASS | ref-unreachable; reflog-protected |
| `refs/heads/archive/reviewed/F19` | `a4dd576d46324824243f71f39d278b9b8cbf4dd5` | commit | PASS | ref-unreachable; reflog-protected |
| `refs/heads/archive/reviewed/F21` | `191b27093221171fb25bf549b4c413f3b97483d1` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F28` | `2df5a891ca7a86524e75d80bf71002cb43172622` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/R01` | `e3736e7fbcc4f6a0914024ac5806fd416545eeac` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/R03` | `a0676f781776fbfb168f0d784c38c84b5bcc7108` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/R04` | `c7162849895197662dbcfc69290e12d011fc3a53` | commit | PASS | reachable from an existing local ref |
| `refs/heads/archive/reviewed/F20a` | `dc9ded88150f10f82bde54133e49ecfc96866e97` | commit | PASS | ref-unreachable; reflog-protected |
| `refs/heads/archive/reviewed/F20b` | `c01518181958c550e1ddce5ed595397daaef7a0b` | commit | PASS | ref-unreachable; reflog-protected |
| `refs/heads/archive/reviewed/F20c` | `c754004541f676151c1d6d41c54e5e52cc7f6059` | commit | PASS | ref-unreachable; reflog-protected |
| `refs/tags/archive/stash-0` | `c88c7a26e2a1e3fa740b24456057e7a7b6160ade` | commit | PASS | tag resolves exactly |
| `refs/tags/archive/stash-1` | `dbe9ba2404dfdc81b252fd267c4eb18841abc67b` | commit | PASS | tag resolves exactly |
| `refs/tags/archive/stash-2` | `1f54abc9fbb0190f59af2fe5744e8e8dfb99c67f` | commit | PASS | tag resolves exactly |
| `refs/tags/archive/stash-3` | `975b91b8336122d55eb8d0955fb6aa09158e5b27` | commit | PASS | tag resolves exactly |
| `refs/tags/archive/stash-4` | `5dc53ed77b98effbd682402ddb10a6c6d6c286fe` | commit | PASS | tag resolves exactly |
| `refs/tags/archive/stash-5` | `29d49b250a6a7e924fa1beb33a07f635fc13c9be` | commit | PASS | tag resolves exactly |

## Six time-sensitive reviewed commits

Each row had `git cat-file -t <sha> = commit`, no containing result from `git for-each-ref --contains <sha>`, and a current reflog entry. Normal `git fsck --unreachable` did not classify them unreachable, while `git fsck --unreachable --no-reflogs` classified all six unreachable. Therefore reflogs are their only current Git reachability protection.

| Node | Object | Containing refs | Newest enumerated reflog entry | Entry date |
|---|---|---|---|---|
| `F17` | `cdb2ee303fbece2cf4ab6d0558f137055055c204` | none | `main-worktree/HEAD@{2145}` | 2026-07-23 12:48:51 -0400 |
| `F18` | `ca5f38bde426036bb1ad69119775f2289f6e9c2c` | none | `refs/heads/main@{64}` | 2026-07-23 17:53:44 -0400 |
| `F19` | `a4dd576d46324824243f71f39d278b9b8cbf4dd5` | none | `refs/heads/main@{61}` | 2026-07-24 01:01:31 -0400 |
| `F20a` | `dc9ded88150f10f82bde54133e49ecfc96866e97` | none | `refs/heads/main@{58}` | 2026-07-24 03:05:29 -0400 |
| `F20b` | `c01518181958c550e1ddce5ed595397daaef7a0b` | none | `refs/heads/main@{56}` | 2026-07-24 20:07:44 -0400 |
| `F20c` | `c754004541f676151c1d6d41c54e5e52cc7f6059` | none | `main-worktree/HEAD@{2047}` | 2026-07-26 00:19:10 -0400 |

**Urgency:** `gc.reflogExpire`, `gc.reflogExpireUnreachable`, and `gc.pruneExpire` are unset locally, so Git defaults apply (normally 90 days for reachable reflog entries, 30 days for unreachable reflog entries, and two weeks for prune age). These entries are only 2-5 days old at capture time, but they have no durable ref. Reflog expiry followed by pruning can permanently remove them. Do not run reflog expiry, aggressive GC, or prune before barrier 1; complete the authorized archive write promptly.

Observation: `git rev-list --walk-reflogs --all` did not enumerate these SHAs even though raw reflog records contain each as a new value and normal `git fsck` recognizes the reflog protection. The raw-record plus `fsck` comparison is the controlling evidence.

## Existing stash-tag resolution

| Local tag | Manifest object | Resolved object | Result |
|---|---|---|---|
| `refs/tags/archive/stash-0` | `c88c7a26e2a1e3fa740b24456057e7a7b6160ade` | `c88c7a26e2a1e3fa740b24456057e7a7b6160ade` | PASS |
| `refs/tags/archive/stash-1` | `dbe9ba2404dfdc81b252fd267c4eb18841abc67b` | `dbe9ba2404dfdc81b252fd267c4eb18841abc67b` | PASS |
| `refs/tags/archive/stash-2` | `1f54abc9fbb0190f59af2fe5744e8e8dfb99c67f` | `1f54abc9fbb0190f59af2fe5744e8e8dfb99c67f` | PASS |
| `refs/tags/archive/stash-3` | `975b91b8336122d55eb8d0955fb6aa09158e5b27` | `975b91b8336122d55eb8d0955fb6aa09158e5b27` | PASS |
| `refs/tags/archive/stash-4` | `5dc53ed77b98effbd682402ddb10a6c6d6c286fe` | `5dc53ed77b98effbd682402ddb10a6c6d6c286fe` | PASS |
| `refs/tags/archive/stash-5` | `29d49b250a6a7e924fa1beb33a07f635fc13c9be` | `29d49b250a6a7e924fa1beb33a07f635fc13c9be` | PASS |

## Commands used

All commands were read-only except creation of this evidence file and the requested local branch/worktree:

```text
git cat-file -t <manifest-object>
git rev-parse --verify <local-stash-tag>
git rev-list --all
git for-each-ref --contains <sha>
git log -g --all
git fsck --unreachable [--no-reflogs] --no-progress
git remote get-url --all recovery-archive
git remote get-url --push --all recovery-archive
git ls-remote https://github.com/jerudnik/jcode-recovery-archive.git
git ls-remote --refs https://github.com/jerudnik/jcode-recovery-archive.git
git ls-remote --symref https://github.com/jerudnik/jcode-recovery-archive.git
```
