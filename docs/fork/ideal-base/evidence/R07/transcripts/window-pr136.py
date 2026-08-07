#!/usr/bin/env python3
"""R07 design.md section 4 maintenance window -- PR #136 (ideal-base integration).

Adapted from window-pr76.py. Two changes over that script, both from defects
this program has already paid for:

  1. PROTECTED-PATH SOURCE. window-pr76.py read the protected list from
     `scripts/required-checks.json`. That file holds 31 entries; the inline
     `protected=( ... )` array in `.github/workflows/governance-root.yml` --
     which is what the gate ACTUALLY enforces -- holds 32. Reading the JSON is
     the same defect class that produced the PR #106 false all-clear. This
     script parses the workflow array, and additionally cross-checks it against
     the JSON, printing any divergence rather than silently preferring one.

  2. BASE-PINNED ENCODER. This PR changes `scripts/governance_compare.py`, so
     importing it from the candidate worktree would let the candidate judge its
     own pre-window baseline. The canonical hash and restoration body use the
     comparator loaded from the captured base commit. The head comparator is
     loaded independently and must produce the same live-body hash before any
     write; both full comparators run again after the merge.

Fail-closed: any unexpected state aborts. Once the window is open, everything
sits inside a try whose `finally` restores, so a transient API error cannot
exit with `main` unguarded.
"""

import hashlib
import json
import os
import re
import subprocess
import sys
import time

PR = 136
REPO = "jerudnik/jcode"
REPO_ID = 1238606714
RULESET_ID = 18509013
HEAD_REF = "automation/s01-fix-1"

# The exact commit this window was reviewed against. CI was read at this SHA:
# Governance Root failed naming exactly the five protected paths below, and every
# other required context passed.
REVIEWED_HEAD = "e8ef0d131a337f8335d11f6d3f365ffb689b97d7"
STEADY_STATE = "43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b"
REQUIRED = ["Governance Root", "Fork CI Gate", "Security Gate", "Nix Gate"]
DROP = "Governance Root"

# The protected paths this window was reviewed to cover. Anything else appearing
# in the PR must stop the procedure for re-review.
EXPECTED_GOVERNANCE_PATHS = [
    "docs/fork/ideal-base/evidence/R07/github-governance.proposed.json",
    "scripts/ambient_roots_allowlist.txt",
    "scripts/governance_compare.py",
    "scripts/required-checks.json",
    "tests/test_governance_compare.py",
]

_env_root = os.environ.get("JCODE_REPOROOT")
if _env_root:
    REPOROOT = _env_root.rstrip("/")
else:
    REPOROOT = ""
    for _anchor in (os.path.dirname(os.path.abspath(__file__)), os.getcwd()):
        _probe = subprocess.run(
            ["git", "-C", _anchor, "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
        )
        if _probe.returncode == 0:
            _candidate = _probe.stdout.strip()
            if os.path.isfile(f"{_candidate}/scripts/governance_compare.py"):
                REPOROOT = _candidate
                break
    if not REPOROOT:
        sys.exit(
            "FATAL: cannot resolve a jcode checkout from the script location or "
            "cwd. Set JCODE_REPOROOT to the jcode working tree."
        )

# Default is DRY. Opening a real governance window requires --commit, typed
# deliberately, every time. See window-pr76.py: a harness believed read-only
# once performed a live governance write because --dry-run was opt-IN.
DRY = "--commit" not in sys.argv
if not DRY and "--dry-run" in sys.argv:
    raise SystemExit("ABORT: --commit and --dry-run are mutually exclusive.")


def gh(*args, method=None, body=None):
    cmd = ["gh", "api"]
    if method:
        cmd += ["-X", method]
    cmd += list(args)
    if body is not None:
        cmd += ["--input", "-"]
        r = subprocess.run(cmd, input=json.dumps(body), capture_output=True, text=True)
    else:
        r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"gh api failed: {' '.join(args)}\n{r.stderr}")
    return json.loads(r.stdout) if r.stdout.strip() else {}


def canon(obj):
    """Canonical hash via the captured base commit's pinned encoder."""
    return hashlib.sha256(canonical(sanitize(obj)).encode()).hexdigest()


def die(msg):
    raise SystemExit(f"ABORT: {msg}")


