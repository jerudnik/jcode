use super::*;

pub(super) fn paste_from_clipboard(app: &mut App) {
    app.set_status_notice("Reading clipboard...");
    spawn_clipboard_paste(app, ClipboardPasteKind::Smart);
}

pub(super) fn is_clipboard_paste_shortcut(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::Char('v' | 'V'))
        && modifiers.intersects(
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::META,
        )
}

fn active_clipboard_session_id(app: &App) -> String {
    app.active_client_session_id()
        .unwrap_or(app.session.id.as_str())
        .to_string()
}

fn publish_clipboard_result(
    session_id: String,
    kind: ClipboardPasteKind,
    content: ClipboardPasteContent,
) {
    Bus::global().publish(BusEvent::ClipboardPasteCompleted(ClipboardPasteCompleted {
        session_id,
        kind,
        content,
    }));
}

fn spawn_clipboard_paste(app: &App, kind: ClipboardPasteKind) {
    let session_id = active_clipboard_session_id(app);
    let task_kind = kind.clone();
    spawn_blocking_or_thread(move || {
        let content = read_clipboard_for_paste(&task_kind);
        publish_clipboard_result(session_id, task_kind, content);
    });
}

fn spawn_blocking_or_thread<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::spawn_blocking(task);
    } else {
        std::thread::spawn(task);
    }
}

fn read_clipboard_text() -> Option<String> {
    if std::env::var("WAYLAND_DISPLAY").is_ok()
        && let Some(text) = read_wayland_clipboard_text()
    {
        return Some(text);
    }

    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return None;
    };
    clipboard.get_text().ok()
}

