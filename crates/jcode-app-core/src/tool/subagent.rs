use super::{Registry, Tool, ToolContext, ToolOutput};
use crate::agent::Agent;
use crate::provider::{ModelRoute, Provider};
use crate::session::Session;
use crate::tool::ambient::AmbientSessionGuard;
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct SubagentSelection {
    model: String,
    provider_key: Option<String>,
    route_api_method: Option<String>,
}

fn inherited_selection(parent: &SubagentParent) -> SubagentSelection {
    SubagentSelection {
        model: parent.model.clone(),
        provider_key: parent.provider_key.clone(),
        route_api_method: parent.route_api_method.clone(),
    }
}

fn is_inherit_sentinel(model: &str) -> bool {
    model.eq_ignore_ascii_case("inherit") || model.eq_ignore_ascii_case("coordinator")
}

fn validate_subagent_model_policy(model: &str, denied_models: &[String]) -> Result<()> {
    if denied_models.iter().any(|denied| denied.trim() == model) {
        anyhow::bail!(
            "Subagent model '{model}' is denied by policy in `agents.swarm_denied_models`."
        );
    }
    Ok(())
}

fn explicit_dual_auth_selection(model: &str) -> Option<SubagentSelection> {
    let resolved = crate::provider::resolve_model_spec(model, crate::config::config());
    let prefix = resolved.explicit_prefix.as_deref()?;
    if !matches!(
        prefix,
        "openai-api" | "openai-oauth" | "claude-api" | "claude-oauth"
    ) {
        return None;
    }
    let route = jcode_provider_core::AuthRoute::parse_explicit_credential_prefix(prefix)?;
    let route_id = route.route_api_method().to_string();
    Some(SubagentSelection {
        model: resolved.bare_model,
        provider_key: Some(route_id.clone()),
        route_api_method: Some(route_id),
    })
}

fn catalog_selection_for_model(
    model: &str,
    routes: &[ModelRoute],
) -> Option<Result<SubagentSelection>> {
    let mut matched = routes
        .iter()
        .filter(|route| route.model == model)
        .peekable();
    matched.peek()?;
    let Some(route) = matched.clone().find(|route| route.available) else {
        let detail = matched
            .map(|route| route.detail.trim())
            .find(|detail| !detail.is_empty())
            .unwrap_or("route unavailable");
        return Some(Err(anyhow::anyhow!(
            "Subagent model '{model}' is listed but currently unavailable: {detail}"
        )));
    };
    let selection = crate::provider::RouteSelection::from_model_route(route);
    Some(Ok(SubagentSelection {
        model: selection.model,
        provider_key: Some(selection.runtime_key.stable_id()),
        route_api_method: Some(selection.api_method),
    }))
}

fn resolve_subagent_selection(
    parent: &SubagentParent,
    requested_model: Option<&str>,
    routes: &[ModelRoute],
    denied_models: &[String],
) -> Result<SubagentSelection> {
    let requested_model = requested_model
        .map(str::trim)
        .filter(|model| !model.is_empty());
    let Some(model) = requested_model else {
        return Ok(inherited_selection(parent));
    };
    if is_inherit_sentinel(model) {
        return Ok(inherited_selection(parent));
    }
    validate_subagent_model_policy(model, denied_models)?;
    if model == parent.model {
        return Ok(inherited_selection(parent));
    }
    if let Some(selection) = explicit_dual_auth_selection(model) {
        return Ok(selection);
    }

    let resolved = crate::provider::resolve_model_spec(model, crate::config::config());
    if resolved.explicit_prefix.is_none()
        && let Some(selection) = catalog_selection_for_model(model, routes)
    {
        return selection;
    }
    let Some(provider_key) = resolved.provider_key else {
        anyhow::bail!(
            "Subagent model '{model}' could not be resolved to a provider or route. Run `swarm list_models` and retry with a listed model, or use an explicit route prefix such as `openai-oauth:`."
        );
    };
    Ok(SubagentSelection {
        model: model.to_string(),
        provider_key: Some(provider_key),
        route_api_method: None,
    })
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
    let parent_session_id = parent.session_id.clone();
    let routes = if model.is_some_and(|model| {
        let model = model.trim();
        !model.is_empty() && !is_inherit_sentinel(model) && model != parent.model.as_str()
    }) {
        provider.model_routes()
    } else {
        Vec::new()
    };
    let selection = resolve_subagent_selection(
        &parent,
        model,
        &routes,
        &crate::config::config().agents.swarm_denied_models,
    )?;
    let mut session = Session::create(
        Some(parent_session_id.clone()),
        Some(format!("{} (@{} swarm)", description, subagent_type)),
    );
    session.model = Some(selection.model);
    session.provider_key = selection.provider_key;
    session.route_api_method = selection.route_api_method;
    if let Some(dir) = parent.working_dir {
        session.working_dir = Some(dir.display().to_string());
    }
    session.save()?;
    let worker_session_id = session.id.clone();

    let mut allowed: HashSet<String> = registry.tool_names().await.into_iter().collect();
    for blocked in ["subagent", "task", "todo", "todowrite", "todoread"] {
        allowed.remove(blocked);
    }
    crate::config::config()
        .tools
        .apply_to_allowed_set(&mut allowed);

    let mut worker = Agent::new_with_session(provider, registry, session, Some(allowed));
    // The worker runs on a FRESH session id that nothing else registers, so
    // without this it would be ungated regardless of its parent. Inherit rather
    // than register unconditionally: an interactive user's subagent must stay
    // ungated. The guard unregisters on drop, including the `?` paths above the
    // await and any error the worker itself returns.
    let _ambient_guard = AmbientSessionGuard::inherit(&parent_session_id, worker_session_id);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parent() -> SubagentParent {
        SubagentParent {
            session_id: "parent".to_string(),
            working_dir: None,
            model: "claude-opus-5".to_string(),
            provider_key: Some("claude-oauth".to_string()),
            route_api_method: Some("anthropic-oauth".to_string()),
        }
    }

    #[test]
    fn model_override_resolves_against_live_catalog_instead_of_inheriting_parent_route() {
        let routes = [ModelRoute {
            model: "gpt-5.6-sol-xhigh-fast".to_string(),
            provider: "Cursor".to_string(),
            api_method: "cursor".to_string(),
            available: true,
            detail: String::new(),
            cheapness: None,
        }];

        let selection =
            resolve_subagent_selection(&parent(), Some("gpt-5.6-sol-xhigh-fast"), &routes, &[])
                .expect("listed model should resolve");

        assert_eq!(selection.model, "gpt-5.6-sol-xhigh-fast");
        assert_eq!(selection.provider_key.as_deref(), Some("cursor"));
        assert_eq!(selection.route_api_method.as_deref(), Some("cursor"));
    }

    #[test]
    fn unresolved_model_override_fails_closed_instead_of_inheriting_parent_route() {
        let error = resolve_subagent_selection(&parent(), Some("definitely-not-a-model"), &[], &[])
            .expect_err("unknown model must fail before the worker session is created");

        assert!(error.to_string().contains("could not be resolved"));
    }

    #[test]
    fn denied_model_override_fails_before_inheriting_the_same_parent_model() {
        let error = resolve_subagent_selection(
            &parent(),
            Some("claude-opus-5"),
            &[],
            &["claude-opus-5".to_string()],
        )
        .expect_err("a concrete denied model must not bypass policy through inheritance");

        assert!(error.to_string().contains("agents.swarm_denied_models"));
    }
}
