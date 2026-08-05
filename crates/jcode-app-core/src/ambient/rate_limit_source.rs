//! Current provider rate-limit headroom, read for the ambient scheduler.
//!
//! Provider runtimes record what a response reported about remaining quota
//! (`jcode_provider_core::rate_limit_headers`). The scheduler wants that as a
//! `RateLimitInfo` so `calculate_interval` can widen the gap between ambient
//! cycles as headroom shrinks, instead of running at a fixed maximum interval
//! and only reacting once requests are already being refused.
//!
//! A reading is deliberately dropped when it is stale: a remaining-token count
//! from an hour ago says nothing about current headroom, and acting on it would
//! be worse than the conservative fallback of no information at all.

use crate::ambient_scheduler::AdaptiveScheduler;
use chrono::Utc;
use jcode_ambient_types::RateLimitInfo;
use jcode_provider_core::rate_limit_headers::{self, RateLimitReading};
use std::time::Duration;

/// How old a reading may be and still describe current headroom. Providers
/// report windows on the order of a minute, so a reading older than this is
/// treated as absent.
pub(super) const MAX_READING_AGE: Duration = Duration::from_secs(300);

/// Latest usable rate-limit reading for `provider`, or `None` when nothing has
/// been recorded or the newest reading is too old to trust.
///
/// Returning `None` is the safe direction: `calculate_interval` falls back to
/// its maximum interval, which is the slowest ambient cadence.
pub(super) fn current_rate_limit(provider: &str) -> Option<RateLimitInfo> {
    reading_to_info(rate_limit_headers::last_seen(provider)?)
}

/// The decision itself, split from the lookup so it can be tested against
/// arbitrary readings without racing the process-global cell.
fn reading_to_info(reading: RateLimitReading) -> Option<RateLimitInfo> {
    let age = reading.age();
    if age > MAX_READING_AGE {
        return None;
    }
    let headers = reading.headers;
    // A reading that carries neither a token count nor a window tells the
    // scheduler nothing it can act on.
    if headers.remaining_tokens.is_none() && headers.reset_in.is_none() {
        return None;
    }
    // The window shrinks by however long the reading has been sitting here, so
    // an older reading does not claim more time than actually remains.
    let reset_at = headers.reset_in.map(|reset_in| {
        let remaining = reset_in.saturating_sub(age);
        let remaining = match chrono::Duration::from_std(remaining) {
            Ok(remaining) => remaining,
            Err(_) => chrono::Duration::zero(),
        };
        Utc::now() + remaining
    });
    Some(RateLimitInfo {
        limit_tokens: headers.limit_tokens,
        remaining_tokens: headers.remaining_tokens,
        limit_requests: headers.limit_requests,
        remaining_requests: headers.remaining_requests,
        reset_at,
    })
}

/// Interval to wait before the next ambient cycle for `provider`.
///
/// The single seam between "what the provider last reported" and "how long the
/// runner sleeps". Both runner call sites go through here so the connection is
/// covered by a test: with a reading present the scheduler sees real headroom,
/// and with none it falls back to its maximum interval.
pub(super) fn next_interval(scheduler: &AdaptiveScheduler, provider: &str) -> Duration {
    scheduler.calculate_interval(current_rate_limit(provider).as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcode_provider_core::rate_limit_headers::RateLimitHeaders;
    use std::time::Instant;

    /// A reading taken `age` ago, so the age policy can be exercised without
    /// sleeping or racing the process-global cell.
    fn aged(headers: RateLimitHeaders, age: Duration) -> RateLimitReading {
        RateLimitReading {
            headers,
            observed_at: Instant::now() - age,
        }
    }

    fn usable() -> RateLimitHeaders {
        RateLimitHeaders {
            remaining_tokens: Some(1_000),
            limit_tokens: Some(100_000),
            reset_in: Some(Duration::from_secs(120)),
            ..RateLimitHeaders::default()
        }
    }

    #[test]
    fn stale_readings_are_rejected() {
        assert!(reading_to_info(aged(usable(), Duration::from_secs(0))).is_some());
        // Just inside the limit: the exact boundary instant is not observable,
        // since time passes between building the reading and reading its age.
        assert!(
            reading_to_info(aged(usable(), MAX_READING_AGE - Duration::from_secs(1))).is_some()
        );
        assert!(
            reading_to_info(aged(usable(), MAX_READING_AGE + Duration::from_secs(1))).is_none(),
            "a reading past the age limit must not be reported as current headroom"
        );
    }

    #[test]
    fn reading_without_tokens_or_window_carries_nothing_actionable() {
        let headers = RateLimitHeaders {
            limit_requests: Some(1_000),
            ..RateLimitHeaders::default()
        };
        assert!(reading_to_info(aged(headers, Duration::from_secs(0))).is_none());
    }

    /// An older reading must not claim more window than actually remains.
    #[test]
    fn window_shrinks_by_the_age_of_the_reading() {
        let info = reading_to_info(aged(usable(), Duration::from_secs(90))).expect("fresh enough");
        let secs = (info.reset_at.expect("window") - Utc::now()).num_seconds();
        assert!(
            (25..=30).contains(&secs),
            "window was {secs}s, expected ~30s"
        );
    }

    #[test]
    fn recorded_headers_become_an_actionable_reading() {
        rate_limit_headers::record("test-provider-actionable", usable());
        let info = current_rate_limit("test-provider-actionable").expect("fresh reading");
        assert_eq!(info.remaining_tokens, Some(1_000));
        assert_eq!(info.limit_tokens, Some(100_000));
        let reset_at = info.reset_at.expect("window carried through");
        let secs = (reset_at - Utc::now()).num_seconds();
        assert!((110..=120).contains(&secs), "window was {secs}s");
    }

    fn scheduler() -> AdaptiveScheduler {
        AdaptiveScheduler::new(crate::ambient_scheduler::AmbientSchedulerConfig {
            min_interval_minutes: 5,
            max_interval_minutes: 120,
            user_budget_reserve: 0.8,
            ..Default::default()
        })
    }

    /// The wire itself: a recorded reading with real headroom must make the
    /// runner's interval shorter than the no-information fallback. This fails
    /// if the runner call sites stop consulting recorded headers.
    #[test]
    fn recorded_headroom_shortens_the_next_interval() {
        rate_limit_headers::record(
            "test-provider-interval",
            RateLimitHeaders {
                remaining_tokens: Some(500_000),
                limit_tokens: Some(1_000_000),
                reset_in: Some(Duration::from_secs(3600)),
                ..RateLimitHeaders::default()
            },
        );
        let scheduler = scheduler();
        let max = Duration::from_secs(120 * 60);
        let with_reading = next_interval(&scheduler, "test-provider-interval");
        assert!(
            with_reading < max,
            "headroom should shorten the interval, got {with_reading:?} (max {max:?})"
        );
    }

    /// Acceptance-side control: without a reading the interval must stay at the
    /// maximum. Every other assertion here says "interval goes down", so an
    /// implementation that always returned the minimum would satisfy them all;
    /// this is the assertion that such an implementation fails.
    #[test]
    fn absent_reading_keeps_the_maximum_interval() {
        let scheduler = scheduler();
        let interval = next_interval(&scheduler, "test-provider-no-reading-ever");
        assert_eq!(
            interval,
            Duration::from_secs(120 * 60),
            "with no information the scheduler must stay at its slowest cadence"
        );
    }

    #[test]
    fn unknown_provider_has_no_reading() {
        assert!(current_rate_limit("test-provider-never-recorded").is_none());
    }
}
