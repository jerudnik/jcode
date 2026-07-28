# Stream S: independent reproduction of the STATE.proposed.json / mapping-ledger.proposed.json ledger

Scope: R07 design §11 Stream S. This document is an independent, from-scratch
reproduction of the coordinator's proposed schema-v2 migration artifacts,
performed before writing any validator code, to confirm the ledger the
validator will be asked to accept is actually consistent. It does not modify
`STATE.json`, `DECISIONS.md`, `WORK_GRAPH.json`, or `.github/workflows`.

Inputs:

- `docs/fork/ideal-base/evidence/R07/STATE.proposed.json` (schema_version 2, 57 records)
- `docs/fork/ideal-base/evidence/R07/mapping-ledger.proposed.json`
  (`baseline_main = 498249777c453c1d551aeb01fc45420d8ca0a585`, 35 entries)
- `docs/fork/ideal-base/STATE.json` (live, schema_version 1)
- `docs/fork/ideal-base/WORK_GRAPH.json` (live)

All checks below were run against the `automation/r07-impl-state` worktree,
which has full (non-shallow) history and `refs/remotes/origin/main` resolved
to the baseline commit.

## 1. Accepted-record count

```
$ python3 -c "import json; d=json.load(open('docs/fork/ideal-base/evidence/R07/STATE.proposed.json')); \
  print(sum(1 for v in d['nodes'].values() if v['state']=='accepted'))"
35
```

Matches the design's stated 35 accepted / 57 total split, and matches the
`mapping-ledger.proposed.json` entry count (35).

## 2. `reviewed_commit` full-SHA expansion consistency against live STATE.json

For every accepted record in `STATE.proposed.json`, its `reviewed_commit` (a
40-hex SHA) was compared against the corresponding live `STATE.json` record's
abbreviated `commit` field. Check: the live abbreviated commit must be a
string-prefix of the proposed full `reviewed_commit`.

```
$ python3 - <<'EOF'
import json
state = json.load(open("docs/fork/ideal-base/evidence/R07/STATE.proposed.json"))
live = json.load(open("docs/fork/ideal-base/STATE.json"))
accepted = {k: v for k, v in state["nodes"].items() if v["state"] == "accepted"}
mismatches = []
for node_id, rec in accepted.items():
    live_commit = live["nodes"].get(node_id, {}).get("commit")
    if live_commit and not rec["reviewed_commit"].startswith(live_commit):
        mismatches.append(node_id)
print(len(accepted), "accepted records checked;", len(mismatches), "mismatches:", mismatches)
EOF
35 accepted records checked; 0 mismatches: []
```

Result: **zero mismatches**. Every `reviewed_commit` in the proposal is a
verbatim 40-hex expansion of the abbreviated commit already recorded live;
this is a lossless schema migration for the reviewed identity, not a change
of record.

## 3. `published_commit` object existence and ancestry-of-baseline

For every accepted record, `published_commit` must (a) exist as a real git
commit object and (b) be an ancestor of `baseline_main`
(`498249777c453c1d551aeb01fc45420d8ca0a585`), per `git merge-base
--is-ancestor`.

```
$ python3 - <<'EOF'
import json, subprocess
state = json.load(open("docs/fork/ideal-base/evidence/R07/STATE.proposed.json"))
ledger = json.load(open("docs/fork/ideal-base/evidence/R07/mapping-ledger.proposed.json"))
baseline = ledger["baseline_main"]
accepted = {k: v for k, v in state["nodes"].items() if v["state"] == "accepted"}
failures = []
for node_id, rec in accepted.items():
    published = rec["published_commit"]
    exists = subprocess.run(["git", "cat-file", "-e", f"{published}^{{commit}}"],
                             capture_output=True).returncode == 0
    ancestor = subprocess.run(["git", "merge-base", "--is-ancestor", published, baseline],
                               capture_output=True).returncode == 0
    if not (exists and ancestor):
        failures.append((node_id, exists, ancestor))
print(len(accepted), "accepted records checked;", len(failures), "failures:", failures)
EOF
35 accepted records checked; 0 failures: []
```

Result: **zero failures**. Every `published_commit` exists and is a genuine
ancestor of the pre-R07 baseline main, i.e. every accepted node's claimed
publication is real, reachable history, not merely a plausible-looking SHA.

## 4. Non-accepted records carry null commit identities

```
$ python3 -c "
import json
d = json.load(open('docs/fork/ideal-base/evidence/R07/STATE.proposed.json'))
bad = [nid for nid, rec in d['nodes'].items()
       if rec['state'] != 'accepted' and (rec.get('reviewed_commit') or rec.get('published_commit'))]
print('violations:', bad)
"
violations: []
```

Result: **zero violations**. No pending/in-progress/other non-terminal record
smuggles a non-null commit identity, consistent with the design §8 rule that
only dependency-complete records may carry commit identities.

## 5. WORK_GRAPH.json node set == STATE.proposed.json node set

```
$ python3 -c "
import json
g = json.load(open('docs/fork/ideal-base/WORK_GRAPH.json'))
s = json.load(open('docs/fork/ideal-base/evidence/R07/STATE.proposed.json'))
graph_ids = set(n['id'] for n in g['all_nodes']) | set(n['id'] for n in g['root_nodes'])
state_ids = set(s['nodes'].keys())
print(len(graph_ids), len(state_ids), graph_ids == state_ids)
"
57 57 True
```

Result: exact match, 57/57, no missing or extra nodes on either side.

## 6. No expansion-consistency violations (root vs. children state)

Using the validator's own `validate_expansion_consistency` (unchanged logic,
run directly against the graph and the proposed state):

```
$ python3 -c "
import scripts.ideal_base_railway as railway
graph = railway.load_json(railway.GRAPH_PATH)
state = railway.load_json(railway.CONTROL_ROOT / 'evidence/R07/STATE.proposed.json')
violations = railway.expansion_violations(graph, state)
print('violations:', violations)
"
violations: {}
```

Result: **zero violations**. No root's recorded state contradicts its
children's states in the proposed migration.

## 7. Full-SHA format of every accepted commit identity

Independently of the validator, confirmed with a bare regex
(`^[0-9a-f]{40}$`) that every accepted record's `reviewed_commit` and
`published_commit` is exactly 40 lowercase hex characters (no abbreviated,
upper-case, or malformed values slipped through the proposal draft). Zero
violations.

## Conclusion

The proposed schema-v2 migration artifacts (`STATE.proposed.json`,
`mapping-ledger.proposed.json`) are internally consistent and consistent with
the live `STATE.json`/`WORK_GRAPH.json` they are meant to replace:

- record count and accepted count match the design's stated numbers,
- every reviewed identity is a lossless full-SHA expansion of the existing
  abbreviated commit,
- every published identity is a real, ancestor-of-baseline commit (i.e. a
  true publication proof, not merely an existing object),
- no non-terminal record carries a commit identity,
- the node set is unchanged (57/57), and
- no root/children expansion-consistency violation was introduced.

This reproduction was performed independently, from the raw ledger and git
history, before the schema-v2 validator implementation in
`scripts/ideal_base_railway.py` was tested against these same files (see
`tests/test_ideal_base_railway.py::SchemaV2ValidatorTests::test_state_proposed_json_validates_as_schema_v2`,
which encodes the same checks through the shipped validator rather than
ad hoc scripts, so the two form independent confirmations of the same
result).
