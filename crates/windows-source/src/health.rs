//! Pure channel-coverage and gap rules.

use chrono::{DateTime, Duration, Utc};

/// Below this channel maximum, rollover can destroy unread records quickly.
pub const MIN_CHANNEL_BYTES: u64 = 64 * 1024 * 1024;
/// Below this horizon, the channel cannot cover a full day of downtime.
pub const MIN_HORIZON: Duration = Duration::hours(24);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelInfo {
    pub max_size_bytes: u64,
    pub oldest_record_time: Option<DateTime<Utc>>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelHealth {
    pub horizon: Option<Duration>,
    pub degraded: bool,
    pub reasons: Vec<&'static str>,
}

pub fn assess_channel(info: &ChannelInfo) -> ChannelHealth {
    let mut reasons = Vec::new();
    if info.max_size_bytes < MIN_CHANNEL_BYTES {
        reasons.push("channel_below_64_mib");
    }
    let horizon = info
        .oldest_record_time
        .map(|oldest| info.observed_at.signed_duration_since(oldest));
    if horizon.is_none_or(|value| value < MIN_HORIZON) {
        reasons.push("horizon_below_24_hours");
    }
    ChannelHealth {
        horizon,
        degraded: !reasons.is_empty(),
        reasons,
    }
}

/// Lower bound on lost channel records when a bookmark is older than the
/// oldest record still available. Eligible losses are not included.
pub fn stale_bookmark_lower_bound(bookmarked: u64, oldest_available: u64) -> Option<u64> {
    if oldest_available <= bookmarked + 1 {
        None
    } else {
        Some(oldest_available - bookmarked - 1)
    }
}

pub fn is_eligible_code(code: &str) -> bool {
    super::ELIGIBLE_CODES.contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(max_size_bytes: u64, oldest: Option<DateTime<Utc>>) -> ChannelInfo {
        ChannelInfo {
            max_size_bytes,
            oldest_record_time: oldest,
            observed_at: "2026-01-10T15:00:00Z".parse().unwrap(),
        }
    }

    #[test]
    fn healthy_channel_has_a_full_day_of_coverage() {
        let health = assess_channel(&info(
            256 * 1024 * 1024,
            Some("2026-01-09T14:00:00Z".parse().unwrap()),
        ));
        assert!(!health.degraded);
        assert!(health.reasons.is_empty());
        assert_eq!(health.horizon, Some(Duration::hours(25)));
    }

    #[test]
    fn small_channel_or_short_horizon_is_degraded() {
        let small = assess_channel(&info(
            32 * 1024 * 1024,
            Some("2026-01-09T14:00:00Z".parse().unwrap()),
        ));
        assert_eq!(small.reasons, vec!["channel_below_64_mib"]);

        let short = assess_channel(&info(
            256 * 1024 * 1024,
            Some("2026-01-10T12:00:00Z".parse().unwrap()),
        ));
        assert_eq!(short.reasons, vec!["horizon_below_24_hours"]);

        let unknown = assess_channel(&info(256 * 1024 * 1024, None));
        assert_eq!(unknown.reasons, vec!["horizon_below_24_hours"]);
    }

    #[test]
    fn stale_bookmark_reports_a_lower_bound_only() {
        assert_eq!(stale_bookmark_lower_bound(100, 106), Some(5));
        assert_eq!(stale_bookmark_lower_bound(100, 101), None);
        assert_eq!(stale_bookmark_lower_bound(100, 100), None);
        assert_eq!(stale_bookmark_lower_bound(100, 90), None);
    }

    #[test]
    fn only_logon_codes_are_eligible() {
        assert!(is_eligible_code("4624"));
        assert!(is_eligible_code("4625"));
        assert!(!is_eligible_code("4634"));
        assert!(!is_eligible_code("4624 "));
    }
}
