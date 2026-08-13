use super::{
    AcpLocation, AcpPermissionContent, AcpPermissionOption, AcpPermissionRequest, AcpToolKind, acp,
};
use serde_json::Value;

pub(super) fn normalize_permission(
    provider: &'static str,
    request: &acp::RequestPermissionRequest,
) -> acp::Result<AcpPermissionRequest> {
    let fields = &request.tool_call.fields;
    Ok(AcpPermissionRequest {
        provider,
        provider_session_id: request.session_id.0.to_string(),
        tool_call_id: request.tool_call.tool_call_id.0.to_string(),
        title: fields.title.as_deref().unwrap_or("").to_owned(),
        kind: fields.kind.map(map_tool_kind).unwrap_or(AcpToolKind::Other),
        raw_input: fields.raw_input.clone(),
        content: fields
            .content
            .as_ref()
            .into_iter()
            .flatten()
            .map(|content| serde_json::to_value(content).map_err(acp::Error::into_internal_error))
            .collect::<acp::Result<Vec<_>>>()?
            .into_iter()
            .map(|value| AcpPermissionContent { value })
            .collect(),
        locations: fields
            .locations
            .as_ref()
            .into_iter()
            .flatten()
            .map(|location| AcpLocation {
                path: location.path.clone(),
                line: location.line,
                meta: location.meta.clone().map(Value::Object),
            })
            .collect(),
        options: request
            .options
            .iter()
            .map(|option| AcpPermissionOption {
                option_id: option.option_id.0.to_string(),
                name: option.name.clone(),
                kind: option.kind,
                meta: option.meta.clone().map(Value::Object),
            })
            .collect(),
        meta: request.meta.clone().map(Value::Object),
    })
}

fn map_tool_kind(kind: acp::ToolKind) -> AcpToolKind {
    match kind {
        acp::ToolKind::Read => AcpToolKind::Read,
        acp::ToolKind::Edit => AcpToolKind::Edit,
        acp::ToolKind::Delete => AcpToolKind::Delete,
        acp::ToolKind::Move => AcpToolKind::Move,
        acp::ToolKind::Search => AcpToolKind::Search,
        acp::ToolKind::Execute => AcpToolKind::Execute,
        acp::ToolKind::Think => AcpToolKind::Think,
        acp::ToolKind::Fetch => AcpToolKind::Fetch,
        _ => AcpToolKind::Other,
    }
}
