# Contributing to jcode

Thanks for contributing.

## Issues vs pull requests

If the problem is easy for me to reproduce, please prefer opening a GitHub issue. A clear issue with reproduction steps, expected behavior, actual behavior, logs, screenshots, or traces is usually the fastest path to a fix.

Pull requests are the normal integration path. They are especially useful when the problem depends on an environment I may not have, such as macOS-specific behavior, Windows-specific behavior, unusual shells, terminal emulators, filesystems, GPU/display setups, provider accounts, or other local configuration.

## Pull request policy

Pull requests are welcome and encouraged.

Most PRs should be focused, reviewable changes that can be merged through the normal branch flow. This project still relies on heavy code generation, so the review should look for subtle correctness, lifecycle, architecture, and maintenance issues instead of trusting a plausible generated diff.

If a submitted PR helps explain the bug, feature request, test case, design direction, or implementation, that is valuable even when the final committed code is adjusted during review. The goal is to land the right change through the normal PR process, not to treat PRs as disposable references.

The best PRs therefore include:

- a clear description of the problem being solved
- a minimal reproduction or failing test when possible
- notes about edge cases and tradeoffs
- focused changes that are easy to review independently
- any relevant logs, screenshots, traces, or benchmarks

Large, generated, or highly invasive PRs may still be split, revised, or closed when needed, but the default governance path is a normal pull request into `main`.

Handwritten by author: My clanker slop may or may not be better than your clanker slop. I know how to work with my clanker slop though.
