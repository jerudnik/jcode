//! Shared parsing for provider rate-limit response headers.
//!
//! `retry_after` handles the error path: how long to wait after a provider has
//! already refused a request. These headers are the success-path counterpart.
//! They report remaining quota while requests are still being served, which is
//! what lets a background scheduler slow down *before* it starts colliding with
//! interactive use.
//!
//! Values are advisory. A missing, malformed, or hostile header yields `None`
//! for that field rather than an error, so a caller can always fall back to its
//! normal behavior. Parsing never allocates a failure path that a caller must
//! handle.

use chrono::DateTime;
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// Longest window a provider reset hint will be honored for. A reset further
/// out than this is treated as "unknown" so a malformed or hostile timestamp
/// cannot park a scheduler at its maximum interval indefinitely.
pub const MAX_RESET_HORIZON: Duration = Duration::from_secs(3_600);

/// Remaining provider quota as reported by response headers.
///
/// Every field is optional and independently parsed: providers differ in which
/// headers they send, and a partial reading is still useful.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RateLimitHeaders {
    /// Maximum tokens allowed in the current window.
    pub limit_tokens: Option<u64>,
    /// Tokens left before the next request is refused.
    pub remaining_tokens: Option<u64>,
    /// Maximum requests allowed in the current window.
    pub limit_requests: Option<u64>,
    /// Requests left before the next request is refused.
    pub remaining_requests: Option<u64>,
    /// How far in the future the window replenishes, relative to the clock
    /// passed to the parser. Already-elapsed resets read as `Duration::ZERO`.
    pub reset_in: Option<Duration>,
}

impl RateLimitHeaders {
    /// True when no field was populated, so a caller can skip publishing a
    /// reading that carries no information.
    pub fn is_empty(self) -> bool {
        self.limit_tokens.is_none()
            && self.remaining_tokens.is_none()
            && self.limit_requests.is_none()
            && self.remaining_requests.is_none()
            && self.reset_in.is_none()
    }
}

/// Header names for one provider's rate-limit reporting.
///
/// Providers publish the same facts under different names, so the family is
/// data rather than a parser per vendor.
#[derive(Clone, Copy, Debug)]
pub struct HeaderNames {
    pub limit_tokens: &'static str,
    pub remaining_tokens: &'static str,
    pub limit_requests: &'static str,
    pub remaining_requests: &'static str,
    pub reset: &'static str,
}

/// Anthropic's `anthropic-ratelimit-*` family.
pub const ANTHROPIC: HeaderNames = HeaderNames {
    limit_tokens: "anthropic-ratelimit-tokens-limit",
    remaining_tokens: "anthropic-ratelimit-tokens-remaining",
    limit_requests: "anthropic-ratelimit-requests-limit",
    remaining_requests: "anthropic-ratelimit-requests-remaining",
    reset: "anthropic-ratelimit-tokens-reset",
};

/// The `x-ratelimit-*` family used by OpenAI and OpenAI-compatible providers.
pub const X_RATELIMIT: HeaderNames = HeaderNames {
    limit_tokens: "x-ratelimit-limit-tokens",
    remaining_tokens: "x-ratelimit-remaining-tokens",
    limit_requests: "x-ratelimit-limit-requests",
    remaining_requests: "x-ratelimit-remaining-requests",
    reset: "x-ratelimit-reset-tokens",
};

/// Read a provider's rate-limit headers against the current clock.
pub fn parse(headers: &HeaderMap, names: HeaderNames) -> RateLimitHeaders {
    parse_at(headers, names, SystemTime::now())
}

fn parse_at(headers: &HeaderMap, names: HeaderNames, now: SystemTime) -> RateLimitHeaders {
    RateLimitHeaders {
        limit_tokens: count(headers, names.limit_tokens),
        remaining_tokens: count(headers, names.remaining_tokens),
        limit_requests: count(headers, names.limit_requests),
        remaining_requests: count(headers, names.remaining_requests),
        reset_in: reset_in(headers, names.reset, now),
    }
}

