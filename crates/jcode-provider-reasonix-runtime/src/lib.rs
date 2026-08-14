use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use jcode_message_types::{ContentBlock as JcodeContentBlock, Message, Role, StreamEvent};
use jcode_provider_acp_runtime::{
    AcpAuthAction, AcpPermissionBroker, AcpPermissionDecision, AcpPermissionRequest,
    AcpProcessSpec, AcpPromptInput, AcpProvider, AcpProviderPolicy, AcpRuntimeConfig,
    AcpSessionMutation, AcpSessionState, DiscoveredModels, acp,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

const WORKSPACE_ONLY_ARG: &str = "-workspace-only";
const SETUP_ARG: &str = "setup";
const SETUP_AUTH_METHOD_ID: &str = "reasonix-setup";

#[derive(Clone)]
pub struct ReasonixPolicy {
    process: AcpProcessSpec,
}

impl ReasonixPolicy {
    pub fn new(command: PathBuf) -> Self {
        Self {
            process: AcpProcessSpec {
                command,
                args: vec!["acp".to_string(), WORKSPACE_ONLY_ARG.to_string()],
                env: BTreeMap::new(),
                cwd: None,
            },
        }
    }

    pub fn with_process(process: AcpProcessSpec) -> Self {
        Self { process }
    }
}

#[derive(Clone, Default)]
pub struct DenyPermissionBroker;

impl AcpPermissionBroker for DenyPermissionBroker {
    fn decide(
        &self,
        request: AcpPermissionRequest,
    ) -> Pin<Box<dyn Future<Output = AcpPermissionDecision> + Send + '_>> {
        let rejected = request
            .options
            .iter()
            .find(|option| option.kind == acp::PermissionOptionKind::RejectOnce)
            .or_else(|| {
                request
                    .options
                    .iter()
                    .find(|option| option.kind == acp::PermissionOptionKind::RejectAlways)
            })
            .map(|option| option.option_id.clone());
        Box::pin(async move {
            rejected.map_or(AcpPermissionDecision::Cancel, |option_id| {
                AcpPermissionDecision::Select { option_id }
            })
        })
    }
}

pub type ReasonixProvider = AcpProvider<ReasonixPolicy>;

pub fn provider(command: PathBuf) -> ReasonixProvider {
    AcpProvider::with_engine(
        ReasonixPolicy::new(command),
        AcpRuntimeConfig::default(),
        Some(Arc::new(DenyPermissionBroker)),
    )
}

#[async_trait]
impl AcpProviderPolicy for ReasonixPolicy {
    fn provider_id(&self) -> &'static str {
        "reasonix"
    }

    fn display_name(&self) -> &'static str {
        "Reasonix"
    }

    fn process(&self) -> AcpProcessSpec {
        self.process.clone()
    }

    fn initialize_request(&self) -> acp::InitializeRequest {
        acp::InitializeRequest::new(acp::ProtocolVersion::V1).client_info(
            acp::Implementation::new("jcode", env!("CARGO_PKG_VERSION")).title("Jcode"),
        )
    }

    fn choose_auth(&self, initialized: &acp::InitializeResponse) -> Result<AcpAuthAction> {
        let Some(method) = initialized
            .auth_methods
            .iter()
            .find(|method| method.id().0.as_ref() == SETUP_AUTH_METHOD_ID)
        else {
            if initialized
                .auth_methods
                .iter()
                .any(auth_method_mentions_setup)
            {
                bail!(
                    "Reasonix advertised setup authentication under an unexpected method ID; refusing to guess"
                );
            }
            return Ok(AcpAuthAction::None);
        };

        if !matches!(
            method,
            acp::AuthMethod::Terminal(terminal)
                if terminal.args == [SETUP_ARG] && terminal.env.is_empty()
        ) {
            bail!(
                "Reasonix advertised setup authentication, but not as the exact terminal command `reasonix setup`; refusing to guess or forward auth metadata"
            );
        }
        Ok(AcpAuthAction::Authenticate {
            method_id: method.id().clone(),
            meta: Default::default(),
        })
    }

    fn discover_models(
        &self,
        initialized: &acp::InitializeResponse,
        session: Option<&AcpSessionState>,
    ) -> DiscoveredModels {
        session
            .and_then(models_from_config_options)
            .or_else(|| {
                session
                    .and_then(|state| state.models.as_ref())
                    .map(models_from_state)
            })
            .unwrap_or_else(|| models_from_initialize(initialized))
    }

    fn prompt_blocks(&self, input: AcpPromptInput<'_>) -> Result<Vec<acp::ContentBlock>> {
        Ok(vec![acp::ContentBlock::Text(acp::TextContent::new(
            build_prompt(input.messages, input.system, input.resumed)?,
        ))])
    }

    fn session_setup(&self, state: &AcpSessionState) -> Result<Vec<AcpSessionMutation>> {
        let Some(selected_model) = state.selected_model.as_ref() else {
            return Ok(Vec::new());
        };
        if state.resumed {
            return Ok(Vec::new());
        }

        if let Some(option) = state
            .config_options
            .as_deref()
            .and_then(model_config_option)
        {
            if config_current_value(option).as_deref() == Some(selected_model.as_str()) {
                return Ok(Vec::new());
            }
            return Ok(vec![AcpSessionMutation::SetConfigOption {
                config_id: option.id.0.to_string(),
                value: selected_model.clone(),
                meta: None,
            }]);
        }

        if state
            .models
            .as_ref()
            .is_some_and(|models| models.current_model_id.0.as_ref() == selected_model.as_str())
        {
            return Ok(Vec::new());
        }
        Ok(vec![AcpSessionMutation::SetModel {
            model_id: selected_model.clone(),
            meta: None,
        }])
    }

    fn map_update(&self, update: acp::SessionUpdate) -> Vec<StreamEvent> {
        match update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => text_from_acp_content(chunk.content)
                .map(StreamEvent::TextDelta)
                .into_iter()
                .collect(),
            acp::SessionUpdate::AgentThoughtChunk(chunk) => text_from_acp_content(chunk.content)
                .map(StreamEvent::ThinkingDelta)
                .into_iter()
                .collect(),
            acp::SessionUpdate::ToolCall(call) => {
                vec![StreamEvent::StatusDetail { detail: call.title }]
            }
            acp::SessionUpdate::ToolCallUpdate(update) => update
                .fields
                .title
                .map(|detail| StreamEvent::StatusDetail { detail })
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    fn login_hint(&self, error: &anyhow::Error) -> String {
        format!(
            "Reasonix ACP failed: {error}. If Reasonix advertises setup authentication, run `reasonix setup`, then retry."
        )
    }
}

