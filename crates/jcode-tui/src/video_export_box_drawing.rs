//! Vector glyphs for Unicode box-drawing characters.
//!
//! Terminal box-drawing glyphs rasterize inconsistently across fonts, so video
//! export draws them as SVG line primitives instead of emitting the character
//! and hoping the renderer has it. Kept in its own module because it is a pure,
//! self-contained lookup with no dependency on export state.

/// Render a single box-drawing character as SVG path/line elements.
/// Returns Some(svg_fragment) if the character is handled, None otherwise.
pub(crate) fn box_drawing_to_svg(
    ch: char,
    px: u32,
    py: u32,
    cw: u32,
    ch_h: u32,
    color: &str,
) -> Option<String> {
    let cx = px + cw / 2;
    let cy = py + ch_h / 2;
    let b = py + ch_h;
    let right = px + cw;

    // Line thickness
    let t = 1.5_f64;
    let t2 = 2.5_f64; // thick/double

    // Helper: horizontal and vertical line segments
    // For each box-drawing char, we draw lines from center to edges
    // L=left, R=right, U=up, D=down
    let (left, right_seg, up, down, thick) = match ch {
        // Light lines
        '─' => (true, true, false, false, false),
        '│' => (false, false, true, true, false),
        '┌' => (false, true, false, true, false),
        '┐' => (true, false, false, true, false),
        '└' => (false, true, true, false, false),
        '┘' => (true, false, true, false, false),
        '├' => (false, true, true, true, false),
        '┤' => (true, false, true, true, false),
        '┬' => (true, true, false, true, false),
        '┴' => (true, true, true, false, false),
        '┼' => (true, true, true, true, false),
        // Rounded corners - quarter-circle arcs connecting to adjacent ─ and │ cells
        // Uses SVG arc (A) for perfect quarter circles
        // Each corner draws: straight segment → arc → straight segment
        '╭' => {
            // Top-left: goes right and down
            let r = cw.min(ch_h) / 2;
            return Some(format!(
                r#"<path d="M {right},{cy} L {arcx},{cy} A {r},{r} 0 0 0 {cx},{arcy} L {cx},{b}" fill="none" stroke="{color}" stroke-width="{t}" stroke-linecap="round"/>"#,
                right = right,
                cy = cy,
                arcx = cx + r,
                r = r,
                cx = cx,
                arcy = cy + r,
                b = b,
                color = color,
                t = t
            ));
        }
        '╮' => {
            // Top-right: goes left and down
            let r = cw.min(ch_h) / 2;
            return Some(format!(
                r#"<path d="M {px},{cy} L {arcx},{cy} A {r},{r} 0 0 1 {cx},{arcy} L {cx},{b}" fill="none" stroke="{color}" stroke-width="{t}" stroke-linecap="round"/>"#,
                px = px,
                cy = cy,
                arcx = cx - r,
                r = r,
                cx = cx,
                arcy = cy + r,
                b = b,
                color = color,
                t = t
            ));
        }
        '╰' => {
            // Bottom-left: goes up and right
            let r = cw.min(ch_h) / 2;
            return Some(format!(
                r#"<path d="M {cx},{py} L {cx},{arcy} A {r},{r} 0 0 0 {arcx},{cy} L {right},{cy}" fill="none" stroke="{color}" stroke-width="{t}" stroke-linecap="round"/>"#,
                cx = cx,
                py = py,
                arcy = cy - r,
                r = r,
                arcx = cx + r,
                cy = cy,
                right = right,
                color = color,
                t = t
            ));
        }
        '╯' => {
            // Bottom-right: goes up and left
            let r = cw.min(ch_h) / 2;
            return Some(format!(
                r#"<path d="M {cx},{py} L {cx},{arcy} A {r},{r} 0 0 1 {arcx},{cy} L {px},{cy}" fill="none" stroke="{color}" stroke-width="{t}" stroke-linecap="round"/>"#,
                cx = cx,
                py = py,
                arcy = cy - r,
                r = r,
                arcx = cx - r,
                cy = cy,
                px = px,
                color = color,
                t = t
            ));
        }
        // Heavy lines
        '━' => (true, true, false, false, true),
        '┃' => (false, false, true, true, true),
        '┏' => (false, true, false, true, true),
        '┓' => (true, false, false, true, true),
        '┗' => (false, true, true, false, true),
        '┛' => (true, false, true, false, true),
        '┣' => (false, true, true, true, true),
        '┫' => (true, false, true, true, true),
        '┳' => (true, true, false, true, true),
        '┻' => (true, true, true, false, true),
        '╋' => (true, true, true, true, true),
        // Double lines
        '═' => {
            let g = 1u32;
            return Some(format!(
                concat!(
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                ),
                px,
                cy - g,
                right,
                cy - g,
                color,
                t,
                px,
                cy + g,
                right,
                cy + g,
                color,
                t,
            ));
        }
        '║' => {
            let g = 1u32;
            return Some(format!(
                concat!(
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                ),
                cx - g,
                py,
                cx - g,
                b,
                color,
                t,
                cx + g,
                py,
                cx + g,
                b,
                color,
                t,
            ));
        }
        // Block elements
        '█' => {
            return Some(format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                px, py, cw, ch_h, color
            ));
        }
        '▀' => {
            return Some(format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                px,
                py,
                cw,
                ch_h / 2,
                color
            ));
        }
        '▄' => {
            return Some(format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                px,
                py + ch_h / 2,
                cw,
                ch_h / 2,
                color
            ));
        }
        '▌' => {
            return Some(format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                px,
                py,
                cw / 2,
                ch_h,
                color
            ));
        }
        '▐' => {
            return Some(format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                px + cw / 2,
                py,
                cw / 2,
                ch_h,
                color
            ));
        }
        '░' | '▒' | '▓' => {
            let opacity = match ch {
                '░' => 0.25,
                '▒' => 0.50,
                '▓' => 0.75,
                _ => 0.5,
            };
            return Some(format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="{}"/>"#,
                px, py, cw, ch_h, color, opacity
            ));
        }
        _ => return None,
    };

    let stroke_w = if thick { t2 } else { t };
    let mut svg = String::new();
    if left {
        svg.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="round"/>"#,
            px, cy, cx, cy, color, stroke_w
        ));
    }
    if right_seg {
        svg.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="round"/>"#,
            cx, cy, right, cy, color, stroke_w
        ));
    }
    if up {
        svg.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="round"/>"#,
            cx, py, cx, cy, color, stroke_w
        ));
    }
    if down {
        svg.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-linecap="round"/>"#,
            cx, cy, cx, b, color, stroke_w
        ));
    }
    Some(svg)
}
