use crate::tui::info_widget;

pub(crate) fn scheduled_notification_text(
    info: Option<&info_widget::AmbientWidgetData>,
) -> Option<String> {
    let info = info?;
    if info.reminder_count == 0 {
        return None;
    }
    let next = info.next_reminder_wake.as_deref()?;
    let suffix = if info.reminder_count > 1 {
        format!(" · {} queued", info.reminder_count)
    } else {
        String::new()
    };
    Some(format!("⏰ next scheduled task {}{}", next, suffix))
}
