# R07C Telemetry, reporting, and analytics consent: lightweight ledger

| Field | Value |
|---|---|
| State | `adjudicated` |
| Baseline | fork `f5a8999d81311d237d1c106a9d980fd86fa34b6e`; upstream `802f6909825809e882d9c2d575b7e478dce57d3b`; merge base `631935dd1d3b2e31e167e2b12ad463e54bcf4b8d` |
| Review mode | `light` (pilot opt-out check required per `RESPONSIBILITIES.md`) |
| Research budget | 10 decisive checkpoints for the R06A/R07C/R13 batch; shared batch consumed 10 |
| Recommended disposition | `retain-fork` |
| Confidence | high for the kill-switch semantics; medium for exhaustive call-site coverage (see gaps) |

R07C owns reporting scope, channel labels, analytics opt-in/opt-out, and prevention of secret or session-content leakage. It excludes discovery ranking (R07B) and provider usage accounting (R02). The R00/R09/R11 overlays bind this ledger. The pilot prerequisite is invariant #7 plus prerequisite 5 in `RESPONSIBILITIES.md`: reporting must be provably disabled for the fixture run and no secret or session content may leave the disposable environment.

## Findings

| Finding | Evidence | Consequence |
|---|---|---|
| One global kill switch gates all sends | `crates/jcode-telemetry-core/src/lib.rs:284-296` `is_enabled()` returns false when `JCODE_NO_TELEMETRY` or `DO_NOT_TRACK` is set, or when `$JCODE_HOME/no_telemetry` exists (`storage::jcode_dir()`, `crates/jcode-storage/src/lib.rs:75-82` honors `JCODE_HOME`); every send-family entry point re-checks it (lines 312, 371, 422, 442, 480, 1195, 1279, 1328, 1411, 1462) | Setting `JCODE_NO_TELEMETRY=1` (or a `no_telemetry` marker inside the disposable `JCODE_HOME`) disables reporting for the whole fixture run |
| Opt-out is enforced by tests, not just code reading | `bash scripts/dev_cargo.sh test -p jcode-telemetry-core --lib` in a clean `JCODE_HOME=$(mktemp -d)` passed 17/17 on 2026-07-15, including `test_opt_out_env_var` and `test_do_not_track` | The disabled path is deterministically verifiable without network |
| Content sharing is a separate, off-by-default consent | `content_sharing_enabled()` (`lib.rs:311-322`) requires base telemetry enabled AND the `$JCODE_HOME/telemetry_share_content` marker; a fresh disposable `JCODE_HOME` has no marker, so it is false | Prompt/transcript content cannot be reported in the fixture run even if the base switch were accidentally left on |
| All egress funnels through one endpoint and one client | `send_payload` (`lib.rs:975`) via `TELEMETRY_HTTP_CLIENT` (`lib.rs:31`) to `TELEMETRY_ENDPOINT` (`lib.rs:24`); the only fork/upstream divergence in `jcode-telemetry-core` is the endpoint constant (fork Workers URL vs upstream `telemetry.jcode.sh`) plus removal of the endpoint-domain test, confirmed by `git diff 802f69098 HEAD -- crates/jcode-telemetry-core` at merge-base-identical semantics otherwise | Blocking one env var blocks the single egress funnel; the semantic surface fork vs upstream is a routing constant, not consent logic |
| Free-text fields are sanitized and explicit-only | Feedback events use `sanitize_feedback_text` (`lib.rs:448-465`) and are only produced by an explicit `/feedback` command; `sanitize_telemetry_label` strips ANSI/controls (`test_sanitize_telemetry_label_strips_ansi_and_controls` passed); `test_discovery_event_serialization_excludes_free_text` passed | No implicit session-content channel exists in the metrics schema; the fixture run issues no `/feedback` |
| Content-sharing path inventory | Opt-in/opt-out surfaces found: env `JCODE_NO_TELEMETRY`, env `DO_NOT_TRACK`, marker `$JCODE_HOME/no_telemetry` (all opt-out); marker `$JCODE_HOME/telemetry_share_content` via `set_content_sharing_enabled` (`lib.rs:325`), written only from the onboarding flow (`crates/jcode-tui/src/tui/ui_onboarding.rs`) (opt-in) | Four consent paths total; the fixture controls all four by using a fresh `JCODE_HOME` plus `JCODE_NO_TELEMETRY=1` |

