pub mod control_log;

use jcode_plan::PlanItem;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::path::PathBuf;

pub const SWARM_COMPLETION_REPORT_MARKER: &str = "SWARM COMPLETION REPORT REQUIRED";

/// Message/report bodies longer than this require a sender-provided `tldr`
/// so receiving UIs can render them collapsed to one line with an expand
/// control instead of dumping the full body into the transcript.
pub const SWARM_TLDR_REQUIRED_OVER_CHARS: usize = 240;

/// Upper bound for a sender-provided `tldr`. Anything longer defeats the
/// purpose of a one-line collapsed summary.
pub const MAX_SWARM_TLDR_CHARS: usize = 200;

/// Validate a sender-provided `tldr` against the message body it summarizes.
///
/// Returns the normalized (trimmed, whitespace-collapsed) tldr when present,
/// `Ok(None)` when the body is short enough to not need one, and a
/// human/model-actionable error when a long body is missing a tldr or the
/// tldr itself is malformed (too long or multi-line).
pub fn validate_swarm_tldr(
    tldr: Option<&str>,
    body: &str,
    context: &str,
) -> Result<Option<String>, String> {
    let normalized = tldr
        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|t| !t.is_empty());

    if let Some(ref tldr) = normalized {
        let chars = tldr.chars().count();
        if chars > MAX_SWARM_TLDR_CHARS {
            return Err(format!(
                "'tldr' for {context} is too long ({chars} chars, max {MAX_SWARM_TLDR_CHARS}). \
                 Provide a single short line summarizing the message."
            ));
        }
        return Ok(normalized);
    }

    let body_chars = body.chars().count();
    if body_chars > SWARM_TLDR_REQUIRED_OVER_CHARS {
        return Err(format!(
            "'tldr' is required for {context} because the body is {body_chars} chars \
             (over {SWARM_TLDR_REQUIRED_OVER_CHARS}). Add a one-line 'tldr' (under \
             {MAX_SWARM_TLDR_CHARS} chars) summarizing it; recipients see the tldr \
             collapsed with an expand control."
        ));
    }

    Ok(None)
}

/// Maximum number of live members (agents) in a single swarm. This is the sole
/// runaway-prevention cap for the task-graph model. There is intentionally no
/// spawn-depth limit and no per-node fan-out limit: the spawn tree may nest and
/// fan out freely until the swarm reaches this many live members, at which point
/// further spawns are refused.
pub const MAX_SWARM_MEMBERS: usize = 1000;

/// Upper bound for a member's derived task label, sized for one-line UI chips.
pub const MAX_SWARM_TASK_LABEL_CHARS: usize = 48;

/// Upper bound for a spawned agent's `subagent_type` tag. Types are short
/// orchestrator-chosen role words (e.g. "explore", "implement", "verify",
/// "security-review"), so this is deliberately small; longer strings are
/// truncated on a char boundary.
pub const MAX_SWARM_SUBAGENT_TYPE_CHARS: usize = 32;

/// Normalize an orchestrator-supplied `subagent_type` into a short, stable tag.
///
/// The type is free-form on purpose (the coordinator picks whatever role word
/// best fits the work), so this only sanitizes rather than validates against a
/// fixed set: it trims, lowercases, collapses internal whitespace to single
/// dashes, drops anything that is not alphanumeric/`-`/`_`, and truncates on a
/// char boundary. Returns `None` for empty/garbage input so callers can treat
/// "no usable type" uniformly.
pub fn normalize_subagent_type(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in text.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            prev_dash = false;
            ch.to_ascii_lowercase()
        } else if ch == '_' {
            prev_dash = false;
            '_'
        } else if ch.is_whitespace() || ch == '-' || ch == '/' {
            // Collapse runs of separators into a single dash.
            if prev_dash || out.is_empty() {
                continue;
            }
            prev_dash = true;
            '-'
        } else {
            // Drop other punctuation entirely.
            continue;
        };
        out.push(mapped);
        if out.chars().count() >= MAX_SWARM_SUBAGENT_TYPE_CHARS {
            break;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Derive a short, stable task label from a spawn prompt or task assignment.
///
/// Takes the first non-empty line, strips common markdown/list prefixes,
/// collapses whitespace, and truncates on a char boundary with an ellipsis.
/// Returns `None` when the text has no usable content.
pub fn derive_swarm_task_label(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let line = line
        .trim_start_matches(['#', '-', '*', '>', ' '])
        .trim_end_matches(':')
        .trim();
    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    if collapsed.chars().count() <= MAX_SWARM_TASK_LABEL_CHARS {
        return Some(collapsed);
    }
    let truncated: String = collapsed
        .chars()
        .take(MAX_SWARM_TASK_LABEL_CHARS.saturating_sub(1))
        .collect();
    Some(format!("{}…", truncated.trim_end()))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SwarmRole {
    Agent,
    Coordinator,
    Other(String),
}

impl SwarmRole {
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            Self::Agent => Cow::Borrowed("agent"),
            Self::Coordinator => Cow::Borrowed("coordinator"),
            Self::Other(value) => Cow::Borrowed(value.as_str()),
        }
    }
}