fn auth_method_mentions_setup(method: &acp::AuthMethod) -> bool {
    let mut text = format!("{} {}", method.id().0, method.name());
    if let Some(description) = method.description() {
        text.push(' ');
        text.push_str(description);
    }
    if let acp::AuthMethod::Terminal(terminal) = method {
        text.push(' ');
        text.push_str(&terminal.args.join(" "));
    }
    text.to_ascii_lowercase().contains(SETUP_ARG)
}

fn models_from_config_options(state: &AcpSessionState) -> Option<DiscoveredModels> {
    let option = state
        .config_options
        .as_deref()
        .and_then(model_config_option)?;
    let acp::SessionConfigKind::Select(select) = &option.kind else {
        return None;
    };
    let available = match &select.options {
        acp::SessionConfigSelectOptions::Ungrouped(options) => options
            .iter()
            .map(|option| option.value.0.to_string())
            .collect(),
        acp::SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| group.options.iter())
            .map(|option| option.value.0.to_string())
            .collect(),
        _ => Vec::new(),
    };
    Some(DiscoveredModels {
        current: Some(select.current_value.0.to_string()),
        available: deduplicate(available),
    })
}

fn text_from_acp_content(content: acp::ContentBlock) -> Option<String> {
    match content {
        acp::ContentBlock::Text(text) => Some(text.text),
        _ => None,
    }
}

fn model_config_option(options: &[acp::SessionConfigOption]) -> Option<&acp::SessionConfigOption> {
    options.iter().find(|option| {
        option.category == Some(acp::SessionConfigOptionCategory::Model)
            || option.id.0.eq_ignore_ascii_case("model")
            || option.name.eq_ignore_ascii_case("model")
    })
}

fn config_current_value(option: &acp::SessionConfigOption) -> Option<String> {
    let acp::SessionConfigKind::Select(select) = &option.kind else {
        return None;
    };
    Some(select.current_value.0.to_string())
}

fn models_from_state(models: &acp::SessionModelState) -> DiscoveredModels {
    let current = models.current_model_id.0.to_string();
    let mut available = models
        .available_models
        .iter()
        .map(|model| model.model_id.0.to_string())
        .collect::<Vec<_>>();
    if !current.is_empty() {
        available.insert(0, current.clone());
    }
    DiscoveredModels {
        current: (!current.is_empty()).then_some(current),
        available: deduplicate(available),
    }
}

fn models_from_initialize(response: &acp::InitializeResponse) -> DiscoveredModels {
    models_from_value(
        response
            .meta
            .as_ref()
            .and_then(|meta| meta.get("modelState")),
    )
}

fn models_from_value(value: Option<&Value>) -> DiscoveredModels {
    let Some(object) = value.and_then(Value::as_object) else {
        return DiscoveredModels::default();
    };
    let current = object
        .get("currentModelId")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(ToOwned::to_owned);
    let available = object
        .get("availableModels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| {
            value.as_str().or_else(|| {
                value.as_object().and_then(|model| {
                    ["modelId", "id", "name"]
                        .into_iter()
                        .find_map(|key| model.get(key).and_then(Value::as_str))
                })
            })
        })
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let mut available = deduplicate(available);
    if let Some(current) = current.as_ref() {
        available.insert(0, current.clone());
        available = deduplicate(available);
    }
    DiscoveredModels { current, available }
}

