use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MouseScrollTraceState {
    chat_offset: usize,
    auto_scroll_paused: bool,
    mouse_queue: i16,
    mouse_target: Option<MouseScrollTarget>,
    diff_offset: usize,
    diff_auto_scroll: bool,
    diagram_focus: bool,
    diagram_x: i32,
    diagram_y: i32,
    diagram_zoom: u8,
    help_scroll: Option<usize>,
    changelog_scroll: Option<usize>,
}

impl MouseScrollTraceState {
    pub(super) fn capture(app: &App) -> Self {
        Self {
            chat_offset: app.scroll_offset,
            auto_scroll_paused: app.auto_scroll_paused,
            mouse_queue: app.mouse_scroll_queue,
            mouse_target: app.mouse_scroll_target,
            diff_offset: app.diff_pane_scroll,
            diff_auto_scroll: app.diff_pane_auto_scroll,
            diagram_focus: app.diagram_focus,
            diagram_x: app.diagram_scroll_x,
            diagram_y: app.diagram_scroll_y,
            diagram_zoom: app.diagram_zoom,
            help_scroll: app.help_scroll,
            changelog_scroll: app.changelog_scroll,
        }
    }

    pub(super) fn summary(&self) -> String {
        format!(
            "chat={} auto={} queue={} target={:?} diff={} diff_auto={} diagram_focus={} diagram=({},{} @ {}%) help={:?} changelog={:?}",
            self.chat_offset,
            self.auto_scroll_paused,
            self.mouse_queue,
            self.mouse_target,
            self.diff_offset,
            self.diff_auto_scroll,
            self.diagram_focus,
            self.diagram_x,
            self.diagram_y,
            self.diagram_zoom,
            self.help_scroll,
            self.changelog_scroll,
        )
    }
}

pub(super) fn tui_mouse_scroll_trace_enabled() -> bool {
    std::env::var_os("JCODE_TUI_SCROLL_TRACE").is_some()
}

pub(super) fn is_mouse_scroll_kind(kind: MouseEventKind) -> bool {
    matches!(
        kind,
        MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight
    )
}
