//! Does this session, or this message, survive the caller's filters?
//!
//! Every predicate here answers that one question for a different shape of
//! input (jcode session, imported external session, message role). They are
//! grouped so the filter semantics can be read and changed in one place rather
//! than traced through the search pipeline.

use super::session_search::{RoleFilter, SearchOptions};
use crate::session::{Session, StoredMessage};
use jcode_import_core::ExternalSessionRecord;
use jcode_session_types::session_search_field_filter_matches as field_filter_matches;

pub(super) fn source_matches_filter(source: &str, options: &SearchOptions) -> bool {
    options
        .source_filter
        .as_deref()
        .map(|filter| source.eq_ignore_ascii_case(filter))
        .unwrap_or(true)
}

pub(super) fn jcode_session_matches_filters(session: &Session, options: &SearchOptions) -> bool {
    if !source_matches_filter("jcode", options) {
        return false;
    }
    if !provider_matches(session.provider_key.as_deref(), "jcode", options) {
        return false;
    }
    if !field_filter_matches(session.model.as_deref(), options.model_filter.as_deref()) {
        return false;
    }
    if options
        .saved_filter
        .is_some_and(|expected| session.saved != expected)
    {
        return false;
    }
    if options
        .debug_filter
        .is_some_and(|expected| session.is_debug != expected)
    {
        return false;
    }
    if options
        .canary_filter
        .is_some_and(|expected| session.is_canary != expected)
    {
        return false;
    }
    true
}

pub(super) fn external_session_matches_filters(
    session: &ExternalSessionRecord,
    options: &SearchOptions,
) -> bool {
    if !source_matches_filter(session.source, options) {
        return false;
    }
    if !provider_matches(session.provider_key.as_deref(), session.source, options) {
        return false;
    }
    if !field_filter_matches(session.model.as_deref(), options.model_filter.as_deref()) {
        return false;
    }
    if options.saved_filter == Some(true)
        || options.debug_filter == Some(true)
        || options.canary_filter == Some(true)
    {
        return false;
    }
    true
}

fn provider_matches(provider_key: Option<&str>, source: &str, options: &SearchOptions) -> bool {
    let Some(filter) = options.provider_filter.as_deref() else {
        return true;
    };
    field_filter_matches(provider_key, Some(filter)) || source.to_ascii_lowercase().contains(filter)
}

pub(super) fn role_filter_allows_metadata(options: &SearchOptions) -> bool {
    options
        .role_filter
        .map(|role| role == RoleFilter::Metadata)
        .unwrap_or(true)
}

pub(super) fn role_filter_allows_evidence(options: &SearchOptions) -> bool {
    options
        .role_filter
        .map(|role| role == RoleFilter::Metadata)
        .unwrap_or(true)
}

pub(super) fn role_filter_allows_message(msg: &StoredMessage, options: &SearchOptions) -> bool {
    let Some(role_filter) = options.role_filter else {
        return true;
    };
    match role_filter {
        RoleFilter::User => msg.role == crate::message::Role::User,
        RoleFilter::Assistant => msg.role == crate::message::Role::Assistant,
        RoleFilter::Metadata => false,
    }
}

pub(super) fn role_filter_allows_external_message(role: &str, options: &SearchOptions) -> bool {
    let Some(role_filter) = options.role_filter else {
        return true;
    };
    match role_filter {
        RoleFilter::User => role.eq_ignore_ascii_case("user"),
        RoleFilter::Assistant => role.eq_ignore_ascii_case("assistant"),
        RoleFilter::Metadata => false,
    }
}
