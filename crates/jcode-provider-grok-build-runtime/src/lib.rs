//! Grok Build subscription provider policy for the shared ACP runtime.
//!
//! This crate deliberately has no xAI HTTP or API-key path. Authentication is
//! delegated to the installed Grok CLI and restricted to its advertised
//! subscription methods.

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use jcode_message_types::{ContentBlock as JcodeContentBlock, Message, Role, StreamEvent};
use jcode_provider_acp_runtime::{
    AcpAuthAction, AcpPermissionBroker, AcpPermissionDecision, AcpPermissionRequest,
    AcpProcessSpec, AcpPromptInput, AcpProvider, AcpProviderPolicy, AcpRuntimeConfig,
    AcpSessionMutation, AcpSessionState, DiscoveredModels, acp,
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

pub const PROVIDER_ID: &str = "grok-build";
pub const DISPLAY_NAME: &str = "Grok Build";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrokBuildPolicy {
    process: AcpProcessSpec,
}

impl GrokBuildPolicy {
    pub fn new(command: PathBuf) -> Self {
        Self {
            process: AcpProcessSpec {
                command,
                args: vec!["agent".to_string(), "stdio".to_string()],
                env: BTreeMap::new(),
                cwd: None,
            },
        }
    }

    pub fn with_process(process: AcpProcessSpec) -> Self {
        Self { process }
    }
}

/// Explicit deny broker used until jcode has a synchronous arbitrary-choice UI.
#[derive(Debug, Default)]
pub struct DenyPermissionBroker;

impl AcpPermissionBroker for DenyPermissionBroker {
    fn decide(
        &self,
        _request: AcpPermissionRequest,
    ) -> Pin<Box<dyn Future<Output = AcpPermissionDecision> + Send + '_>> {
        Box::pin(async { AcpPermissionDecision::Cancel })
    }
}

pub type GrokBuildProvider = AcpProvider<GrokBuildPolicy>;

pub fn provider(command: PathBuf) -> GrokBuildProvider {
    AcpProvider::with_engine(
        GrokBuildPolicy::new(command),
        AcpRuntimeConfig::default(),
        Some(Arc::new(DenyPermissionBroker)),
    )
}

#[async_trait]
impl AcpProviderPolicy for GrokBuildPolicy {
    fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn display_name(&self) -> &'static str {
        DISPLAY_NAME
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
        let method_id = select_subscription_auth_method(initialized)?;
        let mut meta = Map::new();
        meta.insert("headless".to_string(), Value::Bool(true));
        Ok(AcpAuthAction::Authenticate { method_id, meta })
    }

    fn discover_models(
        &self,
        initialized: &acp::InitializeResponse,
        session: Option<&AcpSessionState>,
    ) -> DiscoveredModels {
        let initialized_models = models_from_initialize(initialized);
        if !initialized_models.available.is_empty() || initialized_models.current.is_some() {
            return initialized_models;
        }
        session
            .and_then(|state| state.models.as_ref())
            .map(models_from_session_state)
            .unwrap_or_default()
    }

    fn prompt_blocks(&self, input: AcpPromptInput<'_>) -> Result<Vec<acp::ContentBlock>> {
        Ok(vec![acp::ContentBlock::Text(acp::TextContent::new(
            build_prompt(input.messages, input.system, input.resumed)?,
        ))])
    }

    fn session_setup(&self, state: &AcpSessionState) -> Result<Vec<AcpSessionMutation>> {
        let current_model = state
            .models
            .as_ref()
            .map(|models| models.current_model_id.0.as_ref());
        Ok(match state.selected_model.as_ref() {
            Some(selected) if current_model != Some(selected.as_str()) => {
                vec![AcpSessionMutation::SetModel {
                    model_id: selected.clone(),
                    meta: None,
                }]
            }
            _ => Vec::new(),
        })
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
            "Grok Build authentication failed: {error}. Grok Build uses the official Grok subscription login, not XAI_API_KEY. Run `grok login` or `grok login --device-auth`, then retry"
        )
    }
}

fn select_subscription_auth_method(
    response: &acp::InitializeResponse,
) -> Result<acp::AuthMethodId> {
    let allowed = response.auth_methods.iter().filter(|method| {
        let id = method.id().0.as_ref().to_ascii_lowercase();
        id != "xai.api_key" && !id.contains("api_key") && !id.contains("api-key")
    });
    for preferred in ["cached_token", "grok.com"] {
        if let Some(method) = allowed
            .clone()
            .find(|method| method.id().0.as_ref() == preferred)
        {
            return Ok(method.id().clone());
        }
    }
    if let Some(method) = allowed.into_iter().find(|method| {
        let id = method.id().0.as_ref().to_ascii_lowercase();
        let name = method.name().to_ascii_lowercase();
        id.contains("grok") || id.contains("cached") || name.contains("grok")
    }) {
        return Ok(method.id().clone());
    }
    let advertised = response
        .auth_methods
        .iter()
        .map(|method| method.id().0.as_ref())
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "Grok CLI did not advertise a cached subscription authentication method (advertised: {})",
        if advertised.is_empty() {
            "none"
        } else {
            &advertised
        }
    )
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
    let mut available = Vec::new();
    if let Some(models) = object.get("availableModels").and_then(Value::as_array) {
        for value in models {
            let id = value.as_str().or_else(|| {
                value.as_object().and_then(|model| {
                    ["modelId", "id", "name"]
                        .into_iter()
                        .find_map(|key| model.get(key).and_then(Value::as_str))
                })
            });
            if let Some(id) = id.filter(|id| !id.trim().is_empty())
                && !available.iter().any(|known| known == id)
            {
                available.push(id.to_string());
            }
        }
    }
    if let Some(current) = current.as_ref()
        && !available.iter().any(|model| model == current)
    {
        available.insert(0, current.clone());
    }
    DiscoveredModels { current, available }
}