def comparator_at(commit):
    """Load sanitize/canonical from one exact commit without trusting the worktree."""
    shown = subprocess.run(
        ["git", "show", f"{commit}:scripts/governance_compare.py"],
        cwd=REPOROOT,
        capture_output=True,
        text=True,
    )
    if shown.returncode != 0:
        die(f"cannot load comparator at {commit}: {shown.stderr.strip()}")
    namespace = {
        "__name__": f"governance_compare_at_{commit[:12]}",
        "__file__": f"{commit}:scripts/governance_compare.py",
    }
    exec(compile(shown.stdout, namespace["__file__"], "exec"), namespace)
    for name in ("sanitize", "canonical"):
        if not callable(namespace.get(name)):
            die(f"comparator at {commit} does not define callable {name}()")
    source_hash = hashlib.sha256(shown.stdout.encode()).hexdigest()
    return namespace["sanitize"], namespace["canonical"], source_hash


tag = "  [DRY RUN]" if DRY else ""
print(f"=== R07 section 4 maintenance window: PR #{PR} ==={tag}")

# ---- [0] repository identity ------------------------------------------------
repo = gh(f"repos/{REPO}")
if repo["id"] != REPO_ID:
    die(f"repository id {repo['id']} != {REPO_ID}")
actor = gh("user")
if actor.get("login") != "jerudnik":
    die(f"authenticated actor {actor.get('login')!r} != repository owner 'jerudnik'")
if repo.get("allow_squash_merge") or repo.get("allow_rebase_merge"):
    die("repository is not merge-commit only")
print(f"[0] repository id {REPO_ID} and owner auth confirmed; merge-commit only")

prot = subprocess.run(
    ["gh", "api", f"repos/{REPO}/branches/main/protection"], capture_output=True, text=True
)
if prot.returncode == 0:
    die("classic branch protection present; expected absent")
if "HTTP 404" not in prot.stderr:
    die(f"classic-protection read failed for a reason other than absence: {prot.stderr.strip()}")
print("[0] classic branch protection absent (expected baseline)")

# ---- [1] PR state -----------------------------------------------------------
pr = gh(f"repos/{REPO}/pulls/{PR}")
head = pr["head"]["sha"]
expected_base = pr["base"]["sha"]
print(f"[1] head_sha={head}")
print(f"[1] source={pr['head']['label']}")
print(f"[1] expected_base_sha={expected_base}")

if pr["head"]["repo"]["full_name"] != REPO or pr["head"]["ref"] != HEAD_REF:
    die(
        f"source {pr['head']['repo']['full_name']}:{pr['head']['ref']} != "
        f"{REPO}:{HEAD_REF}"
    )
if pr["base"]["repo"]["full_name"] != REPO or pr["base"]["ref"] != "main":
    die(
        f"target {pr['base']['repo']['full_name']}:{pr['base']['ref']} != {REPO}:main"
    )

if pr.get("merged") or pr.get("state") == "closed":
    _msg = (
        f"PR #{PR} is already merged as {pr.get('merge_commit_sha')}."
        if pr.get("merged")
        else f"PR #{PR} is closed (state={pr.get('state')!r}) and was not merged."
    )
    die(
        f"{_msg}\n"
        "  NOTHING WAS WRITTEN and governance is untouched by this run.\n"
        "  If the window already completed, this is the expected outcome of a rerun."
    )

_live_main = gh(f"repos/{REPO}/commits/main")["sha"]
if _live_main != expected_base:
    die(
        f"`main` is at {_live_main} but the PR base is {expected_base}: main has "
        f"advanced since review. Nothing has been written and governance is "
        f"untouched. Update the PR base and re-review before opening a window."
    )
print("[1] live `main` == expected_base (no drift since review)")

if head != REVIEWED_HEAD:
    die(
        f"PR #{PR} head is {head} but this window was reviewed against "
        f"{REVIEWED_HEAD}: the branch was re-pushed since review.\n"
        "  NOTHING WAS WRITTEN and governance is untouched."
    )
print("[1] head == REVIEWED_HEAD (branch not re-pushed since review)")

if pr.get("mergeable") is not True or pr.get("mergeable_state") != "blocked":
    die(
        f"PR structural state mergeable={pr.get('mergeable')!r}, "
        f"mergeable_state={pr.get('mergeable_state')!r}; expected True/'blocked' "
        "with only Governance Root preventing merge"
    )
if subprocess.run(
    ["git", "merge-base", "--is-ancestor", expected_base, head],
    cwd=REPOROOT,
    capture_output=True,
).returncode != 0:
    die("captured main base is not an ancestor of the reviewed head; PR is behind")
print("[1] PR is structurally mergeable and reviewed head contains current main")

