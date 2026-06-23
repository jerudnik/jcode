---
description: DOX contract for the Jcode desktop application crate.
applyTo: "crates/jcode-desktop/**"
---

# Jcode Desktop Agent Context

## Purpose

This directory is the Jcode desktop application crate.

## Ownership

- Desktop UI, desktop session launch behavior, desktop packaging hooks, and desktop-specific integration code live here.
- Shared behavior should move to shared crates only when the desktop implementation requires it.

## Local Contracts

- When a desktop-launched agent opens here, assume self-development work is focused on the desktop application unless the user says otherwise.
- Prefer targeted desktop checks while iterating: `cargo check -p jcode-desktop` and relevant `jcode-desktop` tests.
- Keep changes scoped to desktop UI/session-launch code when possible.
- Desktop sessions launched by the app default to this directory so local `AGENTS.md` context primes agents for desktop self-dev work.

## Verification

Run `cargo check -p jcode-desktop` for desktop Rust changes, plus targeted tests when behavior changes.
