/// Clicking anywhere on the image body (its placeholder rows) must cycle the
/// expand level, exactly like the label badge. Clicks in the blank area to
/// the RIGHT of a narrow image must not.
#[test]
fn test_click_on_inline_image_body_cycles_level() {
    use crate::tui::ui::inline_image_ui::{
        AllFit, ImageExpandLevel, InlineImageItem, build_section,
    };
    use jcode_tui_messages::PreparedChatFrame;

    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();

    const IMAGE_ID: u64 = 0xBEEF;
    let chat_width: u16 = 80;

    let items = vec![InlineImageItem {
        id: IMAGE_ID,
        width: 320,
        height: 200,
        label: "shot.png".to_string(),
    }];
    let section = build_section(&items, chat_width, 40, false, true, &AllFit);
    let region = *section
        .image_regions
        .iter()
        .find(|r| r.hash == IMAGE_ID)
        .expect("section should carry the image region");
    assert!(region.width > 0, "fit regions record their rendered width");
    assert!(
        region.width < chat_width,
        "test image must be narrower than the chat so the right side is blank"
    );

    let prepared =
        std::sync::Arc::new(PreparedChatFrame::from_single(std::sync::Arc::new(section)));
    let visible_end = prepared.wrapped_plain_line_count();
    let content_area = Rect::new(0, 0, chat_width, visible_end as u16 + 1);

    crate::tui::ui::clear_copy_viewport_snapshot();
    crate::tui::ui::record_copy_viewport_frame_snapshot_for_test(
        prepared,
        0,
        visible_end,
        content_area,
        &vec![0u16; visible_end],
    );

    assert_eq!(app.image_expand_level(IMAGE_ID), ImageExpandLevel::Fit);

    // Click in the middle of the image body (a placeholder row, inside the
    // rendered width). Down then Up, like a real terminal click.
    let body_row = content_area.y + region.abs_line_idx as u16 + 1;
    let body_col = content_area.x + region.width / 2;
    let click = |app: &mut App, col: u16, row: u16| {
        app.handle_mouse_event(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        });
        app.handle_mouse_event(MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        });
    };
    click(&mut app, body_col, body_row);
    assert_eq!(
        app.image_expand_level(IMAGE_ID),
        ImageExpandLevel::Large,
        "clicking the image body should expand Fit -> Large"
    );

    // Clicking the body again advances the cycle.
    click(&mut app, body_col, body_row);
    assert_eq!(
        app.image_expand_level(IMAGE_ID),
        ImageExpandLevel::Full,
        "second body click should expand Large -> Full"
    );
    click(&mut app, body_col, body_row);
    assert_eq!(
        app.image_expand_level(IMAGE_ID),
        ImageExpandLevel::Fit,
        "third body click should wrap Full -> Fit"
    );

    // A click in the blank space to the right of the image must stay inert.
    let far_right = content_area.x + chat_width - 2;
    assert!(far_right > content_area.x + region.width);
    click(&mut app, far_right, body_row);
    assert_eq!(
        app.image_expand_level(IMAGE_ID),
        ImageExpandLevel::Fit,
        "clicking blank space beside the image must not cycle it"
    );
}
