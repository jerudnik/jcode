use super::*;

#[test]
fn test_truncate_line_preserves_width_for_ascii() {
    let line = Line::from(Span::raw("hello world foo bar"));
    let truncated = truncate_line_to_width(&line, 11);
    assert_eq!(truncated.width(), 11);
}

// ---- Mermaid side panel rendering tests ----

const TEST_FONT: Option<(u16, u16)> = Some((8, 16));

#[test]
fn test_vcenter_fitted_image_wide_image_in_narrow_pane() {
    // Wide image (800x200) in a narrow side panel (40 cols x 30 rows).
    // The image width should be the constraining dimension, so the
    // fitted image should fill the panel width.
    let area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 30,
    };
    let result = vcenter_fitted_image_with_font(area, 800, 200, TEST_FONT);
    assert!(
        result.width >= area.width / 2,
        "wide image should fill most of pane width: got {} out of {} (expected >= {})",
        result.width,
        area.width,
        area.width / 2
    );
}

#[test]
fn test_vcenter_fitted_image_square_image_fills_width() {
    // Square image (400x400) in a side panel (40 cols x 40 rows).
    // With typical 8x16 font, terminal cells are 2:1 aspect.
    // 40 cols = 320px, 40 rows = 640px.
    // scale = min(320/400, 640/400) = min(0.8, 1.6) = 0.8
    // fitted_w = (400 * 0.8) / 8 = 40 cells -> fills width
    let area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 40,
    };
    let result = vcenter_fitted_image_with_font(area, 400, 400, TEST_FONT);
    assert!(
        result.width >= area.width * 3 / 4,
        "square image should fill most of pane width: got {} out of {}",
        result.width,
        area.width
    );
}

#[test]
fn test_vcenter_fitted_image_tall_image_in_wide_pane() {
    // Tall image (200x800) in a wide pane (80 cols x 30 rows).
    // Height is constraining. Image won't fill width.
    let area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 30,
    };
    let result = vcenter_fitted_image_with_font(area, 200, 800, TEST_FONT);
    assert!(
        result.width < area.width,
        "tall image should not fill full width: got {} out of {}",
        result.width,
        area.width
    );
    assert!(
        result.height <= area.height,
        "tall image height should not exceed pane: got {} out of {}",
        result.height,
        area.height
    );
}

#[test]
fn test_vcenter_fitted_image_centering_horizontal() {
    // Tall image centered in a wide area - should have x_offset > 0
    let area = Rect {
        x: 10,
        y: 5,
        width: 80,
        height: 20,
    };
    let result = vcenter_fitted_image_with_font(area, 100, 800, TEST_FONT);
    if result.width < area.width {
        assert!(
            result.x > area.x,
            "should be horizontally centered: x={}, area.x={}",
            result.x,
            area.x
        );
    }
}

#[test]
fn test_vcenter_fitted_image_zero_dimensions() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };
    let result = vcenter_fitted_image_with_font(area, 400, 400, TEST_FONT);
    assert_eq!(result, area);

    let area2 = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 30,
    };
    let result2 = vcenter_fitted_image_with_font(area2, 0, 0, TEST_FONT);
    assert_eq!(result2, area2);
}

#[test]
fn test_vcenter_fitted_image_never_exceeds_area() {
    let test_cases: Vec<(Rect, u32, u32)> = vec![
        (
            Rect {
                x: 0,
                y: 0,
                width: 40,
                height: 30,
            },
            800,
            600,
        ),
        (
            Rect {
                x: 5,
                y: 3,
                width: 60,
                height: 20,
            },
            100,
            100,
        ),
        (
            Rect {
                x: 0,
                y: 0,
                width: 120,
                height: 40,
            },
            1920,
            1080,
        ),
        (
            Rect {
                x: 0,
                y: 0,
                width: 30,
                height: 50,
            },
            200,
            800,
        ),
    ];
    for (area, img_w, img_h) in test_cases {
        let result = vcenter_fitted_image_with_font(area, img_w, img_h, TEST_FONT);
        assert!(
            result.x >= area.x,
            "result.x ({}) < area.x ({})",
            result.x,
            area.x
        );
        assert!(
            result.y >= area.y,
            "result.y ({}) < area.y ({})",
            result.y,
            area.y
        );
        assert!(
            result.x + result.width <= area.x + area.width,
            "result right edge ({}) > area right edge ({})",
            result.x + result.width,
            area.x + area.width
        );
        assert!(
            result.y + result.height <= area.y + area.height,
            "result bottom edge ({}) > area bottom edge ({})",
            result.y + result.height,
            area.y + area.height
        );
    }
}

#[test]
fn test_estimate_pinned_diagram_pane_width_tall_image() {
    // A tall image should get a narrower pane (height-constrained)
    let diagram = info_widget::DiagramInfo {
        hash: 11,
        width: 200,
        height: 1600,
        label: None,
    };
    let width = estimate_pinned_diagram_pane_width_with_font(&diagram, 30, 24, Some((8, 16)));
    // Height-constrained: 30 rows - 2 border = 28 inner rows
    // image_w_cells = ceil(200/8) = 25
    // image_h_cells = ceil(1600/16) = 100
    // fit_w_cells = ceil(25*28/100) = 7
    // pane_width = 7 + 2 = 9, but clamped to min 24
    assert_eq!(width, 24, "tall image should be clamped to minimum width");
}

