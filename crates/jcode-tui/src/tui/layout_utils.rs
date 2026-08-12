use super::visual_debug::RectCapture;
pub(crate) use jcode_tui_render::layout::{parse_area_spec, point_in_rect, rect_contains};
use ratatui::layout::Rect;

pub(crate) fn rect_from_capture(rect: RectCapture) -> Rect {
    Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

}
