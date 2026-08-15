//! Tests for `ui_inline_image`.

use super::*;

fn item(width: u32, height: u32) -> InlineImageItem {
    InlineImageItem {
        id: 0xABCD,
        width,
        height,
        label: "test.png".to_string(),
    }
}

/// 1x1 transparent PNG used by the materialize tests below.
const MATERIALIZE_PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

#[test]
fn materialize_visible_probe_is_cheap_after_first_call() {
    let id = mermaid::inline_image_id("image/png", MATERIALIZE_PNG_B64);
    register_payload(id, "image/png", MATERIALIZE_PNG_B64);
    assert!(materialize_visible(id), "first call decodes and caches");
    // Steady state: the in-memory probe alone must report ready, without
    // needing the payload registry at all.
    assert!(
        mermaid::inline_image_is_materialized(id),
        "presence probe should hit after materialization"
    );
    assert!(materialize_visible(id), "repeat call stays true");
}

#[test]
fn prefetch_is_noop_for_materialized_image_without_kitty() {
    // Without a Kitty picker the fit-state path is Unsupported, so a
    // materialized image has nothing to prewarm: prefetch must be a cheap
    // no-op (no panic, no scheduling) and the image stays materialized.
    let id = mermaid::inline_image_id("image/png", MATERIALIZE_PNG_B64);
    register_payload(id, "image/png", MATERIALIZE_PNG_B64);
    assert!(materialize_visible(id));
    prefetch(id, 80, 10);
    assert!(
        mermaid::inline_image_is_materialized(id),
        "prefetch must not disturb already-materialized state"
    );
}

#[test]
fn materialized_image_draws_now_on_non_kitty_protocols() {
    // On a non-Kitty protocol the stable-fit path reports Unsupported. A
    // materialized image must still draw, so the fallback renderers can
    // run; only an undecoded one is worth prewarming.
    assert_eq!(
        draw_action(true, mermaid::InlineFitReadiness::Unsupported),
        DrawAction::DrawNow,
        "materialized image must be drawable on non-Kitty protocols"
    );
    assert_eq!(
        draw_action(false, mermaid::InlineFitReadiness::Unsupported),
        DrawAction::Prewarm
    );
    assert_eq!(
        draw_action(true, mermaid::InlineFitReadiness::Ready),
        DrawAction::DrawNow
    );
    assert_eq!(
        draw_action(true, mermaid::InlineFitReadiness::NeedsPrewarm),
        DrawAction::Prewarm,
        "a decoded image whose Kitty fit state is stale must be re-prewarmed"
    );
    assert_eq!(
        draw_action(false, mermaid::InlineFitReadiness::NeedsPrewarm),
        DrawAction::Prewarm
    );
}

#[test]
fn fit_rows_caps_tall_image_to_viewport_fraction() {
    // A very tall image must be capped so it cannot bury the transcript.
    let rows = fit_rows(100, 100_000, 80, 40);
    let cap = ((40u32 * MAX_VIEWPORT_FRACTION_PERCENT as u32) / 100) as u16;
    assert!(rows <= cap, "rows {rows} should be <= cap {cap}");
    assert!(rows >= MIN_IMAGE_ROWS);
}

#[test]
fn fit_rows_never_below_minimum() {
    let rows = fit_rows(10, 10, 80, 40);
    assert!(rows >= MIN_IMAGE_ROWS);
}

#[test]
fn fit_geometry_height_bound_image_narrows_proportionally() {
    // Tall image hits the viewport cap; the recorded cols must shrink with
    // it so the border/label hug the actual rendered picture.
    let (rows, cols) = fit_geometry(1000, 4000, 100, 40);
    let cap = ((40u32 * MAX_VIEWPORT_FRACTION_PERCENT as u32) / 100) as u16;
    assert!(rows <= cap);
    // Width-bound it would be ~100 cols; height-bound it must be far less.
    assert!(cols < 50, "height-bound image should be narrow, got {cols}");
    assert!(cols > 2, "image must occupy some columns, got {cols}");
}

