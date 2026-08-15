//! Kimi Code official CLI policy for the shared ACP runtime.
//!
//! This provider is intentionally distinct from jcode's direct Kimi API
//! profile. The `kimi` subprocess owns its credentials, model configuration,
//! sessions, and tools; jcode only speaks ACP over clean stdio.

use anyhow::{Result, anyhow, bail};
use async_trait::async_trait;
use jcode_message_types::{ContentBlock as JcodeContentBlock, Message, Role, StreamEvent};
use jcode_provider_acp_runtime::{
    AcpAuthAction, AcpPermissionBroker, AcpPermissionDecision, AcpPermissionRequest,
    AcpProcessSpec, AcpPromptInput, AcpProvider, AcpProviderPolicy, AcpRuntimeConfig,
    AcpSessionMutation, AcpSessionState, DiscoveredModels, TerminalAuthSpec, acp,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

pub const PROVIDER_ID: &str = "kimi-code-acp";
pub const DISPLAY_NAME: &str = "Kimi Code (official CLI)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KimiPromptCapabilities {
    pub image: bool,
    pub embedded_text_resource: bool,
    pub audio: bool,
}

pub const PROMPT_CAPABILITIES: KimiPromptCapabilities = KimiPromptCapabilities {
    image: true,
    embedded_text_resource: true,
    audio: false,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KimiCodePolicy {
    process: AcpProcessSpec,
}

impl KimiCodePolicy {
    pub fn new(command: PathBuf) -> Self {
        Self {
            process: AcpProcessSpec {
                command,
                args: vec!["acp".to_string()],
                env: BTreeMap::new(),
                cwd: None,
            },
        }
    }

    pub fn with_process(process: AcpProcessSpec) -> Self {
        Self { process }
    }

    /// Map Kimi's first-class ACP terminal method to the command the host must
    /// run outside the JSON-RPC channel. The returned args append to `kimi acp`.
    pub fn terminal_login_spec(
        &self,
        initialized: &acp::InitializeResponse,
    ) -> Result<TerminalAuthSpec> {
        let method = initialized
            .auth_methods
            .iter()
            .find(|method| method.id().0.as_ref() == "login")
            .ok_or_else(|| anyhow!("Kimi Code did not advertise terminal login"))?;
        match method {
            acp::AuthMethod::Terminal(terminal) => Ok(TerminalAuthSpec {
                command: None,
                args: terminal.args.clone(),
                meta: terminal.meta.clone().map(Value::Object),
            }),
            _ => bail!("Kimi Code login method was not terminal authentication"),
        }
    }

    /// Legacy ACP clients run the top-level command rather than appending args
    /// to the configured `kimi acp` process.
    pub fn legacy_login_spec(&self) -> TerminalAuthSpec {
        TerminalAuthSpec {
            command: Some(self.process.command.clone()),
            args: vec!["login".to_string()],
            meta: None,
        }
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

pub type KimiCodeProvider = AcpProvider<KimiCodePolicy>;

pub fn provider(command: PathBuf) -> KimiCodeProvider {
    AcpProvider::with_engine(
        KimiCodePolicy::new(command),
        AcpRuntimeConfig::default(),
        Some(Arc::new(DenyPermissionBroker)),
    )
}

#[async_trait]
impl AcpProviderPolicy for KimiCodePolicy {
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
        acp::InitializeRequest::new(acp::ProtocolVersion::V1)
            .client_capabilities(
                acp::ClientCapabilities::new().auth(acp::AuthCapabilities::new().terminal(true)),
            )
            .client_info(
                acp::Implementation::new("jcode", env!("CARGO_PKG_VERSION")).title("Jcode"),
            )
    }

    fn choose_auth(&self, initialized: &acp::InitializeResponse) -> Result<AcpAuthAction> {
        let method = initialized
            .auth_methods
            .iter()
            .find(|method| method.id().0.as_ref() == "login")
            .ok_or_else(|| {
                anyhow!("Kimi Code did not advertise the `login` authentication method")
            })?;
        Ok(AcpAuthAction::Authenticate {
            method_id: method.id().clone(),
            meta: Default::default(),
        })
    }

    fn discover_models(
        &self,
        _initialized: &acp::InitializeResponse,
        session: Option<&AcpSessionState>,
    ) -> DiscoveredModels {
        session
            .and_then(|state| state.config_options.as_deref())
            .and_then(model_config_option)
            .map(models_from_config_option)
            .unwrap_or_default()
    }

    fn prompt_blocks(&self, input: AcpPromptInput<'_>) -> Result<Vec<acp::ContentBlock>> {
        structured_prompt_blocks(input.messages)
    }

    fn session_setup(&self, state: &AcpSessionState) -> Result<Vec<AcpSessionMutation>> {
        let Some(selected) = state.selected_model.as_ref() else {
            return Ok(Vec::new());
        };
        if let Some(option) = state
            .config_options
            .as_deref()
            .and_then(model_config_option)
        {
            let acp::SessionConfigKind::Select(select) = &option.kind else {
                return Ok(Vec::new());
            };
            if select.current_value.0.as_ref() != selected {
                return Ok(vec![AcpSessionMutation::SetConfigOption {
                    config_id: option.id.0.to_string(),
                    value: selected.clone(),
                    meta: None,
                }]);
            }
            return Ok(Vec::new());
        }
        let current_model = state
            .models
            .as_ref()
            .map(|models| models.current_model_id.0.as_ref());
        Ok((current_model != Some(selected.as_str()))
            .then(|| AcpSessionMutation::SetModel {
                model_id: selected.clone(),
                meta: None,
            })
            .into_iter()
            .collect())
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

    fn supports_image_input(&self) -> bool {
        PROMPT_CAPABILITIES.image
    }

    fn login_hint(&self, error: &anyhow::Error) -> String {
        format!(
            "Kimi Code CLI authentication failed: {error}. Run `jcode login --provider {PROVIDER_ID}` or `kimi acp --login`, then retry"
        )
    }
}

fn model_config_option(options: &[acp::SessionConfigOption]) -> Option<&acp::SessionConfigOption> {
    options.iter().find(|option| {
        matches!(option.kind, acp::SessionConfigKind::Select(_))
            && (matches!(
                option.category,
                Some(acp::SessionConfigOptionCategory::Model)
            ) || option.id.0.as_ref() == "model")
    })
}

fn models_from_config_option(option: &acp::SessionConfigOption) -> DiscoveredModels {
    let acp::SessionConfigKind::Select(select) = &option.kind else {
        return DiscoveredModels::default();
    };
    let current = select.current_value.0.to_string();
    let mut available = Vec::new();
    match &select.options {
        acp::SessionConfigSelectOptions::Ungrouped(options) => {
            extend_model_ids(&mut available, options)
        }
        acp::SessionConfigSelectOptions::Grouped(groups) => {
            for group in groups {
                extend_model_ids(&mut available, &group.options);
            }
        }
        _ => {}
    }
    if !current.trim().is_empty() && !available.contains(&current) {
        available.insert(0, current.clone());
    }
    DiscoveredModels {
        current: (!current.trim().is_empty()).then_some(current),
        available,
    }
}

fn extend_model_ids(models: &mut Vec<String>, options: &[acp::SessionConfigSelectOption]) {
    for option in options {
        let id = option.value.0.to_string();
        if !id.trim().is_empty() && !models.contains(&id) {
            models.push(id);
        }
    }
}

fn structured_prompt_blocks(messages: &[Message]) -> Result<Vec<acp::ContentBlock>> {
    let message = messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .ok_or_else(|| anyhow!("No user prompt found for Kimi Code request"))?;
    let mut blocks = Vec::new();
    for block in &message.content {
        match block {
            JcodeContentBlock::Text { text, .. } if !text.is_empty() => {
                blocks.push(acp::ContentBlock::Text(acp::TextContent::new(text.clone())));
            }
            JcodeContentBlock::Image { media_type, data } => {
                blocks.push(acp::ContentBlock::Image(acp::ImageContent::new(
                    data.clone(),
                    media_type.clone(),
                )));
            }
            JcodeContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                let resource = acp::TextResourceContents::new(
                    content.clone(),
                    format!("jcode://tool-result/{tool_use_id}"),
                )
                .mime_type("text/plain");
                blocks.push(acp::ContentBlock::Resource(acp::EmbeddedResource::new(
                    acp::EmbeddedResourceResource::TextResourceContents(resource),
                )));
            }
            _ => {}
        }
    }
    if blocks.is_empty() {
        bail!("Kimi Code request contained no supported prompt content");
    }
    Ok(blocks)
}

fn text_from_acp_content(content: acp::ContentBlock) -> Option<String> {
    match content {
        acp::ContentBlock::Text(text) => Some(text.text),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcode_provider_core::Provider;
    use serde_json::json;

    fn initialized(auth_methods: Value) -> acp::InitializeResponse {
        serde_json::from_value(json!({
            "protocolVersion": 1,
            "agentCapabilities": {
                "promptCapabilities": {
                    "image": true,
                    "audio": false,
                    "embeddedContext": true
                }
            },
            "authMethods": auth_methods
        }))
        .unwrap()
    }

    fn session_state(config_options: Value, selected_model: Option<&str>) -> AcpSessionState {
        AcpSessionState {
            initialized: initialized(json!([{
                "id":"login",
                "type":"terminal",
                "name":"Login",
                "args":["--login"]
            }])),
            session_id: acp::SessionId::new("session"),
            resumed: false,
            selected_model: selected_model.map(ToOwned::to_owned),
            models: None,
            config_options: Some(serde_json::from_value(config_options).unwrap()),
            meta: None,
        }
    }

    #[test]
    fn process_and_terminal_login_commands_match_kimi_cli_contract() {
        let policy = KimiCodePolicy::new(PathBuf::from("/opt/bin/kimi"));
        assert_eq!(policy.process().args, ["acp"]);
        let initialized = initialized(json!([{
            "id":"login",
            "type":"terminal",
            "name":"Login with Kimi account",
            "args":["--login"]
        }]));
        assert_eq!(
            policy.terminal_login_spec(&initialized).unwrap(),
            TerminalAuthSpec {
                command: None,
                args: vec!["--login".to_string()],
                meta: None,
            }
        );
        assert_eq!(
            policy.legacy_login_spec(),
            TerminalAuthSpec {
                command: Some(PathBuf::from("/opt/bin/kimi")),
                args: vec!["login".to_string()],
                meta: None,
            }
        );
    }

    #[test]
    fn config_axes_stay_independent_and_model_uses_set_config_option() {
        let options = json!([
            {"id":"model","name":"Model","category":"model","type":"select","currentValue":"kimi-a","options":[{"value":"kimi-a","name":"A"},{"value":"kimi-b","name":"B"}]},
            {"id":"thinking","name":"Thinking","category":"thought_level","type":"select","currentValue":"high","options":[{"value":"off","name":"Off"},{"value":"high","name":"High"}]},
            {"id":"mode","name":"Mode","category":"mode","type":"select","currentValue":"plan","options":[{"value":"agent","name":"Agent"},{"value":"plan","name":"Plan"}]}
        ]);
        let state = session_state(options, Some("kimi-b"));
        let preserved = state.config_options.as_ref().unwrap();
        assert_eq!(preserved.len(), 3);
        assert_eq!(preserved[1].id.0.as_ref(), "thinking");
        assert_eq!(preserved[2].id.0.as_ref(), "mode");
        assert_eq!(
            KimiCodePolicy::new(PathBuf::from("kimi"))
                .session_setup(&state)
                .unwrap(),
            [AcpSessionMutation::SetConfigOption {
                config_id: "model".to_string(),
                value: "kimi-b".to_string(),
                meta: None,
            }]
        );
    }

    #[test]
    fn prompt_conversion_preserves_structured_text_image_and_resource_blocks() {
        let old = Message::user("old prompt must not replay");
        let mut current = Message::user_with_images(
            "new prompt",
            vec![("image/png".to_string(), "aW1hZ2U=".to_string())],
        );
        current.content.push(JcodeContentBlock::ToolResult {
            tool_use_id: "tool-7".to_string(),
            content: "embedded result".to_string(),
            is_error: Some(false),
        });
        let blocks = structured_prompt_blocks(&[old, current]).unwrap();
        assert!(
            matches!(&blocks[0], acp::ContentBlock::Image(image) if image.mime_type == "image/png")
        );
        assert!(matches!(&blocks[1], acp::ContentBlock::Text(text) if text.text == "new prompt"));
        assert!(matches!(&blocks[2], acp::ContentBlock::Resource(resource)
            if matches!(&resource.resource,
                acp::EmbeddedResourceResource::TextResourceContents(text)
                if text.text == "embedded result" && text.uri == "jcode://tool-result/tool-7")));
        assert!(
            !serde_json::to_string(&blocks)
                .unwrap()
                .contains("old prompt")
        );
    }

    #[test]
    fn provider_exposes_images_and_embedded_text_but_not_audio() {
        let provider = super::provider(PathBuf::from("kimi"));
        assert!(provider.supports_image_input());
        assert_eq!(
            PROMPT_CAPABILITIES,
            KimiPromptCapabilities {
                image: true,
                embedded_text_resource: true,
                audio: false,
            }
        );
    }
}