#[test]
fn test_estimate_pinned_diagram_pane_width_zero_font_size() {
    // With None font size, should use default (8, 16)
    let diagram = info_widget::DiagramInfo {
        hash: 12,
        width: 800,
        height: 600,
        label: None,
    };
    let with_font = estimate_pinned_diagram_pane_width_with_font(&diagram, 20, 24, Some((8, 16)));
    let with_default = estimate_pinned_diagram_pane_width_with_font(&diagram, 20, 24, None);
    assert_eq!(with_font, with_default);
}

#[test]
fn test_estimate_pinned_diagram_pane_height_tall_image() {
    // Tall image (200x1600) in a pane 80 cols wide.
    // Width-constrained, so height depends on the width scaling.
    let diagram = info_widget::DiagramInfo {
        hash: 14,
        width: 200,
        height: 1600,
        label: None,
    };
    let height = estimate_pinned_diagram_pane_height(&diagram, 80, 6);
    assert!(
        height > 6,
        "tall image should need more than minimum height: got {}",
        height
    );
}

#[test]
fn test_is_diagram_poor_fit_wide_in_side_pane() {
    // A very wide diagram in a side pane (narrow+tall) should be a poor fit
    let diagram = info_widget::DiagramInfo {
        hash: 40,
        width: 1600,
        height: 100,
        label: None,
    };
    let area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 40,
    };
    let poor = is_diagram_poor_fit(&diagram, area, crate::config::DiagramPanePosition::Side);
    assert!(
        poor,
        "very wide diagram in narrow side pane should be poor fit"
    );
}

#[test]
fn test_is_diagram_poor_fit_good_fit_cases() {
    // Normal aspect ratio diagrams should not be poor fits
    let diagram = info_widget::DiagramInfo {
        hash: 42,
        width: 600,
        height: 400,
        label: None,
    };
    let side_area = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 40,
    };
    assert!(
        !is_diagram_poor_fit(
            &diagram,
            side_area,
            crate::config::DiagramPanePosition::Side
        ),
        "normal diagram should not be poor fit in side pane"
    );

    let top_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    assert!(
        !is_diagram_poor_fit(&diagram, top_area, crate::config::DiagramPanePosition::Top),
        "normal diagram should not be poor fit in top pane"
    );
}

#[test]
fn test_is_diagram_poor_fit_zero_dimensions() {
    let diagram = info_widget::DiagramInfo {
        hash: 43,
        width: 0,
        height: 0,
        label: None,
    };
    let area = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 40,
    };
    assert!(
        !is_diagram_poor_fit(&diagram, area, crate::config::DiagramPanePosition::Side),
        "zero-dimension diagram should not crash or be poor fit"
    );
}

#[test]
fn test_is_diagram_poor_fit_tiny_area() {
    let diagram = info_widget::DiagramInfo {
        hash: 44,
        width: 800,
        height: 600,
        label: None,
    };
    let area = Rect {
        x: 0,
        y: 0,
        width: 3,
        height: 2,
    };
    assert!(
        !is_diagram_poor_fit(&diagram, area, crate::config::DiagramPanePosition::Side),
        "tiny area should return false (not crash)"
    );
}

#[test]
fn test_div_ceil_u32_basic() {
    assert_eq!(div_ceil_u32(10, 3), 4);
    assert_eq!(div_ceil_u32(9, 3), 3);
    assert_eq!(div_ceil_u32(0, 5), 0);
    assert_eq!(div_ceil_u32(1, 1), 1);
    assert_eq!(div_ceil_u32(7, 0), 7);
}

#[test]
fn test_estimate_pinned_diagram_pane_width_various_fonts() {
    // Different font sizes affect the computed pane width.
    // With a proportionally larger font, the raw image-in-cells count
    // is smaller, but ceiling arithmetic can add a cell back.
    let diagram = info_widget::DiagramInfo {
        hash: 50,
        width: 800,
        height: 600,
        label: None,
    };
    let w_8x16 = estimate_pinned_diagram_pane_width_with_font(&diagram, 30, 24, Some((8, 16)));
    let w_10x20 = estimate_pinned_diagram_pane_width_with_font(&diagram, 30, 24, Some((10, 20)));
    let w_16x32 = estimate_pinned_diagram_pane_width_with_font(&diagram, 30, 24, Some((16, 32)));
    // With a substantially larger font, we should need noticeably fewer cells
    assert!(
        w_16x32 <= w_8x16,
        "much larger font should need fewer or equal cells: 16x32={}, 8x16={}",
        w_16x32,
        w_8x16
    );
    // All should respect the minimum
    assert!(w_8x16 >= 24);
    assert!(w_10x20 >= 24);
    assert!(w_16x32 >= 24);
}