#[test]
fn fit_geometry_small_window_never_exceeds_chat_width() {
    for chat_width in [1u16, 2, 3, 5, 10] {
        for viewport_height in [1u16, 2, 5, 10] {
            let (rows, cols) = fit_geometry(1920, 1080, chat_width, viewport_height);
            assert!(
                cols <= chat_width.max(2),
                "cols {cols} > width {chat_width}"
            );
            assert!(rows >= MIN_IMAGE_ROWS);
        }
    }
}

#[test]
fn fit_geometry_zero_dims_safe() {
    let (rows, cols) = fit_geometry(0, 0, 80, 40);
    assert!(rows >= MIN_IMAGE_ROWS);
    assert!(cols <= 80);
}

#[test]
fn build_section_records_region_width() {
    let items = vec![item(600, 400)];
    let section = build_section(&items, 80, 40, false, true, &AllFit);
    let region = &section.image_regions[0];
    assert!(
        region.width > 2,
        "region width should include the image, got {}",
        region.width
    );
    assert!(region.width <= 80);
}

#[test]
fn build_section_emits_one_fit_region_per_image_with_label() {
    let items = vec![item(600, 400), item(800, 600)];
    let section = build_section(&items, 80, 40, true, true, &AllFit);
    assert_eq!(section.image_regions.len(), 2);
    for region in &section.image_regions {
        assert_eq!(region.render, ImageRegionRender::Fit);
        assert_eq!(region.hash, 0xABCD);
        // The region must point at blank placeholder lines, never the label.
        let first = &section.wrapped_lines[region.abs_line_idx];
        assert!(
            jcode_tui_render::line_plain_text(first).trim().is_empty(),
            "region should start on a blank placeholder line"
        );
        // Region height must match its line span.
        assert_eq!(
            region.end_line - region.abs_line_idx,
            region.height as usize
        );
    }
    // A dim label line precedes the first region.
    let label_line = jcode_tui_render::line_plain_text(&section.wrapped_lines[1]);
    assert!(
        label_line.contains("test.png"),
        "label missing: {label_line:?}"
    );
}

#[test]
fn build_section_is_empty_for_no_items() {
    let section = build_section(&[], 80, 40, false, true, &AllFit);
    assert!(section.wrapped_lines.is_empty());
    assert!(section.image_regions.is_empty());
}

#[test]
fn build_section_hidden_collapses_to_label_stub_with_show_badge() {
    let items = vec![item(600, 400)];
    let section = build_section(&items, 80, 40, false, false, &AllFit);
    assert!(
        section.image_regions.is_empty(),
        "hidden images must not emit drawable regions"
    );
    let text: String = section
        .wrapped_lines
        .iter()
        .map(jcode_tui_render::line_plain_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("test.png"), "label should remain: {text:?}");
    assert!(
        text.contains("show image"),
        "show badge should render: {text:?}"
    );
}

#[test]
fn visible_label_line_advertises_hide_badge() {
    let line = image_label_line(&item(600, 400), 80, true, ImageExpandLevel::Fit);
    let text = jcode_tui_render::line_plain_text(&line);
    assert!(text.contains("[⇧] [I]"), "badge keys missing: {text:?}");
    assert!(text.contains("hide"), "hide hint missing: {text:?}");
}

#[test]
fn generated_image_label_is_truncated_to_one_terminal_row() {
    let mut generated = item(1536, 864);
    generated.label = "/home/jeremy/jcode/.jcode/generated-images/1783707839145-ig_0c5e377de871aba5016a51388779588196944d98eff2c7aa30.png".to_string();

    let line = image_label_line(&generated, 52, true, ImageExpandLevel::Fit);
    let text = jcode_tui_render::line_plain_text(&line);

    assert!(line.width() <= 52, "label exceeded chat width: {text:?}");
    assert!(
        text.contains('…'),
        "truncated label needs an ellipsis: {text:?}"
    );
    assert!(
        text.ends_with("hide"),
        "toggle suffix must remain visible: {text:?}"
    );
    assert!(
        !text.contains('\n'),
        "label must remain one logical line: {text:?}"
    );
}