impl From<String> for SwarmRole {
    fn from(value: String) -> Self {
        match value.as_str() {
            "agent" => Self::Agent,
            "coordinator" => Self::Coordinator,
            _ => Self::Other(value),
        }
    }
}

impl Serialize for SwarmRole {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str().as_ref())
    }
}

impl<'de> Deserialize<'de> for SwarmRole {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from(String::deserialize(deserializer)?))
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MemberLifecycleState {
    #[default]
    Starting,
    Ready,
    Assigned,
    Running,
    Succeeded,
    Failed,
    Stopped,
    Lost,
}

impl MemberLifecycleState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Assigned => "assigned",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Lost => "lost",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Stopped | Self::Lost
        )
    }

    /// Parse both the canonical vocabulary and the temporary compatibility
    /// inputs accepted while older clients and control logs are migrated.
    pub fn from_compatibility_status(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "starting" | "spawned" => Self::Starting,
            "ready" => Self::Ready,
            "assigned" | "queued" => Self::Assigned,
            "running" | "running_stale" | "streaming" | "thinking" | "blocked"
            | "waiting_network" => Self::Running,
            "succeeded" | "completed" | "done" => Self::Succeeded,
            "failed" | "error" => Self::Failed,
            "stopped" | "closed" | "cancelled" => Self::Stopped,
            "lost" | "crashed" | "disconnected" | "unknown" => Self::Lost,
            _ => Self::Starting,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
pub struct SwarmLifecycleStatus {
    pub state: MemberLifecycleState,
    #[serde(default)]
    pub assignment_epoch: u64,
    #[serde(default)]
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub updated_at_unix_ms: u64,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StoredSwarmLifecycleStatus {
    Snapshot {
        state: MemberLifecycleState,
        #[serde(default)]
        assignment_epoch: u64,
        #[serde(default)]
        revision: u64,
        #[serde(default)]
        reason: Option<String>,
        #[serde(default)]
        updated_at_unix_ms: u64,
    },
    Legacy(String),
}

impl<'de> Deserialize<'de> for SwarmLifecycleStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match StoredSwarmLifecycleStatus::deserialize(deserializer)? {
            StoredSwarmLifecycleStatus::Snapshot {
                state,
                assignment_epoch,
                revision,
                reason,
                updated_at_unix_ms,
            } => Self {
                state,
                assignment_epoch,
                revision,
                reason,
                updated_at_unix_ms,
            },
            StoredSwarmLifecycleStatus::Legacy(status) => Self::from(status),
        })
    }
}

impl Default for SwarmLifecycleStatus {
    fn default() -> Self {
        Self::starting(0)
    }
}

#[allow(non_upper_case_globals)]
impl SwarmLifecycleStatus {
    pub const Spawned: Self = Self::constant(MemberLifecycleState::Starting);
    pub const Ready: Self = Self::constant(MemberLifecycleState::Ready);
    pub const Running: Self = Self::constant(MemberLifecycleState::Running);
    pub const RunningStale: Self = Self::constant(MemberLifecycleState::Running);
    pub const Completed: Self = Self::constant(MemberLifecycleState::Succeeded);
    pub const Done: Self = Self::constant(MemberLifecycleState::Succeeded);
    pub const Failed: Self = Self::constant(MemberLifecycleState::Failed);
    pub const Stopped: Self = Self::constant(MemberLifecycleState::Stopped);
    pub const Crashed: Self = Self::constant(MemberLifecycleState::Lost);
    pub const Queued: Self = Self::constant(MemberLifecycleState::Assigned);
    pub const Blocked: Self = Self::constant(MemberLifecycleState::Running);
    pub const Pending: Self = Self::constant(MemberLifecycleState::Starting);
    pub const Todo: Self = Self::constant(MemberLifecycleState::Starting);

