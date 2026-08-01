//! Time utilities for Lilypad.
//!
//! Provides timestamp generation and human-readable formatting.

use chrono::{DateTime, Local, TimeZone, Utc};
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current Unix timestamp in seconds.
///
/// # Examples
///
/// ```
/// use lilypad_common::current_timestamp;
///
/// let ts = current_timestamp();
/// // Should be a reasonable recent timestamp (after 2023).
/// assert!(ts > 1_700_000_000);
/// ```
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Formats a Unix timestamp as a human-readable date/time string.
///
/// # Arguments
/// * `timestamp` - Unix timestamp in seconds
///
/// # Returns
/// A formatted string like "2024-01-15 14:30:45" in local time,
/// or "Unknown" if the timestamp is 0 or invalid.
///
/// # Examples
///
/// ```
/// use lilypad_common::format_timestamp;
///
/// // A zero timestamp returns "Unknown".
/// assert_eq!(format_timestamp(0), "Unknown");
///
/// // A known timestamp contains the expected date parts.
/// let s = format_timestamp(1705320645); // 2024-01-15 in UTC
/// assert!(s.contains("2024"));
/// ```
pub fn format_timestamp(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Unknown".to_string();
    }

    match Utc.timestamp_opt(timestamp as i64, 0) {
        chrono::LocalResult::Single(utc) => {
            let local: DateTime<Local> = utc.into();
            local.format("%Y-%m-%d %H:%M:%S").to_string()
        }
        _ => "Invalid date".to_string(),
    }
}

/// Formats a Unix timestamp as a relative time string.
///
/// # Arguments
/// * `timestamp` - Unix timestamp in seconds
///
/// # Returns
/// A human-readable relative time like "just now", "5 minutes ago",
/// "2 hours ago", "yesterday", or a date if older than a week.
///
/// # Examples
///
/// ```
/// use lilypad_common::{current_timestamp, format_timestamp_relative};
///
/// // A zero timestamp returns "Never".
/// assert_eq!(format_timestamp_relative(0), "Never");
///
/// // The current moment returns "Just now".
/// let now = current_timestamp();
/// assert_eq!(format_timestamp_relative(now), "Just now");
///
/// // Five minutes ago.
/// assert_eq!(format_timestamp_relative(now - 300), "5 minutes ago");
/// ```
pub fn format_timestamp_relative(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Never".to_string();
    }

    let now = current_timestamp();
    if timestamp > now {
        return "In the future".to_string();
    }

    let diff = now - timestamp;

    match diff {
        0..=59 => "Just now".to_string(),
        60..=119 => "1 minute ago".to_string(),
        120..=3599 => format!("{} minutes ago", diff / 60),
        3600..=7199 => "1 hour ago".to_string(),
        7200..=86399 => format!("{} hours ago", diff / 3600),
        86400..=172799 => "Yesterday".to_string(),
        172800..=604799 => format!("{} days ago", diff / 86400),
        _ => format_timestamp(timestamp),
    }
}

