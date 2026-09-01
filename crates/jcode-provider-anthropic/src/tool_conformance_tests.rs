use super::{ApiTool, format_tools};
use async_trait::async_trait;
use jcode_app_core::message::{Message, ToolDefinition};
use jcode_app_core::provider::{EventStream, Provider};
use jcode_app_core::tool::Registry;
use jcode_provider_core::anthropic_map_tool_name_from_oauth;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, LazyLock};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Drift {
    tool: &'static str,
    kind: DriftKind,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DriftKind {
    MissingRequired(String),
    IgnoredAdvertisedField(String),
    SemanticMismatch(String),
}

static KNOWN_DRIFT: LazyLock<Vec<Drift>> = LazyLock::new(|| {
    sorted_drift(vec![
        semantic(
            "Agent",
            "capability hidden: backing schema property model is not advertised",
        ),
        semantic(
            "Bash",
            "capability hidden: backing schema properties notify and wake are not advertised",
        ),
        semantic("Glob", "Glob deserializes with mode=grep; expected find"),
        ignored("Grep", "-A"),
        ignored("Grep", "-B"),
        ignored("Grep", "-C"),
        ignored("Grep", "-i"),
        ignored("Grep", "-n"),
        ignored("Grep", "context"),
        ignored("Grep", "head_limit"),
        ignored("Grep", "multiline"),
        ignored("Grep", "offset"),
        ignored("Grep", "output_mode"),
        ignored("Read", "pages"),
        ignored("ScheduleWakeup", "delaySeconds"),
        ignored("ScheduleWakeup", "prompt"),
        ignored("ScheduleWakeup", "reason"),
        missing("ScheduleWakeup", "schedule_id"),
        missing("ScheduleWakeup", "task"),
        missing("ScheduleWakeup", "wake_at"),
        missing("ScheduleWakeup", "wake_in_minutes"),
    ])
});

const AGENT_REQUIREMENTS: &[RequiredVariant] = &[RequiredVariant {
    name: "launch",
    groups: &[
        RequiredGroup::All(&["description"]),
        RequiredGroup::All(&["prompt"]),
    ],
}];
const BASH_REQUIREMENTS: &[RequiredVariant] = &[RequiredVariant {
    name: "foreground-or-background",
    groups: &[RequiredGroup::All(&["command"])],
}];
const EDIT_REQUIREMENTS: &[RequiredVariant] = &[RequiredVariant {
    name: "replace",
    groups: &[
        RequiredGroup::All(&["file_path"]),
        RequiredGroup::All(&["old_string"]),
        RequiredGroup::All(&["new_string"]),
    ],
}];
const GLOB_REQUIREMENTS: &[RequiredVariant] = &[RequiredVariant {
    name: "find",
    groups: &[RequiredGroup::Any(&["query", "path", "glob", "type"])],
}];
const GREP_REQUIREMENTS: &[RequiredVariant] = &[RequiredVariant {
    name: "grep",
    groups: &[RequiredGroup::All(&["query"])],
}];
const READ_REQUIREMENTS: &[RequiredVariant] = &[RequiredVariant {
    name: "read",
    groups: &[RequiredGroup::All(&["file_path"])],
}];
const SCHEDULE_REQUIREMENTS: &[RequiredVariant] = &[
    RequiredVariant {
        name: "create-with-relative-wake",
        groups: &[
            RequiredGroup::All(&["task"]),
            RequiredGroup::All(&["wake_in_minutes"]),
        ],
    },
    RequiredVariant {
        name: "create-with-absolute-wake",
        groups: &[
            RequiredGroup::All(&["task"]),
            RequiredGroup::All(&["wake_at"]),
        ],
    },
    RequiredVariant {
        name: "list",
        groups: &[],
    },
    RequiredVariant {
        name: "cancel",
        groups: &[RequiredGroup::All(&["schedule_id"])],
    },
];
const SKILL_REQUIREMENTS: &[RequiredVariant] = &[RequiredVariant {
    name: "load",
    groups: &[RequiredGroup::All(&["name"])],
}];
const WRITE_REQUIREMENTS: &[RequiredVariant] = &[RequiredVariant {
    name: "write",
    groups: &[
        RequiredGroup::All(&["file_path"]),
        RequiredGroup::All(&["content"]),
    ],
}];

#[derive(Clone, Copy)]
enum RequiredGroup {
    All(&'static [&'static str]),
    Any(&'static [&'static str]),
}