fn read_wayland_clipboard_text() -> Option<String> {
    let types_output = std::process::Command::new("wl-paste")
        .arg("--list-types")
        .output()
        .ok()?;
    if !types_output.status.success() {
        return None;
    }

    let types = String::from_utf8_lossy(&types_output.stdout);
    let wl_type = preferred_wayland_text_type(&types)?;
    let output = std::process::Command::new("wl-paste")
        .args(["--type", wl_type, "--no-newline"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

fn preferred_wayland_text_type(types: &str) -> Option<&'static str> {
    let has_type = |needle: &str| types.lines().any(|line| line.trim() == needle);
    if has_type("text/plain;charset=utf-8") {
        Some("text/plain;charset=utf-8")
    } else if has_type("text/plain") {
        Some("text/plain")
    } else if has_type("UTF8_STRING") {
        Some("UTF8_STRING")
    } else if has_type("TEXT") {
        Some("TEXT")
    } else if has_type("STRING") {
        Some("STRING")
    } else {
        None
    }
}

fn image_content(media_type: String, base64_data: String) -> ClipboardPasteContent {
    ClipboardPasteContent::Image {
        media_type,
        base64_data,
    }
}

fn download_image_url_content(url: &str) -> Option<ClipboardPasteContent> {
    crate::tui::app::helpers::download_image_url(url)
        .map(|(media_type, base64_data)| image_content(media_type, base64_data))
}

fn read_clipboard_for_paste(kind: &ClipboardPasteKind) -> ClipboardPasteContent {
    read_clipboard_for_paste_with(
        kind,
        read_clipboard_text,
        crate::tui::app::helpers::clipboard_image,
        download_image_url_content,
    )
}

fn read_clipboard_for_paste_with<ReadText, ReadImage, DownloadImageUrl>(
    kind: &ClipboardPasteKind,
    mut read_text: ReadText,
    mut read_image: ReadImage,
    mut download_image_url: DownloadImageUrl,
) -> ClipboardPasteContent
where
    ReadText: FnMut() -> Option<String>,
    ReadImage: FnMut() -> Option<(String, String)>,
    DownloadImageUrl: FnMut(&str) -> Option<ClipboardPasteContent>,
{
    match kind {
        ClipboardPasteKind::Smart => {
            // Only treat the clipboard as text when it has *non-empty* text.
            // Image-only clipboards (especially on Wayland/arboard) frequently
            // expose an empty text target, which previously short-circuited the
            // image path and produced a silent "0 char" paste.
            if let Some(text) = read_text().filter(|t| !t.trim().is_empty()) {
                if let Some(url) = crate::tui::app::helpers::extract_image_url(&text)
                    && let Some(content) = download_image_url(&url)
                {
                    return content;
                }
                return ClipboardPasteContent::Text(text);
            }
            if let Some((media_type, base64_data)) = read_image() {
                return image_content(media_type, base64_data);
            }
            ClipboardPasteContent::Empty
        }
        ClipboardPasteKind::ImageOnly => {
            if let Some((media_type, base64_data)) = read_image() {
                return image_content(media_type, base64_data);
            }
            if let Some(text) = read_text() {
                if let Some(url) = crate::tui::app::helpers::extract_image_url(&text) {
                    return download_image_url(&url).unwrap_or_else(|| {
                        ClipboardPasteContent::Error("Failed to download image".to_string())
                    });
                }
                return ClipboardPasteContent::Text(text);
            }
            ClipboardPasteContent::Empty
        }
        ClipboardPasteKind::ImageUrl { fallback_text } => {
            let Some(url) = fallback_text
                .as_deref()
                .and_then(crate::tui::app::helpers::extract_image_url)
            else {
                return ClipboardPasteContent::Empty;
            };
            download_image_url(&url).unwrap_or_else(|| {
                ClipboardPasteContent::Error("Failed to download image".to_string())
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClipboardPasteContent, ClipboardPasteKind, dropped_image_files,
        is_clipboard_paste_shortcut, parse_dropped_paths, preferred_wayland_text_type,
        read_clipboard_for_paste_with, shifted_printable_fallback, text_input_for_key,
    };
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn dropped_paths_accept_quotes_shell_escapes_and_file_urls() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first image.png");
        let second = dir.path().join("second.jpg");
        std::fs::write(&first, b"png").unwrap();
        std::fs::write(&second, b"jpeg").unwrap();

        let quoted = parse_dropped_paths(&format!("'{}'", first.display())).unwrap();
        assert_eq!(quoted, vec![first.clone()]);
        let escaped =
            parse_dropped_paths(&first.display().to_string().replace(' ', "\\ ")).unwrap();
        assert_eq!(escaped, vec![first.clone()]);
        let url = url::Url::from_file_path(&second).unwrap();
        assert_eq!(parse_dropped_paths(url.as_str()).unwrap(), vec![second]);
    }

    #[test]
    fn dropped_images_load_all_supported_files_and_reject_mixed_text() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.png");
        let jpeg = dir.path().join("b.jpeg");
        std::fs::write(&png, b"png bytes").unwrap();
        std::fs::write(&jpeg, b"jpeg bytes").unwrap();

        let images =
            dropped_image_files(&format!("'{}' '{}'", png.display(), jpeg.display())).unwrap();
        assert_eq!(images[0], ("image/png".to_string(), b"png bytes".to_vec()));
        assert_eq!(
            images[1],
            ("image/jpeg".to_string(), b"jpeg bytes".to_vec())
        );
        assert!(dropped_image_files("ordinary pasted text").is_none());
    }

    #[test]
    fn smart_paste_prefers_normal_text_when_clipboard_has_text() {
        let content = read_clipboard_for_paste_with(
            &ClipboardPasteKind::Smart,
            || Some("plain text".to_string()),
            || Some(("image/png".to_string(), "base64".to_string())),
            |_| None,
        );

        match content {
            ClipboardPasteContent::Text(text) => assert_eq!(text, "plain text"),
            other => panic!("expected text paste, got {other:?}"),
        }
    }

    #[test]
    fn smart_paste_uses_image_when_text_is_absent_or_blank() {
        // Image-only clipboards either report no text target at all or
        // advertise a blank one; both must paste the image rather than
        // producing a silent empty text paste.
        for (case, text) in [
            ("no text target", None),
            ("blank text target", Some("   ".to_string())),
        ] {
            let content = read_clipboard_for_paste_with(
                &ClipboardPasteKind::Smart,
                || text.clone(),
                || Some(("image/png".to_string(), "base64".to_string())),
                |_| None,
            );

            match content {
                ClipboardPasteContent::Image {
                    media_type,
                    base64_data,
                } => {
                    assert_eq!(media_type, "image/png", "{case}: media type");
                    assert_eq!(base64_data, "base64", "{case}: payload");
                }
                other => panic!("{case}: expected image paste, got {other:?}"),
            }
        }
    }

    #[test]
    fn smart_paste_empty_clipboard_stays_empty_not_dictation() {
        let content =
            read_clipboard_for_paste_with(&ClipboardPasteKind::Smart, || None, || None, |_| None);

        assert!(
            matches!(content, ClipboardPasteContent::Empty),
            "expected empty paste, got {content:?}"
        );
    }

    #[test]
    fn paste_shortcut_accepts_control_alt_command_and_meta_v() {
        for modifiers in [
            KeyModifiers::CONTROL,
            KeyModifiers::ALT,
            KeyModifiers::SUPER,
            KeyModifiers::META,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyModifiers::ALT | KeyModifiers::SHIFT,
            KeyModifiers::SUPER | KeyModifiers::SHIFT,
        ] {
            assert!(
                is_clipboard_paste_shortcut(KeyCode::Char('v'), modifiers),
                "{modifiers:?}+v should paste clipboard contents"
            );
            assert!(
                is_clipboard_paste_shortcut(KeyCode::Char('V'), modifiers),
                "{modifiers:?}+V should paste clipboard contents"
            );
        }

        assert!(!is_clipboard_paste_shortcut(
            KeyCode::Char('v'),
            KeyModifiers::empty()
        ));
    }

    #[test]
    fn wayland_text_type_prefers_utf8_plain_text() {
        let types = "text/plain\ntext/plain;charset=utf-8\nTEXT\nSTRING\nUTF8_STRING\n";

        assert_eq!(
            preferred_wayland_text_type(types),
            Some("text/plain;charset=utf-8")
        );
    }

    #[test]
    fn shifted_printable_fallback_uppercases_ascii_letters() {
        assert_eq!(shifted_printable_fallback('a', KeyModifiers::SHIFT), 'A');
        assert_eq!(shifted_printable_fallback('z', KeyModifiers::SHIFT), 'Z');
    }

    #[test]
    fn shifted_printable_fallback_preserves_terminal_translated_symbols() {
        assert_eq!(shifted_printable_fallback('/', KeyModifiers::SHIFT), '/');
        assert_eq!(shifted_printable_fallback('?', KeyModifiers::SHIFT), '?');
        assert_eq!(shifted_printable_fallback('(', KeyModifiers::SHIFT), '(');
        assert_eq!(shifted_printable_fallback('&', KeyModifiers::SHIFT), '&');
    }

    #[test]
    fn shifted_printable_fallback_does_not_synthesize_us_symbol_layout() {
        assert_eq!(shifted_printable_fallback('7', KeyModifiers::SHIFT), '7');
        assert_eq!(shifted_printable_fallback('8', KeyModifiers::SHIFT), '8');
        assert_eq!(shifted_printable_fallback('=', KeyModifiers::SHIFT), '=');
    }

    #[test]
    fn text_input_for_shifted_symbols_preserves_layout_translated_char() {
        for c in ['/', '?', '(', ')', '&', '=', '"'] {
            assert_eq!(
                text_input_for_key(KeyCode::Char(c), KeyModifiers::SHIFT),
                Some(c.to_string()),
                "shifted {c:?} should be treated as terminal/layout-translated text"
            );
        }
    }

    #[test]
    fn text_input_for_altgr_symbols_preserves_layout_translated_char() {
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;

        for c in ['@', '{', '}', '\\', '€', 'ą'] {
            assert_eq!(
                text_input_for_key(KeyCode::Char(c), altgr),
                Some(c.to_string()),
                "AltGr-style {c:?} should be treated as terminal/layout-translated text"
            );
        }
    }

    #[test]
    fn text_input_for_control_shortcut_letters_stays_non_text() {
        assert_eq!(
            text_input_for_key(
                KeyCode::Char('q'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            ),
            None
        );
        assert_eq!(
            text_input_for_key(KeyCode::Char('@'), KeyModifiers::CONTROL),
            None
        );
    }
}

pub(in crate::tui::app) fn cut_input_line_to_clipboard(app: &mut App) -> bool {
    cut_input_line_to_clipboard_with(app, crate::tui::app::helpers::copy_to_clipboard)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::tui::app) fn cut_input_line_to_clipboard_with<F>(
    app: &mut App,
    mut copy_text: F,
) -> bool
where
    F: FnMut(&str) -> bool,
{
    if app.input.is_empty() {
        return false;
    }

    if !copy_text(&app.input) {
        app.set_status_notice("Failed to copy input line");
        return false;
    }

    app.remember_input_undo_state();
    app.input.clear();
    app.cursor_pos = 0;
    app.reset_tab_completion();
    app.sync_model_picker_preview_from_input();
    app.set_status_notice("✂ Cut input line");
    true
}

pub(in crate::tui::app) fn handle_paste(app: &mut App, text: String) {
    // Note: clipboard_image() is NOT checked here. Bracketed paste events from the
    // terminal always deliver text. Checking clipboard_image() here caused a bug where
    // text pastes were misidentified as images when the clipboard also had image data
    // (common on Wayland where apps advertise multiple MIME types). Image pasting is
    // handled by explicit clipboard shortcuts instead (Ctrl+V/Alt+V/Cmd+V smart-paste).
    if let Some(paths) = parse_dropped_paths(&text) {
        let item_count = paths.len();
        let mut image_count = 0;
        let mut file_count = 0;

        for (index, path) in paths.into_iter().enumerate() {
            if index > 0 {
                insert_input_text(app, " ");
            }

            if let Some(media_type) = image_media_type(&path)
                && let Ok(data) = std::fs::read(&path)
            {
                attach_image(
                    app,
                    media_type.to_string(),
                    base64::engine::general_purpose::STANDARD.encode(data),
                );
                image_count += 1;
            } else {
                insert_input_text(app, &format_dropped_path(&path, item_count > 1));
                file_count += 1;
            }
        }

        let notice = match (image_count, file_count) {
            (images, 0) => format!(
                "Dropped {images} image{}",
                if images == 1 { "" } else { "s" }
            ),
            (0, files) => format!("Dropped {files} file{}", if files == 1 { "" } else { "s" }),
            (images, files) => format!(
                "Dropped {images} image{} and {files} file{}",
                if images == 1 { "" } else { "s" },
                if files == 1 { "" } else { "s" }
            ),
        };
        app.set_status_notice(notice);
    } else if let Some(url) = crate::tui::app::helpers::extract_image_url(&text) {
        crate::logging::info(&format!("Downloading image from pasted URL: {}", url));
        app.set_status_notice("Downloading image...");
        let session_id = active_clipboard_session_id(app);
        spawn_blocking_or_thread(move || {
            let content = download_image_url_content(&url).unwrap_or_else(|| {
                ClipboardPasteContent::Error("Failed to download image".to_string())
            });
            publish_clipboard_result(
                session_id,
                ClipboardPasteKind::ImageUrl {
                    fallback_text: Some(text),
                },
                content,
            );
        });
    } else {
        handle_text_paste(app, text);
    }
}

fn format_dropped_path(path: &std::path::Path, quote_whitespace: bool) -> String {
    let value = path.to_string_lossy();
    if quote_whitespace && value.chars().any(char::is_whitespace) {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.into_owned()
    }
}

fn dropped_image_files(text: &str) -> Option<Vec<(String, Vec<u8>)>> {
    let paths = parse_dropped_paths(text)?;
    paths
        .into_iter()
        .map(|path| {
            let media_type = image_media_type(&path)?;
            let data = std::fs::read(path).ok()?;
            Some((media_type.to_string(), data))
        })
        .collect()
}

/// Terminal emulators normally send file drops as bracketed paste, but some send
/// the path as ordinary key events. Promote a complete image-path-only composer
/// value before command/skill routing so an absolute `/...` path is never treated
/// as a slash command.
pub(in crate::tui::app) fn promote_dropped_images(app: &mut App) -> bool {
    let Some(images) = dropped_image_files(&app.input) else {
        return false;
    };
    let count = images.len();
    app.input.clear();
    app.cursor_pos = 0;
    for (media_type, data) in images {
        attach_image(
            app,
            media_type,
            base64::engine::general_purpose::STANDARD.encode(data),
        );
    }
    app.set_status_notice(format!(
        "Dropped {count} image{}",
        if count == 1 { "" } else { "s" }
    ));
    true
}

pub(super) fn parse_dropped_paths(text: &str) -> Option<Vec<PathBuf>> {
    let trimmed = text.trim();
    let literal_path = PathBuf::from(trimmed);
    if literal_path.is_file() {
        return Some(vec![literal_path]);
    }

    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in trimmed.chars() {
        if escaped {
            token.push(ch);
            escaped = false;
        } else if ch == '\\' && quote != Some('\'') {
            escaped = true;
        } else if matches!(ch, '\'' | '"') {
            if quote == Some(ch) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(ch);
            } else {
                token.push(ch);
            }
        } else if ch.is_whitespace() && quote.is_none() {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
        } else {
            token.push(ch);
        }
    }
    if escaped || quote.is_some() {
        return None;
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    if tokens.is_empty() {
        return None;
    }

    tokens
        .into_iter()
        .map(|token| {
            let path = if token.starts_with("file://") {
                url::Url::parse(&token).ok()?.to_file_path().ok()?
            } else {
                PathBuf::from(token)
            };
            path.is_file().then_some(path)
        })
        .collect()
}

fn image_media_type(path: &std::path::Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "tif" | "tiff" => Some("image/tiff"),
        _ => None,
    }
}

pub(in crate::tui::app) fn handle_text_paste(app: &mut App, text: String) {
    crate::logging::info(&format!(
        "Text paste: {} chars, {} lines",
        text.len(),
        text.lines().count()
    ));

    let line_count = text.lines().count().max(1);
    if line_count < 5 {
        insert_input_text(app, &text);
    } else {
        let placeholder = paste_placeholder(&text);
        app.pasted_contents.push(text);
        insert_input_text(app, &placeholder);
    }
}
