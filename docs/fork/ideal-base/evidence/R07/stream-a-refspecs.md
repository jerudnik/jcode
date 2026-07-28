# R07 Stream A atomic archive refspec plan

Prepared: `2026-07-28T22:27:03Z`
Source manifest: `archive-manifest.proposed.json`
Target repository: `jerudnik/jcode-recovery-archive`

## Authorization boundary

**Prepared and locally validated only. Do not execute yet.** Barrier 0 requires the coordinator to prove the exact target repository is private and authorize the external write. Immediately before execution, repeat canonical URL checks and confirm both managed namespaces still match the pre-write expectation. There is no non-atomic fallback: if the server rejects `--atomic`, stop.

## Exact integration command

After privacy proof and coordinator authorization, integration runs this single atomic command exactly. Every refspec is `source SHA:target ref`, one per line. It creates 33 reviewed heads and six lightweight stash tags without force, deletion, or movement of any unrelated ref.

```bash
verified_archive_push_url='https://github.com/jerudnik/jcode-recovery-archive.git'
git push --atomic "$verified_archive_push_url" \
  'b238d7034fdef981a2430224e71b9e6daed2cf23:refs/heads/archive/reviewed/W0' \
  '4df63d04ef645ea07abafb46ff90c2cc908e1334:refs/heads/archive/reviewed/W1' \
  '61ba2e2656e258b636b1137f114b91031d30aa0a:refs/heads/archive/reviewed/W2' \
  'd5d0adaaf1120bd412246dce428b0d00dbe8238e:refs/heads/archive/reviewed/W0.1' \
  'fb00ab840df36bf71762f08f4f8339d38a001123:refs/heads/archive/reviewed/W0.2' \
  'b238d7034fdef981a2430224e71b9e6daed2cf23:refs/heads/archive/reviewed/W0.3' \
  'a70db370025c53b449cca4138be7cfd5e55c5f17:refs/heads/archive/reviewed/F01' \
  '2b560788231e10741267d3dbfe74dc48368225a8:refs/heads/archive/reviewed/F02' \
  'd8c223d29e35ee8b3b37070686fb1be19cf8b799:refs/heads/archive/reviewed/F03' \
  '9c4c99897b88456257525d359f11fa357669c134:refs/heads/archive/reviewed/F04' \
  '9f4d34d11e9c54e8023538d6bb4ceff0780f0dfe:refs/heads/archive/reviewed/F05' \
  '84dc0aa2b5989a14a3ca7d4d215636fb4ecf0c1b:refs/heads/archive/reviewed/F06' \
  '58a80640134e72b5183da2420cdff3280209cbd5:refs/heads/archive/reviewed/F07' \
  '003419d9b71e3e95e48ea79487dfa96f3eb1e9e5:refs/heads/archive/reviewed/F08' \
  'd5d388028ed56518e8fec4a81c87a0b47a9fad70:refs/heads/archive/reviewed/F09' \
  '4b66de27c0a2c3430b7046c95d03a6e03e6dd43c:refs/heads/archive/reviewed/F10' \
  '77582db053e54096d004277b9bbe97c63bbb757b:refs/heads/archive/reviewed/F11' \
  'a1c9075afa7fad83a2ef1877e2fcb31ffc7adc7a:refs/heads/archive/reviewed/F12' \
  '080bfc9d735d130086197b8c9195361cc3f905ec:refs/heads/archive/reviewed/F13' \
  'e47efacdb6b5c02cfb6b43278c8413d975f3408c:refs/heads/archive/reviewed/F14' \
  'bb53da1476b83c3a35a9e8893f706eb0030d3123:refs/heads/archive/reviewed/F15' \
  '8971ed1dbe1ee8d44d7cedc370c3a61cd2fac178:refs/heads/archive/reviewed/F16' \
  'cdb2ee303fbece2cf4ab6d0558f137055055c204:refs/heads/archive/reviewed/F17' \
  'ca5f38bde426036bb1ad69119775f2289f6e9c2c:refs/heads/archive/reviewed/F18' \
  'a4dd576d46324824243f71f39d278b9b8cbf4dd5:refs/heads/archive/reviewed/F19' \
  '191b27093221171fb25bf549b4c413f3b97483d1:refs/heads/archive/reviewed/F21' \
  '2df5a891ca7a86524e75d80bf71002cb43172622:refs/heads/archive/reviewed/F28' \
  'e3736e7fbcc4f6a0914024ac5806fd416545eeac:refs/heads/archive/reviewed/R01' \
  'a0676f781776fbfb168f0d784c38c84b5bcc7108:refs/heads/archive/reviewed/R03' \
  'c7162849895197662dbcfc69290e12d011fc3a53:refs/heads/archive/reviewed/R04' \
  'dc9ded88150f10f82bde54133e49ecfc96866e97:refs/heads/archive/reviewed/F20a' \
  'c01518181958c550e1ddce5ed595397daaef7a0b:refs/heads/archive/reviewed/F20b' \
  'c754004541f676151c1d6d41c54e5e52cc7f6059:refs/heads/archive/reviewed/F20c' \
  'c88c7a26e2a1e3fa740b24456057e7a7b6160ade:refs/tags/archive/stash-0' \
  'dbe9ba2404dfdc81b252fd267c4eb18841abc67b:refs/tags/archive/stash-1' \
  '1f54abc9fbb0190f59af2fe5744e8e8dfb99c67f:refs/tags/archive/stash-2' \
  '975b91b8336122d55eb8d0955fb6aa09158e5b27:refs/tags/archive/stash-3' \
  '5dc53ed77b98effbd682402ddb10a6c6d6c286fe:refs/tags/archive/stash-4' \
  '29d49b250a6a7e924fa1beb33a07f635fc13c9be:refs/tags/archive/stash-5'
```

## Local validation performed

No `git push`, including no `git push --dry-run`, was executed.

- `39/39` source strings are full 40-character lowercase hexadecimal object IDs.
- `39/39` sources pass `git cat-file -t` as `commit` and `git rev-parse --verify <sha>^{commit}`.
- `39/39` destination names pass `git check-ref-format`.
- Destination refs are unique: `39/39`.
- Namespace counts are exact: `33` `refs/heads/archive/reviewed/*` and `6` `refs/tags/archive/stash-*`.
- The six tag source SHAs equal the current local lightweight-tag resolutions.
- The shell command passes `bash -n` syntax validation.
- The pre-write remote snapshot advertises zero refs in either managed namespace.

## Required post-write verification

Follow design §10 without weakening it: capture `ls-remote --refs` from this same canonical URL, require exact manifest equality in both managed namespaces, fresh-fetch into a temporary bare repository, verify all objects as commits, run `git fsck --full --no-dangling`, prove every stash commit remains multi-parent, and prove the public fork has no `archive/stash-*` tags.