/// Read a header value as text, treating absent and non-ASCII values alike.
fn text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let value = headers.get(name)?;
    let Ok(text) = value.to_str() else {
        return None;
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed)
}

/// Parse a non-negative count with saturation, so an arbitrarily long digit
/// string is bounded rather than wrapping or erroring.
fn count(headers: &HeaderMap, name: &str) -> Option<u64> {
    let value = text(headers, name)?;
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(value.bytes().fold(0u64, |total, byte| {
        total
            .saturating_mul(10)
            .saturating_add(u64::from(byte - b'0'))
    }))
}

/// Parse a reset hint expressed either as an RFC 3339 timestamp (Anthropic) or
/// as a relative duration such as `6m0s` or `1.5s` (OpenAI).
fn reset_in(headers: &HeaderMap, name: &str, now: SystemTime) -> Option<Duration> {
    let value = text(headers, name)?;
    let delay = match httpdate::parse_http_date(value) {
        Ok(reset_at) => reset_at.duration_since(now).unwrap_or(Duration::ZERO),
        Err(_) => match parse_rfc3339(value, now) {
            Some(delay) => delay,
            None => parse_relative(value)?,
        },
    };
    Some(delay.min(MAX_RESET_HORIZON))
}

/// Parse the RFC 3339 form providers use for reset timestamps, for example
/// `2024-05-06T07:08:09Z`. Only UTC is accepted; a zone offset would let a
/// hostile upstream shift the window by hours.
fn parse_rfc3339(value: &str, now: SystemTime) -> Option<Duration> {
    if !value.ends_with('Z') {
        return None;
    }
    let Ok(reset_at) = DateTime::parse_from_rfc3339(value) else {
        return None;
    };
    let reset_at = SystemTime::from(reset_at);
    Some(reset_at.duration_since(now).unwrap_or(Duration::ZERO))
}

/// Parse a relative duration such as `6m0s`, `1.5s`, or `250ms`.
fn parse_relative(value: &str) -> Option<Duration> {
    let mut total = Duration::ZERO;
    let mut number = String::new();
    let mut saw_unit = false;
    let mut rest = value;
    while !rest.is_empty() {
        let unit_start = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());
        if unit_start == 0 {
            return None;
        }
        number.clear();
        number.push_str(&rest[..unit_start]);
        rest = &rest[unit_start..];
        let (unit_len, scale) = if rest.starts_with("ms") {
            (2, 0.001)
        } else if let Some(first) = rest.as_bytes().first() {
            match first {
                b's' => (1, 1.0),
                b'm' => (1, 60.0),
                b'h' => (1, 3_600.0),
                _ => return None,
            }
        } else {
            return None;
        };
        rest = &rest[unit_len..];
        let Ok(magnitude) = number.parse::<f64>() else {
            return None;
        };
        if !magnitude.is_finite() || magnitude < 0.0 {
            return None;
        }
        total = total.saturating_add(Duration::from_secs_f64(magnitude * scale));
        saw_unit = true;
    }
    if saw_unit { Some(total) } else { None }
}

/// A rate-limit reading together with when it was taken.
///
/// The scheduler needs the age of a reading as much as the reading itself: a
/// remaining-token count from an hour ago says nothing about current headroom.
#[derive(Clone, Copy, Debug)]
pub struct RateLimitReading {
    pub headers: RateLimitHeaders,
    pub observed_at: Instant,
}

impl RateLimitReading {
    /// How long ago this reading was taken.
    pub fn age(self) -> Duration {
        Instant::now().saturating_duration_since(self.observed_at)
    }
}

/// Last rate-limit reading seen per provider, written by provider runtimes on
/// the response path and read by background schedulers.
///
/// A process-wide cell rather than a channel: readings are a latest-wins
/// snapshot, and a reader that misses one is not harmed by seeing only the
/// newest. Poisoning is treated as "no reading" so a panic in an unrelated
/// writer cannot take down a scheduler.
static LAST_SEEN: LazyLock<Mutex<HashMap<String, RateLimitReading>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Record a reading for `provider`. Empty readings are dropped so a provider
/// that sends no rate-limit headers does not overwrite a useful earlier one.
pub fn record(provider: &str, headers: RateLimitHeaders) {
    if headers.is_empty() {
        return;
    }
    let Ok(mut cell) = LAST_SEEN.lock() else {
        return;
    };
    cell.insert(
        provider.to_string(),
        RateLimitReading {
            headers,
            observed_at: Instant::now(),
        },
    );
}

