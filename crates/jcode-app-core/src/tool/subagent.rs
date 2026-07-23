use super::{Registry, Tool, ToolContext, ToolOutput};
use crate::agent::Agent;
use crate::provider::Provider;
use crate::session::Session;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct SubagentInput {
    description: String,
    prompt: String,
    #[serde(default = "default_subagent_type")]
    subagent_type: String,
    #[serde(default)]
    run_in_background: bool,
    model: Option<String>,
}

fn default_subagent_type() -> String {
    "general-purpose".to_string()
}

#[derive(Clone, Debug)]
pub(crate) struct SubagentParent {
    pub session_id: String,
    pub working_dir: Option<PathBuf>,
    pub model: String,
    pub provider_key: Option<String>,
    pub route_api_method: Option<String>,
}

impl SubagentParent {
    fn from_session(
        session: Session,
        working_dir: Option<PathBuf>,
        provider: &dyn Provider,
    ) -> Self {
        Self {
            session_id: session.id,
            working_dir: working_dir.or_else(|| session.working_dir.map(PathBuf::from)),
            model: session.model.unwrap_or_else(|| provider.model()),
            provider_key: session.provider_key,
            route_api_method: session.route_api_method,
        }
    }
}

pub(crate) async fn run_subagent_worker(
    provider: Arc<dyn Provider>,
    registry: Registry,
    parent: SubagentParent,
    description: &str,
    subagent_type: &str,
    prompt: &str,
    model: Option<&str>,
) -> Result<String> {
    let mut session = Session::create(
        Some(parent.session_id),
        Some(format!("{} (@{} swarm)", description, subagent_type)),
    );
    session.model = Some(model.unwrap_or(&parent.model).to_string());
    session.provider_key = parent.provider_key;
    session.route_api_method = parent.route_api_method;
    if let Some(dir) = parent.working_dir {
        session.working_dir = Some(dir.display().to_string());
    }
    session.save()?;

    let mut allowed: HashSet<String> = registry.tool_names().await.into_iter().collect();
    for blocked in ["subagent", "task", "todo", "todowrite", "todoread"] {
        allowed.remove(blocked);
    }
    crate::config::config()
        .tools
        .apply_to_allowed_set(&mut allowed);

    let mut worker = Agent::new_with_session(provider, registry, session, Some(allowed));
    worker.run_once_capture(prompt).await
}

pub(crate) struct SubagentTool {
    provider: Arc<dyn Provider>,
    registry: Registry,
}

impl SubagentTool {
    pub(crate) fn new(provider: Arc<dyn Provider>, registry: Registry) -> Self {
        Self { provider, registry }
    }
}

#[async_trait]
impl Tool for SubagentTool {
    fn name(&self) -> &str {
        "subagent"
    }

    fn description(&self) -> &str {
        "Launch a worker agent through the swarm execution path and return its captured output."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": { "type": "string" },
                "prompt": { "type": "string" },
                "subagent_type": { "type": "string" },
                "run_in_background": { "type": "boolean" },
                "model": { "type": "string" },
                "intent": super::intent_schema_property()
            },
            "required": ["description", "prompt"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value, ctx: ToolContext) -> Result<ToolOutput> {
        let input: SubagentInput =
            serde_json::from_value(input).context("invalid subagent input")?;
        let parent_session = Session::load(&ctx.session_id)
            .with_context(|| format!("failed to load parent session {}", ctx.session_id))?;
        let parent = SubagentParent::from_session(
            parent_session,
            ctx.working_dir.clone(),
            self.provider.as_ref(),
        );
        let output = run_subagent_worker(
            self.provider.fork(),
            self.registry.clone(),
            parent,
            &input.description,
            &input.subagent_type,
            &input.prompt,
            input.model.as_deref(),
        )
        .await?;

        if input.run_in_background {
            Ok(ToolOutput::new(format!(
                "Background execution is not yet detached; the worker completed synchronously.\n\n{output}"
            )))
        } else {
            Ok(ToolOutput::new(output))
        }
    }
}
