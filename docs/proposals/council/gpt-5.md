As the GPT-5.5 council member, my view: true “move everything live” is the wrong target. A TLS/SSE LLM stream and an arbitrary child process cannot be cleanly transplanted into a new Rust process in a portable, robust way. The design should combine **drain live work**, **checkpoint semantic state**, and **isolate reload-sensitive ownership behind stable supervisors**.

`jcode` already has the right base: single shared daemon, Unix socket clients, reconnect, and durable swarm control log per [SERVER_ARCHITECTURE.md](/Users/jrudnik/labs/jcode/docs/SERVER_ARCHITECTURE.md) and [SESSION_LIFECYCLE_ROADMAP.md](/Users/jrudnik/labs/jcode/docs/SESSION_LIFECYCLE_ROADMAP.md).

**1. Blue-Green Daemon With Graceful Drain**

Mechanism: start new daemon binary alongside old. A small stable launcher/registry owns the canonical socket path or passes listener FDs to generations. New clients attach to generation N+1; old generation stops accepting new work and drains existing turns, children, MCP calls, and attached clients. When old has no live turns/children or a timeout expires, it exits.

This mirrors nginx live binary upgrade: old and new masters coexist; old workers shut down gracefully after clients finish, with rollback possible if the new binary is bad. Envoy’s hot restart is even closer: two processes coordinate over Unix sockets; new initializes, gets listen sockets, then old drains. Envoy explicitly does **not** transfer existing connections; they finish in the old process or are closed. That limitation should be accepted for jcode too. Sources: nginx control docs and Envoy hot restart docs. ([nginx.org](https://nginx.org/en/docs/control.html)) ([envoyproxy.io](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/operations/hot_restart))

Preserves: active LLM streams, child OS processes, MCP servers, and in-memory per-turn state by keeping the old daemon alive. Clients can reconnect to the new daemon when idle; headless work keeps running where it already is. Swarm graph durability remains W1/W2-backed.

Hard parts: generation routing, “new work forbidden” state in old daemon, client affinity while a session is still draining, and bounded shutdown so old daemons do not pile up. Child processes are preserved only because their parent stays alive; they are not migrated.

Effort/risk: medium. This is the cheapest high-value architecture and fits current `exec()` reload semantics with fewer conceptual changes than full process decomposition.

**2. Stable Control Host Plus Reloadable Worker Daemon**

Mechanism: split `jcode serve` into a tiny stable host and reloadable engine workers. The host owns the canonical Unix socket, registry, auth/config identity, generation table, client routing, and lifecycle policy. A worker generation owns session execution. Reload starts a new worker; the host routes new/idle sessions to it while old workers drain.

This is systemd socket activation plus app-level routing. `sd_listen_fds` shows the transferable pattern: an external manager keeps bound sockets and hands descriptors to restarted services; Unix sockets also support passing FDs with ancillary data. ([man7.org](https://man7.org/linux/man-pages/man3/sd_listen_fds.3.html)) ([man7.org](https://man7.org/linux/man-pages/man7/unix.7.html))

Preserves: client socket identity, attached-client continuity, registry stability, reload provenance, and a single place to enforce no-orphan invariants from [SERVER_LIFECYCLE_INVARIANTS.md](/Users/jrudnik/labs/jcode/docs/SERVER_LIFECYCLE_INVARIANTS.md). Live execution still drains in old workers.

Hard parts: protocol boundary between host and worker, version negotiation, backpressure, debug socket routing, and avoiding a second giant in-process state bag. This resembles the stable desktop-host plan in [DESKTOP_STABLE_HOST_RELOAD_STARTUP.md](/Users/jrudnik/labs/jcode/docs/DESKTOP_STABLE_HOST_RELOAD_STARTUP.md): keep the thing that must survive in a stable process, put volatile product logic behind a protocol.

Effort/risk: medium-high, but strategically clean. This is the long-term shape I would prefer over teaching one daemon to be both supervisor and reload target.

**3. Per-Turn / Per-Child Supervisors For True Zero-Loss Work**

Mechanism: move the non-migratable live resources out of the reloadable daemon. A per-turn “LLM stream broker” process owns the HTTP/TLS/SSE request and appends token deltas, tool-call fragments, request IDs, and final response events to disk. The daemon observes/controls it over UDS. A per-session or per-tool child supervisor owns spawned OS processes, stdio pipes, pids, and exit status. On daemon reload, the new generation reconnects to these supervisors.

This is the tmux/mosh idea applied internally: detach presentation/control from the long-lived execution endpoint. Mosh keeps the user experience stable across broken client connectivity; Erlang hot code loading is the opposite end of the spectrum, but it only works because code and process state obey strict VM-level rules that Rust/native child processes do not. ([mosh.org](https://mosh.org/)) ([erlang.org](https://www.erlang.org/doc/system/code_loading.html))

Preserves: in-flight LLM token streams across daemon binary reload, child process stdio, MCP server lifetimes, and exit accounting. This is the closest to actual zero-loss.

Hard parts: this is effectively a new supervision substrate. Passing a child pipe FD to a new process is possible, but the new daemon cannot portably become the child’s parent or `waitpid` owner. Linux `pidfd` helps, macOS less so. For LLM streams, passing the TCP fd is not enough because TLS and HTTP parser state live in library memory. CRIU can checkpoint Linux processes, including sockets in some cases, but it is Linux-specific, operationally heavy, and inappropriate as the default jcode reload primitive. ([criu.org](https://criu.org/Main_Page))

Effort/risk: high. Worth doing only for the most expensive live resources, probably LLM streams first.

**Recommendation**

Stage it.

First: implement **blue-green graceful drain**. Stop using `execve()` for normal reload. Spawn generation N+1, mark N as draining, route new sessions to N+1, keep N alive until in-flight LLM turns and child processes finish or hit a configurable ceiling. This will eliminate most token loss quickly without pretending live sockets can be migrated.

Second: add a **stable host/registry** that owns the canonical socket and generation routing. This makes reload behavior explicit, debuggable, and compatible with frequent selfdev reloads.

Third: introduce **LLM stream brokers** for high-cost turns. Do not try to migrate provider SSE streams; keep them alive outside the reloadable daemon and checkpoint semantic deltas. Child-process supervisors can follow, but only where losing the process is materially worse than retrying from the W1/W2 control log.

Manual switchover should be rejected. Work-stashing is useful only at semantic boundaries: checkpoint prompts, partial assistant deltas, tool intent, and swarm task progress; do not try to stash opaque TLS streams or arbitrary process memory.