/// Parse and record in one step, for the common runtime call site.
pub fn observe(provider: &str, headers: &HeaderMap, names: HeaderNames) {
    record(provider, parse(headers, names));
}

/// Most recent reading for `provider`, if any.
pub fn last_seen(provider: &str) -> Option<RateLimitReading> {
    let Ok(cell) = LAST_SEEN.lock() else {
        return None;
    };
    cell.get(provider).copied()
}

/// Drop every recorded reading. Test-only: production readers tolerate stale
/// entries by checking `age`.
#[cfg(test)]
fn clear() {
    if let Ok(mut cell) = LAST_SEEN.lock() {
        cell.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    fn map(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    #[test]
    fn parses_anthropic_counts_and_reset() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let headers = map(&[
            ("anthropic-ratelimit-tokens-limit", "2000000"),
            ("anthropic-ratelimit-tokens-remaining", "1500000"),
            ("anthropic-ratelimit-requests-limit", "1000"),
            ("anthropic-ratelimit-requests-remaining", "998"),
            ("anthropic-ratelimit-tokens-reset", "2023-11-14T22:14:20Z"),
        ]);
        let parsed = parse_at(&headers, ANTHROPIC, now);
        assert_eq!(parsed.limit_tokens, Some(2_000_000));
        assert_eq!(parsed.remaining_tokens, Some(1_500_000));
        assert_eq!(parsed.limit_requests, Some(1_000));
        assert_eq!(parsed.remaining_requests, Some(998));
        assert_eq!(parsed.reset_in, Some(Duration::from_secs(60)));
    }

    #[test]
    fn parses_openai_relative_reset() {
        let headers = map(&[
            ("x-ratelimit-limit-tokens", "150000"),
            ("x-ratelimit-remaining-tokens", "149000"),
            ("x-ratelimit-reset-tokens", "6m0s"),
        ]);
        let parsed = parse_at(&headers, X_RATELIMIT, SystemTime::UNIX_EPOCH);
        assert_eq!(parsed.remaining_tokens, Some(149_000));
        assert_eq!(parsed.reset_in, Some(Duration::from_secs(360)));
    }

    #[test]
    fn parses_fractional_and_millisecond_resets() {
        for (value, expected) in [
            ("1.5s", Duration::from_millis(1_500)),
            ("250ms", Duration::from_millis(250)),
            ("1h30m", Duration::from_secs(3_600)), // capped at the horizon
        ] {
            let headers = map(&[("x-ratelimit-reset-tokens", value)]);
            let parsed = parse_at(&headers, X_RATELIMIT, SystemTime::UNIX_EPOCH);
            assert_eq!(parsed.reset_in, Some(expected), "value={value}");
        }
    }

    #[test]
    fn missing_headers_yield_empty_reading() {
        let parsed = parse_at(&HeaderMap::new(), ANTHROPIC, SystemTime::UNIX_EPOCH);
        assert_eq!(parsed, RateLimitHeaders::default());
        assert!(parsed.is_empty());
    }

    #[test]
    fn malformed_values_are_ignored_field_by_field() {
        let headers = map(&[
            ("anthropic-ratelimit-tokens-limit", "not-a-number"),
            ("anthropic-ratelimit-tokens-remaining", "1500"),
            ("anthropic-ratelimit-tokens-reset", "yesterday"),
        ]);
        let parsed = parse_at(&headers, ANTHROPIC, SystemTime::UNIX_EPOCH);
        assert_eq!(parsed.limit_tokens, None);
        assert_eq!(parsed.remaining_tokens, Some(1_500));
        assert_eq!(parsed.reset_in, None);
        assert!(!parsed.is_empty());
    }

    #[test]
    fn negative_and_signed_counts_are_rejected() {
        let headers = map(&[("anthropic-ratelimit-tokens-remaining", "-5")]);
        let parsed = parse_at(&headers, ANTHROPIC, SystemTime::UNIX_EPOCH);
        assert_eq!(parsed.remaining_tokens, None);
    }

    #[test]
    fn oversized_count_saturates_instead_of_wrapping() {
        let headers = map(&[(
            "anthropic-ratelimit-tokens-remaining",
            "99999999999999999999999999999999",
        )]);
        let parsed = parse_at(&headers, ANTHROPIC, SystemTime::UNIX_EPOCH);
        assert_eq!(parsed.remaining_tokens, Some(u64::MAX));
    }

    #[test]
    fn past_reset_reads_as_zero_not_a_negative_wait() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let headers = map(&[("anthropic-ratelimit-tokens-reset", "2023-11-14T22:12:20Z")]);
        let parsed = parse_at(&headers, ANTHROPIC, now);
        assert_eq!(parsed.reset_in, Some(Duration::ZERO));
    }

    #[test]
    fn far_future_reset_is_capped_at_the_horizon() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let headers = map(&[("anthropic-ratelimit-tokens-reset", "2033-11-14T22:14:20Z")]);
        let parsed = parse_at(&headers, ANTHROPIC, now);
        assert_eq!(parsed.reset_in, Some(MAX_RESET_HORIZON));
    }

    #[test]
    fn zoned_rfc3339_is_rejected_rather_than_misread() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let headers = map(&[(
            "anthropic-ratelimit-tokens-reset",
            "2023-11-14T22:14:20+05:00",
        )]);
        assert_eq!(parse_at(&headers, ANTHROPIC, now).reset_in, None);
    }

    #[test]
    fn http_date_reset_is_also_accepted() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let value = httpdate::fmt_http_date(now + Duration::from_secs(45));
        let headers = map(&[("anthropic-ratelimit-tokens-reset", value.as_str())]);
        assert_eq!(
            parse_at(&headers, ANTHROPIC, now).reset_in,
            Some(Duration::from_secs(45))
        );
    }

    #[test]
    fn header_families_do_not_read_each_others_values() {
        let headers = map(&[("x-ratelimit-remaining-tokens", "1234")]);
        assert_eq!(
            parse_at(&headers, ANTHROPIC, SystemTime::UNIX_EPOCH).remaining_tokens,
            None
        );
        assert_eq!(
            parse_at(&headers, X_RATELIMIT, SystemTime::UNIX_EPOCH).remaining_tokens,
            Some(1_234)
        );
    }

    /// One test drives the whole cell: it is process-global, so splitting these
    /// into separate `#[test]` functions would let them race each other.
    #[test]
    fn last_seen_cell_round_trips_and_ignores_empty_readings() {
        clear();
        assert!(last_seen("anthropic").is_none());

        observe(
            "anthropic",
            &map(&[("anthropic-ratelimit-tokens-remaining", "4200")]),
            ANTHROPIC,
        );
        let reading = last_seen("anthropic").expect("reading recorded");
        assert_eq!(reading.headers.remaining_tokens, Some(4_200));
        assert!(reading.age() < Duration::from_secs(5));

        // A response with no rate-limit headers must not erase a useful reading.
        observe("anthropic", &HeaderMap::new(), ANTHROPIC);
        assert_eq!(
            last_seen("anthropic").and_then(|r| r.headers.remaining_tokens),
            Some(4_200)
        );

        // Providers are tracked independently.
        assert!(last_seen("openai").is_none());
        observe(
            "openai",
            &map(&[("x-ratelimit-remaining-tokens", "77")]),
            X_RATELIMIT,
        );
        assert_eq!(
            last_seen("openai").and_then(|r| r.headers.remaining_tokens),
            Some(77)
        );

        // Latest write wins.
        observe(
            "anthropic",
            &map(&[("anthropic-ratelimit-tokens-remaining", "10")]),
            ANTHROPIC,
        );
        assert_eq!(
            last_seen("anthropic").and_then(|r| r.headers.remaining_tokens),
            Some(10)
        );
        clear();
    }
}
