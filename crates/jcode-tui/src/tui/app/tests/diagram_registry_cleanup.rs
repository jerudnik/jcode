/// ACTIVE_DIAGRAMS cap behavior: registering a 129th diagram evicts the
/// oldest. If the evicted diagram is the one currently parked in the pinned
/// pane, the pinned selection index silently lands on a different diagram (no
/// crash, no reset: the count stays at the cap so `normalize_diagram_state`
/// never clamps).
#[test]
fn test_active_diagrams_cap_eviction_swaps_currently_shown_diagram() {
    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();
    app.diagram_mode = crate::config::DiagramDisplayMode::Pinned;
    app.diagram_pane_enabled = true;

    crate::tui::mermaid::clear_active_diagrams();
    for i in 1..=128u64 {
        crate::tui::mermaid::register_active_diagram(i, 100, 80, None);
    }
    assert_eq!(crate::tui::mermaid::active_diagram_count(), 128);

    // Park on the OLDEST diagram (hash 1, last position in newest-first
    // order).
    app.diagram_index = 127;
    app.sync_diagram_fit_context();
    assert_eq!(app.last_visible_diagram_hash, Some(1));

    // The 129th diagram evicts hash 1 (the one being shown).
    crate::tui::mermaid::register_active_diagram(129, 100, 80, None);
    let diagrams = crate::tui::mermaid::get_active_diagrams();
    assert_eq!(diagrams.len(), 128, "cap holds at ACTIVE_DIAGRAMS_MAX");
    assert!(
        !diagrams.iter().any(|d| d.hash == 1),
        "the shown diagram was evicted from the registry"
    );

    // Count stayed at the cap, so index 127 is still in range: no clamp, no
    // reset, the pane just shows a different diagram.
    app.normalize_diagram_state();
    assert_eq!(app.diagram_index, 127);
    assert_eq!(
        app.last_visible_diagram_hash,
        Some(2),
        "claim 5 CONFIRMED: eviction silently swaps the shown diagram (1 -> 2)"
    );

    crate::tui::mermaid::clear_active_diagrams();
}

/// The Margin info widget copies the ACTIVE_DIAGRAMS registry (newest-first)
/// registry ONLY in Margin mode (tui_state.rs:1456-1460); Pinned mode (which
/// uses the dedicated pane) gets an empty list.
#[test]
fn test_info_widget_diagram_list_populated_only_in_margin_mode() {
    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();

    crate::tui::mermaid::clear_active_diagrams();
    crate::tui::mermaid::register_active_diagram(0xA1, 100, 80, None);
    crate::tui::mermaid::register_active_diagram(0xA2, 120, 90, None);

    app.diagram_mode = crate::config::DiagramDisplayMode::Margin;
    let margin_data = crate::tui::TuiState::info_widget_data(&app);
    assert_eq!(
        margin_data
            .diagrams
            .iter()
            .map(|d| d.hash)
            .collect::<Vec<_>>(),
        vec![0xA2, 0xA1],
        "Margin mode copies the registry (newest-first) into the info widget"
    );

    app.diagram_mode = crate::config::DiagramDisplayMode::Pinned;
    let pinned_data = crate::tui::TuiState::info_widget_data(&app);
    assert!(
        pinned_data.diagrams.is_empty(),
        "Pinned mode must NOT feed the margin info widget (dedicated pane instead)"
    );

    crate::tui::mermaid::clear_active_diagrams();
}

