use super::*;

pub(super) fn ambient_widget_data_from(
    state: crate::ambient::AmbientState,
    queue_items: &[crate::ambient::ScheduledItem],
    ambient_enabled: bool,
) -> Option<AmbientWidgetData> {
    let queue_count = queue_items.len();
    let next_queue_item = queue_items.iter().min_by_key(|item| item.scheduled_for);
    let reminder_items: Vec<_> = queue_items
        .iter()
        .filter(|item| item.target.is_direct_delivery())
        .collect();
    let reminder_count = reminder_items.len();
    let next_reminder_item = reminder_items
        .iter()
        .min_by_key(|item| item.scheduled_for)
        .copied();

    if !ambient_enabled && reminder_count == 0 {
        return None;
    }

    let last_run_ago = state.last_run.map(|t| {
        let ago = chrono::Utc::now() - t;
        if ago.num_hours() > 0 {
            format!("{}h ago", ago.num_hours())
        } else {
            format!("{}m ago", ago.num_minutes().max(0))
        }
    });
    let next_wake = match &state.status {
        crate::ambient::AmbientStatus::Scheduled { next_wake } => {
            Some(format_countdown_until(*next_wake))
        }
        _ => None,
    };

    let next_queue_preview = next_queue_item.map(|item| {
        item.task_description
            .as_deref()
            .unwrap_or(&item.context)
            .to_string()
    });
    let next_reminder_preview = next_reminder_item.map(|item| {
        item.task_description
            .as_deref()
            .unwrap_or(&item.context)
            .to_string()
    });

    Some(AmbientWidgetData {
        show_widget: ambient_enabled || reminder_count > 1,
        status: state.status,
        queue_count,
        next_queue_preview,
        reminder_count,
        next_reminder_preview,
        last_run_ago,
        last_summary: state.last_summary,
        next_wake,
        next_reminder_wake: next_reminder_item
            .map(|item| format_countdown_until(item.scheduled_for)),
        budget_percent: None,
    })
}