#[test]
fn expand_level_cycle_visits_every_level_and_wraps() {
    assert_eq!(ImageExpandLevel::Fit.next(), ImageExpandLevel::Large);
    assert_eq!(ImageExpandLevel::Large.next(), ImageExpandLevel::Full);
    assert_eq!(ImageExpandLevel::Full.next(), ImageExpandLevel::Fit);
}

#[test]
fn expand_level_caps_grow_monotonically() {
    assert!(
        ImageExpandLevel::Fit.anchored_cap_rows() < ImageExpandLevel::Large.anchored_cap_rows()
    );
    assert!(
        ImageExpandLevel::Large.anchored_cap_rows() < ImageExpandLevel::Full.anchored_cap_rows()
    );
    // Full must stay under kitty's virtual-placement row limit (296) so
    // stable fit rendering keeps working at every level.
    assert!(ImageExpandLevel::Full.anchored_cap_rows() < 296);
}

#[test]
fn visible_label_line_stays_single_purpose_without_expand_badge() {
    // The label must stay a short single line: no expand badge, no dots.
    let line = image_label_line(&item(600, 400), 80, true, ImageExpandLevel::Fit);
    let text = jcode_tui_render::line_plain_text(&line);
    assert!(
        !text.contains("expand") && !text.contains('○') && !text.contains('●'),
        "label line must not carry an expand badge: {text:?}"
    );
}

#[test]
fn hidden_label_line_omits_expand_badge() {
    let line = image_label_line(&item(600, 400), 80, false, ImageExpandLevel::Fit);
    let text = jcode_tui_render::line_plain_text(&line);
    assert!(text.contains("show image"), "show badge missing: {text:?}");
    assert!(
        !text.contains("expand"),
        "hidden image must hide expand badge: {text:?}"
    );
}

#[test]
fn expanded_level_makes_anchored_image_taller() {
    let fit = fit_geometry_anchored(1000, 4000, 100, ImageExpandLevel::Fit).0;
    let large = fit_geometry_anchored(1000, 4000, 100, ImageExpandLevel::Large).0;
    let full = fit_geometry_anchored(1000, 4000, 100, ImageExpandLevel::Full).0;
    assert!(large > fit, "Large ({large}) should exceed Fit ({fit})");
    assert!(full > large, "Full ({full}) should exceed Large ({large})");
}

#[test]
fn anchored_image_lines_hidden_emit_no_placeholder_markers() {
    let items = vec![item(600, 400)];
    let lines = anchored_image_lines(&items, 80, false, &AllFit);
    assert!(
        lines
            .iter()
            .filter_map(mermaid::parse_inline_image_placeholder)
            .next()
            .is_none(),
        "hidden images must not emit geometry markers"
    );
    let text: String = lines
        .iter()
        .map(jcode_tui_render::line_plain_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("show image"), "show badge missing: {text:?}");
}

/// The registry holds full base64 payloads, so it must be bounded by bytes
/// as well as entry count, and eviction must keep the byte accounting and
/// the map/order queue in sync. A single over-budget payload must survive
/// (or its image could never materialize).
#[test]
fn payload_registry_evicts_by_byte_budget() {
    let mut reg = PayloadRegistry::new();
    // Payloads sized so ~5 of them exceed the byte budget long before the
    // 512-entry count bound.
    let payload = "x".repeat(PAYLOAD_REGISTRY_MAX_BYTES / 4);
    for id in 0..8u64 {
        reg.insert(id, "image/png", &payload);
        assert!(
            reg.total_bytes <= PAYLOAD_REGISTRY_MAX_BYTES || reg.order.len() == 1,
            "byte budget exceeded with {} entries / {} bytes",
            reg.order.len(),
            reg.total_bytes
        );
        assert_eq!(reg.map.len(), reg.order.len(), "map/order desynced");
    }
    // Newest payload always survives.
    assert!(reg.get(7).is_some(), "newest payload must not be evicted");
    // Oldest payloads were evicted to make room.
    assert!(reg.get(0).is_none(), "oldest payload should be evicted");
    // A single payload larger than the whole budget still gets stored.
    let mut solo = PayloadRegistry::new();
    let huge = "y".repeat(PAYLOAD_REGISTRY_MAX_BYTES + 1);
    solo.insert(99, "image/png", &huge);
    assert!(
        solo.get(99).is_some(),
        "an over-budget payload must stay resident so its image can draw"
    );
}