/// Margin-mode selection semantics: there is no per-diagram selection at all.
/// `diagram_available()` requires Pinned mode (navigation.rs:336-340), so
/// Ctrl+arrow cycling is unreachable; `normalize_diagram_state` force-resets
/// `diagram_index` to 0 in any non-Pinned mode (navigation.rs:342-349); and
/// the margin widget always renders `diagrams[0]` (info_widget.rs:1361). So
/// after the list changes, the "selection" is always the newest diagram:
/// a stale index can never be pointed at a stale entry in Margin mode.
#[test]
fn test_margin_mode_has_no_diagram_selection_and_always_shows_newest() {
    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();
    app.diagram_mode = crate::config::DiagramDisplayMode::Margin;
    app.diagram_pane_enabled = true;

    crate::tui::mermaid::clear_active_diagrams();
    crate::tui::mermaid::register_active_diagram(0xB1, 100, 80, None);
    crate::tui::mermaid::register_active_diagram(0xB2, 100, 80, None);
    crate::tui::mermaid::register_active_diagram(0xB3, 100, 80, None);

    // Cycling is unreachable: diagram_available() is Pinned-only, so the
    // Ctrl-key handler refuses the cycle keys even with diagrams present.
    assert!(
        !app.diagram_available(),
        "Margin mode reports no cyclable diagram pane"
    );
    app.diagram_focus = true; // even with focus somehow set
    assert!(
        !app.handle_diagram_ctrl_key(KeyCode::Left, app.diagram_available()),
        "Ctrl+Left does not cycle in Margin mode"
    );
    assert!(
        !app.handle_diagram_ctrl_key(KeyCode::Right, app.diagram_available()),
        "Ctrl+Right does not cycle in Margin mode"
    );

    // A parked/stale index from a previous Pinned session is force-reset by
    // normalize_diagram_state's non-Pinned branch, so it can never select a
    // stale entry after the list changes.
    app.diagram_index = 2;
    app.diagram_scroll_x = 5;
    app.diagram_scroll_y = 7;
    app.normalize_diagram_state();
    assert_eq!(
        app.diagram_index, 0,
        "non-Pinned normalize resets the index"
    );
    assert!(
        !app.diagram_focus,
        "non-Pinned normalize drops diagram focus"
    );
    assert_eq!(app.diagram_scroll_x, 0);
    assert_eq!(app.diagram_scroll_y, 0);
    assert_eq!(
        app.last_visible_diagram_hash, None,
        "no visible-diagram anchor is tracked in Margin mode"
    );

    // The widget input is newest-first, and the margin renderer draws only
    // element 0, so a new registration immediately becomes the shown diagram.
    let before = crate::tui::TuiState::info_widget_data(&app).diagrams;
    assert_eq!(before[0].hash, 0xB3, "newest diagram is the rendered one");
    crate::tui::mermaid::register_active_diagram(0xB4, 100, 80, None);
    let after = crate::tui::TuiState::info_widget_data(&app).diagrams;
    assert_eq!(
        after.iter().map(|d| d.hash).collect::<Vec<_>>(),
        vec![0xB4, 0xB3, 0xB2, 0xB1],
        "stale entries stay listed behind the newest one"
    );
    assert_eq!(
        after[0].hash, 0xB4,
        "the margin widget switches to the new diagram (index 0) with no \
         selection to go stale"
    );

    crate::tui::mermaid::clear_active_diagrams();
}

/// set_streaming_preview_diagram on a complete fenced block).
fn seed_streaming_preview(app: &mut App, hash: u64) {
    crate::tui::mermaid::clear_active_diagrams();
    app.streaming.streaming_text = "```mermaid\ngraph TD; A-->B\n```".to_string();
    app.is_processing = true;
    crate::tui::mermaid::set_streaming_preview_diagram(hash, 320, 240, Some("preview".to_string()));
    assert_eq!(
        crate::tui::mermaid::get_active_diagrams()
            .first()
            .map(|d| d.hash),
        Some(hash),
        "seed: streaming preview occupies index 0 (what Margin mode draws)"
    );
}

fn assert_streaming_preview_cleared(hash: u64, path: &str) {
    assert!(
        !crate::tui::mermaid::get_active_diagrams()
            .iter()
            .any(|d| d.hash == hash),
        "{path}: streaming preview diagram must not survive the transcript mutation"
    );
}

