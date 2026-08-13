//! Shared typo-resistant fuzzy matching adapters for TUI slash commands.

pub(crate) fn fuzzy_score(needle: &str, haystack: &str) -> Option<i32> {
    jcode_fuzzy::command_fuzzy_score(needle, haystack)
}

pub(crate) fn fuzzy_match_positions(needle: &str, haystack: &str) -> Vec<usize> {
    jcode_fuzzy::command_fuzzy_match_positions(needle, haystack)
}
