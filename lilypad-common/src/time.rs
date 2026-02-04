//! Time utilities for Lilypad.
//!
//! Provides timestamp generation and human-readable formatting.

use chrono::{DateTime, Local, TimeZone, Utc};
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current Unix timestamp in seconds.
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
}
