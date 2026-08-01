#!/usr/bin/env python3
"""R07 design.md section 4 maintenance window — PR #64.

Reconstructed from docs/fork/ideal-base/evidence/R07/transcripts/maintenance-window-pr55.txt,
including the two corrections that transcript records:
  1. canonical hash covers the SEMANTIC BODY ONLY (the six fields PUT controls),
     never the whole API response (which carries volatile server metadata).
  2. the step-2 capture is asserted equal to the known-good steady state BEFORE any write.

Fail-closed: any unexpected state aborts. If a write already happened, the
restore path runs before exiting.
"""

import hashlib
import json
import os
import re
import subprocess
import sys
import time

PR = 64
REPO = "jerudnik/jcode"
REPO_ID = 1238606714
RULESET_ID = 18509013

# The exact commit this window was reviewed and audited against. Everything in
# the evidence record -- the 27-file protected-path analysis, the green-check
# survey, the Linux clippy run -- describes THIS commit. See D22.
REVIEWED_HEAD = "97aa4963cdbea479c71112008e94eec89c9ef8cd"
STEADY_STATE = "43ba61a7a57ffded7a4276917192cdd6028f79d58755cae870cbb2df07494f2b"
BODY_FIELDS = ["name", "target", "enforcement", "bypass_actors", "conditions", "rules"]
REQUIRED = ["Governance Root", "Fork CI Gate", "Security Gate", "Nix Gate"]
DROP = "Governance Root"
# Resolved from git so the committed script works from any checkout. Anchored to
# the SCRIPT's own directory, not the caller's cwd: once committed this file
# lives inside the repo, and resolving via cwd made the script depend on where
# it happened to be launched from (it crashed when run from a staging dir).
# JCODE_REPOROOT overrides for the pre-commit case where the script is staged
# outside the working tree.
_env_root = os.environ.get("JCODE_REPOROOT")
if _env_root:
    REPOROOT = _env_root.rstrip("/")