#[derive(Clone, Copy)]
struct RequiredVariant {
    #[allow(dead_code)]
    name: &'static str,
    groups: &'static [RequiredGroup],
}

#[derive(Clone, Copy)]
enum MirrorKind {
    Agent,
    Bash,
    Edit,
    AgentGrep,
    Read,
    Schedule,
    Skill,
    Write,
}

struct ToolCase {
    advertised: &'static str,
    backing: &'static str,
    mirror: MirrorKind,
    variants: &'static [RequiredVariant],
    hidden_capability_note: Option<(&'static [&'static str], &'static str)>,
    expected_mode: Option<&'static str>,
}

fn tool_cases() -> [ToolCase; 9] {
    [
        ToolCase {
            advertised: "Agent",
            backing: "subagent",
            mirror: MirrorKind::Agent,
            variants: AGENT_REQUIREMENTS,
            hidden_capability_note: Some((
                &["model"],
                "capability hidden: backing schema property model is not advertised",
            )),
            expected_mode: None,
        },
        ToolCase {
            advertised: "Bash",
            backing: "bash",
            mirror: MirrorKind::Bash,
            variants: BASH_REQUIREMENTS,
            hidden_capability_note: Some((
                &["notify", "wake"],
                "capability hidden: backing schema properties notify and wake are not advertised",
            )),
            expected_mode: None,
        },
        ToolCase {
            advertised: "Edit",
            backing: "edit",
            mirror: MirrorKind::Edit,
            variants: EDIT_REQUIREMENTS,
            hidden_capability_note: None,
            expected_mode: None,
        },
        ToolCase {
            advertised: "Glob",
            backing: "agentgrep",
            mirror: MirrorKind::AgentGrep,
            variants: GLOB_REQUIREMENTS,
            hidden_capability_note: None,
            expected_mode: Some("find"),
        },
        ToolCase {
            advertised: "Grep",
            backing: "agentgrep",
            mirror: MirrorKind::AgentGrep,
            variants: GREP_REQUIREMENTS,
            hidden_capability_note: None,
            expected_mode: Some("grep"),
        },
        ToolCase {
            advertised: "Read",
            backing: "read",
            mirror: MirrorKind::Read,
            variants: READ_REQUIREMENTS,
            hidden_capability_note: None,
            expected_mode: None,
        },
        ToolCase {
            advertised: "ScheduleWakeup",
            backing: "schedule",
            mirror: MirrorKind::Schedule,
            variants: SCHEDULE_REQUIREMENTS,
            hidden_capability_note: None,
            expected_mode: None,
        },
        ToolCase {
            advertised: "Skill",
            backing: "skill_manage",
            mirror: MirrorKind::Skill,
            variants: SKILL_REQUIREMENTS,
            hidden_capability_note: None,
            expected_mode: None,
        },
        ToolCase {
            advertised: "Write",
            backing: "write",
            mirror: MirrorKind::Write,
            variants: WRITE_REQUIREMENTS,
            hidden_capability_note: None,
            expected_mode: None,
        },
    ]
}

struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> anyhow::Result<EventStream> {
        anyhow::bail!("provider-boundary conformance tests never execute the provider")
    }

    fn name(&self) -> &str {
        "conformance-test"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(Self)
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AgentMirror {
    description: String,
    prompt: String,
    #[serde(default)]
    subagent_type: String,
    #[serde(default)]
    run_in_background: bool,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BashMirror {
    command: String,
    #[serde(default)]
    intent: Option<String>,
    #[serde(default)]
    timeout: Option<u64>,
    #[serde(default)]
    run_in_background: Option<bool>,
    #[serde(default)]
    notify: bool,
    #[serde(default)]
    wake: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct EditMirror {
    #[serde(default)]
    intent: Option<String>,
    file_path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AgentGrepMirror {
    #[serde(default = "default_agentgrep_mode")]
    mode: String,
    #[serde(default, alias = "pattern")]
    query: Option<String>,
    #[serde(default, alias = "file_path")]
    file: Option<String>,
    #[serde(default)]
    terms: Option<Vec<String>>,
    #[serde(default)]
    regex: Option<bool>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, alias = "include")]
    glob: Option<String>,
    #[serde(rename = "type", default)]
    file_type: Option<String>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default)]
    no_ignore: Option<bool>,
    #[serde(default)]
    max_files: Option<usize>,
    #[serde(default)]
    max_regions: Option<usize>,
    #[serde(default)]
    full_region: Option<String>,
    #[serde(default)]
    debug_plan: Option<bool>,
    #[serde(default)]
    debug_score: Option<bool>,
    #[serde(default)]
    paths_only: Option<bool>,
}