/// Local `/clear` -> reset_current_session (commands_review.rs) now clears
/// the streaming render state, including the preview slot.
#[test]
fn test_local_clear_command_clears_streaming_preview_diagram() {
    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();
    let hash: u64 = 0x0005_17EA_11ED_0001;
    seed_streaming_preview(&mut app, hash);

    assert!(super::commands::handle_session_command(&mut app, "/clear"));

    assert_streaming_preview_cleared(hash, "local /clear");
    assert!(
        app.streaming.streaming_text.is_empty(),
        "local /clear: in-flight streaming text is dropped with the transcript"
    );
    crate::tui::mermaid::clear_active_diagrams();
}

/// Local `/rewind N` and `/rewind undo` (commands.rs) rebuild the transcript;
/// both must drop the streaming preview slot.
#[test]
fn test_local_rewind_and_undo_clear_streaming_preview_diagram() {
    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();

    app.session.replace_messages(Vec::new());
    for idx in 1..=2 {
        let text = format!("msg-{idx}");
        app.add_provider_message(Message::user(&text));
        app.session.add_message(
            Role::User,
            vec![ContentBlock::Text {
                text,
                cache_control: None,
            }],
        );
    }

    let hash: u64 = 0x0005_17EA_11ED_0002;
    seed_streaming_preview(&mut app, hash);
    assert!(super::commands::handle_session_command(
        &mut app,
        "/rewind 1"
    ));
    assert_streaming_preview_cleared(hash, "local /rewind N");

    seed_streaming_preview(&mut app, hash);
    assert!(super::commands::handle_session_command(
        &mut app,
        "/rewind undo"
    ));
    assert_streaming_preview_cleared(hash, "local /rewind undo");
    crate::tui::mermaid::clear_active_diagrams();
}

/// Ctrl+R recovery (recover_session_without_tools, conversation_state.rs) is
/// reachable mid-stream from the turn.rs key loops with a live preview and no
/// prior commit, so it must clear the preview slot itself.
#[test]
fn test_recover_session_without_tools_clears_streaming_preview_diagram() {
    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();
    let hash: u64 = 0x0005_17EA_11ED_0003;
    seed_streaming_preview(&mut app, hash);

    app.recover_session_without_tools();

    assert_streaming_preview_cleared(hash, "local Ctrl+R recovery");
    assert!(
        app.streaming.streaming_text.is_empty(),
        "recovery: in-flight streaming text is dropped with the transcript"
    );
    crate::tui::mermaid::clear_active_diagrams();
}

/// `commit_pending_streaming_assistant_message` early-returns when the live
/// buffer is empty (tool-only boundary). The buffer can become empty *after*
/// a preview was rendered only via `replace_streaming_text` (remote
/// TextReplace, server_events.rs:644, and debug snapshot restore,
/// debug.rs:539), which does not touch the preview slot. The commit boundary
/// is the mirror point: an empty buffer means any surviving preview is stale,
/// so the early return must clear the slot instead of leaking it
/// (input.rs commit_pending_streaming_assistant_message).
#[test]
fn test_commit_with_emptied_stream_buffer_clears_streaming_preview_diagram() {
    let _render_lock = scroll_render_test_lock();
    let mut app = create_test_app();
    let hash: u64 = 0x0005_17EA_11ED_0004;
    seed_streaming_preview(&mut app, hash);

    // Simulate a TextReplace-style rewrite that drops the fenced block the
    // preview was rendered from, leaving the buffer empty while the preview
    // slot is still occupied.
    app.replace_streaming_text(String::new());
    assert_eq!(
        crate::tui::mermaid::get_active_diagrams()
            .first()
            .map(|d| d.hash),
        Some(hash),
        "precondition: replace_streaming_text alone leaves the preview live"
    );

    let committed = app.commit_pending_streaming_assistant_message();

    assert!(!committed, "empty buffer commits nothing");
    assert_streaming_preview_cleared(hash, "commit with emptied stream buffer");
    crate::tui::mermaid::clear_active_diagrams();
}