# The candidate changes the comparator. Bind restoration to the implementation
# already published at the captured base, and keep the candidate implementation
# independent so a disagreement becomes a pre-write stop.
sanitize, canonical, base_comparator_hash = comparator_at(expected_base)
head_sanitize, head_canonical, head_comparator_hash = comparator_at(head)
print(f"[1] base comparator source SHA-256={base_comparator_hash}")
print(f"[1] head comparator source SHA-256={head_comparator_hash}")
if base_comparator_hash == head_comparator_hash:
    die("expected this PR to change governance_compare.py, but comparator sources are equal")

runs = gh(f"repos/{REPO}/commits/{head}/check-runs?per_page=100")["check_runs"]
concl = {}
for ctx in REQUIRED:
    matching = [c for c in runs if c["name"] == ctx]
    if len(matching) != 1:
        die(f"{ctx} has {len(matching)} check runs on the reviewed head, expected exactly one")
    check = matching[0]
    app = check.get("app") or {}
    if app.get("id") != 15368 or app.get("slug") != "github-actions":
        die(f"{ctx} emitter {app.get('id')}/{app.get('slug')} != 15368/github-actions")
    if check.get("status") != "completed":
        die(f"{ctx} status {check.get('status')!r} != 'completed'")
    concl[ctx] = check.get("conclusion")
    print(f"[1] {ctx}: {concl[ctx]} (GitHub Actions app 15368)")
if concl.get(DROP) != "failure":
    die(f"{DROP} is {concl.get(DROP)}; window is only for its expected protected-path failure")
for ctx in REQUIRED:
    if ctx == DROP:
        continue
    if concl.get(ctx) != "success":
        die(f"{ctx} is {concl.get(ctx)}; every other required check must be success")

files = [f["filename"] for f in gh(f"repos/{REPO}/pulls/{PR}/files?per_page=100")]
declared = pr.get("changed_files")
if declared is None:
    die("PR object has no changed_files; cannot prove the file list is complete")
if len(files) != declared:
    die(
        f"file list is truncated or inconsistent: endpoint returned {len(files)}, "
        f"PR declares {declared} changed files; refusing to scan a partial diff"
    )
print(f"[1] PR changes {len(files)} files (matches declared changed_files)")
for f in sorted(files):
    print(f"    {f}")

# The list the GATE enforces is the inline bash array in the workflow, not
# scripts/required-checks.json. Reading the JSON is what produced the PR #106
# false all-clear. Parse the workflow, and cross-check the JSON so a divergence
# is reported rather than hidden.
_wf_path = f"{REPOROOT}/.github/workflows/governance-root.yml"
_wf = open(_wf_path).read()
_m = re.search(r"protected=\(\s*(.*?)\s*\)", _wf, re.S)
if not _m:
    die(f"cannot locate the inline protected=( ... ) array in {_wf_path}")
protected = [x or y for x, y in re.findall(r'"([^"]+)"|(\S+)', _m.group(1))]
protected = [p for p in protected if not p.startswith("#")]
# An empty parse is an ARTIFACT, not an all-clear. This program has been burned
# by a zero-pattern parse reading as "no protected paths changed".
if not protected:
    die("protected-path parse yielded ZERO patterns; refusing to treat that as an all-clear")
print(f"[1] protected patterns parsed from governance-root.yml: {len(protected)}")

_json_protected = json.load(open(f"{REPOROOT}/scripts/required-checks.json"))[
    "protected_paths"
]["required"]
if not _json_protected:
    die("required-checks.json protected list is EMPTY; artifact, not an answer")
_only_wf = sorted(set(protected) - set(_json_protected))
_only_json = sorted(set(_json_protected) - set(protected))
if _only_wf or _only_json:
    print(
        f"[1] NOTE: protected sources diverge "
        f"(workflow={len(protected)}, json={len(_json_protected)}); "
        f"workflow-only={_only_wf} json-only={_only_json}. "
        f"The WORKFLOW list is authoritative because it is what the gate runs."
    )


def is_protected(path):
    # the workflow uses `git diff -- <path>`, so a directory entry covers
    # everything beneath it
    return any(path == p or path.startswith(p.rstrip("/") + "/") for p in protected)


named = sorted(f for f in files if is_protected(f))
if named != EXPECTED_GOVERNANCE_PATHS:
    die(
        f"governance-touched paths {named} != reviewed expectation "
        f"{EXPECTED_GOVERNANCE_PATHS}; re-review before opening a window"
    )
print(f"[1] governance-named paths ({len(named)}) all belong to this PR: {named}")

# Same answer under the other source? Recorded, not assumed.
_named_json = sorted(
    f
    for f in files
    if any(f == p or f.startswith(p.rstrip("/") + "/") for p in _json_protected)
)
print(f"[1] cross-check: same protected set under required-checks.json: {_named_json == named}")

