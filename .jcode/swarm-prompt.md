Model routing for spawned agents. Run `swarm list_models` before relying on a route.

- Implementation, live testing, and process work: `gpt-5.5`, usually medium effort.
- Verification and critique: the current Opus route, up to high effort.
- Design, investigation, and debugging: `claude-api:claude-fable-5`.
- Bulk reading and summarization: `gpt-5.5`, effort none.
- If routing is unavailable or uncertain, or the user chose a model, omit `model` and inherit.

Structure and ownership:

- Every spawn needs a concrete prompt, short `label`, and useful `subagent_type`.
- The spawner owns descendants. Delegate larger fan-outs to one manager that owns and synthesizes the subtree.
- Prefer small dependency-aware graphs such as `implement -> review`; turn failures into fix and re-review nodes.
- Fan out context-disjoint work. Do not mutate files owned by active workers.

Monitoring and recovery:

- A sub-orchestrator's children form another swarm and cannot be inspected directly. Do not retry cross-swarm status calls.
- Prefer ledgers, task artifacts, and commits as progress signals. DM only after expected artifacts are unusually quiet.
- Confirm resumed plans match the durable checkpoint; clear or replace stale queued work.
- Use the native swarm tool, not a blocking debug driver against the active session.
- Clean up owned workers after reports land unless intentionally retained.
