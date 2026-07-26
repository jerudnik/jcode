//! Flicker detection: samples every finalized frame, recognizes layout and
//! visible-content oscillation patterns, and surfaces a copyable UI notice.
//!
//! Split out of `ui_frame_metrics.rs`, which retains the frame perf stats,
//! slow-frame history, draw-call attribution, and host-pressure sampling this
//! module feeds on. The flicker history is process-global in production and
//! per-thread under test; see `flicker_frame_history` for the full rationale.

use super::super::*;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;
#[cfg(not(test))]
use std::sync::OnceLock;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FlickerFrameSample {
    pub timestamp_ms: u64,
    pub session_id: Option<String>,
    pub session_name: Option<String>,
    pub display_messages_version: u64,
    pub diff_mode: String,
    pub centered: bool,
    pub is_processing: bool,
    pub auto_scroll_paused: bool,
    pub scroll: usize,
    pub visible_end: usize,
    pub visible_lines: usize,
    pub total_wrapped_lines: usize,
    pub prompt_preview_lines: u16,
    pub messages_area_width: u16,
    pub messages_area_height: u16,
    pub content_width: u16,
    pub chat_scrollbar_visible: bool,
    pub visible_hash: u64,
    pub visible_streaming_hash: u64,
    pub visible_batch_progress_hash: u64,
    pub total_ms: f64,
    pub prepare_ms: f64,
    pub draw_ms: f64,
}

#[derive(Clone, Debug, Serialize)]
struct FlickerEvent {
    pub timestamp_ms: u64,
    kind: String,
    pub session_id: Option<String>,
    pub session_name: Option<String>,
    previous: FlickerFrameSample,
    current: FlickerFrameSample,
}

#[derive(Clone, Debug)]
pub(crate) struct FlickerUiNotice {
    pub(crate) summary: String,
    pub(crate) hint: String,
}

// Keep this outside h/j/k/l for the same reason as COPY_BADGE_KEYS.
pub(crate) const FLICKER_NOTICE_COPY_KEY: char = 'z';
#[derive(Default)]
struct FlickerFrameHistory {
    samples: VecDeque<FlickerFrameSample>,
    events: VecDeque<FlickerEvent>,
    last_log_at_ms: Option<u64>,
}
const FLICKER_HISTORY_MAX_SAMPLES: usize = 256;
const FLICKER_HISTORY_MAX_EVENTS: usize = 128;
const FLICKER_LOG_INTERVAL_MS: u64 = 500;
#[cfg(not(test))]
const FLICKER_UI_NOTICE_MAX_AGE_MS: u64 = 30_000;
// Only the production accessor reads this; under `cfg(test)` the history is
// per-thread (see `flicker_frame_history`).
#[cfg(not(test))]
static FLICKER_FRAME_HISTORY: OnceLock<Mutex<FlickerFrameHistory>> = OnceLock::new();
/// Flicker history, process-global in production and per-thread under test.
///
/// In production this is one history for one TUI, which is what the flicker
/// diagnostics are describing. Under `cargo test` it is shared by every test
/// in the binary, and *every* `ui::draw` records a sample, so a test that
/// clears the history and then asserts on its contents is racing every other
/// rendering test running in parallel. That is not hypothetical: with the full
/// `tui::ui::tests` filter, `test_changelog_overlay_repeated_renders_are_stable`
/// failed ~1 run in 6 with "buffered_samples: 2, expected 3", one sample short
/// because a sibling test cleared the history mid-assertion.
///
/// Cargo gives each test its own thread, so thread-local storage under
/// `cfg(test)` makes the isolation exact rather than cooperative: a test can no
/// longer see or clobber another's samples, and `clear_..._for_tests()` becomes
/// a statement about *this* test only. The `Mutex` is kept in both shapes so
/// callers are identical; under test it is simply never contended.
#[cfg(not(test))]
fn flicker_frame_history() -> &'static Mutex<FlickerFrameHistory> {
    FLICKER_FRAME_HISTORY.get_or_init(|| Mutex::new(FlickerFrameHistory::default()))
}

