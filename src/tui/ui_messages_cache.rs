use super::*;

pub(super) use jcode_tui_messages::{centered_wrap_width, left_pad_lines_for_centered_mode};

/// Compute the per-render isolation fingerprint used by the static
/// `MESSAGE_CACHE` to keep one TUI process's rendered Line-vecs scoped to
/// the (session, workspace) they were rendered for. This is render-only, so
/// only session_id + workspace_root + SCHEMA_VERSION are folded in
/// (trust_tier / provider / model are intentionally omitted — see the
/// MESSAGE_CACHE plan in TASK-89).
fn render_isolation_fp(app: &dyn TuiState) -> u64 {
    use jcode_cache_isolation::{IsolationKey, TrustTier};
    let workspace = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let session_id = app.current_session_id().unwrap_or_default();
    IsolationKey::new(
        session_id,
        &workspace,
        "", // provider: intentionally unset for render cache
        "", // model: intentionally unset for render cache
        0,  // content_hash: not used; we want context_fingerprint only
        TrustTier::Trusted,
    )
    .context_fingerprint()
}

pub(crate) fn get_cached_message_lines<F>(
    app: &dyn TuiState,
    msg: &DisplayMessage,
    width: u16,
    diff_mode: crate::config::DiffDisplayMode,
    render: F,
) -> Vec<Line<'static>>
where
    F: FnOnce(&DisplayMessage, u16, crate::config::DiffDisplayMode) -> Vec<Line<'static>>,
{
    jcode_tui_messages::get_cached_message_lines(
        msg,
        width,
        diff_mode,
        jcode_tui_messages::MessageCacheContext {
            diagram_mode: crate::config::config().display.diagram_mode,
            centered: markdown::center_code_blocks(),
            mermaid_epoch: crate::tui::mermaid::deferred_render_epoch(),
            mermaid_aspect_bucket: crate::tui::mermaid::current_preferred_aspect_ratio_bucket(),
            isolation_fp: render_isolation_fp(app),
        },
        render,
    )
}