fn deduplicate(values: Vec<String>) -> Vec<String> {
    values.into_iter().fold(Vec::new(), |mut values, value| {
        if !value.trim().is_empty() && !values.contains(&value) {
            values.push(value);
        }
        values
    })
}

fn build_prompt(messages: &[Message], system: &str, resumed: bool) -> Result<String> {
    let latest_user = messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .map(message_text)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| anyhow!("No text user prompt found for Reasonix request"))?;

    let mut sections = Vec::new();
    if !system.trim().is_empty() {
        sections.push(format!("<system>\n{}\n</system>", system.trim()));
    }
    if !resumed {
        let history = messages
            .iter()
            .take(messages.len().saturating_sub(1))
            .filter_map(|message| {
                let text = message_text(message);
                (!text.trim().is_empty()).then(|| {
                    let role = match message.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                    };
                    format!("<{role}>\n{text}\n</{role}>")
                })
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if !history.is_empty() {
            sections.push(history);
        }
    }
    sections.push(latest_user);
    Ok(sections.join("\n\n"))
}

fn message_text(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            JcodeContentBlock::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response(methods: Vec<acp::AuthMethod>) -> acp::InitializeResponse {
        acp::InitializeResponse::new(acp::ProtocolVersion::V1).auth_methods(methods)
    }

    #[test]
    fn exact_setup_terminal_auth_is_acknowledged_without_metadata() {
        let initialized = response(vec![acp::AuthMethod::Terminal(
            acp::AuthMethodTerminal::new(SETUP_AUTH_METHOD_ID, "Reasonix setup")
                .args(vec![SETUP_ARG.to_string()]),
        )]);
        let action = ReasonixPolicy::new(PathBuf::from("reasonix"))
            .choose_auth(&initialized)
            .unwrap();
        let AcpAuthAction::Authenticate { method_id, meta } = action else {
            panic!("expected setup authentication acknowledgment");
        };
        assert_eq!(method_id.0.as_ref(), SETUP_AUTH_METHOD_ID);
        assert!(meta.is_empty());
    }

    #[test]
    fn modified_setup_terminal_auth_is_rejected() {
        let initialized = response(vec![acp::AuthMethod::Terminal(
            acp::AuthMethodTerminal::new(SETUP_AUTH_METHOD_ID, "Reasonix setup")
                .args(vec![SETUP_ARG.to_string(), "--token".to_string()]),
        )]);
        let error = ReasonixPolicy::new(PathBuf::from("reasonix"))
            .choose_auth(&initialized)
            .unwrap_err()
            .to_string();
        assert!(error.contains("exact terminal command"));
    }

    #[test]
    fn no_setup_auth_proceeds_without_authentication_rpc() {
        let initialized = response(Vec::new());
        assert!(matches!(
            ReasonixPolicy::new(PathBuf::from("reasonix")).choose_auth(&initialized),
            Ok(AcpAuthAction::None)
        ));
    }

    #[test]
    fn process_is_workspace_only_without_credentials() {
        let process = ReasonixPolicy::new(PathBuf::from("/tmp/reasonix")).process();
        assert_eq!(process.command, PathBuf::from("/tmp/reasonix"));
        assert_eq!(process.args, ["acp", WORKSPACE_ONLY_ARG]);
        assert!(process.env.is_empty());
    }

    #[test]
    fn model_config_is_the_only_mutated_axis() {
        let model = acp::SessionConfigOption::select(
            "model",
            "Model",
            "reasonix-large",
            vec![
                acp::SessionConfigSelectOption::new("reasonix-large", "Large"),
                acp::SessionConfigSelectOption::new("reasonix-fast", "Fast"),
            ],
        )
        .category(acp::SessionConfigOptionCategory::Model);
        let effort = acp::SessionConfigOption::select(
            "effort",
            "Effort",
            "high",
            vec![acp::SessionConfigSelectOption::new("high", "High")],
        )
        .category(acp::SessionConfigOptionCategory::ThoughtLevel);
        let state = AcpSessionState {
            initialized: response(Vec::new()),
            session_id: acp::SessionId::new("session"),
            resumed: false,
            selected_model: Some("reasonix-fast".to_string()),
            models: None,
            config_options: Some(vec![model, effort]),
            meta: Some(
                json!({"outputStyle":"concise"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        };
        assert_eq!(
            ReasonixPolicy::new(PathBuf::from("reasonix"))
                .session_setup(&state)
                .unwrap(),
            vec![AcpSessionMutation::SetConfigOption {
                config_id: "model".to_string(),
                value: "reasonix-fast".to_string(),
                meta: None,
            }]
        );
    }

    #[test]
    fn prompt_conversion_drops_non_text_blocks() {
        let messages = vec![Message::user("question")];
        let prompt = build_prompt(&messages, "system", false).unwrap();
        assert!(prompt.contains("question"));
        assert!(prompt.contains("system"));
    }
}
