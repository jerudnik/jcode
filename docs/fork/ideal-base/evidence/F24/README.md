# F24 evidence: reproducible Nix output, provenance, and SBOM

Node: `F24` (implement), parent `W4`, base commit
`2dd288ec1e00895f74332e105f9f57e2f2077bb0`.
Acceptance context: `ACCEPTANCE_STANDARD.md` A6.

The implementation and independent code review were completed at
`e9fb3220abac523b88ac73c2f17a78049145af9d` on branch
`automation/w4-f24`. This evidence record is a documentation-only closeout
commit after that measured and reviewed code SHA.

## Acceptance outcome

| Gate | Outcome |
| --- | --- |
| Two clean builds match within the declared reproducible scope | **PASS.** A fresh JRTI build and two subsequent forced rebuilds produced the same output path, NAR hash and size, manifest hash, installed-file hash-list hash, and runtime version. |
| Artifacts expose verifiable provenance and SBOM | **PASS.** The real provenance and SBOM outputs rebuilt byte-identically, the independent verifier matched them against separately captured product facts, and the SBOM passed the official CycloneDX 1.5 JSON schema. |
| Independent exact-SHA review | **PASS.** Opus review at `e9fb3220a` found no merge-blocking defect after all prior findings were resolved. |

Two matching rebuilds are empirical evidence, not a mathematical proof. The
claim is deliberately narrow: the installed `packages.x86_64-linux.jcode`
output. Compatibility archives, `scripts/build_linux_compat.sh`, and release
assets are excluded unless they receive a separate reproducibility contract.

## What changed

- Updated the Nix fixed-output hashes for the two Git dependencies currently
  locked in `Cargo.lock`.
- Added `packages.x86_64-linux.jcode-provenance`, which emits
  `share/jcode/provenance.json` with source revision, version, platform,
  derivation, output NAR, SBOM linkage, scope, and exclusions.
- Added `packages.x86_64-linux.jcode-sbom`, which emits a deterministic
  CycloneDX 1.5 Cargo component inventory at
  `share/jcode/sbom.cdx.json`.
- Added an independent structural verifier and a fixture check that consumes
  the real SBOM derivation output.
- Added three observed-failing plants: provenance revision mismatch, locked
  Git dependency omission, and duplicate `bom-ref`.
- Documented the exact reproducibility scope and rebuild procedure in
  `docs/NIX.md`.

## Exact-SHA JRTI measurement

The measurement ran in a clean checkout on JRTI:

```text
host=just-read-the-instructions
system=x86_64-linux
nix=nix (Determinate Nix 3.21.1) 2.34.7
source_revision=e9fb3220abac523b88ac73c2f17a78049145af9d
flake_lock_sha256=6a167fbed830fe38b2015ff67bbe1072258c0ce649af15f314363b85a9159c93
```

The first operation was a fresh `--no-substitute` realization because Nix
correctly rejects `--rebuild` when no valid prior output exists. It was
followed by two `--rebuild --no-substitute` checks. Each of the three builds
compiled the product from source.

All three observations matched:

```text
derivation_path=/nix/store/sxxdrf3n9gdhzkhv5w7hwv6yfdx053za-jcode-0.46.0.drv
output_path=/nix/store/ywfv5vv3yi74hhjzh7w3wh8074hg9cbd-jcode-0.46.0
nar_hash=sha256-0cLrywUSggKv4E3Rd+Ha/3bN03L13GccoHnfF5EqKbg=
nar_size=143049768
manifest_sha256=4ff0d3cd8d4990021935029720e2580971aff232cd7412edd87214dc4613ec52
files_sha256=554f593865a756b44b352b79bc10993f2f2608b921d095014431060310f25993
runtime_version=jcode v0.46.0 (e9fb322)
```

The dry run before the fresh build named exactly the top-level
`jcode-0.46.0.drv` as needing realization. The forced rebuild commands then
used Nix's output comparison in addition to the independently captured
fingerprints above.

## Provenance and SBOM companion verification

The companion outputs were realized and then forced-rebuilt in the same exact
JRTI checkout. Baseline and check files compared byte-for-byte equal:

```text
sbom_output=/nix/store/3k8f55hw3iaa4w56w0a7h6aprc6sa8h9-jcode-sbom-0.46.0
sbom_sha256=5acaa1185fe83c3dd3fc6e420e9d20e086ac14c225d83757c3078db80752bdac
provenance_output=/nix/store/ygdwfglq1xycsi6m3d2n8w0pbls6ygmz-jcode-provenance-0.46.0
provenance_sha256=6627d73ed6dceb6fc4d4683c0c6c1622fb336e811963a633c7a0dd1021bd9cca
```

The coordinator recovered those exact JRTI artifacts and ran
`nix/verify-provenance-sbom.py` against independently supplied expected facts
from the product measurement. The verifier passed and confirmed:

- source revision `e9fb3220abac523b88ac73c2f17a78049145af9d`;
- artifact scope `packages.x86_64-linux.jcode`;
- the exact derivation, output path, NAR hash, and NAR size above;
- the complete three-item exclusion list;
- SBOM hash `5acaa118...52bdac`.

The recovered real SBOM also passed:

```sh
check-jsonschema \
  --schemafile https://cyclonedx.org/schema/bom-1.5.schema.json \
  sbom.json
```

Result: `ok -- validation done`.

The generated document uses the canonical CycloneDX 1.5 schema identifier
`http://cyclonedx.org/schema/bom-1.5.schema.json`. The optional serial number is
omitted: a random UUID would break byte reproducibility, while a nil UUID would
claim a false instance identity. Component references are derived from
`name`, `version`, and source, and the verifier enforces global `bom-ref`
uniqueness.

## Non-vacuity and structural gates

The clean exact SHA passed:

```sh
nix flake check --no-build --all-systems --accept-flake-config
nix build --accept-flake-config \
  .#checks.x86_64-linux.provenance-sbom-fixtures
```

The real-SBOM fixture produced one green baseline and three intended red
mutations:

| Plant | Intended failure observed |
| --- | --- |
| Change `provenance.source.full_revision` to `wrong` | `expected 'full', got 'wrong'` |
| Remove one real VCS component | `locked git dependency omitted from SBOM: agentgrep ...` |
| Append a duplicate real component | `duplicate bom-ref in SBOM: urn:jcode:cargo:...` |

The fixture uses the actual `jcode-sbom` derivation output rather than a
synthetic approximation, so generator and verifier cannot drift while the
fixture remains green. The verifier checks all 950 non-root packages in the
current `Cargo.lock`: registry, Git, and workspace packages.

## Independent review

The first Opus review at `0477cd0e7` returned `DO-NOT-MERGE` with three medium
and four low findings. The implementation was corrected rather than waived:

1. provenance artifact and rebuild guidance are parameterized by Nix system;
2. the fixture consumes the real generated SBOM;
3. the nil UUID is removed and the reproducibility rationale is documented;
4. all exclusions are matched exactly;
5. both system fields are independently verified;
6. this durable evidence closes the empty-evidence observation;
7. registry `purl` and Git VCS references remain intentionally asymmetric and
   CycloneDX-compliant.

The final read-only exact-SHA Opus review at `e9fb3220a` returned **PASS** with
high confidence, no merge-blocking defect, and no new finding. The coordinator
then completed the checks the reviewer explicitly did not perform: clean Nix
evaluation, formal CycloneDX schema validation, and the exact-SHA JRTI rebuild
measurement.

## Failed approaches retained as evidence

- A first forced rebuild before any baseline failed with Nix's explicit
  `cannot check a missing output` diagnostic. The procedure was corrected to
  fresh baseline followed by two forced checks; the failed attempt is not
  counted as evidence.
- A local invocation using the configured remote-builder scheduler later
  reported `Unable to start any build` while another direct JRTI measurement
  owned the builder. It was not retried. Companion equality was recovered and
  independently verified from the already completed direct-JRTI outputs.
- An initial provenance verifier helper used `python3` on JRTI, where that
  command is not in the login shell PATH. The artifact files and hashes had
  already been produced; verification was completed locally with
  `/usr/bin/python3` against the recovered exact bytes.

These failures are setup/procedure findings, not product passes, and none is
used to inflate the acceptance result.

## Files changed by F24

- `flake.nix`
- `nix/package.nix`
- `nix/provenance.nix`
- `nix/sbom.nix`
- `nix/verify-provenance-sbom.py`
- `docs/NIX.md`
- `docs/fork/ideal-base/evidence/F24/README.md`