else:
    # Prefer the script's own directory (correct once committed in-repo), then
    # fall back to cwd (correct while the script is still staged elsewhere).
    REPOROOT = ""
    for _anchor in (os.path.dirname(os.path.abspath(__file__)), os.getcwd()):
        _probe = subprocess.run(
            ["git", "-C", _anchor, "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
        )
        if _probe.returncode == 0:
            _candidate = _probe.stdout.strip()
            if os.path.isfile(f"{_candidate}/scripts/required-checks.json"):
                REPOROOT = _candidate
                break
    if not REPOROOT:
        sys.exit(
            "FATAL: cannot resolve a jcode checkout from the script location or "
            "cwd. Set JCODE_REPOROOT to the jcode working tree."
        )
if not os.path.isfile(f"{REPOROOT}/scripts/required-checks.json"):
    sys.exit(
        f"FATAL: {REPOROOT} does not look like a jcode checkout "
        "(scripts/required-checks.json missing).\n"
        "  Point JCODE_REPOROOT at the jcode working tree, or run from inside it."
    )
# The single protected path this window was reviewed to cover. Anything else
# appearing in the PR must stop the procedure for re-review.
EXPECTED_GOVERNANCE_PATHS = ["scripts/ideal_base_railway.py"]

# The default is DRY. Opening a real governance window requires --commit,
# typed deliberately, every time. This inversion exists because of a real
# incident (AUDIT.md, pass 12): a harness that was believed to be read-only
# invoked this script without a flag and performed a live governance write,
# dropping `Governance Root` from `main` for ~40 seconds. `--dry-run` being
# opt-IN meant every mistake defaulted to the destructive branch. Now a
# mistake defaults to a no-op and only an explicit, unambiguous word writes.
# `--dry-run` is still accepted so existing muscle memory and docs keep working.
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
    return hashlib.sha256(
        json.dumps(obj, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def semantic(rs):
    return {k: rs[k] for k in BODY_FIELDS}


def die(msg):
    raise SystemExit(f"ABORT: {msg}")


tag = "  [DRY RUN]" if DRY else ""
print(f"=== R07 §4 maintenance window: PR #{PR} ==={tag}")

# ---- [0] repository identity ------------------------------------------------
repo = gh(f"repos/{REPO}")
if repo["id"] != REPO_ID:
    die(f"repository id {repo['id']} != {REPO_ID}")
if repo.get("allow_squash_merge") or repo.get("allow_rebase_merge"):
    die("repository is not merge-commit only")
print(f"[0] repository id {REPO_ID} confirmed; merge-commit only")

prot = subprocess.run(
    ["gh", "api", f"repos/{REPO}/branches/main/protection"], capture_output=True, text=True
)
if prot.returncode == 0:
    die("classic branch protection present; expected absent")
print("[0] classic branch protection absent (expected baseline)")

# ---- [1] PR state -----------------------------------------------------------
pr = gh(f"repos/{REPO}/pulls/{PR}")
head = pr["head"]["sha"]
expected_base = pr["base"]["sha"]
print(f"[1] head_sha={head}")
print(f"[1] source={pr['head']['label']}")
print(f"[1] expected_base_sha={expected_base}")

# Rerun guard. If this window already ran successfully, every later check would
# still ABORT safely -- but with a message about base drift or a non-failing
# Governance Root, which reads like something went wrong and invites the
# operator to "fix" a repository that is already in its correct final state.
# Detect the benign case explicitly and say so.
if pr.get("merged") or pr.get("state") == "closed":
    _msg = (
        f"PR #{PR} is already merged as {pr.get('merge_commit_sha')}."
        if pr.get("merged")
        else f"PR #{PR} is closed (state={pr.get('state')!r}) and was not merged."
    )
    die(
        f"{_msg}\n"
        "  NOTHING WAS WRITTEN and governance is untouched by this run.\n"
        "  If the window already completed, this is the expected outcome of a\n"
        "  rerun and there is nothing to do; confirm with:\n"
        f"    python3 verify.py\n"
        "  which should report all checks passing."
    )

# `main` must still BE the reviewed base before anything is written. If it has
# advanced since the PR was opened, then step 7's `expected_base..merge` range
# would span commits that predate the window (inflating the merge count into a
# false CONCURRENT MERGE abort), and the merge itself would combine with commits
# that were never part of what was reviewed. Cheap to check, and it fails while
# governance is still fully intact.
_live_main = gh(f"repos/{REPO}/commits/main")["sha"]
if _live_main != expected_base:
    die(
        f"`main` is at {_live_main} but the PR base is {expected_base}: main has "
        f"advanced since review. Nothing has been written and governance is "
        f"untouched. Update the PR base and re-review before opening a window."
    )
print(f"[1] live `main` == expected_base (no drift since review)")

# Symmetric with the base guard above. `sha=head` on the merge call closes the
# read->merge race, but it binds to whatever head was read moments earlier, NOT
# to the commit that was actually reviewed. If the branch was re-pushed, this
# script would merge the new head while the evidence record, the audit, and the
# check survey all describe the old one. Assert the identity explicitly.
if head != REVIEWED_HEAD:
    die(
        f"PR #{PR} head is {head} but this window was reviewed against "
        f"{REVIEWED_HEAD}: the branch was re-pushed since review.\n"
        "  NOTHING WAS WRITTEN and governance is untouched.\n"
        "  Re-audit the new head, update REVIEWED_HEAD in this script and the\n"
        "  pinned SHA in the evidence draft, then re-run."
    )
print(f"[1] head == REVIEWED_HEAD (branch not re-pushed since review)")

runs = gh(f"repos/{REPO}/commits/{head}/check-runs?per_page=100")["check_runs"]
concl = {}
for c in sorted(runs, key=lambda r: (r.get("started_at") or "", r.get("id") or 0)):
    # a re-run adds a second check-run under the same name; sorting by start
    # time means the newest conclusion wins rather than whatever order the
    # API happened to return.
    concl[c["name"]] = c["conclusion"]
for ctx in REQUIRED:
    print(f"[1] {ctx}: {concl.get(ctx)}")
if concl.get(DROP) != "failure":
    die(f"{DROP} is {concl.get(DROP)}; window is only for its expected protected-path failure")
for ctx in REQUIRED:
    if ctx == DROP:
        continue
    if concl.get(ctx) != "success":
        die(f"{ctx} is {concl.get(ctx)}; every other required check must be success")

# The files endpoint pages at 100 and caps at 300. A silently truncated list
# would make the protected-path scan below fail OPEN, so cross-check the count
# against the PR object's authoritative changed_files and refuse to proceed on
# any mismatch rather than scanning a partial diff.
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

# Derive the governance-touched set from the live protected list rather than
# asserting a hard-coded expectation. A hard-coded list can only confirm the
# path I already know about; it cannot notice the PR touching a *second*
# protected path, which is exactly the case that must block the window.
protected = json.load(open(f"{REPOROOT}/scripts/required-checks.json"))["protected_paths"][
    "required"
]


def is_protected(path):
    # workflow uses `git diff -- <path>`, so a directory entry covers everything under it
    return any(path == p or path.startswith(p.rstrip("/") + "/") for p in protected)


named = sorted(f for f in files if is_protected(f))
if named != EXPECTED_GOVERNANCE_PATHS:
    die(
        f"governance-touched paths {named} != reviewed expectation "
        f"{EXPECTED_GOVERNANCE_PATHS}; re-review before opening a window"
    )
print(f"[1] governance-named paths ({len(named)}) all belong to this PR: {named}")

# ---- [2] capture and validate the live ruleset ------------------------------
live = gh(f"repos/{REPO}/rulesets/{RULESET_ID}")
pre_body = semantic(live)
pre_hash = canon(pre_body)
print(f"[2] pre-change canonical SHA-256={pre_hash}")
if pre_hash != STEADY_STATE:
    die(f"pre-change hash != known-good steady state {STEADY_STATE}")
print("[2] pre-change hash == known-good steady state (PR #49/#55 record)")

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
print(f"[2] prospective dropped-body hash={canon(dropped)} (validated pre-write)")

if DRY:
    print("\nDRY RUN: all preflight and prospective checks passed; no write performed.")
    raise SystemExit(0)

def close_window():
    # The restore is the one call that must not give up: until it lands, `main`
    # is unguarded. A transient API failure here is exactly when retrying
    # matters, so never let a single failed PUT abort the close.
    last = None
    for attempt in range(1, 6):
        try:
            gh(f"repos/{REPO}/rulesets/{RULESET_ID}", method="PUT", body=pre_body)
            r = semantic(gh(f"repos/{REPO}/rulesets/{RULESET_ID}"))
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
        f"RESTORE FAILED after 5 attempts ({last}) — `main` may still be UNGUARDED. "
        f"Restore ruleset {RULESET_ID} to hash {pre_hash} by hand NOW."
    )


# ---- [3] OPEN ---------------------------------------------------------------
opened = time.strftime("%H:%M:%SZ", time.gmtime())
gh(f"repos/{REPO}/rulesets/{RULESET_ID}", method="PUT", body=dropped)

# From this line until close_window() lands, `main` is UNGUARDED. Everything
# below must therefore sit inside the try whose `finally` restores -- including
# the read-back, which calls gh() and so raises SystemExit on any transient API
# error (502, timeout, rate limit). When the read-back sat ABOVE the try, such
# an error exited the process with the window still OPEN and no message saying
# so. (AUDIT.md D20.)
try:
    back = semantic(gh(f"repos/{REPO}/rulesets/{RULESET_ID}"))
    if canon(back) != canon(dropped):
        die("read-back after open did not match")
    print(f"[3] window OPEN {opened}; `{DROP}` dropped, read-back exact")

    # ---- [4] merge, conditioned on the exact reviewed head ------------------
    res = gh(
        f"repos/{REPO}/pulls/{PR}/merge",
        method="PUT",
        body={"merge_method": "merge", "sha": head},
    )
    # design.md section 4 step 5 requires this explicitly. The tip/parent
    # assertions below would also catch a failed merge, but a non-merged
    # response must not be read as success in the first place.
    if res.get("merged") is not True:
        die(f"merge response merged={res.get('merged')!r}, expected True")
    merge_sha = res.get("sha")
    if not isinstance(merge_sha, str) or len(merge_sha) != 40:
        # The merge may well have LANDED even if the response is malformed, so
        # this must not be reported as a clean failure. `finally` still restores
        # governance; step 7 is then unreachable, so say exactly what to check.
        die(
            f"merge response has no usable sha ({merge_sha!r}). The merge MAY HAVE "
            f"LANDED. Governance is being restored; verify `main` by hand and run "
            f"verify.py before assuming this window failed."
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
# design.md section 4 step 7 mandates a first-parent COMMIT-RANGE walk, not a
# timestamp query. The earlier `commits?since=` form was wrong twice over: it
# counted every multi-parent commit the API returned, including merges that
# legitimately exist INSIDE the reviewed branch, and it depended on wall-clock
# skew. Measured against this repository's own history, the two disagree on 5
# of the last 40 merges to main (at 2be9f0b22: first-parent 1 vs all-merges 6),
# so the old form would have aborted an ordinary window with a false violation.
post_restore_main_sha = gh(f"repos/{REPO}/commits/main")["sha"]
if post_restore_main_sha != merge_sha:
    die(f"post-restore main {post_restore_main_sha} != merge_sha {merge_sha}")

# Fetch only the commits step 7 needs, by SHA, from the remote that actually
# hosts REPO. `--all` was wrong: this clone also has `recovery-archive`, a
# DIFFERENT repository, and AGENTS.md forbids assuming a remote named `origin`.
# Fetching by explicit SHA also works on a shallow clone, so no --unshallow.
# Match on the normalized path SUFFIX, not a substring: the archive's URL
# (.../jerudnik/jcode-recovery-archive.git) contains "jerudnik/jcode".
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
        f"PROOF is missing. Add a remote for {REPO} and re-run step 7 by hand: "
        f"git rev-list --first-parent --merges {expected_base}..{post_restore_main_sha}"
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
            f"The merge and restore already SUCCEEDED; only the proof is missing. "
            f"Fetch it and re-run: git rev-list --first-parent --merges "
            f"{expected_base}..{post_restore_main_sha}"
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
        f"unguarded — but per design.md:490 step 7 DETECTS this case rather than "
        f"preventing it, meaning another change landed on `main` while "
        f"`{DROP}` was dropped and did NOT face it. UNREVIEWED COMMITS: "
        f"{intruders or '(none: the expected merge is missing instead)'}. "
        f"Inspect each with `git show <sha>`, confirm it would have passed "
        f"`{DROP}`, and record the outcome in the window evidence before "
        f"treating `main` as trusted."
    )
print(f"[7] exactly one first-parent merge in window, == {merge_sha}")

print(f"\nWINDOW OK  pr=#{PR} merge_sha={merge_sha} open={opened} close={closed}")
print("[8] run fork-health.sh --live at expected_base_sha AND merge_sha (separate step)")
