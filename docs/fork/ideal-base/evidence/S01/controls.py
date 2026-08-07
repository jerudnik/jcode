#!/usr/bin/env python3
"""S01 normalizer controls D1-D4.

Each control fails on a DIFFERENT assertion. A passing run of the normalizer is
not evidence until these fail in the ways predicted by NORMALIZER_SPEC.md.

Run: python3 docs/fork/ideal-base/evidence/S01/controls.py
Exit 0 only if all four controls behave as predicted.
"""
import hashlib
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
NORM = os.path.join(HERE, "normalize.py")
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", "..", ".."))
# Real specimen: a frozen copy of the F14 two-round matrix transcript.
# Copied into S01 rather than read from evidence/F14/ because the S01 matrix
# re-runs run_lifecycle_matrix.sh, which OVERWRITES the F14 log. A control
# whose specimen is rewritten by the thing it is controlling is not a control.
SPECIMEN = os.path.join(HERE, "specimen-f14.log")

results = []


def run_norm(path, want_hash=True):
    cmd = [sys.executable, NORM] + (["--hash"] if want_hash else []) + [path]
    p = subprocess.run(cmd, capture_output=True, text=True)
    return p.returncode, p.stdout, p.stderr


def record(cid, desc, ok, detail):
    results.append((cid, desc, ok, detail))
    print(f"{'PASS' if ok else 'FAIL'} {cid}: {desc}")
    print(f"      {detail}")


def main():
    # Precondition: the specimen must exist and be non-trivial. An absent
    # specimen would make every control vacuously "pass".
    if not os.path.isfile(SPECIMEN):
        print(f"ABORT: specimen missing: {SPECIMEN}")
        return 1
    raw = open(SPECIMEN, errors="replace").read()
    n = raw.count("\n")
    if n < 100:
        print(f"ABORT: specimen only {n} lines; too small to be a real test")
        return 1
    print(f"specimen: {SPECIMEN}")
    print(f"specimen lines: {n}\n")

    rc, out, err = run_norm(SPECIMEN)
    if rc != 0:
        print(f"ABORT: baseline normalize failed rc={rc}: {err}")
        return 1
    base_hash = out.split()[0]
    print(f"baseline H = {base_hash}\n")

    tmpd = tempfile.mkdtemp(prefix="s01ctl_")

    # ---- D1: planted one-character diff MUST move H --------------------
    # Change a verdict token, the single most outcome-bearing thing there is.
    assert "PASS residue" in raw, "specimen lacks the token D1 plants into"
    d1_text = raw.replace("PASS residue", "FAIL residue", 1)
    assert d1_text != raw, "D1 mutation did not apply"
    d1p = os.path.join(tmpd, "d1.log")
    open(d1p, "w").write(d1_text)
    # assert the mutation is present ON DISK before reading any result
    assert "FAIL residue" in open(d1p).read(), "D1 mutation absent on disk"
    rc, out, err = run_norm(d1p)
    d1_hash = out.split()[0] if rc == 0 else None
    ok = rc == 0 and d1_hash != base_hash
    record("D1", "planted one-char verdict diff moves H", ok,
           f"rc={rc} H={d1_hash} (baseline {base_hash[:16]}...) "
           f"{'moved' if ok else 'DID NOT MOVE - normalizer is blind'}")

    # ---- D2: legitimate variation MUST be erased, H holds --------------
    d2_text = raw
    # a different timestamp on every bracketed stamp
    import re as _re
    d2_text = _re.sub(r"\[\d{2}:\d{2}:\d{2}\]", "[23:59:59]", d2_text)
    # a different elapsed duration
    d2_text = _re.sub(r"finished in \d+(?:\.\d+)?s", "finished in 99.99s", d2_text)
    assert d2_text != raw, "D2 mutation did not apply"
    d2p = os.path.join(tmpd, "d2.log")
    open(d2p, "w").write(d2_text)
    assert "[23:59:59]" in open(d2p).read(), "D2 mutation absent on disk"
    rc, out, err = run_norm(d2p)
    d2_hash = out.split()[0] if rc == 0 else None
    ok = rc == 0 and d2_hash == base_hash
    record("D2", "listed legitimate variation is erased, H holds", ok,
           f"rc={rc} H={d2_hash} {'held' if ok else 'MOVED - erasure incomplete'}")

    # ---- D3: acceptance side, unmutated input passes -------------------
    rc, out, err = run_norm(SPECIMEN)
    ok = rc == 0 and out.split()[0] == base_hash and not err.strip()
    record("D3", "acceptance: clean specimen normalizes, exit 0, no stderr", ok,
           f"rc={rc} stderr={err.strip()[:60]!r}")

    # ---- D4: empty and short transcripts MUST be refused ---------------
    emptyp = os.path.join(tmpd, "empty.log")
    open(emptyp, "w").write("")
    rc_e, _, err_e = run_norm(emptyp)
    shortp = os.path.join(tmpd, "short.log")
    open(shortp, "w").write("a\nb\nc\n")
    rc_s, _, err_s = run_norm(shortp)
    ok = rc_e != 0 and rc_s != 0
    record("D4", "empty and short transcripts are refused, not hashed", ok,
           f"empty rc={rc_e}, short rc={rc_s} "
           f"{'both refused' if ok else 'A SHORT CAPTURE WOULD HASH STABLY'}")

    print()
    n_fail = sum(1 for _, _, ok, _ in results if not ok)
    print(f"controls: {len(results)} run, {n_fail} failed")
    return 0 if n_fail == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