    const fn constant(state: MemberLifecycleState) -> Self {
        Self {
            state,
            assignment_epoch: 0,
            revision: 0,
            reason: None,
            updated_at_unix_ms: 0,
        }
    }

    pub const fn starting(updated_at_unix_ms: u64) -> Self {
        Self {
            state: MemberLifecycleState::Starting,
            assignment_epoch: 0,
            revision: 0,
            reason: None,
            updated_at_unix_ms,
        }
    }

    pub fn as_str(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.state.as_str())
    }

    pub const fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    pub const fn next_assignment_epoch(&self) -> u64 {
        self.assignment_epoch.saturating_add(1)
    }

    pub fn reduce(&mut self, event: MemberLifecycleEvent, updated_at_unix_ms: u64) -> bool {
        let previous = self.clone();
        match event {
            MemberLifecycleEvent::SpawnRequested => {
                if self.revision == 0 {
                    self.state = MemberLifecycleState::Starting;
                    self.reason = None;
                }
            }
            MemberLifecycleEvent::WorkerReady => {
                // Idle declarations arrive both from workers that just came
                // up (Starting) and from workers whose turn ended without a
                // structured report (Assigned/Running via the compatibility
                // bridge). Terminal states stay terminal.
                if matches!(
                    self.state,
                    MemberLifecycleState::Starting
                        | MemberLifecycleState::Ready
                        | MemberLifecycleState::Assigned
                        | MemberLifecycleState::Running
                ) {
                    self.state = MemberLifecycleState::Ready;
                    self.reason = None;
                }
            }
            MemberLifecycleEvent::AssignmentCreated { epoch } => {
                if epoch > self.assignment_epoch {
                    self.assignment_epoch = epoch;
                    self.state = MemberLifecycleState::Assigned;
                    self.reason = None;
                }
            }
            MemberLifecycleEvent::TurnStarted { epoch } => {
                if epoch == self.assignment_epoch
                    && matches!(
                        self.state,
                        MemberLifecycleState::Assigned | MemberLifecycleState::Running
                    )
                {
                    self.state = MemberLifecycleState::Running;
                    self.reason = None;
                }
            }
            MemberLifecycleEvent::TurnSucceeded { epoch } => {
                if epoch == self.assignment_epoch && !self.state.is_terminal() {
                    self.state = MemberLifecycleState::Succeeded;
                    self.reason = None;
                }
            }
            MemberLifecycleEvent::TurnFailed { epoch, reason } => {
                if epoch == self.assignment_epoch && !self.state.is_terminal() {
                    self.state = MemberLifecycleState::Failed;
                    self.reason = reason;
                }
            }
            MemberLifecycleEvent::StopConfirmed { epoch, reason } => {
                if epoch == self.assignment_epoch && !self.state.is_terminal() {
                    self.state = MemberLifecycleState::Stopped;
                    self.reason = reason;
                }
            }
            MemberLifecycleEvent::ProcessLost { reason } => {
                if !self.state.is_terminal() {
                    self.state = MemberLifecycleState::Lost;
                    self.reason = reason;
                }
            }
        }

        if *self == previous {
            return false;
        }
        self.revision = previous.revision.saturating_add(1);
        self.updated_at_unix_ms = updated_at_unix_ms;
        true
    }
}

