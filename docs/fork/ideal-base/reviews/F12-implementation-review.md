# F12 implementation review cycle (adversarial, opus-class, 3 rounds)

## Round 1: FAIL (base 814a6f707)

BLOCKING: pooled-children cap was check-then-act on clients.len();
concurrent leaders for different servers (connect_all with join_all) all
read the same stale count and overshot without bound relative to the cap.
Important: background cap same TOCTOU (smaller window); refusal invisible
at the bash tool layer ("Command started in background" for a refused
task); cap-refused leaders' waiters got the vague no-handle fallback.
Minor: env-mutation test lock gap; huge env values accepted silently;
adopt/detached paths ruled sound (no loophole); count semantics and F07
eviction release verified correct.

## Round 2: FAIL (fix 0a93900bc)

Slot-reservation approach correct in structure (connecting-map entries as
pool reservations; RAII SpawnSlot for background) but the two-map count
read clients THEN connecting: a leader completing finish_connect between
the reads could be missed in both maps, letting two leaders share one
free slot (cap+1). Same wrong order in the background count. Also missed:
write_initial-failure path returned refused: None; communicate run_plan
and selfdev build_queue ignored .refused.

## Round 3: PASS (fix a1c9075af)

Ordering verified: finish_connect inserts into clients strictly before
the sync guard Drop removes from connecting (no await between, cancel-safe),
so connecting-first reads see any transitioning leader in at least one
map; over-counting refuses conservatively. Background insert-then-drop
verified with no await between. Early returns before tokio::spawn release
capacity for tasks that never run. All caller gaps closed (bash and
run_plan error out; build_queue logs and relies on status-file follow-up).

Non-blocking follow-ups recorded: background cancellation window between
tokio::spawn and live-map insert can transiently overshoot (pre-existing);
finish_connect handle/client insert gap under cancellation (pre-existing);
comment could state the symmetric-counting argument.

## Gates

Low caps bound counts (cap tests with per-instance overrides); refusal
explicit at manager AND tool layer; capacity releases on terminal
cleanup/eviction. Suites: mcp 48, background 44, tool::bash 21, selfdev
36, communicate 89, all green.
