# G05: authorization blocked

**State: `authorization_blocked`. Not incomplete, and not verified.**

## What the node requires

> Run an authorized disposable remote Nix/Cachix acquisition and launch smoke:
> acquire a pinned fork revision through Nix in a disposable environment, verify
> Cachix/substituter behavior without credentials, and launch the resulting
> binary.

This requires provisioning a throwaway remote host. That is external
infrastructure the agent must not create unilaterally.

## Why this is blocked rather than pending

As with G02, there is no local work left. The blocker is authorization to
provision infrastructure, not anything in the repository.

## Named next action

With explicit user authorization:

1. Provision a disposable host.
2. Acquire the pinned fork revision through Nix **with no credentials present**.
   That absence is itself the assertion: the public Cachix path must work
   unauthenticated, so running this with credentials available would prove
   nothing about the end-user acquisition path.
3. Launch the binary.
4. Record the transcript under `docs/fork/ideal-base/evidence/G05/`.

## Related signal already in hand

G03 built the flake and verified that the resulting binary runs and serves the
browser control surface correctly. So a G05 failure would isolate to
**acquisition**, not to the package. That narrows what this gate can still
discover, and it is worth being explicit that it does not substitute for it: G03
built locally, which is exactly the path G05 exists to *not* exercise.

## Not verified

No disposable-host acquisition has been run. This file records a decision about
authorization, not a result.