fn default_agentgrep_mode() -> String {
    "grep".to_string()
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ReadMirror {
    file_path: String,
    #[serde(default)]
    start_line: Option<usize>,
    #[serde(default)]
    end_line: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ScheduleMirror {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    schedule_id: Option<String>,
    #[serde(default)]
    task: Option<String>,
    #[serde(default)]
    wake_in_minutes: Option<u32>,
    #[serde(default)]
    wake_at: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    relevant_files: Vec<String>,
    #[serde(default)]
    background_context: Option<String>,
    #[serde(default)]
    success_criteria: Option<String>,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SkillMirror {
    #[serde(default = "default_skill_action")]
    action: String,
    #[serde(default, alias = "skill")]
    name: Option<String>,
    #[serde(default)]
    args: Option<String>,
}

fn default_skill_action() -> String {
    "load".to_string()
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WriteMirror {
    #[serde(default)]
    intent: Option<String>,
    file_path: String,
    content: String,
}

#[derive(Default)]
struct Coverage {
    ignored: Vec<String>,
    mode: Option<String>,
}

fn deserialize_collecting_ignored<T>(payload: &Value) -> Result<(T, Vec<String>), String>
where
    T: DeserializeOwned,
{
    let encoded = serde_json::to_string(payload).map_err(|error| error.to_string())?;
    let mut deserializer = serde_json::Deserializer::from_str(&encoded);
    let mut ignored = Vec::new();
    let parsed = serde_ignored::deserialize(&mut deserializer, |path| {
        ignored.push(path.to_string().trim_start_matches('.').to_string());
    })
    .map_err(|error| error.to_string())?;
    ignored.sort();
    Ok((parsed, ignored))
}

fn deserialize_with_coverage(kind: MirrorKind, payload: &Value) -> Result<Coverage, String> {
    match kind {
        MirrorKind::Agent => {
            deserialize_collecting_ignored::<AgentMirror>(payload).map(|(_, ignored)| Coverage {
                ignored,
                mode: None,
            })
        }
        MirrorKind::Bash => {
            deserialize_collecting_ignored::<BashMirror>(payload).map(|(_, ignored)| Coverage {
                ignored,
                mode: None,
            })
        }
        MirrorKind::Edit => {
            deserialize_collecting_ignored::<EditMirror>(payload).map(|(_, ignored)| Coverage {
                ignored,
                mode: None,
            })
        }
        MirrorKind::AgentGrep => {
            deserialize_collecting_ignored::<AgentGrepMirror>(payload).map(|(parsed, ignored)| {
                Coverage {
                    ignored,
                    mode: Some(parsed.mode),
                }
            })
        }
        MirrorKind::Read => {
            deserialize_collecting_ignored::<ReadMirror>(payload).map(|(_, ignored)| Coverage {
                ignored,
                mode: None,
            })
        }
        MirrorKind::Schedule => {
            deserialize_collecting_ignored::<ScheduleMirror>(payload).map(|(_, ignored)| Coverage {
                ignored,
                mode: None,
            })
        }
        MirrorKind::Skill => {
            deserialize_collecting_ignored::<SkillMirror>(payload).map(|(_, ignored)| Coverage {
                ignored,
                mode: None,
            })
        }
        MirrorKind::Write => {
            deserialize_collecting_ignored::<WriteMirror>(payload).map(|(_, ignored)| Coverage {
                ignored,
                mode: None,
            })
        }
    }
}

fn maximal_payload(schema: &Value) -> Value {
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .expect("curated tool schema must expose object properties");
    Value::Object(
        properties
            .iter()
            .map(|(name, property)| (name.clone(), schema_exemplar(property)))
            .collect(),
    )
}

fn schema_exemplar(schema: &Value) -> Value {
    if let Some(value) = schema
        .get("enum")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
    {
        return value.clone();
    }
    for keyword in ["oneOf", "anyOf"] {
        if let Some(branch) = schema
            .get(keyword)
            .and_then(Value::as_array)
            .and_then(|branches| {
                branches
                    .iter()
                    .find(|branch| branch.get("type").and_then(Value::as_str) != Some("null"))
            })
        {
            return schema_exemplar(branch);
        }
    }

    let schema_type = match schema.get("type") {
        Some(Value::String(kind)) => Some(kind.as_str()),
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .find(|kind| *kind != "null"),
        _ => None,
    };
    match schema_type {
        Some("string") => Value::String("example".to_string()),
        Some("integer") => Value::Number(integer_exemplar(schema).into()),
        Some("number") => json!(integer_exemplar(schema)),
        Some("boolean") => Value::Bool(true),
        Some("array") => Value::Array(vec![schema_exemplar(
            schema.get("items").unwrap_or(&Value::Null),
        )]),
        Some("object") | None => {
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            Value::Object(
                properties
                    .iter()
                    .map(|(name, property)| (name.clone(), schema_exemplar(property)))
                    .collect(),
            )
        }
        Some("null") => Value::Null,
        Some(other) => panic!("unsupported schema type in curated conformance fixture: {other}"),
    }
}

fn integer_exemplar(schema: &Value) -> i64 {
    if let Some(minimum) = schema.get("exclusiveMinimum").and_then(Value::as_i64) {
        return minimum.saturating_add(1);
    }
    schema.get("minimum").and_then(Value::as_i64).unwrap_or(1)
}

fn advertised_name(case: &ToolCase, internal_name: &str) -> &'static str {
    match (case.backing, internal_name) {
        ("agentgrep", "query") => "pattern",
        ("skill_manage", "name") => "skill",
        _ => match internal_name {
            "description" => "description",
            "prompt" => "prompt",
            "command" => "command",
            "file_path" => "file_path",
            "old_string" => "old_string",
            "new_string" => "new_string",
            "path" => "path",
            "glob" => "glob",
            "type" => "type",
            "task" => "task",
            "wake_in_minutes" => "wake_in_minutes",
            "wake_at" => "wake_at",
            "schedule_id" => "schedule_id",
            "content" => "content",
            other => panic!("missing advertised-name fixture for {other}"),
        },
    }
}

fn required_names(schema: &Value) -> BTreeSet<&str> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

fn property_names(schema: &Value) -> BTreeSet<&str> {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(Map::keys)
        .map(String::as_str)
        .collect()
}

fn observe_required_drift(
    case: &ToolCase,
    advertised_schema: &Value,
    internal_schema: &Value,
    drift: &mut Vec<Drift>,
) {
    let advertised_required = required_names(advertised_schema);
    for required in required_names(internal_schema) {
        let wire_name = advertised_name(case, required);
        if !advertised_required.contains(wire_name) {
            drift.push(missing(case.advertised, required));
        }
    }

    for variant in case.variants {
        for group in variant.groups {
            let (satisfied, label) = match group {
                RequiredGroup::All(fields) => (
                    fields
                        .iter()
                        .all(|field| advertised_required.contains(advertised_name(case, field))),
                    fields.join("+"),
                ),
                RequiredGroup::Any(fields) => (
                    fields
                        .iter()
                        .any(|field| advertised_required.contains(advertised_name(case, field))),
                    fields.join("|"),
                ),
            };
            if !satisfied {
                drift.push(missing(case.advertised, &label));
            }
        }
    }
}

async fn observed_drift() -> Vec<Drift> {
    let registry = Registry::new(Arc::new(MockProvider)).await;
    let definitions = registry.definitions(None).await;
    let internal: BTreeMap<&str, &ToolDefinition> = definitions
        .iter()
        .map(|definition| (definition.name.as_str(), definition))
        .collect();
    let formatted = format_tools(&definitions, true, false);
    let advertised: BTreeMap<&str, &ApiTool> = formatted
        .iter()
        .map(|tool| (tool.name.as_str(), tool))
        .collect();
    let mut drift = Vec::new();

    for case in tool_cases() {
        let mapped = anthropic_map_tool_name_from_oauth(case.advertised);
        let resolved = jcode_tool_types::resolve_tool_name(&mapped);
        assert_eq!(
            resolved, case.backing,
            "{} routed through {mapped} to {resolved}, expected {}",
            case.advertised, case.backing
        );

        let advertised_tool = advertised
            .get(case.advertised)
            .unwrap_or_else(|| panic!("missing curated tool {}", case.advertised));
        let internal_tool = internal
            .get(case.backing)
            .unwrap_or_else(|| panic!("missing backing tool {}", case.backing));
        let payload = maximal_payload(&advertised_tool.input_schema);
        let coverage = deserialize_with_coverage(case.mirror, &payload).unwrap_or_else(|error| {
            panic!(
                "{} maximal advertised payload did not deserialize into {} mirror: {error}; payload={payload}",
                case.advertised, case.backing
            )
        });
        for field in coverage.ignored {
            drift.push(ignored(case.advertised, &field));
        }

        observe_required_drift(
            &case,
            &advertised_tool.input_schema,
            &internal_tool.input_schema,
            &mut drift,
        );

        if let Some(expected_mode) = case.expected_mode
            && coverage.mode.as_deref() != Some(expected_mode)
        {
            drift.push(semantic(
                case.advertised,
                &format!(
                    "{} deserializes with mode={}; expected {expected_mode}",
                    case.advertised,
                    coverage.mode.as_deref().unwrap_or("<none>")
                ),
            ));
        }

        if let Some((hidden, note)) = case.hidden_capability_note {
            let internal_properties = property_names(&internal_tool.input_schema);
            let advertised_properties = property_names(&advertised_tool.input_schema);
            if hidden
                .iter()
                .all(|field| internal_properties.contains(field))
                && hidden
                    .iter()
                    .any(|field| !advertised_properties.contains(field))
            {
                drift.push(semantic(case.advertised, note));
            }
        }
    }

    sorted_drift(drift)
}

fn sorted_drift(mut drift: Vec<Drift>) -> Vec<Drift> {
    drift.sort();
    drift.dedup();
    drift
}

fn missing(tool: &'static str, field: &str) -> Drift {
    Drift {
        tool,
        kind: DriftKind::MissingRequired(field.to_string()),
    }
}

fn ignored(tool: &'static str, field: &str) -> Drift {
    Drift {
        tool,
        kind: DriftKind::IgnoredAdvertisedField(field.to_string()),
    }
}

fn semantic(tool: &'static str, note: &str) -> Drift {
    Drift {
        tool,
        kind: DriftKind::SemanticMismatch(note.to_string()),
    }
}

#[tokio::test]
async fn oauth_curated_tool_drift_matches_known_inventory() {
    assert_eq!(
        observed_drift().await,
        *KNOWN_DRIFT,
        "provider-boundary drift changed; update the implementation and shrink KNOWN_DRIFT, or add a reviewed disposition for newly observed drift"
    );
}

/// Intentionally red until the curated schemas are generated from or adapted to
/// their backing contracts. Run explicitly with:
/// `JCODE_REMOTE_CARGO=0 ./scripts/dev_cargo.sh test -p jcode-provider-anthropic oauth_curated_tools_have_no_known_drift -- --ignored --nocapture`
#[tokio::test]
#[ignore = "known provider-boundary drift is pinned by oauth_curated_tool_drift_matches_known_inventory"]
async fn oauth_curated_tools_have_no_known_drift() {
    assert_eq!(
        observed_drift().await,
        Vec::<Drift>::new(),
        "curated OAuth tools still drift from their backing contracts"
    );
}
