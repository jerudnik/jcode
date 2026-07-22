pub(in super::super) fn looks_like_oauth_callback_input(input: &str) -> bool {
    let input = input.trim();
    input.starts_with("http://")
        || input.starts_with("https://")
        || input.starts_with('?')
        || input.contains("code=")
        || input.contains("state=")
}

pub(in super::super) fn antigravity_input_requires_state_validation(
    input: &str,
    expected_state: Option<&str>,
) -> bool {
    expected_state.is_some() && looks_like_oauth_callback_input(input)
}