# ---- [2] capture and validate the live ruleset ------------------------------
live = gh(f"repos/{REPO}/rulesets/{RULESET_ID}")
pre_body = sanitize(live)
pre_hash = canon(live)
print(f"[2] pre-change canonical SHA-256={pre_hash}")
if pre_hash != STEADY_STATE:
    die(f"pre-change hash != known-good steady state {STEADY_STATE}")
print("[2] pre-change hash == known-good steady state")

head_pre_hash = hashlib.sha256(head_canonical(head_sanitize(live)).encode()).hexdigest()
print(f"[2] head-comparator live-body SHA-256={head_pre_hash}")
if head_pre_hash != pre_hash:
    die(
        f"base comparator hash {pre_hash} != candidate comparator hash {head_pre_hash}; "
        "candidate changed restoration semantics"
    )
print("[2] base and head comparators agree on the complete live ruleset body")

rsc = next(r for r in pre_body["rules"] if r["type"] == "required_status_checks")
live_ctx = [c["context"] for c in rsc["parameters"]["required_status_checks"]]
print(f"[2] live required contexts: {live_ctx}")
if sorted(live_ctx) != sorted(REQUIRED):
    die("live required contexts differ from expected")
if pre_body["enforcement"] != "active" or pre_body["target"] != "branch":
    die("enforcement/target unexpected")
if pre_body["bypass_actors"] != []:
    die("bypass_actors is not empty")
if not rsc["parameters"]["strict_required_status_checks_policy"]:
    die("strict policy is off")
if any(c["integration_id"] != 15368 for c in rsc["parameters"]["required_status_checks"]):
    die("unexpected integration id")
print("[2] semantics verified: enforcement/target/bypass/strictness/integration ids")

dropped = json.loads(json.dumps(pre_body))
d_rsc = next(r for r in dropped["rules"] if r["type"] == "required_status_checks")
d_rsc["parameters"]["required_status_checks"] = [
    c for c in d_rsc["parameters"]["required_status_checks"] if c["context"] != DROP
]
if len(d_rsc["parameters"]["required_status_checks"]) != len(REQUIRED) - 1:
    die("dropped body did not remove exactly one context")

# Field-by-field on the SANITIZED bodies, never GET-hash vs PUT-hash: the two
# shapes are not required to be identical documents. The drop must differ in
# exactly one field, and the restore in none.
_dk, _pk = set(sanitize(dropped)), set(pre_body)
if _dk != _pk:
    die(f"dropped body key set {sorted(_dk)} != pre-change key set {sorted(_pk)}")
_diff = sorted(k for k in pre_body if sanitize(dropped).get(k) != pre_body.get(k))
if _diff != ["rules"]:
    die(f"dropped body differs in {_diff}, expected exactly ['rules']")
print(f"[2] drop body: key sets equal, differing values exactly {_diff}")
print(f"[2] prospective dropped-body hash={canon(dropped)} (validated pre-write)")

if DRY:
    print("\nDRY RUN: all preflight and prospective checks passed; no write performed.")
    raise SystemExit(0)


def close_window():
    # The restore is the one call that must not give up: until it lands, `main`
    # is unguarded. A transient API failure here is exactly when retrying matters.
    last = None
    for attempt in range(1, 6):
        try:
            gh(f"repos/{REPO}/rulesets/{RULESET_ID}", method="PUT", body=pre_body)
            r = gh(f"repos/{REPO}/rulesets/{RULESET_ID}")
            if canon(r) == pre_hash:
                stamp = time.strftime("%H:%M:%SZ", time.gmtime())
                print(f"[6] window CLOSED {stamp}; restored body hash == step-2 hash exactly")
                return stamp
            last = "read-back hash mismatch"
        except SystemExit as e:
            last = str(e)
        print(f"[6] restore attempt {attempt} failed ({last}); retrying", file=sys.stderr)
        time.sleep(2 * attempt)
    die(
        f"RESTORE FAILED after 5 attempts ({last}) -- `main` may still be UNGUARDED. "
        f"Restore ruleset {RULESET_ID} to hash {pre_hash} by hand NOW."
    )


# ---- [3] OPEN ---------------------------------------------------------------
opened = time.strftime("%H:%M:%SZ", time.gmtime())
gh(f"repos/{REPO}/rulesets/{RULESET_ID}", method="PUT", body=dropped)

