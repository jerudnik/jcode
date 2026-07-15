# Recovery baselines

Append a dated baseline at the start of every recovery phase that depends on refreshed refs. Do not rewrite older snapshots.

## 2026-07-15 pre-scaffold snapshot

| Item | Value |
|---|---|
| Last code commit before recovery docs | `3d80eaf343e690aaa8b428d0b3ed6de64b7464d0` |
| Upstream ref | `upstream/master` |
| Upstream commit | `802f6909825809e882d9c2d575b7e478dce57d3b` |
| Merge base | `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Fork-only commits before scaffold | 286 |
| Upstream-only commits | 246 |
| Fork-changed files since merge base | 927 |
| Upstream-changed files since merge base | 425 |
| Files changed on both sides | 406 |
| Curated sync | `b3ed82a6b`, one parent |
| Visibility ref | `vendor/upstream` at the merge base |

Reproduction:

```bash
up=upstream/master
base=$(git merge-base HEAD "$up")
git rev-list --left-right --count HEAD..."$up"
git show -s --format='%H%n%P%n%aI%n%s' b3ed82a6b
python3 - <<'PY'
import subprocess
up = 'upstream/master'
base = subprocess.check_output(['git', 'merge-base', 'HEAD', up], text=True).strip()
def names(rev):
    out = subprocess.check_output(['git', 'diff', '--name-only', f'{base}..{rev}'], text=True)
    return set(out.splitlines())
fork, upstream = names('HEAD'), names(up)
print(len(fork), len(upstream), len(fork & upstream))
PY
```

The recovery scaffold itself adds documentation commits after this code snapshot. The next session must append its own refreshed baseline rather than editing this one.
