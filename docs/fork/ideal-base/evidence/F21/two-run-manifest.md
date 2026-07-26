# F21 two-run integration gate manifest

- source identity: `2add7453f3a63fa362cffe99e6aa1e41af0d98cf`
- single commit across runs: yes
- runs: 2
- all checks passed: yes
- runs agree: yes

| check | run 1 | run 2 | agree |
|---|---|---|---|
| `suites.jcode-base` | PASS `passed=1205 failed=0` | PASS `passed=1205 failed=0` | yes |
| `suites.jcode-tui` | PASS `passed=1867 failed=0` | PASS `passed=1867 failed=0` | yes |
| `suites.jcode-app-core` | PASS `passed=1136 failed=0` | PASS `passed=1136 failed=0` | yes |
| `package.nix_build` | PASS `fa0mbkdylvqnr3r66dx9if4m743y01d9-jcode-0.46.0` | PASS `fa0mbkdylvqnr3r66dx9if4m743y01d9-jcode-0.46.0` | yes |
| `install.assets` | PASS `present=bin/jcode,share/jcode/web/jcode-mobile` | PASS `present=bin/jcode,share/jcode/web/jcode-mobile` | yes |
| `install.mobile_entrypoint` | PASS `index.html=yes` | PASS `index.html=yes` | yes |
| `install.launches` | PASS `jcode v0.46.0 (2add745)` | PASS `jcode v0.46.0 (2add745)` | yes |
| `updater.declines_self_update` | PASS `declined=True downloaded=False` | PASS `declined=True downloaded=False` | yes |
| `updater.no_retired_layout_written` | PASS `builds_dir=absent` | PASS `builds_dir=absent` | yes |
| `updater.doctor_origin` | PASS `origin=nix` | PASS `origin=nix` | yes |
| `residue.real_home_untouched` | PASS `added=none` | PASS `added=none` | yes |
| `residue.no_leaked_sessions` | PASS `session_delta=0` | PASS `session_delta=0` | yes |
