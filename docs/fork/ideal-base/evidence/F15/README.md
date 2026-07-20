# F15: CI test hermeticity audit and ignored-test census

Date: 2026-07-20. Scope: whole workspace at HEAD (main, 4d39d7c76 base). Census
method: `rg -n '#\[ignore' crates/ src/ tests/ --type rust` plus `rg -n 'ignore ='`
for reason strings, cross-checked against `.github/workflows/*.yml`.

## 1. CI rail summary

| Workflow | Trigger | What actually executes | Compile-only / advisory |
|---|---|---|---|
| `ci.yml` (upstream, "CI") | **workflow_dispatch only** (fork policy header, ci.yml:3-8). Does NOT gate the fork. | quality guardrails (fmt, check, clippy -D warnings, ratchets); Build & Test matrix (ubuntu, macos): release build, `--lib --bins --no-run`, then RUNS `--test provider_matrix` and `--test e2e`; windows job runs targeted validation + e2e smoke | lib/bin tests are `--no-run` (compile-only) on all platforms; Windows ARM64 xwin check is `continue-on-error` (ci.yml:441-442) |
| `fork-ci.yml` (the real gate) | push/PR to main + weekly cron `0 8 * * 1` | **blocking**: quality guardrails; macOS release build + binary launch; macOS compile of workspace lib tests; macOS provider_matrix + e2e RUN; Linux job: workspace lib tests RUN (excl. jcode-tui, jcode-app-core split out), app-core lib RUN, provider_matrix RUN, e2e RUN | **advisory**: macOS workspace lib-test RUN and macOS app-core lib RUN (`continue-on-error: true`, fork-ci.yml:272-291, exit condition in-file: "Promote to blocking by deleting continue-on-error once a clean week of runs shows they are stable"); Linux Tests job is `continue-on-error` on push/PR but **blocking on the weekly schedule** (fork-ci.yml:313-317); latest-stable canary is advisory by design (toolchain drift detector, fork-ci.yml:219-221) |
| Others | freebsd-smoke, windows-smoke, nix, security, sync, release, fork-health | smoke/infra rails, no workspace test execution | n/a |

Key structural facts:

- `jcode-tui` lib tests are **compile-only on every rail** ("compile-only by
  design", fork-ci.yml:346-348). Reason: TUI suites unaudited for terminal
  hermeticity; stale upstream too. Exit condition: audit + un-ignore rail like
  the Linux lib promotion in 46e8a13fb (`ci: execute workspace library tests on
  Linux (was compile-only)`).
- macOS lib-test execution advisory: reason and exit condition are source-backed
  in fork-ci.yml:272-276 (hermeticity not audited the way Linux was, per
  docs/fork/patch-ledger.md).
- Linux Tests advisory on push/PR: reason in fork-ci.yml:313-316 (hosted-runner
  timing flakes whose fixes grew fork divergence); exit condition: weekly
  scheduled run is blocking, so regressions surface with bounded delay.
- No `--ignored` flag appears anywhere in `.github/workflows/`, so every
  `#[ignore]` test below is **never executed in CI**.

## 2. Ignore census (58 ignored items, all classified)

Buckets: **deterministic** (7), **live** (9), **GUI** (18), **platform** (2),
**helper** (6), **performance** (16).

### deterministic (7) — could be promoted; see shortlist

| Location | Test | Reason today | Exit condition |
|---|---|---|---|
| tests/e2e/binary_integration.rs:138 | binary_integration_reload_handoff | needs prebuilt `target/release/jcode` (doc comment L133-136) | run after the release-build step that fork-ci macOS already performs; pass binary path via env |
| tests/e2e/binary_integration.rs:266 | binary_integration_selfdev_reload_reconnects_quickly | same + PTY client (unix-only) | same; PTY is available on hosted runners |
| tests/e2e/binary_integration.rs:372 | binary_integration_selfdev_client_reload_resumes_session | same | same |
| tests/e2e/binary_integration.rs:534 | binary_integration_selfdev_full_reload_resumes_session_quickly | same, plus "older starter binary" fixture | same; starter binary needs a build-cache story |
| crates/jcode-tui-mermaid/tests/layout_cache_pixel_parity.rs:56 | layout_cache_hit_renders_pixel_identical_png... | "pixel-parity probe: run explicitly" — but it is deterministic (renders PNGs in-process, no display) | verify runtime cost is acceptable, then un-ignore |
| crates/jcode-tui-mermaid/tests/layout_cache_cross_width_parity.rs:304 | layout_cache_cross_width_parity | "cross-width parity probe" — deterministic byte comparison; ignored for cost and multi-run nonce protocol (file header L51) | fold the two-run nonce protocol into one test or accept single-run coverage |
| crates/jcode-base/src/provider/tests.rs:1083 | new_session_fork_reloads_changed_config_provider_and_model | "upstream-new test assumes upstream's provider_for_model selection; fork routing picks the OpenRouter stub" (attr string) | rewrite assertions against fork routing, or delete; fork_for_new_session already covered by e2e per the attr |

### live (9) — need credentials/network; correctly ignored

| Location | Test | Reason |
|---|---|---|
| tests/e2e/binary_integration.rs:42 | binary_integration_independent_claude | "Requires Claude credentials" |
| tests/e2e/binary_integration.rs:74 | binary_integration_openai_provider | "Requires OpenAI/Codex credentials" |
| crates/jcode-provider-anthropic-runtime/src/anthropic_tests.rs:671 | live_anthropic_reasoning_smoke | needs ANTHROPIC_API_KEY / OAuth opt-in |
| crates/jcode-provider-openai-runtime/src/openai_tests/transport_runtime.rs:2,45 | live_openai_catalog..., live_openai_gpt_5_4... | "requires real OpenAI OAuth credentials" |
| crates/jcode-provider-openrouter-runtime/src/openrouter_tests.rs:1471 | live_openrouter_unified_reasoning_smoke | needs OPENROUTER_API_KEY |
| crates/jcode-provider-bedrock/src/lib.rs:1904 | bedrock_live_smoke_test | "requires AWS credentials and enabled Bedrock model access" |
| crates/jcode-app-core/src/channel.rs:855 | test_relay_live_roundtrip | "requires live Jade relay credentials" |
| crates/jcode-base/src/auth/claude_tests.rs:535 | live_keychain_native_credentials_detected_and_parsed | "live: reads the real macOS Keychain" (also platform-bound, primary blocker is live user credentials) |

Exit condition for the whole bucket: a scheduled credentialed live-smoke rail
(secrets in GH environment), out of scope for hermetic CI.

### GUI (18) — need display/permissions; correctly ignored

- crates/jcode-app-core/src/tool/computer/tests.rs:177,187,199,208,224,233,246,255,267,278
  — 10 tests, all `ignore = "requires GUI + permissions"` (live_check_permissions,
  live_cursor_and_move, live_screenshot, live_ui_tree, live_ocr_full_screen,
  live_ocr_region, live_list_windows, live_clipboard_roundtrip, live_applescript,
  live_background_set_value).
- crates/jcode-app-core/src/tool/computer/coverage_tests.rs:49,65,80,91,110,125,148,169
  — 8 tests `ignore = "live"`, mutate the desktop (TextEdit, clipboard, windows);
  file header mandates `--ignored --test-threads=1` invocation.

Exit condition: a macOS runner with Accessibility/Screen Recording permissions
granted (self-hosted). Not promotable on hosted runners.

### platform (2)

| Location | Test | Reason |
|---|---|---|
| crates/jcode-app-core/src/tool/bash_tests.rs:77,121 | test_stdin_forwarding_single_line, (second stdin test) | `cfg_attr(target_os = "macos", ignore = ...)`: libproc thread-state stdin-wait detection unreliable on macOS; Linux /proc path IS exercised in CI. Exit condition: reliable macOS stdin-wait detection (e.g. kqueue/proc_pidinfo rework) |

### helper (6) — not real tests

| Location | Item | Role |
|---|---|---|
| crates/jcode-base/src/platform_tests.rs:57 | spawn_detached_child_probe | explicit `ignore = "helper process for spawn_detached_creates_new_session"` — re-exec probe child |
| crates/jcode-app-core/src/tool/tests.rs:539 | print_tool_definition_token_report | prints token-cost report for humans, asserts nothing meaningful for CI |
| crates/jcode-tui/src/tui/info_widget_stability_tests.rs:166,276,325,380 | demo_quantify, demo_content_anchor, demo_info_tradeoff, demo_lookahead_sweep | print stability-metric tables for developer inspection; the assertable versions of these metrics already run un-ignored in the same file |

### performance (16) — benchmarks/probes, timing- or environment-sensitive

- tests/e2e/burst_spawn.rs:654 burst_spawn_resume_attach_scales_to_100_clients
  ("resource-heavy scale validation"; the smaller-N variants run in CI e2e).
- crates/jcode-tui/src/tui/session_picker_tests.rs:103,169,1431,1488,1529,1580,1615
  — 7 `benchmark_resume_*` ("developer benchmark", two of them read real JCODE_HOME).
- crates/jcode-tui/src/tui/session_picker/loading_tests.rs:831,953 — 2 real-/resume
  loading benchmarks (real session directory).
- crates/jcode-tui/src/tui/app/state_ui_input_helpers.rs:1782
  onboarding_suggestion_scan_cost (reads real ~/.codex and ~/.claude).
- crates/jcode-embedding/tests/embed_latency_probe.rs:27 (requires installed model).
- crates/jcode-tui-markdown/tests/highlight_backend_probe.rs:97 (perf/memory probe).
- crates/jcode-tui-mermaid/tests/layout_cache_resize_probe.rs:134 (wall-clock).
- crates/jcode-tui-mermaid/tests/layout_cache_memory_probe.rs:54 (memory probe).
- crates/jcode-app-core/src/tool/session_search_tests.rs:186 (real ~/.jcode corpus).

Exit condition: a nightly perf rail with recorded baselines, not the PR gate.

## 3. Flakiness root-cause table

| Symptom | Root cause | Fix commit | Bug class |
|---|---|---|---|
| bash background-output test failed intermittently | fixed 250ms sleep before asserting background output | 89af6a2a4 `test(bash): poll for background output instead of fixed 250ms sleep` | fixed-sleep race |
| background completion test flaked under runner load | 30x25ms poll budget too tight on loaded hosted runners | be928382a / fbbe4eb8e `test(background): widen completion poll budget to de-flake under load` | load sensitivity |
| mcp test hung on child that never completes handshake | no health deadline pre-handshake, child could wedge the test | edde05580 `test(mcp): set short health deadline after handshake; harden F08 gate env` | missing-deadline hang |
| client_lifecycle test intermittently corrupted env for parallel tests | env-var mutation without lock; also deferred cleanup retry firing inside tests | 4b66de27c `fix(server): gate deferred cleanup retry out of tests; serialize env-mutating lifecycle test` | env-var leakage |
| Linux e2e rail flaked repeatedly on hosted ubuntu | hosted-runner timing sensitivity; "fixes" grew fork divergence | policy fix, fork-ci.yml:313-317 (advisory on push, blocking weekly) | load sensitivity (systemic) |
| watch-channel lost wakeups in swarm/lease state machine tests | send/recv race in state transitions | d8c223d29 (F03 watch-send fix), 58a806401 (lost-wakeup guard) | lost-wakeup race |

`git log --grep='flake'` also surfaces 8a81c60b2 (B4 flake parity proof docs).

## 4. Promotion shortlist for F16 (ordered by value)

1. **binary_integration_reload_handoff** (tests/e2e/binary_integration.rs:138)
   — highest value: reload/handoff is the fork's riskiest surface; the release
   binary it needs is already built earlier in the fork-ci macOS job.
2. **binary_integration_selfdev_client_reload_resumes_session** (:372) — PTY
   client resume path, unix-only, same prerequisite.
3. **binary_integration_selfdev_reload_reconnects_quickly** (:266) — carries a
   latency assertion, so promote with a generous CI bound or split the timing
   assertion out (avoid re-creating the load-sensitivity class above).
4. **layout_cache_pixel_parity + cross_width_parity** (jcode-tui-mermaid) —
   deterministic byte-level regression nets, cheap wins if runtime is bounded.
5. **new_session_fork_reloads_changed_config_provider_and_model** — lowest
   value: rewrite-or-delete decision; e2e already covers fork_for_new_session.

Anti-candidate warning: binary_integration_selfdev_full_reload_resumes_session_quickly
(:534) needs an "older starter binary" fixture; defer until a binary-cache story
exists.

## 5. Findings

- ci.yml is dispatch-only for the fork (by documented policy), but its Linux/macOS
  Build & Test jobs never run lib tests even upstream (`--no-run` only); e2e and
  provider_matrix are the only executed suites there.
- The only rail that RUNS workspace lib tests blocking is fork-ci Linux weekly.
  On push/PR the Linux run is advisory, so a lib-test regression can land and
  sit up to 7 days before a blocking signal.
- jcode-tui lib tests execute on NO rail (compile-only everywhere), and its
  ignored benchmarks mask that the un-ignored TUI tests are also unexecuted.
- Two bare `#[ignore]` attributes without any reason string existed only in
  binary_integration.rs (reasons are in doc comments above) and
  info_widget/tool tests (demo/report helpers); no truly unexplained ignore found.
- All 58 ignored items classified; zero unclassified remain.
