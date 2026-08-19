use super::*;

pub(super) fn parameters_schema() -> Value {
    let mut schema = json!({
        "type": "object",
        "required": ["action"],
        "properties": {
            "intent": super::super::intent_schema_property(),
            "action": {
                "type": "string",
                "enum": ["message", "broadcast", "dm", "list",
                         "propose_plan", "approve_plan", "reject_plan", "spawn", "stop", "assign_role",
                         "status", "report", "plan_status", "summary", "read_context", "resync_plan", "assign_task", "assign_next", "fill_slots", "run_plan", "cleanup",
                         "task_graph", "expand_node", "complete_node", "inject_gap",
                         "start", "start_task", "wake", "resume", "retry", "reassign", "replace", "salvage", "freeze", "unfreeze",
                         "await_members", "list_models", "list_swarms"],
                "description": "Action. Spawn requires a nonblank label and should include prompt with the initial task so the new agent starts useful work immediately. Use list_models to see which models/routes are available for per-spawn model selection. Use list_swarms for the live fleet dashboard snapshot."
            },
            "message": {
                "type": "string",
                "description": "Message body. For action=message, routes by fields provided: with to_session it is a DM, with neither it broadcasts to your spawned subtree. For action=report, this is the completion report body."
            },
            "tldr": {
                "type": "string",
                "description": "One-line summary (aim for under 120 chars) of the message/report. Required for message/broadcast/dm/report when the body is longer than 240 chars. The recipient's UI shows this collapsed with an expand control instead of the full body."
            },
            "status": {
                "type": "string",
                "description": "For action=report: completion status to record, usually ready, blocked, failed, or completed. Defaults to ready."
            },
            "validation": {
                "type": "string",
                "description": "For action=report: tests or validation performed."
            },
            "follow_up": {
                "type": "string",
                "description": "For action=report: blockers or follow-up work."
            },
            "to_session": {
                "type": "string",
                "description": "Target session for actions that address one agent (dm, and as an alias for target_session). Accepts an exact session ID or a unique friendly name within the swarm. Interchangeable with target_session. If a friendly name is ambiguous, run swarm list and use the exact session ID."
            },
            "proposer_session": { "type": "string" },
            "reason": { "type": "string" },
            "target_session": {
                "type": "string",
                "description": "Target session for management actions (assign_role, summary, status, stop, start, resume, wake, etc.). Accepts an exact session ID or a unique friendly name. Interchangeable with to_session."
            },
            "role": {
                "type": "string",
                "enum": ["agent", "coordinator"]
            },
            "label": {
                "type": "string",
                "minLength": 1,
                "description": "Required for spawn. Short nonblank label shown on the spawned agent's chip in swarm UI (e.g. 'api reviewer')."
            },
            "subagent_type": {
                "type": "string",
                "description": "Optional free-form subagent type/role for spawn, chosen per-call to fit the work (e.g. 'explore', 'implement', 'verify', 'synthesize', 'reviewer', 'debugger'). Surfaces in swarm UI for observability and injects a light role-posture nudge into the worker's first turn. Well-known values (explore/implement/verify/synthesize) get an extra tuned hint; any other string is accepted and still shown."
            },
            "working_dir": {
                "type": "string",
                "description": "Optional working directory for spawn."
            },
            "prompt": {
                "type": "string",
                "description": "Preferred for spawn. Initial task/instructions for the new agent. Spawning without prompt usually creates an idle agent that needs follow-up assignment."
            },
            "initial_message": {
                "type": "string",
                "description": "Explicit initial task/instructions for spawn. If both initial_message and prompt are supplied, initial_message wins."
            },
            "limit": {
                "type": "integer",
                "minimum": 1,
                "description": "Optional max items for summary-style reads."
            },
            "task_id": {
                "type": "string",
                "description": "Optional plan task ID. If omitted for assign_task/assign_next, the coordinator picks a runnable task. If omitted for resume/wake/retry/start with target_session, the server resumes the unique assigned task for that session."
            },
            "spawn_if_needed": {
                "type": "boolean",
                "description": "For assign_task without an explicit target_session: if no reusable agent is available, spawn a fresh agent and retry the assignment automatically."
            },
            "prefer_spawn": {
                "type": "boolean",
                "description": "For assign_task without an explicit target_session: prefer a fresh spawned agent even if reusable workers are available."
            },
            "spawn_mode": {
                "type": "string",
                "enum": ["visible", "headless", "inline", "auto"],
                "description": "Per-call spawn mode for swarm-created agents. Overrides agents.swarm_spawn_mode config when set. 'visible' opens a terminal window, 'headless' runs in-process with no UI, 'inline' runs in-process and renders a live gallery viewport in the coordinator, 'auto' tries visible then falls back to headless. Defaults to inline."
            },
            "model": {
                "type": "string",
                "description": "Optional model for the spawned agent (spawn, and spawns triggered by assign_task/assign_next/run_plan). Overrides the agents.swarm_model config pin for this call. Accepts a bare model name (e.g. 'gpt-5.5') or an auth-route-prefixed form (e.g. 'openai-api:gpt-5.5', 'claude-api:claude-fable-5'). Use 'inherit' to force coordinator inheritance. Omit to use the configured/coordinator default. Run action=list_models to see available models and routes."
            },
            "effort": {
                "type": "string",
                "enum": ["none", "low", "medium", "high", "xhigh", "max"],
                "description": "Optional reasoning effort for the spawned agent. Omit for the model's default. Only meaningful with spawn-creating actions."
            },
            "session_ids": {
                "type": "array",
                "items": {"type": "string"}
            },
            "mode": {
                "type": "string",
                "enum": ["all", "any", "deep", "light"],
                "description": "For await_members, use all or any. For task_graph, use deep for recursive gates and typed artifacts or light for flat fan-out."
            },
            "target_status": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Optional completion statuses for await_members. Defaults to ready/completed/stopped/failed."
            },
            "timeout_minutes": {
                "type": "integer",
                "minimum": 1,
                "description": "Optional timeout for await_members."
            },
            "background": {
                "type": "boolean",
                "description": "For run_plan: run as a detached background task (default true); set false to block until the plan resolves. await_members is always asynchronous and ignores false so the agent stays responsive; its result is delivered later via notify/wake."
            },
            "notify": {
                "type": "boolean",
                "description": "For await_members/run_plan: surface a notification card when the background task resolves. Defaults to true."
            },
            "concurrency_limit": {
                "type": "integer",
                "minimum": 1,
                "description": "Max swarm worker agents active at once. For fill_slots this is required. For run_plan it is optional and overrides the mode-based default (deep fans out wide up to agents.swarm_max_concurrent_agents; light uses a small default). Total agents over the whole run is still bounded only by the swarm member cap."
            },
            "force": {
                "type": "boolean",
                "description": "For stop/cleanup: allow stopping non-owned/user-created swarm sessions. Defaults to false."
            },
            "retain_agents": {
                "type": "boolean",
                "description": "For run_plan: keep spawned workers after the plan reaches a terminal state. Defaults to false, so owned workers are cleaned up."
            },
            "wake": {
                "type": "boolean",
                "description": "Optional wake hint for messages. For await_members/run_plan: wake this agent with the result when the background task resolves (default true); if false, only notify."
            },
            "delivery": {
                "type": "string",
                "enum": ["notify", "interrupt", "wake"],
                "description": "Optional delivery mode for dm messaging."
            },
            "plan_items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": true
                }
            }
        }
    });

    // Task-DAG properties are added after the macro to keep `json!` nesting
    // depth under the macro recursion limit.
    if let Some(props) = schema
        .get_mut("properties")
        .and_then(|value| value.as_object_mut())
    {
        props.insert(
            "node_id".to_string(),
            json!({
                "type": "string",
                "description": "Task-DAG node id for expand_node/complete_node."
            }),
        );
        props.insert(
                "gate_id".to_string(),
                json!({
                    "type": "string",
                    "description": "Gate node id for inject_gap (a critique/verify gate the caller owns)."
                }),
            );
        props.insert(
                "nodes".to_string(),
                json!({
                    "type": "array",
                    "description": "Task-DAG node specs for task_graph (seed), expand_node (children), or inject_gap (gap/fix nodes). Each: {id, content, kind?, depends_on?, priority?}. kind is one of explore|implement|verify|fix|synthesize.",
                    "items": { "type": "object", "additionalProperties": true }
                }),
            );
        props.insert(
                "replace_existing".to_string(),
                json!({
                    "type": "boolean",
                    "description": "For task_graph only. Defaults to true so every task_graph invocation starts a fresh workflow and atomically clears old nodes, metadata, and progress while preserving live swarm participants. Replacement is rejected while any node is assigned or running. Set false only to make the server reject a non-empty graph instead of replacing it; use expand_node/inject_gap to extend an existing graph."
                }),
            );
        props.insert(
                "artifact".to_string(),
                json!({
                    "type": "object",
                    "description": "Typed handoff artifact for complete_node. In deep mode requires non-empty 'findings', a 'what_i_did_not_check' list, and a 'confidence' of low|medium|high (report low honestly; it routes follow-up work). Deep gates cannot pass while a low-confidence sibling is unaddressed: inject_gap or name the id in findings. Fields: findings, evidence[], edge_cases_considered[], validation, open_questions[], confidence, what_i_did_not_check[].",
                    "additionalProperties": true
                }),
            );
    }

    // `swarm` is a multi-action tool, so putting `label` in the top-level
    // `required` array would incorrectly require it for read/list/message and
    // every other action. Use mutually exclusive action branches instead:
    // the spawn branch requires label, while the non-spawn branch does not.
    // `anyOf` object branches are supported by our provider schema adapters
    // and avoid the less-portable JSON Schema `if`/`then` keywords.
    let non_spawn_actions: Vec<Value> = schema["properties"]["action"]["enum"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|action| action.as_str() != Some("spawn"))
        .cloned()
        .collect();
    schema["anyOf"] = json!([
        {
            "type": "object",
            "required": ["action", "label"],
            "properties": {
                "action": { "type": "string", "enum": ["spawn"] }
            }
        },
        {
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": { "type": "string", "enum": non_spawn_actions }
            }
        }
    ]);

    schema
}