# From this line until close_window() lands, `main` is UNGUARDED. Everything
# below sits inside the try whose `finally` restores -- including the read-back,
# which calls gh() and so raises SystemExit on any transient API error.
try:
    back = gh(f"repos/{REPO}/rulesets/{RULESET_ID}")
    if canon(back) != canon(dropped):
        die("read-back after open did not match")
    _bctx = sorted(
        c["context"]
        for r in sanitize(back)["rules"]
        if r["type"] == "required_status_checks"
        for c in r["parameters"]["required_status_checks"]
    )
    print(f"[3] window OPEN {opened}; `{DROP}` dropped, read-back exact, contexts={_bctx}")

    # ---- [4] merge, conditioned on the exact reviewed head ------------------
    res = gh(
        f"repos/{REPO}/pulls/{PR}/merge",
        method="PUT",
        body={"merge_method": "merge", "sha": head},
    )
    if res.get("merged") is not True:
        die(f"merge response merged={res.get('merged')!r}, expected True")
    merge_sha = res.get("sha")
    if not isinstance(merge_sha, str) or len(merge_sha) != 40:
        die(
            f"merge response has no usable sha ({merge_sha!r}). The merge MAY HAVE "
            f"LANDED. Governance is being restored; verify `main` by hand before "
            f"assuming this window failed."
        )
    print(f"[4] merged; merge_sha={merge_sha}")

    # ---- [5] verify main and parents ---------------------------------------
    main = gh(f"repos/{REPO}/commits/main")
    if main["sha"] != merge_sha:
        die(f"main {main['sha']} != merge {merge_sha}")
    parents = [p["sha"] for p in main["parents"]]
    if parents != [expected_base, head]:
        die(f"parents {parents} != [{expected_base}, {head}]")
    print("[5] main==merge_sha; parents exactly [base, head]")
finally:
    closed = close_window()

# ---- [7] executable proof no other merge or commit landed in the window -----
post_restore_main_sha = gh(f"repos/{REPO}/commits/main")["sha"]
if post_restore_main_sha != merge_sha:
    die(f"post-restore main {post_restore_main_sha} != merge_sha {merge_sha}")


def _remote_url(name):
    u = subprocess.run(
        ["git", "remote", "get-url", name], cwd=REPOROOT, capture_output=True, text=True
    ).stdout
    return re.sub(r"\.git/?$", "", u.strip().rstrip("/")).lower()


_remote = next(
    (
        r
        for r in subprocess.run(
            ["git", "remote"], cwd=REPOROOT, capture_output=True, text=True
        ).stdout.split()
        if _remote_url(r).endswith("/" + REPO.lower())
    ),
    None,
)
if _remote is None:
    die(
        f"step 7 cannot verify: no configured remote points at {REPO}. "
        f"The merge and restore already SUCCEEDED; only the no-concurrent-merge "
        f"PROOF is missing."
    )
subprocess.run(
    ["git", "fetch", "--quiet", _remote, expected_base, post_restore_main_sha],
    cwd=REPOROOT,
    check=False,
)
for sha in (expected_base, post_restore_main_sha):
    if subprocess.run(
        ["git", "cat-file", "-e", f"{sha}^{{commit}}"], cwd=REPOROOT, capture_output=True
    ).returncode != 0:
        die(
            f"step 7 cannot verify: {sha[:12]} not present locally after fetch. "
            f"The merge and restore already SUCCEEDED; only the proof is missing."
        )

rv = subprocess.run(
    [
        "git", "rev-list", "--first-parent", "--merges",
        f"{expected_base}..{post_restore_main_sha}",
    ],
    cwd=REPOROOT,
    capture_output=True,
    text=True,
)
if rv.returncode != 0:
    die(f"step 7 rev-list failed: {rv.stderr.strip()}")
merges = rv.stdout.split()
if merges != [merge_sha]:
    intruders = [m for m in merges if m != merge_sha]
    die(
        f"CONCURRENT MERGE DETECTED. first-parent merges in {expected_base[:9]}.."
        f"{post_restore_main_sha[:9]} = {merges}, expected exactly [{merge_sha}]. "
        f"Governance IS restored (window closed cleanly), so nothing further is "
        f"unguarded -- but step 7 DETECTS this case rather than preventing it. "
        f"UNREVIEWED COMMITS: {intruders or '(none: the expected merge is missing instead)'}. "
        f"This is a governance incident requiring out-of-band investigation, NOT "
        f"something to close by re-running the restore."
    )
print(f"[7] exactly one first-parent merge in window, == {merge_sha}")

print(f"\nWINDOW OK  pr=#{PR} merge_sha={merge_sha} open={opened} close={closed}")
print("[8] run fork-health.sh --live at expected_base_sha AND merge_sha (separate step)")