impl From<String> for SwarmLifecycleStatus {
    fn from(value: String) -> Self {
        Self {
            state: MemberLifecycleState::from_compatibility_status(&value),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberLifecycleEvent {
    SpawnRequested,
    WorkerReady,
    AssignmentCreated { epoch: u64 },
    TurnStarted { epoch: u64 },
    TurnSucceeded { epoch: u64 },
    TurnFailed { epoch: u64, reason: Option<String> },
    StopConfirmed { epoch: u64, reason: Option<String> },
    ProcessLost { reason: Option<String> },
}

/// Durable, persistable portion of a swarm member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmMemberRecord {
    pub session_id: String,
    pub working_dir: Option<PathBuf>,
    pub swarm_id: Option<String>,
    pub swarm_enabled: bool,
    pub status: SwarmLifecycleStatus,
    pub detail: Option<String>,
    /// Stable label of the task/role this member was spawned or assigned for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_label: Option<String>,
    /// Free-form subagent type chosen at spawn (observability + spawn nudge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    pub friendly_name: Option<String>,
    pub report_back_to_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_prompt_delivered: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_completion_report: Option<String>,
    pub role: SwarmRole,
    pub is_headless: bool,
}

pub fn append_swarm_completion_report_instructions(message: &str) -> String {
    if message.contains(SWARM_COMPLETION_REPORT_MARKER) {
        return message.to_string();
    }

    let mut out = message.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str("<system-reminder>\n");
    out.push_str(SWARM_COMPLETION_REPORT_MARKER);
    out.push_str(
        "\nBefore finishing, call the swarm tool with action=\"report\" to submit your completion report. \
Include a concise message, validation/tests performed, and blockers or follow-ups. \
After the report tool succeeds, also write a brief final assistant response. \
Do not finish with only tool output, a lifecycle status change, or no final response. \
Do not send a separate DM for the final report unless you need interactive coordination before finishing.\n",
    );
    out.push_str("</system-reminder>");
    out
}

/// Idempotency marker for [`append_deep_node_instructions`].
pub const SWARM_DEEP_NODE_MARKER: &str = "DEEP TASK GRAPH NODE";

/// Append the deep-mode execution contract to a task-graph node assignment.
///
/// Deep mode's comprehensiveness is structural: it only materializes when every
/// worker knows it can decompose its node into parallel children and must close
/// its node with a typed artifact. A freshly spawned worker has none of that
/// context (the seeding session's `swarm-deep` directive is not inherited), so
/// without this the budget goes unused: workers grind through nodes serially
/// and auto-complete without artifacts, silently downgrading deep mode to
/// light. This directive travels with the assignment itself, so it reaches
/// every worker at any spawn depth. Idempotent via [`SWARM_DEEP_NODE_MARKER`].
pub fn append_deep_node_instructions(message: &str, node_id: &str) -> String {
    if message.contains(SWARM_DEEP_NODE_MARKER) {
        return message.to_string();
    }

    let mut out = message.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str("<system-reminder>\n");
    out.push_str(SWARM_DEEP_NODE_MARKER);
    out.push_str(&format!(
        "\nYou are executing node '{node_id}' of a deep task graph with a large parallel agent \
	budget (up to {MAX_SWARM_MEMBERS} live agents per swarm). That budget is a ceiling, not a \
	target: use parallelism when it shortens independent work, not to multiply coordination. \
	First honor any explicit `EXECUTION SHAPE: ATOMIC` or `EXECUTION SHAPE: COMPOSITE` contract \
	in the assignment. Choose one of exactly two finishes for this node:\n\
	1. Decompose for parallelism: when the node is declared COMPOSITE, or when execution reveals \
	two or more independently checkable outputs with disjoint scopes, call the swarm tool with \
	action=\"expand_node\", node_id=\"{node_id}\", and the smallest sufficient child set \
	(normally two to six). Give each child a distinct output and ownership boundary. Add \
	depends_on edges only for real data dependencies so the ready set stays wide. Do not split a \
	cohesive leaf merely to keep worker slots busy. Then finish your turn; the children fan out \
	and you will be re-woken to synthesize their results.\n\
	2. Execute atomically: when the node is declared ATOMIC or is one cohesive task, do the work \
	and stop once its acceptance is proved or its falsification condition triggers. Then call the \
	swarm tool with action=\"complete_node\", node_id=\"{node_id}\", and a typed artifact: \
	findings, evidence (file:line refs), validation, open_questions, a REQUIRED confidence (low, \
	medium, or high; report low honestly, it routes follow-up work to shore up your scope instead \
	of counting against you), and an honest what_i_did_not_check (the critique gate turns \
	material omissions into new nodes).\n\
	These are the ONLY two ways this node can close: a turn that ends without expand_node or \
	complete_node gets the node re-queued to a fresh agent, and a repeat fails it.\n"
    ));
    out.push_str("</system-reminder>");
    out
}

/// Idempotency marker for [`append_subagent_type_instructions`].
pub const SWARM_SUBAGENT_TYPE_MARKER: &str = "SWARM SUBAGENT TYPE";

/// Append a light, type-appropriate behavioral nudge to a spawned worker's
/// assignment.
///
/// The `subagent_type` is a free-form role word the orchestrator picks per
/// spawn to fit the work (e.g. "explore", "implement", "verify", "review",
/// "security-audit"). This is deliberately NOT a static persona table: the tag
/// is echoed back to the worker with a short reminder to adopt the working
/// style that role implies, plus a couple of well-known anchors so common types
/// bias toward the right behavior without constraining novel ones. The heavy
/// contracts (completion report, deep-node protocol) are layered separately, so
/// this stays a nudge, not a second rulebook.
///
/// Idempotent via [`SWARM_SUBAGENT_TYPE_MARKER`]; a `None`/blank type is a
/// no-op. The caller should pass the normalized type from
/// [`normalize_subagent_type`].
pub fn append_subagent_type_instructions(message: &str, subagent_type: &str) -> String {
    let Some(kind) = normalize_subagent_type(subagent_type) else {
        return message.to_string();
    };
    if message.contains(SWARM_SUBAGENT_TYPE_MARKER) {
        return message.to_string();
    }

    let mut out = message.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str("<system-reminder>\n");
    out.push_str(SWARM_SUBAGENT_TYPE_MARKER);
    out.push_str(&format!(
        ": {kind}\nYou were spawned as a '{kind}' agent. Adopt the working style that role \
implies for this task and let it shape where you spend effort:\n\
- explore / research / investigate: cast wide, gather evidence and file:line \
references, and report findings and open questions rather than editing.\n\
- implement / build / fix: make the change end to end, keep it idiomatic and \
minimal, and validate it (build/tests) before reporting done.\n\
- verify / review / test / audit: be adversarial and read-mostly; hunt for \
what is wrong, missing, or unchecked, and cite concrete evidence for each call.\n\
- synthesize / summarize: integrate prior results into one coherent answer \
without redoing the work.\n\
If your type is not listed, infer the analogous posture from its name. This is \
a lightweight nudge, not a restriction: use whatever tools the task needs.\n"
    ));
    out.push_str("</system-reminder>");
    out
}

/// Append the deep-mode gate contract to a critique/verify gate assignment.
///
/// Gates are the adversarial half of deep mode: they exist to spend budget on
/// gaps. A gate that just rubber-stamps its parent wastes the swarm's capacity,
/// so the directive names the two legal finishes (`inject_gap` with new nodes,
/// or `complete_node` when genuinely clean) and reminds the gate to mine the
/// children's `what_i_did_not_check` lists. `audited_ids` is the gate's audit
/// scope: the server rejects a pass whose artifact does not account for each of
/// these ids by name (enumerated accounting is what separates an audit from a
/// rubber stamp), so the directive lists them up front. `low_confidence_siblings`
/// are completed scope nodes whose artifacts self-reported low confidence: the
/// strictest debts, named as priority probe targets. Shares the idempotency
/// marker with [`append_deep_node_instructions`] since a single assignment gets
/// exactly one deep directive.
pub fn append_deep_gate_instructions(
    message: &str,
    gate_id: &str,
    audited_ids: &[String],
    low_confidence_siblings: &[String],
) -> String {
    if message.contains(SWARM_DEEP_NODE_MARKER) {
        return message.to_string();
    }

    let mut out = message.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str("<system-reminder>\n");
    out.push_str(SWARM_DEEP_NODE_MARKER);
    out.push_str(&format!(
        "\nYou are executing critique/verify gate '{gate_id}' of a deep task graph. Your job is \
to find gaps, not to pass work through. Read every audited artifact, especially each \
what_i_did_not_check list, and probe them. Finish in one of exactly two ways:\n\
	1. Material gaps or failures found: call the swarm tool with action=\"inject_gap\", \
	gate_id=\"{gate_id}\", and one focused node per independent, evidence-backed gap. Combine \
	symptoms with the same root cause. A gap must affect acceptance, correctness, safety, or the \
	claimed coverage; do not inject speculative nice-to-haves merely to grow the graph. The nodes \
	run in parallel and you re-run afterwards. Injecting justified nodes is SUCCESS for a gate, \
	not failure.\n\
2. Genuinely clean: call the swarm tool with action=\"complete_node\", node_id=\"{gate_id}\", \
and an artifact whose findings account for EVERY node you audited BY ID with what you \
checked and why no gaps remain. The server rejects a pass whose findings/open_questions \
do not name each audited node id.\n"
    ));
    if !audited_ids.is_empty() {
        out.push_str(&format!(
            "AUDIT SCOPE: you are auditing node(s) [{}]. A passing artifact must address each \
of these ids explicitly.\n",
            audited_ids.join(", ")
        ));
    }
    if !low_confidence_siblings.is_empty() {
        out.push_str(&format!(
            "PRIORITY: sibling node(s) [{}] completed with LOW confidence. The server will \
REJECT your pass unless you either inject follow-up nodes that shore up that work, or name \
each of those ids in your artifact findings with why the low confidence is acceptable. \
Injecting follow-ups adds breadth but does not erase the record: when you re-run after they \
drain, your passing artifact must STILL name each low-confidence id (e.g. 'X was shored up \
by Y').\n",
            low_confidence_siblings.join(", ")
        ));
    }
    out.push_str("Do not pass the gate without doing one of these.\n");
    out.push_str("</system-reminder>");
    out
}

pub fn format_structured_completion_report(
    message: &str,
    validation: Option<&str>,
    follow_up: Option<&str>,
) -> String {
    let mut report = message.trim().to_string();
    if let Some(validation) = validation.map(str::trim).filter(|value| !value.is_empty()) {
        if !report.is_empty() {
            report.push_str("\n\n");
        }
        report.push_str("Validation:\n");
        report.push_str(validation);
    }
    if let Some(follow_up) = follow_up.map(str::trim).filter(|value| !value.is_empty()) {
        if !report.is_empty() {
            report.push_str("\n\n");
        }
        report.push_str("Follow-ups/blockers:\n");
        report.push_str(follow_up);
    }
    report
}

pub fn normalize_completion_report(report: Option<String>) -> Option<String> {
    let report = report?.trim().to_string();
    (!report.is_empty()).then_some(report)
}

fn completion_status_intro(name: &str, status: &str) -> String {
    match status {
        "succeeded" => format!("Agent {} finished their work successfully.", name),
        "failed" => format!("Agent {} finished with status failed.", name),
        "stopped" => format!("Agent {} stopped.", name),
        "lost" => format!("Agent {} was lost while working.", name),
        _ => format!("Agent {} completed their work.", name),
    }
}

fn completion_followup(status: &str, has_report: bool) -> &'static str {
    match (status, has_report) {
        ("succeeded", true) => {
            "Use assign_task to give them more work, stop to remove them, or summary/read_context for full context."
        }
        ("succeeded", false) => {
            "Use summary/read_context to inspect results, assign_task for more work, or stop to remove them."
        }
        ("failed", true) => {
            "Use summary/read_context for full context, retry with guidance, or stop to remove them."
        }
        ("failed", false) => {
            "Use summary/read_context to inspect results, assign_task to retry with guidance, or stop to remove them."
        }
        ("stopped", _) => "Use summary/read_context to inspect results or stop to remove them.",
        ("lost", _) => {
            "Any swarm task assignments they held are requeued automatically where possible. \
             Check plan_status, and spawn a replacement or use retry/assign_task if work remains."
        }
        (_, true) => {
            "Use assign_task to give them new work, stop to remove them, or summary/read_context for full context."
        }
        (_, false) => "Use assign_task to give them new work, or stop to remove them.",
    }
}

pub fn completion_notification_message(name: &str, status: &str, report: Option<&str>) -> String {
    let intro = completion_status_intro(name, status);
    let followup = completion_followup(status, report.is_some());
    match report {
        Some(report) => format!("{intro}\n\nReport:\n{report}\n\n{followup}"),
        None => format!("{intro}\n\nNo final textual report was produced. {followup}"),
    }
}

pub fn truncate_detail(text: &str, max_len: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    let max_len = max_len.max(1);
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }
    if max_len <= 3 {
        return trimmed.chars().take(max_len).collect();
    }
    let mut out: String = trimmed.chars().take(max_len - 3).collect();
    out.push_str("...");
    out
}