/// Re-registering a payload must clear the prewarm failure memo so a fresh
/// payload gets its decode retries back.
#[test]
fn reregistering_payload_resets_prewarm_failures() {
    const ID: u64 = 0xFA11_ED01;
    for _ in 0..PREWARM_FAILURE_MAX_ATTEMPTS {
        record_prewarm_failure(ID);
    }
    assert!(
        prewarm_failures_exhausted(ID),
        "failure memo should suspend prewarm after max attempts"
    );
    register_payload(ID, "image/png", "BBBB");
    assert!(
        !prewarm_failures_exhausted(ID),
        "fresh payload registration must reset the failure memo"
    );
}

/// Materialization must release the staged base64 payload (the decoded
/// bytes are persisted in the render cache + cache dir), and later
/// re-registrations for a materialized image must be no-ops so the payload
/// is never staged twice. Draws must keep working afterwards.
#[test]
fn materialize_releases_payload_and_blocks_restaging() {
    // Distinct payload so this test's id cannot collide with others.
    let id = mermaid::inline_image_id("image/png", MATERIALIZE_PNG_B64_RELEASE);
    register_payload(id, "image/png", MATERIALIZE_PNG_B64_RELEASE);
    assert!(
        PAYLOAD_REGISTRY.lock().unwrap().get(id).is_some(),
        "payload staged before materialization"
    );
    assert!(materialize_visible(id), "materialization succeeds");
    assert!(
        PAYLOAD_REGISTRY.lock().unwrap().get(id).is_none(),
        "payload must be released after materialization"
    );
    // Prepare passes keep calling register_payload; it must stay empty.
    register_payload(id, "image/png", MATERIALIZE_PNG_B64_RELEASE);
    assert!(
        PAYLOAD_REGISTRY.lock().unwrap().get(id).is_none(),
        "re-registering a materialized image must not restage its payload"
    );
    // And the image must still be drawable without the payload.
    assert!(
        materialize_visible(id),
        "materialized image stays visible after payload release"
    );
}

/// 1x1 red PNG distinct from `MATERIALIZE_PNG_B64` so payload-release
/// tests own their image id.
const MATERIALIZE_PNG_B64_RELEASE: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

/// PayloadRegistry::remove must keep byte accounting and the eviction
/// queue in sync.
#[test]
fn payload_registry_remove_keeps_accounting_consistent() {
    let mut reg = PayloadRegistry::new();
    reg.insert(1, "image/png", "AAAA");
    reg.insert(2, "image/png", "BBBBBBBB");
    let bytes_with_both = reg.total_bytes;
    reg.remove(1);
    assert!(reg.get(1).is_none());
    assert_eq!(reg.map.len(), reg.order.len(), "map/order desynced");
    assert_eq!(
        reg.total_bytes,
        bytes_with_both - ("image/png".len() + "AAAA".len()),
        "byte accounting must shrink by exactly the removed entry"
    );
    // Removing an absent id is a no-op.
    reg.remove(42);
    assert_eq!(reg.map.len(), 1);
}

/// 1x1 transparent PNG, used to exercise the real header parse.
const TINY_PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

