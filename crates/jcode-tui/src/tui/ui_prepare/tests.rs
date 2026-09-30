use super::*;

#[test]
fn centered_mode_centers_unstructured_messages_and_preserves_structured_left_blocks() {
    for role in ["user", "assistant", "meta", "usage", "error", "memory"] {
        assert_eq!(
            default_message_alignment(role, true),
            ratatui::layout::Alignment::Center,
            "role {role} should default to centered alignment"
        );
    }
    for role in ["tool", "system", "swarm", "background_task"] {
        assert_eq!(
            default_message_alignment(role, true),
            ratatui::layout::Alignment::Left,
            "role {role} should keep left/default alignment"
        );
    }
}

#[test]
fn prepare_body_preserves_multiline_user_prompt_lines() {
    let mut lines = Vec::new();
    let mut raw_plain_lines = Vec::new();
    let mut line_raw_overrides = Vec::new();
    let mut line_copy_offsets = Vec::new();
    let mut user_line_indices = Vec::new();

    push_user_prompt_lines(
        &mut lines,
        &mut raw_plain_lines,
        &mut line_raw_overrides,
        &mut line_copy_offsets,
        &mut user_line_indices,
        1,
        user_color(),
        "first line\nsecond line\n\nthird line",
        ratatui::layout::Alignment::Left,
    );

    let plain: Vec<String> = lines.iter().map(ui::line_plain_text).collect();

    assert_eq!(plain.len(), 4);
    assert_eq!(plain[0], "1› first line");
    assert_eq!(plain[1], "   second line");
    assert_eq!(plain[2], "   ");
    assert_eq!(plain[3], "   third line");
    assert_eq!(
        raw_plain_lines,
        vec!["first line", "second line", "", "third line"]
    );
    assert_eq!(user_line_indices, vec![0]);
    assert_eq!(line_copy_offsets, vec![3, 3, 3, 3]);
}

fn assert_wrapped_copy_selection(
    prepared: &PreparedMessages,
    width: u16,
    first: &str,
    last: &str,
    expected: &str,
) {
    ui::record_copy_viewport_snapshot(
        prepared.wrapped_plain_lines.clone(),
        prepared.wrapped_copy_offsets.clone(),
        prepared.raw_plain_lines.clone(),
        prepared.wrapped_line_map.clone(),
        0,
        prepared.wrapped_lines.len(),
        ratatui::layout::Rect::new(0, 0, width, prepared.wrapped_lines.len() as u16),
        &[],
    );
    let point = |needle: &str, after: bool| {
        let (row, text, byte) = prepared
            .wrapped_plain_lines
            .iter()
            .enumerate()
            .find_map(|(row, text)| text.find(needle).map(|byte| (row, text, byte)))
            .unwrap_or_else(|| panic!("missing {needle:?} at width {width}"));
        let byte = byte + if after { needle.len() } else { 0 };
        let column = unicode_width::UnicodeWidthStr::width(&text[..byte]);
        ui::copy_viewport_point_from_screen(column as u16, row as u16)
            .expect("selection endpoint inside viewport")
    };
    let start = point(first, false);
    let end = point(last, true);
    assert!(end.abs_line > start.abs_line, "selection must cross a wrap");
    for (start, end) in [(start, end), (end, start)] {
        assert_eq!(
            ui::copy_selection_text(crate::tui::CopySelectionRange { start, end }).as_deref(),
            Some(expected),
            "logical source selection at width {width}, {start:?}..{end:?}"
        );
    }
    assert_eq!(
        ui::copy_selection_text(crate::tui::CopySelectionRange { start, end: start }),
        Some(String::new()),
        "an empty selection must not copy a repeated prefix"
    );
}

fn assert_markdown_copy_selection(markdown: &str, first: &str, last: &str, expected: &str) {
    for width in [16, 24, 40] {
        let lines = markdown::render_markdown_with_width(markdown, Some(width as usize));
        // Streaming/header and body preparation use different wrapping paths.
        for prepared in [
            wrap_lines(lines.clone(), &[], &[], &[], width),
            wrap_lines_with_map(lines, &[], &[], &[], &[], &[], width, &[], &[], &[]),
        ] {
            assert_wrapped_copy_selection(&prepared, width, first, last, expected);
        }
    }
}

const WRAPPED_COPY_SOURCE: &str = "alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango";
const WRAPPED_COPY_SELECTION: &str =
    "juliet kilo lima mike november oscar papa quebec romeo sierra";

#[test]
fn wrapped_copy_narrow_lists_match_logical_source() {
    for prefix in ["1. ", "- "] {
        assert_markdown_copy_selection(
            &format!("{prefix}{WRAPPED_COPY_SOURCE}"),
            "juliet",
            "sierra",
            WRAPPED_COPY_SELECTION,
        );
    }
}