fn models_from_session_state(models: &acp::SessionModelState) -> DiscoveredModels {
    let current = models.current_model_id.0.to_string();
    let mut available = models
        .available_models
        .iter()
        .map(|model| model.model_id.0.to_string())
        .filter(|id| !id.trim().is_empty())
        .fold(Vec::new(), |mut models, id| {
            if !models.contains(&id) {
                models.push(id);
            }
            models
        });
    if !current.trim().is_empty() && !available.contains(&current) {
        available.insert(0, current.clone());
    }
    DiscoveredModels {
        current: (!current.trim().is_empty()).then_some(current),
        available,
    }
}

fn text_from_acp_content(content: acp::ContentBlock) -> Option<String> {
    match content {
        acp::ContentBlock::Text(text) => Some(text.text),
        _ => None,
    }
}

fn build_prompt(messages: &[Message], system: &str, resumed: bool) -> Result<String> {
    let latest_user = messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .map(message_text)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| anyhow!("No user prompt found for Grok Build request"))?;

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
            JcodeContentBlock::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcode_provider_core::Provider;
    use serde_json::json;

    fn response(methods: Vec<acp::AuthMethod>) -> acp::InitializeResponse {
        acp::InitializeResponse::new(acp::ProtocolVersion::V1).auth_methods(methods)
    }

    #[test]
    fn api_key_only_initialize_is_rejected() {
        let initialized = response(vec![acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
            "xai.api_key",
            "xAI API key",
        ))]);
        assert!(select_subscription_auth_method(&initialized).is_err());
    }

    #[test]
    fn cached_token_beats_grok_com() {
        let initialized = response(vec![
            acp::AuthMethod::Agent(acp::AuthMethodAgent::new("grok.com", "Grok.com")),
            acp::AuthMethod::Agent(acp::AuthMethodAgent::new("cached_token", "Cached token")),
        ]);
        assert_eq!(
            select_subscription_auth_method(&initialized)
                .unwrap()
                .0
                .as_ref(),
            "cached_token"
        );
    }

    #[test]
    fn model_state_accepts_string_and_object_rows() {
        let state = json!({
            "currentModelId": "grok-4.5",
            "availableModels": [
                "grok-code-fast-1",
                {"modelId": "grok-4.5"},
                {"id": "grok-4"},
                {"name": "grok-3"},
                {"modelId": "grok-4.5"}
            ]
        });
        let models = models_from_value(Some(&state));
        assert_eq!(models.current.as_deref(), Some("grok-4.5"));
        assert_eq!(
            models.available,
            ["grok-code-fast-1", "grok-4.5", "grok-4", "grok-3"]
        );
    }

    #[test]
    fn provider_has_no_forced_default_before_explicit_selection() {
        let provider = provider(PathBuf::from("grok"));
        assert_eq!(provider.model(), "unknown");
        assert!(provider.available_models_display().is_empty());
    }

    #[test]
    fn session_model_changes_only_after_explicit_selection() {
        let policy = GrokBuildPolicy::new(PathBuf::from("grok"));
        let initialized = response(Vec::new());
        let state = AcpSessionState {
            initialized,
            session_id: acp::SessionId::new("session"),
            resumed: false,
            selected_model: None,
            models: None,
            config_options: None,
            meta: None,
        };
        assert!(policy.session_setup(&state).unwrap().is_empty());

        let selected = AcpSessionState {
            selected_model: Some("grok-4.5".to_string()),
            ..state
        };
        assert_eq!(
            policy.session_setup(&selected).unwrap(),
            vec![AcpSessionMutation::SetModel {
                model_id: "grok-4.5".to_string(),
                meta: None,
            }]
        );
    }

    #[test]
    fn login_hint_never_recommends_an_api_key() {
        let hint =
            GrokBuildPolicy::new(PathBuf::from("grok")).login_hint(&anyhow!("auth required"));
        assert!(hint.contains("grok login"));
        assert!(hint.contains("not XAI_API_KEY"));
    }

    #[test]
    fn process_uses_current_agent_stdio_command() {
        let process = GrokBuildPolicy::new(PathBuf::from("/tmp/grok")).process();
        assert_eq!(process.command, PathBuf::from("/tmp/grok"));
        assert_eq!(process.args, ["agent", "stdio"]);
    }
}