fn rendered_image(
    anchor: Option<crate::session::RenderedImageAnchor>,
) -> crate::session::RenderedImage {
    crate::session::RenderedImage {
        media_type: "image/png".to_string(),
        data: TINY_PNG_B64.to_string(),
        label: Some("tiny.png".to_string()),
        source: crate::session::RenderedImageSource::ToolResult {
            tool_name: "read".to_string(),
        },
        anchor,
    }
}

#[test]
fn resolve_anchored_items_buckets_by_anchor() {
    let images = vec![
        rendered_image(Some(crate::session::RenderedImageAnchor::ToolCall {
            id: "tool-1".to_string(),
        })),
        rendered_image(Some(crate::session::RenderedImageAnchor::UserPrompt {
            ordinal: 2,
        })),
        rendered_image(None),
    ];
    let anchored = resolve_anchored_items(&images);
    assert!(anchored.has_anchored());
    assert_eq!(anchored.by_tool.get("tool-1").map(Vec::len), Some(1));
    assert_eq!(anchored.by_prompt.get(&2).map(Vec::len), Some(1));
    assert_eq!(anchored.unanchored.len(), 1);
}

#[test]
fn unplaced_items_falls_back_for_missing_anchor_targets() {
    use jcode_tui_messages::DisplayMessage;

    let images = vec![
        rendered_image(Some(crate::session::RenderedImageAnchor::ToolCall {
            id: "tool-present".to_string(),
        })),
        rendered_image(Some(crate::session::RenderedImageAnchor::ToolCall {
            id: "tool-missing".to_string(),
        })),
        rendered_image(Some(crate::session::RenderedImageAnchor::UserPrompt {
            ordinal: 0,
        })),
        rendered_image(Some(crate::session::RenderedImageAnchor::UserPrompt {
            ordinal: 5,
        })),
        rendered_image(None),
    ];
    let anchored = resolve_anchored_items(&images);

    let tool_call = crate::message::ToolCall {
        id: "tool-present".to_string(),
        name: "read".to_string(),
        input: serde_json::Value::Null,
        intent: None,
        thought_signature: None,
    };
    let messages = vec![
        DisplayMessage::user("show me"),
        DisplayMessage::tool("output", tool_call),
    ];

    let unplaced = anchored.unplaced_items(&messages);
    // tool-missing (1) + prompt ordinal 5 (1) + unanchored (1) = 3.
    // tool-present and prompt 0 are placed in the body, not here.
    assert_eq!(unplaced.len(), 3);
}

#[test]
fn anchored_image_lines_round_trip_through_region_scan() {
    let items = vec![item(600, 400)];
    let lines = anchored_image_lines(&items, 80, true, &AllFit);
    // Find the marker line and verify its geometry parse.
    let parsed: Vec<(u64, u16, u16)> = lines
        .iter()
        .filter_map(mermaid::parse_inline_image_placeholder)
        .collect();
    assert_eq!(parsed.len(), 1);
    let (hash, rows, cols) = parsed[0];
    assert_eq!(hash, 0xABCD);
    let (expected_rows, expected_cols) = fit_geometry_anchored(600, 400, 80, ImageExpandLevel::Fit);
    assert_eq!(rows, expected_rows);
    assert_eq!(cols, expected_cols);
    // Marker line is followed by rows-1 blank placeholder lines.
    let marker_idx = lines
        .iter()
        .position(|line| mermaid::parse_inline_image_placeholder(line).is_some())
        .unwrap();
    for offset in 1..rows as usize {
        let line = &lines[marker_idx + offset];
        assert!(
            jcode_tui_render::line_plain_text(line).trim().is_empty(),
            "placeholder row {offset} should be blank"
        );
    }
}

#[test]
fn anchored_geometry_is_viewport_independent() {
    // The anchored fit must not depend on any viewport height so the body
    // cache (keyed by width only) stays valid across resizes.
    let (rows, cols) = fit_geometry_anchored(1920, 1080, 100, ImageExpandLevel::Fit);
    assert!(rows >= MIN_IMAGE_ROWS);
    assert!(rows <= ANCHORED_MAX_ROWS);
    assert!(cols <= 100);
}