pub fn summarize_plan_items(items: &[PlanItem], max_items: usize) -> String {
    if items.is_empty() {
        return "no items".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    for item in items.iter().take(max_items.max(1)) {
        parts.push(item.content.clone());
    }
    let mut summary = parts.join("; ");
    if items.len() > max_items.max(1) {
        summary.push_str(&format!(" (+{} more)", items.len() - max_items.max(1)));
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_item(id: &str, content: &str) -> PlanItem {
        PlanItem {
            id: id.to_string(),
            content: content.to_string(),
            status: "queued".to_string(),
            priority: "normal".to_string(),
            subsystem: None,
            file_scope: Vec::new(),
            blocked_by: Vec::new(),
            assigned_to: None,
        }
    }

    #[test]
    fn truncate_detail_collapses_whitespace_and_ellipsizes() {
        assert_eq!(truncate_detail("hello   there\nworld", 11), "hello th...");
    }

    #[test]
    fn validate_swarm_tldr_allows_short_body_without_tldr() {
        assert_eq!(validate_swarm_tldr(None, "quick note", "this DM"), Ok(None));
    }

    #[test]
    fn validate_swarm_tldr_requires_tldr_for_long_body() {
        let body = "x".repeat(SWARM_TLDR_REQUIRED_OVER_CHARS + 1);
        let err = validate_swarm_tldr(None, &body, "this DM").unwrap_err();
        assert!(err.contains("'tldr' is required"), "{err}");
        assert!(err.contains("this DM"), "{err}");
    }

    #[test]
    fn validate_swarm_tldr_normalizes_whitespace() {
        let body = "x".repeat(SWARM_TLDR_REQUIRED_OVER_CHARS + 1);
        assert_eq!(
            validate_swarm_tldr(Some("  did\nthe   thing  "), &body, "this report"),
            Ok(Some("did the thing".to_string()))
        );
    }

    #[test]
    fn validate_swarm_tldr_rejects_overlong_tldr() {
        let tldr = "y".repeat(MAX_SWARM_TLDR_CHARS + 1);
        let err = validate_swarm_tldr(Some(&tldr), "body", "this message").unwrap_err();
        assert!(err.contains("too long"), "{err}");
    }

    #[test]
    fn validate_swarm_tldr_blank_tldr_counts_as_missing() {
        let body = "x".repeat(SWARM_TLDR_REQUIRED_OVER_CHARS + 1);
        assert!(validate_swarm_tldr(Some("   "), &body, "this DM").is_err());
        assert_eq!(
            validate_swarm_tldr(Some("   "), "short", "this DM"),
            Ok(None)
        );
    }

    #[test]
    fn summarize_plan_items_limits_output() {
        let items = vec![
            plan_item("a", "first"),
            plan_item("b", "second"),
            plan_item("c", "third"),
        ];
        assert_eq!(summarize_plan_items(&items, 2), "first; second (+1 more)");
    }

    #[test]
    fn append_swarm_completion_report_instructions_is_idempotent() {
        let prompt = "Do work";
        let with_instructions = append_swarm_completion_report_instructions(prompt);
        assert!(with_instructions.contains(SWARM_COMPLETION_REPORT_MARKER));
        assert_eq!(
            append_swarm_completion_report_instructions(&with_instructions),
            with_instructions
        );
    }

    #[test]
    fn subagent_type_normalizes_free_form_input() {
        assert_eq!(normalize_subagent_type("Explore"), Some("explore".into()));
        assert_eq!(
            normalize_subagent_type("  Security Review  "),
            Some("security-review".into())
        );
        assert_eq!(
            normalize_subagent_type("verify/test"),
            Some("verify-test".into())
        );
        assert_eq!(
            normalize_subagent_type("impl_worker"),
            Some("impl_worker".into())
        );
        // Collapses separator runs, strips leading/trailing dashes and punctuation.
        assert_eq!(
            normalize_subagent_type("--build!! it --"),
            Some("build-it".into())
        );
        // Empty/garbage -> None so callers treat "no type" uniformly.
        assert_eq!(normalize_subagent_type("   "), None);
        assert_eq!(normalize_subagent_type("***"), None);
        // Truncated on a char boundary at the cap.
        let long = "a".repeat(MAX_SWARM_SUBAGENT_TYPE_CHARS + 20);
        assert_eq!(
            normalize_subagent_type(&long).unwrap().chars().count(),
            MAX_SWARM_SUBAGENT_TYPE_CHARS
        );
    }

    #[test]
    fn subagent_type_instructions_nudge_and_are_idempotent() {
        let out = append_subagent_type_instructions("Review the auth module", "Security Review");
        assert!(out.starts_with("Review the auth module"));
        assert!(out.contains(SWARM_SUBAGENT_TYPE_MARKER));
        // The normalized type is echoed back so the worker sees its role.
        assert!(out.contains("security-review"));
        // Idempotent: re-appending (even with a different type) is a no-op.
        assert_eq!(append_subagent_type_instructions(&out, "implement"), out);
        // A blank/garbage type is a no-op passthrough.
        assert_eq!(
            append_subagent_type_instructions("Do work", "  "),
            "Do work"
        );
        assert_eq!(
            append_subagent_type_instructions("Do work", "***"),
            "Do work"
        );
    }

    #[test]
    fn deep_node_instructions_carry_expand_and_artifact_contract() {
        let out = append_deep_node_instructions("Investigate the parser", "explore.parser");
        assert!(out.starts_with("Investigate the parser"));
        assert!(out.contains(SWARM_DEEP_NODE_MARKER));
        // The two legal finishes must both name the node id explicitly.
        assert!(out.contains("action=\"expand_node\", node_id=\"explore.parser\""));
        assert!(out.contains("action=\"complete_node\", node_id=\"explore.parser\""));
        // The budget is advertised so workers know fan-out is expected.
        assert!(out.contains(&MAX_SWARM_MEMBERS.to_string()));
        assert!(out.contains("ceiling, not a target"));
        assert!(out.contains("smallest sufficient child set"));
        assert!(out.contains("EXECUTION SHAPE: ATOMIC"));
        assert!(out.contains("what_i_did_not_check"));
        // Idempotent: re-appending (even with a different id) is a no-op.
        assert_eq!(append_deep_node_instructions(&out, "other"), out);
    }

    #[test]
    fn deep_gate_instructions_carry_inject_gap_contract() {
        let out = append_deep_gate_instructions("Critique the work", "root::gate", &[], &[]);
        assert!(out.contains(SWARM_DEEP_NODE_MARKER));
        assert!(out.contains("action=\"inject_gap\", gate_id=\"root::gate\""));
        assert!(out.contains("action=\"complete_node\", node_id=\"root::gate\""));
        assert!(out.contains("evidence-backed gap"));
        assert!(out.contains("speculative nice-to-haves"));
        assert!(out.contains("what_i_did_not_check"));
        // No audit scope / low-confidence siblings: no callouts.
        assert!(!out.contains("AUDIT SCOPE"));
        assert!(!out.contains("PRIORITY"));
        // Shares the marker with the node directive: one deep directive per assignment.
        assert_eq!(
            append_deep_gate_instructions(&out, "root::gate", &[], &[]),
            out
        );
        assert_eq!(append_deep_node_instructions(&out, "root::gate"), out);
    }

    #[test]
    fn deep_gate_instructions_enumerate_audit_scope() {
        let scope = vec!["root.a".to_string(), "root.b".to_string()];
        let out = append_deep_gate_instructions("Critique the work", "root::gate", &scope, &[]);
        assert!(out.contains("AUDIT SCOPE"));
        assert!(out.contains("root.a, root.b"));
        // The coverage contract is stated: each id must be addressed.
        assert!(out.contains("address each"));
    }

    #[test]
    fn deep_gate_instructions_name_low_confidence_probe_targets() {
        let shaky = vec!["root.shaky".to_string(), "root.wobble".to_string()];
        let out = append_deep_gate_instructions("Critique the work", "root::gate", &shaky, &shaky);
        assert!(out.contains("PRIORITY"));
        assert!(out.contains("root.shaky, root.wobble"));
        assert!(out.contains("LOW confidence"));
        // The enforcement is explained: pass is rejected unless addressed.
        assert!(out.contains("REJECT"));
    }

    #[test]
    fn completion_report_normalization_trims_and_preserves_long_reports() {
        assert_eq!(
            normalize_completion_report(Some("  done  ".to_string())),
            Some("done".to_string())
        );
        assert_eq!(normalize_completion_report(Some("   ".to_string())), None);
        let long = format!("{}\nTAIL: all recommendations arrived.", "Δ".repeat(4_100));
        assert_eq!(normalize_completion_report(Some(long.clone())), Some(long));
    }

    #[test]
    fn task_label_takes_first_line_strips_prefixes_and_collapses_whitespace() {
        assert_eq!(
            derive_swarm_task_label("Fix the   parser\n\nMore detail here"),
            Some("Fix the parser".to_string())
        );
        assert_eq!(
            derive_swarm_task_label("\n\n  ## Investigate flaky test:  \nbody"),
            Some("Investigate flaky test".to_string())
        );
        assert_eq!(
            derive_swarm_task_label("- review PR #42"),
            Some("review PR #42".to_string())
        );
    }

    #[test]
    fn task_label_truncates_long_prompts_with_ellipsis() {
        let long = "implement the entire authentication subsystem including oauth flows";
        let label = derive_swarm_task_label(long).unwrap();
        assert!(label.chars().count() <= MAX_SWARM_TASK_LABEL_CHARS);
        assert!(label.ends_with('…'), "got: {label}");
    }

    #[test]
    fn task_label_rejects_empty_or_marker_only_text() {
        assert_eq!(derive_swarm_task_label(""), None);
        assert_eq!(derive_swarm_task_label("   \n\t\n"), None);
        assert_eq!(derive_swarm_task_label("###"), None);
    }
}
