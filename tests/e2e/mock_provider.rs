//! Mock provider for e2e tests
//!
//! Returns pre-scripted StreamEvent sequences for deterministic testing.

use anyhow::Result;
use async_stream::stream;
use jcode::message::{Message, StreamEvent, ToolDefinition};
use jcode::provider::{EventStream, Provider};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// The (role, text) of one message, flattened across its content blocks.
type CapturedMessage = (String, String);

/// The messages of a single `complete()` call.
type CapturedCall = Vec<CapturedMessage>;

/// What each `complete()` call was given, so tests can assert on the payload the
/// provider actually receives rather than on internal state.
#[derive(Clone, Default)]
pub struct MockCaptures {
    pub system_prompts: Arc<Mutex<Vec<String>>>,
    pub resume_session_ids: Arc<Mutex<Vec<Option<String>>>>,
    pub models: Arc<Mutex<Vec<String>>>,
    /// Per call, the (role, text) of every message sent, flattened across blocks.
    pub messages: Arc<Mutex<Vec<CapturedCall>>>,
}

pub struct MockProvider {
    responses: Arc<Mutex<VecDeque<Vec<StreamEvent>>>>,
    models: Vec<&'static str>,
    current_model: Arc<Mutex<String>>,
    /// Captured inputs from complete() calls (for testing)
    pub captures: MockCaptures,
}

impl MockProvider {
    pub fn new() -> Self {
        Self::with_models(Vec::new())
    }

    pub fn with_models(models: Vec<&'static str>) -> Self {
        let current = models
            .first()
            .map(|m| (*m).to_string())
            .unwrap_or_else(|| "mock".to_string());
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
            models,
            current_model: Arc::new(Mutex::new(current)),
            captures: MockCaptures::default(),
        }
    }

    /// Queue a response (sequence of StreamEvents) to be returned on next complete() call
    pub fn queue_response(&self, events: Vec<StreamEvent>) {
        self.responses.lock().unwrap().push_back(events);
    }
}

#[async_trait::async_trait]
impl Provider for MockProvider {
    async fn complete(
        &self,
        messages: &[Message],
        _tools: &[ToolDefinition],
        system: &str,
        resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        let c = &self.captures;
        c.system_prompts.lock().unwrap().push(system.to_string());
        c.resume_session_ids
            .lock()
            .unwrap()
            .push(resume_session_id.map(|s| s.to_string()));
        c.models.lock().unwrap().push(self.model());
        c.messages.lock().unwrap().push(
            messages
                .iter()
                .map(|m| {
                    let role = format!("{:?}", m.role).to_lowercase();
                    let text = m
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            jcode::message::ContentBlock::Text { text, .. } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    (role, text)
                })
                .collect(),
        );

        let events = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_default();

        let stream = stream! {
            for event in events {
                yield Ok(event);
            }
        };

        Ok(Box::pin(stream))
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn model(&self) -> String {
        self.current_model.lock().unwrap().clone()
    }

    fn set_model(&self, model: &str) -> Result<()> {
        if !self.models.is_empty() && !self.models.contains(&model) {
            anyhow::bail!("Unknown model: {}", model);
        }
        *self.current_model.lock().unwrap() = model.to_string();
        Ok(())
    }

    fn available_models(&self) -> Vec<&'static str> {
        self.models.clone()
    }

    fn fork(&self) -> Arc<dyn Provider> {
        let current = self.current_model.lock().unwrap().clone();
        Arc::new(MockProvider {
            responses: self.responses.clone(),
            models: self.models.clone(),
            current_model: Arc::new(Mutex::new(current)),
            captures: self.captures.clone(),
        })
    }
}
