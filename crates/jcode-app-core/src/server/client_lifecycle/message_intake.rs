use crate::protocol::ServerEvent;

pub(super) struct ProcessingMessage {
    pub(super) id: u64,
    pub(super) content: String,
    pub(super) images: Vec<(String, String)>,
    pub(super) system_reminder: Option<String>,
}

pub(super) fn is_empty_turn(
    content: &str,
    system_reminder: Option<&str>,
    images: &[(String, String)],
) -> bool {
    content.trim().is_empty() && system_reminder.is_none() && images.is_empty()
}

pub(super) fn empty_turn_error(id: u64) -> ServerEvent {
    ServerEvent::Error {
        id,
        message: "Empty message requires a system reminder or image".to_string(),
        retry_after_secs: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_turn_requires_blank_content_without_reminder_or_images() {
        let image = [("image/png".to_string(), "data".to_string())];

        assert!(is_empty_turn("", None, &[]));
        assert!(is_empty_turn(" \n\t", None, &[]));
        assert!(!is_empty_turn("hello", None, &[]));
        assert!(!is_empty_turn("", Some("continue"), &[]));
        assert!(!is_empty_turn("", None, &image));
    }
}
