/// End-to-end: a swarm notification carrying a sender-provided tldr renders
/// collapsed (tldr + `▸ expand` badge, body hidden) through a REAL draw, and a
/// left click on the badge expands it in place (body visible, `▾ collapse`
/// badge). Exercises the full path: collapsible encoding -> body render ->
/// live copy-viewport snapshot -> `swarm_expand_target_from_screen` ->
/// `toggle_swarm_message_expand`.
#[test]
fn test_click_on_swarm_expand_badge_toggles_tldr_collapse() {
    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();

    let body = "The flaky test was caused by a race in the setup helper. \
                I rewrote it to use a barrier and verified 200 consecutive runs pass.";
    let content =
        jcode_tui_messages::encode_collapsible_swarm_content("fixed the flaky test", body);
    app.display_messages = vec![
        DisplayMessage::user("hi"),
        DisplayMessage::swarm("DM from sheep", content),
    ];
    app.bump_display_messages_version();
    app.scroll_offset = 0;
    app.auto_scroll_paused = false;
    app.is_processing = false;
    app.status = ProcessingStatus::Idle;
    app.session.short_name = Some("test".to_string());

    let backend = ratatui::backend::TestBackend::new(90, 30);
    let mut terminal = ratatui::Terminal::new(backend).expect("failed to create test terminal");

    let collapsed = render_and_snap(&app, &mut terminal);
    assert!(
        collapsed.contains("fixed the flaky test"),
        "collapsed card must show the tldr:\n{collapsed}"
    );
    assert!(
        collapsed.contains("▸ expand"),
        "collapsed card must show the expand badge:\n{collapsed}"
    );
    assert!(
        !collapsed.contains("race in the setup helper"),
        "collapsed card must hide the body:\n{collapsed}"
    );

    // Locate the badge in the real frame buffer and click its first cell.
    let buf = terminal.backend().buffer();
    let area = *buf.area();
    let mut badge: Option<(u16, u16)> = None;
    'rows: for row in 0..area.height {
        let mut line = String::new();
        for col in 0..area.width {
            line.push_str(buf[(col, row)].symbol());
        }
        if let Some(byte) = line.find("▸ expand") {
            let col = line[..byte].chars().count() as u16;
            badge = Some((col, row));
            break 'rows;
        }
    }
    let (badge_col, badge_row) = badge.expect("expand badge must be visible in the frame");

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

    // A click on the tldr text (left of the badge) must NOT toggle.
    click(&mut app, badge_col.saturating_sub(6), badge_row);
    assert!(
        jcode_tui_messages::parse_collapsible_swarm_content(&app.display_messages[1].content)
            .is_some_and(|parsed| !parsed.expanded),
        "click left of the badge must not expand the card"
    );

    click(&mut app, badge_col + 2, badge_row);
    let parsed =
        jcode_tui_messages::parse_collapsible_swarm_content(&app.display_messages[1].content)
            .expect("content stays collapsible after toggle");
    assert!(parsed.expanded, "badge click must expand the card");
    assert_eq!(
        app.status_notice(),
        Some("Swarm message expanded".to_string())
    );

    let expanded = render_and_snap(&app, &mut terminal);
    assert!(
        expanded.contains("race in the setup helper"),
        "expanded card must show the body:\n{expanded}"
    );
    assert!(
        expanded.contains("▾ collapse"),
        "expanded card must show the collapse badge:\n{expanded}"
    );

    // Click the collapse badge to fold it back down.
    let buf = terminal.backend().buffer();
    let area = *buf.area();
    let mut collapse_badge: Option<(u16, u16)> = None;
    'rows2: for row in 0..area.height {
        let mut line = String::new();
        for col in 0..area.width {
            line.push_str(buf[(col, row)].symbol());
        }
        if let Some(byte) = line.find("▾ collapse") {
            let col = line[..byte].chars().count() as u16;
            collapse_badge = Some((col, row));
            break 'rows2;
        }
    }
    let (collapse_col, collapse_row) =
        collapse_badge.expect("collapse badge must be visible in the frame");
    click(&mut app, collapse_col + 2, collapse_row);
    assert!(
        jcode_tui_messages::parse_collapsible_swarm_content(&app.display_messages[1].content)
            .is_some_and(|parsed| !parsed.expanded),
        "collapse badge click must fold the card back down"
    );
}

#[cfg(test)]
#[path = "../../tests_input_scroll.rs"]
mod input_scroll_tests;
