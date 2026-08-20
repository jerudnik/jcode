use ratatui::style::Style;
use ratatui::text::Span;

#[derive(serde::Deserialize)]
#[serde(untagged)]
pub(super) enum TodoCardPayload {
    Current {
        #[serde(flatten)]
        session: TodoCardSession,
        #[serde(default)]
        todos: Vec<crate::todo::TodoItem>,
        #[serde(default)]
        goals: Vec<crate::todo::TodoGoal>,
    },
    Legacy(Vec<crate::todo::TodoItem>),
}

#[derive(Default, serde::Deserialize)]
pub(super) struct TodoCardSession {
    #[serde(default)]
    pub(super) session_id: Option<String>,
    #[serde(default)]
    pub(super) session_name: Option<String>,
}

impl TodoCardPayload {
    pub(super) fn into_parts(
        self,
    ) -> (
        TodoCardSession,
        Vec<crate::todo::TodoItem>,
        Vec<crate::todo::TodoGoal>,
    ) {
        match self {
            Self::Current {
                session,
                todos,
                goals,
            } => (session, todos, goals),
            Self::Legacy(todos) => (TodoCardSession::default(), todos, Vec::new()),
        }
    }
}

pub(super) fn render_session_spans(
    session: &TodoCardSession,
    style: Style,
) -> Option<Vec<Span<'static>>> {
    if session.session_id.is_none() && session.session_name.is_none() {
        return None;
    }
    let label = match session.session_name.as_deref() {
        Some(name) => format!("{} · ", name),
        None => String::new(),
    };
    let id = session.session_id.as_deref().unwrap_or("unknown");
    Some(vec![Span::styled(
        format!("Todos · {}session `{}`", label, id),
        style,
    )])
}