#[test]
fn wrapped_copy_narrow_quotes_match_logical_source() {
    assert_markdown_copy_selection(
        &format!("> {WRAPPED_COPY_SOURCE}"),
        "juliet",
        "sierra",
        WRAPPED_COPY_SELECTION,
    );
}

#[test]
fn wrapped_copy_nested_prefixes_match_logical_source() {
    for prefix in ["> > 1. ", "- parent\n    - "] {
        assert_markdown_copy_selection(
            &format!("{prefix}{WRAPPED_COPY_SOURCE}"),
            "juliet",
            "sierra",
            WRAPPED_COPY_SELECTION,
        );
    }
}

#[test]
fn wrapped_copy_multiline_selection_matches_logical_source() {
    let second = "uniform victor whiskey xray yankee zulu";
    assert_markdown_copy_selection(
        &format!("1. {WRAPPED_COPY_SOURCE}\n2. {second}"),
        "juliet",
        "xray",
        &format!("{WRAPPED_COPY_SELECTION} tango\n2. uniform victor whiskey xray"),
    );
}

#[test]
fn wrapped_copy_unicode_matches_logical_source() {
    let selected = "東京 🙂 delta 世界 echo 🚀 foxtrot 界🙂";
    assert_markdown_copy_selection(
        &format!("- alpha bravo charlie {selected} golf hotel"),
        "東京",
        "界🙂",
        selected,
    );
}

#[test]
fn wrapped_copy_user_prompt_matches_logical_source() {
    for width in [12, 24, 40] {
        let mut lines = Vec::new();
        let mut raws = Vec::new();
        let mut maps = Vec::new();
        let mut offsets = Vec::new();
        let mut users = Vec::new();
        push_user_prompt_lines(
            &mut lines,
            &mut raws,
            &mut maps,
            &mut offsets,
            &mut users,
            1,
            user_color(),
            &format!("{WRAPPED_COPY_SOURCE}\nuniform victor whiskey xray yankee zulu"),
            ratatui::layout::Alignment::Left,
        );
        let prepared = wrap_lines_with_map(
            lines,
            &raws,
            &maps,
            &offsets,
            &users,
            &[],
            width,
            &[],
            &[],
            &[],
        );
        assert_wrapped_copy_selection(
            &prepared,
            width,
            "juliet",
            "xray",
            &format!("{WRAPPED_COPY_SELECTION} tango\nuniform victor whiskey xray"),
        );
    }
}

/// Regression coverage for issue #344: loading older compacted history above
/// an unchanged tail must be detected as a suffix match so scrolling to the
/// start of a long session reuses the prepared tail instead of re-rendering
/// the whole transcript per chunk.
#[test]
fn matching_suffix_len_detects_prepended_history() {
    use jcode_tui_messages::MessageBoundary;

    let old: Vec<DisplayMessage> = (0..4)
        .map(|i| DisplayMessage::system(format!("msg {i}")))
        .collect();

    // Base prepared from the old transcript: boundaries in transcript order.
    let base = PreparedMessages {
        wrapped_lines: Vec::new(),
        wrapped_plain_lines: Arc::new(Vec::new()),
        wrapped_copy_offsets: Arc::new(Vec::new()),
        raw_plain_lines: Arc::new(Vec::new()),
        wrapped_line_map: Arc::new(Vec::new()),
        wrapped_user_indices: Vec::new(),
        wrapped_user_prompt_starts: Vec::new(),
        wrapped_user_prompt_ends: Vec::new(),
        user_prompt_texts: Vec::new(),
        image_regions: Vec::new(),
        edit_tool_ranges: Vec::new(),
        copy_targets: Vec::new(),
        message_boundaries: old
            .iter()
            .map(|m| MessageBoundary {
                msg_hash: m.stable_cache_hash(),
                wrapped_len: 0,
                raw_len: 0,
                user_prompt_len: 0,
            })
            .collect(),
        mermaid_pending_epoch: None,
    };

    // New transcript: two older-history messages prepended, tail unchanged.
    let mut new_msgs: Vec<DisplayMessage> = vec![
        DisplayMessage::system("older history a"),
        DisplayMessage::system("older history b"),
    ];
    new_msgs.extend(old.iter().cloned());
    assert_eq!(matching_suffix_len(&base, &new_msgs), 4);

    // Changed tail: no suffix reuse.
    let mut changed = new_msgs.clone();
    changed.last_mut().unwrap().content = "edited".to_string();
    assert_eq!(matching_suffix_len(&base, &changed), 0);

    // Identical transcript: full suffix match.
    assert_eq!(matching_suffix_len(&base, &old), 4);
}
