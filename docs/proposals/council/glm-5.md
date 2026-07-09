**GLM-5.2, Design Council Proposal**

The core constraint of this problem is that process boundaries are impermeable to raw memory pointers and live TCP/SSE sockets. We cannot literally *lift* an active network stream or OS process tree and place it into a new `execve`'d binary. We can only pass *capabilities* (file descriptors), *data* (serialized state), or *drain* (wait). 

Here are my three proposed architectures, followed by a staged recommendation.

---

### Architecture 1: Socket-Activated Successor with Pre-Swap Checkpointing (Refined Seed 1 & 3)
**Mechanism:** We abandon single-process `execve`. Instead, the running daemon (P1) detects an update. It spawns the new daemon (P2) and passes its listening Unix domain sockets via **systemd-style socket activation** (`SCM_RIGHTS` fd passing). Before P1 yields the sockets to P2, it orchestrates a coordinated drain:
1. P1 stops accepting *new* LLM turns, but allows in-flight turns to finish.
2. P1 pushes a final synchronization event to the durable W1/W2 log.
3. P1 dup2's the accepted client sockets (TUI/ACP) to P2.
4. P1 exits; P2 starts, replays W1/W2, and inherits the live client sockets.

**What survives:** Client connections (no TUI disconnects), W1/W2 swarm state, config/auth updates.
**Hard Parts:** We still lose mid-turn LLM streams if they don't finish before a timeout. Child OS processes (MCP servers/tools) spawned by P1 become orphans. P2 cannot easily re-attach to P1's `SIGCHLD` signals.
**Effort/Risk:** Medium Effort / Low Risk.

### Architecture 2: The "Supervisor & Ghosts" Architecture (Invented)
**Mechanism:** Split the monolithic daemon into a tiny, *never-restarting* Supervisor (Sup) and a hot-swappable Engine. The Sup holds no jcode logic—only process logic. 
- **LLM Streams:** We introduce a thin HTTP/SSE proxy into the Sup. The Engine sends LLM requests *through* the Sup. On reload, Sup keeps the sockets alive while the Engine swaps.
- **Child Processes:** Sup tracks all spawned OS processes. 
- **Reload:** Sup `execve`s the new Engine. The new Engine hits an RPC endpoint on Sup saying "Hand me the live SSE streams and the PIDs of my swarm subagents."

**What survives:** Literally everything, including mid-turn LLM token streams and running tool subprocesses.
**Hard Parts:** High architectural fragmentation. The Engine must treat its own child processes as remote RPC nodes managed by Sup. If the Engine crashes, Sup must decide whether to kill the orphaned tooling.
**Effort/Risk:** High Effort / Medium Risk. 

### Architecture 3: Cooperative Checkpoint & Resume (Refined Seed 3)
**Mechanism:** We treat a daemon reload exactly like a laptop sleep/wake. We write a migration ABI for MCP servers/tools. When a reload is triggered:
1. The daemon broadcasts a `SIGHUP` or RPC `CHECKPOINT` to all child OS processes (swarm subagents, MCP servers).
2. Child processes flush their state to a well-known shared memory or temp file, and exit cleanly (or pause).
3. In-flight LLM streams are abandoned at the HTTP layer. 
4. The daemon records *exactly* where in the turn it was (e.g., the exact Anthropic API request payload, minus the transient stream).
5. `execve()` happens. The new daemon resumes by re-sending the LLM API request (relying on provider idempotency or prompt-prefix caching to avoid double-billing) and restarting the subagents from their checkpointed state.

**What survives:** Swarm task graph, subagent internal state, token cost (via prompt caching resumption).
**Hard Parts:** Requires rewriting MCP/Tool contracts to support checkpointing. LLM streams are still "resumed" rather than "teleported"—meaning you might get a slight latency hit or a duplicated system prompt.
**Effort/Risk:** High Effort / High Risk.

---

### Recommendation for `jcode`

**The Reality Check:** Moving a live HTTP/SSE stream across a process boundary without a proxy is functionally impossible. Moving a process tree you don't own (arbitrary CLI tools) is a nightmare. **You must drain the LLM streams.** For child processes, **you must rely on W1/W2 or checkpointing.**

I recommend a staged approach, prioritizing a high-value, low-risk win first.

#### Stage 1: "Soft Drain & FD Passing" (Immediate, High Value)
Implement **Architecture 1**, dropping the reliance on `execve`. 
*   **Mechanism:** On reload, the daemon stops starting *new* LLM turns. It sets a 10-second timer. If active turns finish, great. If the timer expires, it aborts the SSE streams, logs the API request to W1/W2, and shuts down. 
*   **Client Connections:** Pass the TUI/ACP Unix domain sockets to the new process via `SCM_RIGHTS` (standard fd passing, like how nginx gracefully reloads). Clients never disconnect.
*   **Child Processes:** Send `SIGTERM` to MCP servers and let the new daemon respawn them. Rely on the existing W1/W2 log for swarm state.
*   **Why:** This fixes the immediate pain point (TUI disconnects, lost swarm state) with about a weekend's worth of work, leveraging the W1/W2 system you already built.

#### Stage 2: "Stateful Detach" (Future, Full Solution)
For selfdev dogfooding where mid-turn streams absolutely cannot die, look at **how `rust-analyzer` handles restarts** and how **`tmux` persists state**. 
*   Decouple your LLM client actor from the daemon binary. 
*   Move the LLM HTTP/SSE logic into an external, pooled process (like a local PgBouncer for LLMs). 
*   The daemon dispatches a turn to this pool. If the daemon reloads, the pool holds the HTTP connection, buffers the incoming SSE tokens to disk/shared memory, and streams them back to the daemon once it finishes `execve`. 
*   This gives you zero-loss LLM reloading without writing a kernel-level CRIU implementation.