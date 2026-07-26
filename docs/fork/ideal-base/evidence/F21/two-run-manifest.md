# F21 two-run integration gate manifest

- source identity: `44638413148b2c93daea5ba1dd582440c8bd8c71`
- single commit across runs: yes
- runs: 2
- all checks passed: yes
- runs agree: yes

| check | run 1 | run 2 | agree |
|---|---|---|---|
| `suites.jcode-base` | PASS `passed=1205 failed=0` | PASS `passed=1205 failed=0` | yes |
| `suites.jcode-tui` | PASS `passed=1867 failed=0` | PASS `passed=1867 failed=0` | yes |
| `suites.jcode-app-core` | PASS `passed=1136 failed=0` | PASS `passed=1136 failed=0` | yes |
| `package.nix_build` | PASS `dpz15lf2l8kbr8qw4aqah0rmiil5kcf1-jcode-0.46.0` | PASS `dpz15lf2l8kbr8qw4aqah0rmiil5kcf1-jcode-0.46.0` | yes |
| `install.assets` | PASS `present=bin/jcode,share/jcode/web/jcode-mobile` | PASS `present=bin/jcode,share/jcode/web/jcode-mobile` | yes |
| `install.mobile_entrypoint` | PASS `index.html=yes` | PASS `index.html=yes` | yes |
| `install.launches` | PASS `jcode v0.46.0 (4463841)` | PASS `jcode v0.46.0 (4463841)` | yes |
| `updater.declines_self_update` | PASS `declined=True downloaded=False` | PASS `declined=True downloaded=False` | yes |
| `updater.no_retired_layout_written` | PASS `builds_dir=absent` | PASS `builds_dir=absent` | yes |
| `updater.doctor_origin` | PASS `origin=nix` | PASS `origin=nix` | yes |
