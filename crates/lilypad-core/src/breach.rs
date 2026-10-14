//! Have-I-Been-Pwned breach checking via the k-anonymity range API.
//!
//! Privacy model: a password NEVER leaves the machine. Its SHA-1 hash is
//! computed locally, only the first 5 hex characters are sent to
//! `api.pwnedpasswords.com/range/`, and the response (every breached suffix in
//! that bucket, padded server-side so the response size leaks nothing) is
//! matched locally against the remaining 35 characters.
//!
//! This is an explicit, user-triggered network operation - it is never run
//! implicitly as part of the offline health report.

use crate::error::{AppError, Result};
use crate::secret::open_secret;
use crate::session::Session;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use zeroize::Zeroize;

/// One breached entry: which label, and how many times the password appears in
/// known breaches.
#[derive(Debug, Clone)]
pub struct BreachedEntry {
    pub label: String,
    pub count: u64,
}

/// The outcome of a vault-wide breach check.
#[derive(Debug, Clone, Default)]
pub struct BreachReport {
    /// Unique passwords checked (identical passwords are queried once).
    pub unique_checked: usize,
    /// HIBP range buckets queried (one HTTP request each).
    pub requests: usize,
    /// Entries whose password appears in known breaches, worst first.
    pub breached: Vec<BreachedEntry>,
}

/// SHA-1 of a password as (5-char range prefix, 35-char suffix), uppercase hex.
/// The intermediate digest buffer is wiped.
fn hash_split(password: &str) -> (String, String) {
    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    let mut hex: String = digest.iter().map(|byte| format!("{byte:02X}")).collect();
    let mut padded = format!("{:0>40}", hex);
    hex.zeroize();
    let suffix = padded.split_off(5);
    (padded, suffix)
}

/// Queries one HIBP range bucket and returns `suffix -> count` for it.
fn query_range(client: &reqwest::blocking::Client, prefix: &str) -> Result<HashMap<String, u64>> {
    let url = format!("https://api.pwnedpasswords.com/range/{prefix}");
    let response = client
        .get(&url)
        // Padding makes every bucket the same shape, so the response size
        // cannot fingerprint which bucket was asked for.
        .header("Add-Padding", "true")
        .send()
        .map_err(|e| AppError::Network(format!("HIBP request failed: {e}")))?;
    if !response.status().is_success() {
        return Err(AppError::Network(format!(
            "HIBP returned HTTP {}",
            response.status()
        )));
    }
    let body = response
        .text()
        .map_err(|e| AppError::Network(format!("HIBP response unreadable: {e}")))?;
    let mut map = HashMap::new();
    for line in body.lines() {
        if let Some((suffix, count)) = line.trim().split_once(':') {
            let count: u64 = count.trim().parse().unwrap_or(0);
            // Padding entries have count 0 - not real breaches.
            if count > 0 {
                map.insert(suffix.to_ascii_uppercase(), count);
            }
        }
    }
    Ok(map)
}

/// Fast, local, offline step: decrypts each live entry, hashes its password,
/// and returns labels grouped per hash. No plaintext is retained (each
/// decrypted password is wiped right after hashing) and nothing touches the
/// network - so a GUI can run this on the UI thread and ship the result (it is
/// `Send`) to a background task for [`query_hashes`].
pub fn collect_hashes(session: &Session) -> Vec<((String, String), Vec<String>)> {
    let mut by_hash: HashMap<(String, String), Vec<String>> = HashMap::new();
    for entry in session.vault().entries.iter().filter(|e| e.deleted_at == 0) {
        let Ok(secret) = open_secret(session.key(), entry) else {
            continue;
        };
        if secret.password.is_empty() {
            continue;
        }
        let key = hash_split(&secret.password);
        by_hash.entry(key).or_default().push(entry.label.clone());
        // `secret` (RevealedSecret) drops here, wiping the plaintext.
    }
    by_hash.into_iter().collect()
}

/// Network step: queries HIBP for the collected hashes. Identical passwords
/// are queried once; requests are grouped per range bucket, so N entries cost
/// at most `min(N, unique buckets)` HTTP round-trips.
pub fn query_hashes(hashes: Vec<((String, String), Vec<String>)>) -> Result<BreachReport> {
    let by_hash: HashMap<(String, String), Vec<String>> = hashes.into_iter().collect();
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("lilypad/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Network(format!("HTTP client failed: {e}")))?;

    // Group unique hashes by range bucket: one request per bucket.
    let mut by_prefix: HashMap<String, Vec<(String, Vec<String>)>> = HashMap::new();
    let unique_checked = by_hash.len();
    for ((prefix, suffix), labels) in by_hash {
        by_prefix.entry(prefix).or_default().push((suffix, labels));
    }

    let mut report = BreachReport {
        unique_checked,
        requests: by_prefix.len(),
        ..Default::default()
    };
    for (prefix, candidates) in by_prefix {
        let bucket = query_range(&client, &prefix)?;
        for (suffix, labels) in candidates {
            if let Some(&count) = bucket.get(&suffix) {
                for label in labels {
                    report.breached.push(BreachedEntry { label, count });
                }
            }
        }
    }
    report.breached.sort_by_key(|b| std::cmp::Reverse(b.count));
    Ok(report)
}

/// Convenience: full check in one call (hash locally, then query). Blocking
/// network I/O - GUIs should run [`collect_hashes`] on the UI thread and
/// [`query_hashes`] in a background task instead.
pub fn breach_check(session: &Session) -> Result<BreachReport> {
    query_hashes(collect_hashes(session))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_split_matches_known_sha1() {
        // SHA-1("password") = 5BAA61E4C9B93F3F0682250B6CF8331B7EE68FD8
        let (prefix, suffix) = hash_split("password");
        assert_eq!(prefix, "5BAA6");
        assert_eq!(suffix, "1E4C9B93F3F0682250B6CF8331B7EE68FD8");
        assert_eq!(prefix.len(), 5);
        assert_eq!(suffix.len(), 35);
    }

    #[test]
    fn range_response_parsing_ignores_padding_lines() {
        // Simulate parsing logic on a canned body (padding lines have count 0).
        let body = "1E4C9B93F3F0682250B6CF8331B7EE68FD8:10437277\r\n\
                    0018A45C4D1DEF81644B54AB7F969B88D65:0\r\n\
                    BADLINE\n";
        let mut map = HashMap::new();
        for line in body.lines() {
            if let Some((suffix, count)) = line.trim().split_once(':') {
                let count: u64 = count.trim().parse().unwrap_or(0);
                if count > 0 {
                    map.insert(suffix.to_ascii_uppercase(), count);
                }
            }
        }
        assert_eq!(map.len(), 1);
        assert_eq!(map["1E4C9B93F3F0682250B6CF8331B7EE68FD8"], 10437277);
    }
}
