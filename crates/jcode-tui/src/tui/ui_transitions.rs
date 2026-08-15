#[cfg(test)]
use ratatui::text::Line;

#[cfg(test)]
pub(crate) fn extract_line_text(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}