## Negative findings

- No telemetry send path was found that bypasses `is_enabled()`: every `pub fn` in `lib.rs` that reaches `send_payload` checks it first (grep at lines listed above); `lifecycle.rs:325` sends only through `send_payload`.
- No telemetry call site was found in the evidence or session persistence code paths (`crates/jcode-base/src/session/`), so R06A storage does not leak into R07C reporting.
- One environment-sensitivity hazard was found and root-caused: with the developer's real `~/.jcode/no_telemetry` marker visible, 5 of 17 `jcode-telemetry-core` tests fail (`test_error_counters` and friends) because `begin_session_with_mode` (`lib.rs:1456-1464`) early-returns when disabled. With `JCODE_HOME` pointed at a fresh temp dir, 17/17 pass. This is a test-isolation defect, not a consent defect. It also proves the kill switch works.
- `DEFAULT_DISCOVERY_ENDPOINT` (`lib.rs:29`) is R07B's surface; the no-tool pilot never calls discovery, and R07B remains `defer`.

## Pilot consent posture (required by prerequisite 5)

Fixture entry checks:

1. Run with `JCODE_HOME=<disposable dir>` and `JCODE_NO_TELEMETRY=1` exported for every pilot process.
2. Assert `is_enabled()` is false in-process, or equivalently create `$JCODE_HOME/no_telemetry` as a second, redundant switch.
3. Assert `$JCODE_HOME/telemetry_share_content` does not exist.

Fixture exit checks:

4. No process in the pilot opened a connection to the telemetry endpoint (cheapest deterministic form: the disabled-path unit tests above plus absence of `begin telemetry session` lines in `$JCODE_HOME/logs`).
5. Nothing outside the disposable `JCODE_HOME` and the pilot worktree was written (compare a before/after listing of `~/.jcode` if the real home is reachable at all).

## Reproduction

```bash
git diff 802f6909825809e882d9c2d575b7e478dce57d3b HEAD -- crates/jcode-telemetry-core  # endpoint constant + test removal only
JCODE_HOME=$(mktemp -d) bash scripts/dev_cargo.sh test -p jcode-telemetry-core --lib -- --test-threads=1  # 17 passed
grep -n "JCODE_NO_TELEMETRY\|DO_NOT_TRACK" crates/jcode-telemetry-core/src/lib.rs      # lines 285, 315
grep -n "is_enabled()" crates/jcode-telemetry-core/src/lib.rs                          # every send family gated
```

## Explicit gaps

- The `telemetry-worker/` server-side code and `TELEMETRY.md` schema prose were skimmed, not audited line-by-line; they are receive-side and irrelevant when sends are disabled.
- Upstream's 134-line `jcode-telemetry-core` evolution since the merge base (`git diff --stat 631935dd..802f69098`) was not semantically reviewed; both sides converge on the same kill-switch code at the fixed refs, which is what the pilot needs.
- The onboarding UI flow that writes `telemetry_share_content` was located but not executed; the fixture never runs onboarding.

## Disposition and conditions

- Recommended disposition: `retain-fork`. Consent logic is semantically identical to upstream at the fixed refs; the fork differs only in the endpoint constant, which is a deliberate fork routing choice and not a consent regression.
- Acceptance or retirement condition: accepted for the pilot when the coordinator approves and the entry/exit checks above pass in the disposable environment; retires into a full review only if telemetry consent logic is changed or a bypass send path is discovered.
- Escalate to full review if: any send path is found that does not check `is_enabled()`; content sharing activates without the marker; the fixture run produces telemetry log lines despite `JCODE_NO_TELEMETRY=1`; or the pilot scope grows to include discovery/network consent (that escalates R07B, and jointly this seam).
- Coordinator approval: `pass`; the four consent paths and fresh-`JCODE_HOME` 17-test disabled-path baseline were reproduced before integration.
- Independent Opus review: `pass` with no correction required; see [`2026-07-15-pilot-prereq-ledgers-opus-review.md`](../../reviews/2026-07-15-pilot-prereq-ledgers-opus-review.md), SHA-256 `bb763b0924cd16196785e9129663531990e6364225a7d57467f0a834e4bf73b4`.
