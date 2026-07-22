use super::AppRuntimeMode;
use crate::message::{Message, StreamEvent};
use crate::provider::Provider;
use anyhow::Result;
use std::sync::Arc;

/// Inert provider used by runtime modes whose output is supplied by another source.
///
/// Remote clients render server events. Replay renders recorded events. Neither mode may call a
/// live provider from the TUI process.
pub(super) struct InertRuntimeProvider {
    runtime_mode: AppRuntimeMode,
}

impl InertRuntimeProvider {
    pub(super) fn new(runtime_mode: AppRuntimeMode) -> Self {
        Self { runtime_mode }
    }

    fn provider_label(&self) -> &'static str {
        match self.runtime_mode {
            AppRuntimeMode::RemoteClient => "remote",
            AppRuntimeMode::Replay => "replay",
            AppRuntimeMode::TestHarness => "test-harness",
        }
    }
}

#[async_trait::async_trait]
impl Provider for InertRuntimeProvider {
    fn name(&self) -> &str {
        self.provider_label()
    }
    fn model(&self) -> String {
        "unknown".to_string()
    }

    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[crate::message::ToolDefinition],
        _system: &str,
        _session_id: Option<&str>,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<StreamEvent>> + Send>>> {
        Err(anyhow::anyhow!(
            "{} runtime does not allow live provider completion from the TUI",
            self.provider_label()
        ))
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(InertRuntimeProvider::new(self.runtime_mode))
    }
}