#[cfg(test)]
fn flicker_frame_history() -> &'static Mutex<FlickerFrameHistory> {
    thread_local! {
        static PER_TEST_HISTORY: &'static Mutex<FlickerFrameHistory> =
            Box::leak(Box::new(Mutex::new(FlickerFrameHistory::default())));
    }
    // Leaked once per test thread, which is bounded by the test count and
    // reclaimed when the process exits. Borrowing from the thread-local
    // directly would not satisfy the `'static` the callers expect.
    PER_TEST_HISTORY.with(|history| *history)
}
fn flicker_detection_enabled() -> bool {
    #[cfg(test)]
    {
        true
    }

    #[cfg(not(test))]
    {
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var("JCODE_TUI_FLICKER_DETECTION")
                .ok()
                .map(|raw| {
                    matches!(
                        raw.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(false)
        })
    }
}
fn same_flicker_state_key(a: &FlickerFrameSample, b: &FlickerFrameSample) -> bool {
    a.session_id == b.session_id
        && a.display_messages_version == b.display_messages_version
        && a.diff_mode == b.diff_mode
        && a.centered == b.centered
        && a.is_processing == b.is_processing
        && a.auto_scroll_paused == b.auto_scroll_paused
        && a.scroll == b.scroll
        && a.visible_end == b.visible_end
        && a.visible_lines == b.visible_lines
        && a.total_wrapped_lines == b.total_wrapped_lines
        && a.prompt_preview_lines == b.prompt_preview_lines
        && a.messages_area_width == b.messages_area_width
        && a.messages_area_height == b.messages_area_height
        && a.visible_streaming_hash == b.visible_streaming_hash
        && a.visible_batch_progress_hash == b.visible_batch_progress_hash
}

fn same_flicker_context_key(a: &FlickerFrameSample, b: &FlickerFrameSample) -> bool {
    a.session_id == b.session_id
        && a.display_messages_version == b.display_messages_version
        && a.diff_mode == b.diff_mode
        && a.centered == b.centered
        && a.is_processing == b.is_processing
        && a.auto_scroll_paused == b.auto_scroll_paused
        && a.messages_area_width == b.messages_area_width
        && a.messages_area_height == b.messages_area_height
}

fn sample_has_visible_transient_content(sample: &FlickerFrameSample) -> bool {
    sample.visible_streaming_hash != 0 || sample.visible_batch_progress_hash != 0
}

fn push_flicker_event(history: &mut FlickerFrameHistory, event: FlickerEvent) {
    history.events.push_back(event.clone());
    while history.events.len() > FLICKER_HISTORY_MAX_EVENTS {
        history.events.pop_front();
    }

    let severe = event.kind.contains("oscillation");
    let should_log = severe
        || history
            .last_log_at_ms
            .map(|last| event.timestamp_ms.saturating_sub(last) >= FLICKER_LOG_INTERVAL_MS)
            .unwrap_or(true);
    if should_log {
        history.last_log_at_ms = Some(event.timestamp_ms);
        if let Ok(payload) = serde_json::to_string(&event) {
            crate::logging::warn(&format!("TUI_FLICKER_EVENT {}", payload));
        } else {
            crate::logging::warn(&format!(
                "TUI_FLICKER_EVENT kind={} session={:?}",
                event.kind, event.session_name
            ));
        }
    }
}
fn maybe_record_flicker_event(history: &mut FlickerFrameHistory, current: &FlickerFrameSample) {
    let Some(previous) = history.samples.back().cloned() else {
        return;
    };

    let len = history.samples.len();
    if len >= 2 {
        let earlier = history.samples.get(len - 2).cloned();
        if let Some(earlier) = earlier
            && same_flicker_state_key(&earlier, current)
            && same_flicker_state_key(&earlier, &previous)
            && earlier.visible_hash == current.visible_hash
            && earlier.chat_scrollbar_visible == current.chat_scrollbar_visible
            && earlier.content_width == current.content_width
            && (earlier.chat_scrollbar_visible != previous.chat_scrollbar_visible
                || earlier.content_width != previous.content_width)
        {
            push_flicker_event(
                history,
                FlickerEvent {
                    timestamp_ms: current.timestamp_ms,
                    kind: "layout_oscillation".to_string(),
                    session_id: current.session_id.clone(),
                    session_name: current.session_name.clone(),
                    previous,
                    current: current.clone(),
                },
            );
            return;
        }
    }

    if len >= 2 {
        let earlier = history.samples.get(len - 2).cloned();
        if let Some(earlier) = earlier
            && same_flicker_context_key(&earlier, current)
            && same_flicker_context_key(&earlier, &previous)
            && !current.auto_scroll_paused
            && earlier.visible_hash == current.visible_hash
            && earlier.content_width == current.content_width
            && earlier.chat_scrollbar_visible == current.chat_scrollbar_visible
            && (previous.visible_hash != current.visible_hash
                || previous.content_width != current.content_width
                || previous.chat_scrollbar_visible != current.chat_scrollbar_visible)
        {
            push_flicker_event(
                history,
                FlickerEvent {
                    timestamp_ms: current.timestamp_ms,
                    kind: "layout_feedback_oscillation".to_string(),
                    session_id: current.session_id.clone(),
                    session_name: current.session_name.clone(),
                    previous,
                    current: current.clone(),
                },
            );
            return;
        }
    }

    if same_flicker_state_key(&previous, current) {
        if previous.chat_scrollbar_visible != current.chat_scrollbar_visible
            || previous.content_width != current.content_width
        {
            push_flicker_event(
                history,
                FlickerEvent {
                    timestamp_ms: current.timestamp_ms,
                    kind: "layout_toggle_same_state".to_string(),
                    session_id: current.session_id.clone(),
                    session_name: current.session_name.clone(),
                    previous: previous.clone(),
                    current: current.clone(),
                },
            );
        } else if previous.visible_hash != current.visible_hash
            && !sample_has_visible_transient_content(&previous)
            && !sample_has_visible_transient_content(current)
        {
            push_flicker_event(
                history,
                FlickerEvent {
                    timestamp_ms: current.timestamp_ms,
                    kind: "visible_hash_changed_same_state".to_string(),
                    session_id: current.session_id.clone(),
                    session_name: current.session_name.clone(),
                    previous,
                    current: current.clone(),
                },
            );
        }
    }
}

pub(crate) fn record_flicker_frame_sample(sample: FlickerFrameSample) {
    if !flicker_detection_enabled() {
        return;
    }

    let mut history = flicker_frame_history()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    maybe_record_flicker_event(&mut history, &sample);
    history.samples.push_back(sample);
    while history.samples.len() > FLICKER_HISTORY_MAX_SAMPLES {
        history.samples.pop_front();
    }
}
pub(crate) fn debug_flicker_frame_history(limit: usize) -> serde_json::Value {
    let history = flicker_frame_history()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let take_samples = limit.clamp(1, FLICKER_HISTORY_MAX_SAMPLES);
    let samples: Vec<FlickerFrameSample> = history
        .samples
        .iter()
        .rev()
        .take(take_samples)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let events: Vec<FlickerEvent> = history
        .events
        .iter()
        .rev()
        .take(limit.clamp(1, FLICKER_HISTORY_MAX_EVENTS))
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    serde_json::json!({
        "enabled": flicker_detection_enabled(),
        "buffered_samples": history.samples.len(),
        "returned_samples": samples.len(),
        "buffered_events": history.events.len(),
        "returned_events": events.len(),
        "summary": {
            "layout_toggle_events": events.iter().filter(|event| event.kind == "layout_toggle_same_state").count(),
            "layout_oscillation_events": events.iter().filter(|event| event.kind == "layout_oscillation").count(),
            "layout_feedback_oscillation_events": events.iter().filter(|event| event.kind == "layout_feedback_oscillation").count(),
            "visible_hash_change_events": events.iter().filter(|event| event.kind == "visible_hash_changed_same_state").count(),
        },
        "events": events,
        "samples": samples,
    })
}

fn flicker_event_label(kind: &str) -> &str {
    match kind {
        "layout_toggle_same_state" => "layout toggle",
        "layout_oscillation" => "layout oscillation",
        "layout_feedback_oscillation" => "layout feedback oscillation",
        "visible_hash_changed_same_state" => "same-state redraw",
        _ => kind,
    }
}

fn abbreviate_flicker_log_path(path: &std::path::Path) -> String {
    let rendered = path.display().to_string();
    if let Some(home) = dirs::home_dir() {
        let home = home.display().to_string();
        if rendered == home {
            return "~".to_string();
        }
        if let Some(rest) = rendered.strip_prefix(&home) {
            return format!("~{}", rest);
        }
    }
    rendered
}

pub(crate) fn recent_flicker_ui_notice() -> Option<FlickerUiNotice> {
    if !flicker_detection_enabled() {
        return None;
    }

    let history = flicker_frame_history()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let event = history.events.back()?.clone();
    drop(history);

    #[cfg(not(test))]
    {
        let now = wall_clock_ms();
        if now.saturating_sub(event.timestamp_ms) > FLICKER_UI_NOTICE_MAX_AGE_MS {
            return None;
        }
    }

    let log_hint = crate::logging::log_path()
        .map(|path| abbreviate_flicker_log_path(&path))
        .unwrap_or_else(|| "~/.jcode/logs/".to_string());
    let summary = format!("⚠ flicker detected ({})", flicker_event_label(&event.kind));
    let hint = format!("logs: {} · debug: client:flicker-frames 32", log_hint);
    Some(FlickerUiNotice { summary, hint })
}

pub(crate) fn recent_flicker_copy_target_for_key(key: char) -> Option<VisibleCopyTarget> {
    if !key.eq_ignore_ascii_case(&FLICKER_NOTICE_COPY_KEY) {
        return None;
    }

    let notice = recent_flicker_ui_notice()?;
    Some(VisibleCopyTarget {
        key: FLICKER_NOTICE_COPY_KEY,
        kind_label: "flicker hint".to_string(),
        copied_notice: "Copied flicker hint".to_string(),
        content: notice.hint,
    })
}
#[cfg(test)]
pub(crate) fn clear_flicker_frame_history_for_tests() {
    let mut history = flicker_frame_history()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    history.samples.clear();
    history.events.clear();
    history.last_log_at_ms = None;
    set_last_chat_scrollbar_visible(false);
}