/// Formats a duration in seconds as a human-readable string.
///
/// # Arguments
/// * `seconds` - Duration in seconds
///
/// # Returns
/// A formatted string like "30 seconds", "5 minutes", "2 hours".
///
/// # Examples
///
/// ```
/// use lilypad_common::time::format_duration;
///
/// assert_eq!(format_duration(0), "instant");
/// assert_eq!(format_duration(1), "1 second");
/// assert_eq!(format_duration(45), "45 seconds");
/// assert_eq!(format_duration(60), "1 minute");
/// assert_eq!(format_duration(3600), "1 hour");
/// ```
pub fn format_duration(seconds: u64) -> String {
    match seconds {
        0 => "instant".to_string(),
        1 => "1 second".to_string(),
        2..=59 => format!("{seconds} seconds"),
        60 => "1 minute".to_string(),
        61..=3599 => format!("{} minutes", seconds / 60),
        3600 => "1 hour".to_string(),
        _ => format!("{} hours", seconds / 3600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        assert!(ts > 1700000000); // After Nov 2023
    }

    #[test]
    fn test_format_timestamp_zero() {
        assert_eq!(format_timestamp(0), "Unknown");
    }

    #[test]
    fn test_format_timestamp_valid() {
        let formatted = format_timestamp(1705320645); // 2024-01-15 13:30:45 UTC
        assert!(formatted.contains("2024"));
        assert!(formatted.contains("01"));
        assert!(formatted.contains("15"));
    }

    #[test]
    fn test_format_timestamp_relative_now() {
        let now = current_timestamp();
        assert_eq!(format_timestamp_relative(now), "Just now");
    }

    #[test]
    fn test_format_timestamp_relative_minutes() {
        let now = current_timestamp();
        let five_min_ago = now - 300;
        assert_eq!(format_timestamp_relative(five_min_ago), "5 minutes ago");
    }

    #[test]
    fn test_format_timestamp_relative_hours() {
        let now = current_timestamp();
        let two_hours_ago = now - 7200;
        assert_eq!(format_timestamp_relative(two_hours_ago), "2 hours ago");
    }

    #[test]
    fn test_format_timestamp_relative_yesterday() {
        let now = current_timestamp();
        let yesterday = now - 100000;
        assert_eq!(format_timestamp_relative(yesterday), "Yesterday");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "instant");
        assert_eq!(format_duration(1), "1 second");
        assert_eq!(format_duration(30), "30 seconds");
        assert_eq!(format_duration(60), "1 minute");
        assert_eq!(format_duration(300), "5 minutes");
        assert_eq!(format_duration(3600), "1 hour");
        assert_eq!(format_duration(7200), "2 hours");
    }

    #[test]
    fn test_format_timestamp_relative_days() {
        let now = current_timestamp();
        // 3 days ago = 3 * 86400 seconds
        let three_days_ago = now - 3 * 86400;
        assert_eq!(format_timestamp_relative(three_days_ago), "3 days ago");

        // 5 days ago
        let five_days_ago = now - 5 * 86400;
        assert_eq!(format_timestamp_relative(five_days_ago), "5 days ago");

        // 2 days ago (just past the "Yesterday" boundary at 172800)
        let two_days_ago = now - 2 * 86400;
        assert_eq!(format_timestamp_relative(two_days_ago), "2 days ago");

        // 6 days ago (still within the "X days ago" range < 604800)
        let six_days_ago = now - 6 * 86400;
        assert_eq!(format_timestamp_relative(six_days_ago), "6 days ago");
    }

    #[test]
    fn test_format_timestamp_relative_months() {
        let now = current_timestamp();
        // 30 days ago should fall into the format_timestamp fallback (beyond 7 days)
        let thirty_days_ago = now - 30 * 86400;
        let result = format_timestamp_relative(thirty_days_ago);
        // Should NOT be "X days ago" since it's beyond 7 days
        assert!(
            !result.contains("days ago"),
            "30 days ago should use date format, got: {}",
            result
        );
        // Should be a formatted date (contains year)
        assert!(
            result.contains("2026") || result.contains("2025"),
            "should contain a year in formatted date, got: {}",
            result
        );

        // 90 days ago
        let ninety_days_ago = now - 90 * 86400;
        let result = format_timestamp_relative(ninety_days_ago);
        assert!(
            !result.contains("days ago"),
            "90 days ago should use date format, got: {}",
            result
        );

        // Zero timestamp
        assert_eq!(format_timestamp_relative(0), "Never");

        // Future timestamp
        let future = now + 1000;
        assert_eq!(format_timestamp_relative(future), "In the future");
    }

    #[test]
    fn test_format_duration_seconds() {
        // Test various short durations
        assert_eq!(format_duration(2), "2 seconds");
        assert_eq!(format_duration(10), "10 seconds");
        assert_eq!(format_duration(59), "59 seconds");

        // Boundary: 1 second singular
        assert_eq!(format_duration(1), "1 second");

        // Boundary: instant
        assert_eq!(format_duration(0), "instant");

        // Just past seconds into minutes
        assert_eq!(format_duration(61), "1 minutes");
        assert_eq!(format_duration(120), "2 minutes");

        // Large hour values
        assert_eq!(format_duration(36000), "10 hours");
    }
}
