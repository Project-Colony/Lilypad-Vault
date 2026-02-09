//! Password health analysis and vault security assessment.
//!
//! This module provides comprehensive security analysis for password vaults,
//! including health scores, issue detection, and recommendations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::validation::{validate_password_strength, PasswordStrength};

/// Overall health score for a vault (0-100).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    /// Overall score from 0 (critical) to 100 (excellent).
    pub score: u8,
    /// Grade based on score (A, B, C, D, F).
    pub grade: HealthGrade,
    /// Number of critical issues found.
    pub critical_issues: usize,
    /// Number of warning issues found.
    pub warning_issues: usize,
    /// Number of info issues found.
    pub info_issues: usize,
    /// Detailed breakdown of score components.
    pub breakdown: ScoreBreakdown,
}

/// Health grade based on overall score.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthGrade {
    /// 90-100: Excellent security
    A,
    /// 80-89: Good security
    B,
    /// 70-79: Fair security
    C,
    /// 60-69: Poor security
    D,
    /// 0-59: Critical security issues
    F,
}

impl HealthGrade {
    /// Returns the grade from a score.
    pub fn from_score(score: u8) -> Self {
        match score {
            90..=100 => HealthGrade::A,
            80..=89 => HealthGrade::B,
            70..=79 => HealthGrade::C,
            60..=69 => HealthGrade::D,
            _ => HealthGrade::F,
        }
    }

    /// Returns a description of the grade.
    pub fn description(&self) -> &'static str {
        match self {
            HealthGrade::A => "Excellent - Your vault security is outstanding",
            HealthGrade::B => "Good - Your vault security is strong with minor improvements possible",
            HealthGrade::C => "Fair - Your vault has some security concerns to address",
            HealthGrade::D => "Poor - Your vault has significant security issues",
            HealthGrade::F => "Critical - Your vault requires immediate attention",
        }
    }

    /// Returns the color associated with this grade (for UI).
    pub fn color(&self) -> &'static str {
        match self {
            HealthGrade::A => "#22C55E", // Green
            HealthGrade::B => "#84CC16", // Lime
            HealthGrade::C => "#EAB308", // Yellow
            HealthGrade::D => "#F97316", // Orange
            HealthGrade::F => "#EF4444", // Red
        }
    }
}

/// Breakdown of the health score by category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    /// Score for password strength (0-25).
    pub password_strength: u8,
    /// Score for password uniqueness (0-25).
    pub uniqueness: u8,
    /// Score for password age/expiry (0-25).
    pub freshness: u8,
    /// Score for 2FA coverage (0-25).
    pub two_factor: u8,
}

/// A security issue found in the vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    /// Severity of the issue.
    pub severity: IssueSeverity,
    /// Category of the issue.
    pub category: IssueCategory,
    /// Human-readable title.
    pub title: String,
    /// Detailed description.
    pub description: String,
    /// Entry labels affected (if applicable).
    pub affected_entries: Vec<String>,
    /// Recommended action to fix.
    pub recommendation: String,
}

/// Severity levels for health issues.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueSeverity {
    /// Informational - suggestions for improvement.
    Info,
    /// Warning - should be addressed soon.
    Warning,
    /// Critical - requires immediate attention.
    Critical,
}

impl IssueSeverity {
    /// Returns the color for this severity (for UI).
    pub fn color(&self) -> &'static str {
        match self {
            IssueSeverity::Info => "#3B82F6",    // Blue
            IssueSeverity::Warning => "#EAB308", // Yellow
            IssueSeverity::Critical => "#EF4444", // Red
        }
    }

    /// Returns an icon identifier for this severity.
    pub fn icon(&self) -> &'static str {
        match self {
            IssueSeverity::Info => "info",
            IssueSeverity::Warning => "warning",
            IssueSeverity::Critical => "error",
        }
    }
}

/// Categories of health issues.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueCategory {
    /// Weak password issues.
    WeakPassword,
    /// Reused/duplicate password issues.
    ReusedPassword,
    /// Expired or old password issues.
    ExpiredPassword,
    /// Missing 2FA issues.
    Missing2FA,
    /// Compromised password (found in breach).
    Compromised,
    /// Empty or missing data.
    IncompleteEntry,
}

impl IssueCategory {
    /// Returns a human-readable name for this category.
    pub fn name(&self) -> &'static str {
        match self {
            IssueCategory::WeakPassword => "Weak Password",
            IssueCategory::ReusedPassword => "Reused Password",
            IssueCategory::ExpiredPassword => "Expired Password",
            IssueCategory::Missing2FA => "Missing 2FA",
            IssueCategory::Compromised => "Compromised",
            IssueCategory::IncompleteEntry => "Incomplete Entry",
        }
    }
}

/// Input data for health analysis (decrypted entry info).
#[derive(Debug, Clone)]
pub struct EntryHealthData {
    /// Entry label.
    pub label: String,
    /// The password (for strength analysis).
    pub password: String,
    /// Whether the entry has a username.
    pub has_username: bool,
    /// Whether the entry has a URL.
    pub has_url: bool,
    /// Whether the entry has TOTP enabled.
    pub has_totp: bool,
    /// Password age in days.
    pub password_age_days: u64,
    /// Days until password expires (None if no expiry).
    pub days_until_expiry: Option<i64>,
    /// Whether password is expired.
    pub is_expired: bool,
    /// Whether the password has been found in a known data breach.
    pub is_compromised: bool,
}

/// Complete health report for a vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Overall health score.
    pub score: HealthScore,
    /// List of issues found.
    pub issues: Vec<HealthIssue>,
    /// Statistics about the vault.
    pub stats: VaultStats,
    /// Timestamp when the report was generated.
    pub generated_at: u64,
}

/// Statistics about the vault contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStats {
    /// Total number of entries.
    pub total_entries: usize,
    /// Number of entries with strong passwords.
    pub strong_passwords: usize,
    /// Number of entries with weak passwords.
    pub weak_passwords: usize,
    /// Number of entries with reused passwords.
    pub reused_passwords: usize,
    /// Number of entries with expired passwords.
    pub expired_passwords: usize,
    /// Number of entries with 2FA enabled.
    pub with_2fa: usize,
    /// Number of entries missing 2FA.
    pub without_2fa: usize,
    /// Number of unique passwords.
    pub unique_passwords: usize,
    /// Average password age in days.
    pub avg_password_age_days: u64,
    /// Number of entries expiring within 7 days.
    pub expiring_soon: usize,
}

/// Analyzes vault health and generates a report.
pub fn analyze_vault_health(entries: &[EntryHealthData]) -> HealthReport {
    let mut issues = Vec::new();
    let mut stats = VaultStats {
        total_entries: entries.len(),
        strong_passwords: 0,
        weak_passwords: 0,
        reused_passwords: 0,
        expired_passwords: 0,
        with_2fa: 0,
        without_2fa: 0,
        unique_passwords: 0,
        avg_password_age_days: 0,
        expiring_soon: 0,
    };

    if entries.is_empty() {
        return HealthReport {
            score: HealthScore {
                score: 100,
                grade: HealthGrade::A,
                critical_issues: 0,
                warning_issues: 0,
                info_issues: 0,
                breakdown: ScoreBreakdown {
                    password_strength: 25,
                    uniqueness: 25,
                    freshness: 25,
                    two_factor: 25,
                },
            },
            issues,
            stats,
            generated_at: crate::time::current_timestamp(),
        };
    }

    // Track passwords for duplicate detection
    let mut password_counts: HashMap<String, Vec<String>> = HashMap::new();
    let mut total_age: u64 = 0;

    // Analyze each entry
    let mut weak_entries = Vec::new();
    let mut very_weak_entries = Vec::new();
    let mut expired_entries = Vec::new();
    let mut expiring_soon_entries = Vec::new();
    let mut old_password_entries = Vec::new();
    let mut no_2fa_entries = Vec::new();
    let mut incomplete_entries = Vec::new();
    let mut compromised_entries = Vec::new();

    for entry in entries {
        // Password strength analysis
        let strength = validate_password_strength(&entry.password);
        match strength {
            PasswordStrength::VeryWeak | PasswordStrength::Weak => {
                stats.weak_passwords += 1;
                if matches!(strength, PasswordStrength::VeryWeak) {
                    very_weak_entries.push(entry.label.clone());
                } else {
                    weak_entries.push(entry.label.clone());
                }
            }
            PasswordStrength::Strong | PasswordStrength::VeryStrong => {
                stats.strong_passwords += 1;
            }
            _ => {}
        }

        // Track password for duplicates
        password_counts
            .entry(entry.password.clone())
            .or_default()
            .push(entry.label.clone());

        // Password age
        total_age += entry.password_age_days;

        // Expiry checking
        if entry.is_expired {
            stats.expired_passwords += 1;
            expired_entries.push(entry.label.clone());
        } else if let Some(days) = entry.days_until_expiry {
            if (0..=7).contains(&days) {
                stats.expiring_soon += 1;
                expiring_soon_entries.push(entry.label.clone());
            }
        }

        // Old password (> 90 days without expiry set)
        if entry.password_age_days > 90 && entry.days_until_expiry.is_none() {
            old_password_entries.push(entry.label.clone());
        }

        // 2FA coverage
        if entry.has_totp {
            stats.with_2fa += 1;
        } else {
            stats.without_2fa += 1;
            if entry.has_url {
                // Only flag missing 2FA for entries with URLs (likely web services)
                no_2fa_entries.push(entry.label.clone());
            }
        }

        // Incomplete entries
        if !entry.has_username && entry.has_url {
            incomplete_entries.push(entry.label.clone());
        }

        // Compromised passwords
        if entry.is_compromised {
            compromised_entries.push(entry.label.clone());
        }
    }

    // Count unique passwords
    stats.unique_passwords = password_counts.len();
    stats.avg_password_age_days = total_age / entries.len() as u64;

    // Find reused passwords
    let mut reused_entries: Vec<String> = Vec::new();
    for labels in password_counts.values() {
        if labels.len() > 1 {
            stats.reused_passwords += labels.len();
            reused_entries.extend(labels.clone());
        }
    }

    // Generate issues

    // Critical: Very weak passwords
    if !very_weak_entries.is_empty() {
        issues.push(HealthIssue {
            severity: IssueSeverity::Critical,
            category: IssueCategory::WeakPassword,
            title: "Very weak passwords detected".to_string(),
            description: format!(
                "{} entries have very weak passwords that could be easily guessed or cracked.",
                very_weak_entries.len()
            ),
            affected_entries: very_weak_entries,
            recommendation: "Update these passwords to use at least 12 characters with a mix of uppercase, lowercase, numbers, and symbols.".to_string(),
        });
    }

    // Critical: Expired passwords
    if !expired_entries.is_empty() {
        issues.push(HealthIssue {
            severity: IssueSeverity::Critical,
            category: IssueCategory::ExpiredPassword,
            title: "Expired passwords".to_string(),
            description: format!(
                "{} entries have passwords that have expired and should be changed immediately.",
                expired_entries.len()
            ),
            affected_entries: expired_entries,
            recommendation: "Change these passwords as soon as possible to maintain security.".to_string(),
        });
    }

    // Warning: Weak passwords
    if !weak_entries.is_empty() {
        issues.push(HealthIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::WeakPassword,
            title: "Weak passwords detected".to_string(),
            description: format!(
                "{} entries have weak passwords that should be strengthened.",
                weak_entries.len()
            ),
            affected_entries: weak_entries,
            recommendation: "Consider using the password generator to create stronger passwords.".to_string(),
        });
    }

    // Warning: Reused passwords
    if !reused_entries.is_empty() {
        issues.push(HealthIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::ReusedPassword,
            title: "Password reuse detected".to_string(),
            description: format!(
                "{} entries share the same password. If one account is compromised, all others are at risk.",
                reused_entries.len()
            ),
            affected_entries: reused_entries,
            recommendation: "Use unique passwords for each account to limit damage from potential breaches.".to_string(),
        });
    }

    // Warning: Expiring soon
    if !expiring_soon_entries.is_empty() {
        issues.push(HealthIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::ExpiredPassword,
            title: "Passwords expiring soon".to_string(),
            description: format!(
                "{} entries have passwords expiring within 7 days.",
                expiring_soon_entries.len()
            ),
            affected_entries: expiring_soon_entries,
            recommendation: "Plan to update these passwords before they expire.".to_string(),
        });
    }

    // Info: Old passwords
    if !old_password_entries.is_empty() {
        issues.push(HealthIssue {
            severity: IssueSeverity::Info,
            category: IssueCategory::ExpiredPassword,
            title: "Old passwords".to_string(),
            description: format!(
                "{} entries have passwords older than 90 days.",
                old_password_entries.len()
            ),
            affected_entries: old_password_entries,
            recommendation: "Consider rotating these passwords periodically for better security.".to_string(),
        });
    }

    // Critical: Compromised passwords
    if !compromised_entries.is_empty() {
        issues.push(HealthIssue {
            severity: IssueSeverity::Critical,
            category: IssueCategory::Compromised,
            title: "Compromised passwords detected".to_string(),
            description: format!(
                "{} entries have passwords found in known data breaches. These should be changed immediately.",
                compromised_entries.len()
            ),
            affected_entries: compromised_entries,
            recommendation: "Change these passwords immediately. Use the password generator to create strong, unique replacements.".to_string(),
        });
    }

    // Info: Missing 2FA
    if !no_2fa_entries.is_empty() && no_2fa_entries.len() <= 20 {
        issues.push(HealthIssue {
            severity: IssueSeverity::Info,
            category: IssueCategory::Missing2FA,
            title: "2FA not enabled".to_string(),
            description: format!(
                "{} web accounts don't have 2FA configured in Lilypad.",
                no_2fa_entries.len()
            ),
            affected_entries: no_2fa_entries,
            recommendation: "Enable two-factor authentication on these services if available, and store the TOTP secret in Lilypad.".to_string(),
        });
    }

    // Info: Incomplete entries
    if !incomplete_entries.is_empty() {
        issues.push(HealthIssue {
            severity: IssueSeverity::Info,
            category: IssueCategory::IncompleteEntry,
            title: "Incomplete entries".to_string(),
            description: format!(
                "{} entries have a URL but are missing a username.",
                incomplete_entries.len()
            ),
            affected_entries: incomplete_entries,
            recommendation: "Add usernames to these entries for better organization and autofill support.".to_string(),
        });
    }

    // Calculate scores
    let password_strength_score = calculate_strength_score(&stats);
    let uniqueness_score = calculate_uniqueness_score(&stats);
    let freshness_score = calculate_freshness_score(&stats);
    let two_factor_score = calculate_2fa_score(&stats);

    let total_score = password_strength_score + uniqueness_score + freshness_score + two_factor_score;

    let critical_count = issues.iter().filter(|i| i.severity == IssueSeverity::Critical).count();
    let warning_count = issues.iter().filter(|i| i.severity == IssueSeverity::Warning).count();
    let info_count = issues.iter().filter(|i| i.severity == IssueSeverity::Info).count();

    // Sort issues by severity (critical first)
    issues.sort_by(|a, b| b.severity.cmp(&a.severity));

    HealthReport {
        score: HealthScore {
            score: total_score,
            grade: HealthGrade::from_score(total_score),
            critical_issues: critical_count,
            warning_issues: warning_count,
            info_issues: info_count,
            breakdown: ScoreBreakdown {
                password_strength: password_strength_score,
                uniqueness: uniqueness_score,
                freshness: freshness_score,
                two_factor: two_factor_score,
            },
        },
        issues,
        stats,
        generated_at: crate::time::current_timestamp(),
    }
}

fn calculate_strength_score(stats: &VaultStats) -> u8 {
    if stats.total_entries == 0 {
        return 25;
    }
    let strong_ratio = stats.strong_passwords as f64 / stats.total_entries as f64;
    (strong_ratio * 25.0) as u8
}

fn calculate_uniqueness_score(stats: &VaultStats) -> u8 {
    if stats.total_entries == 0 {
        return 25;
    }
    let unique_ratio = stats.unique_passwords as f64 / stats.total_entries as f64;
    (unique_ratio * 25.0) as u8
}

fn calculate_freshness_score(stats: &VaultStats) -> u8 {
    if stats.total_entries == 0 {
        return 25;
    }
    // Penalize for expired and old passwords
    let fresh_entries = stats.total_entries - stats.expired_passwords - stats.expiring_soon;
    let fresh_ratio = fresh_entries as f64 / stats.total_entries as f64;
    (fresh_ratio * 25.0) as u8
}

fn calculate_2fa_score(stats: &VaultStats) -> u8 {
    if stats.total_entries == 0 {
        return 25;
    }
    // 2FA is optional, so we're more lenient
    let ratio = stats.with_2fa as f64 / stats.total_entries as f64;
    // Even 0% 2FA gets 10 points, 100% gets 25 points
    (10.0 + ratio * 15.0) as u8
}

/// Detects duplicate passwords across entries.
/// Returns a map of password hash to list of entry labels using that password.
pub fn detect_duplicates(entries: &[EntryHealthData]) -> HashMap<String, Vec<String>> {
    let mut password_map: HashMap<String, Vec<String>> = HashMap::new();

    for entry in entries {
        password_map
            .entry(entry.password.clone())
            .or_default()
            .push(entry.label.clone());
    }

    // Only return duplicates (2+ entries with same password)
    password_map.into_iter().filter(|(_, v)| v.len() > 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_grade_from_score() {
        assert_eq!(HealthGrade::from_score(95), HealthGrade::A);
        assert_eq!(HealthGrade::from_score(85), HealthGrade::B);
        assert_eq!(HealthGrade::from_score(75), HealthGrade::C);
        assert_eq!(HealthGrade::from_score(65), HealthGrade::D);
        assert_eq!(HealthGrade::from_score(55), HealthGrade::F);
    }

    #[test]
    fn test_empty_vault_health() {
        let report = analyze_vault_health(&[]);
        assert_eq!(report.score.score, 100);
        assert_eq!(report.score.grade, HealthGrade::A);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn test_weak_password_detection() {
        let entries = vec![
            EntryHealthData {
                label: "test".to_string(),
                password: "weak".to_string(),
                has_username: true,
                has_url: true,
                has_totp: false,
                password_age_days: 10,
                days_until_expiry: None,
                is_expired: false,
                is_compromised: false,
            },
        ];
        let report = analyze_vault_health(&entries);
        assert!(report.issues.iter().any(|i| i.category == IssueCategory::WeakPassword));
    }

    #[test]
    fn test_compromised_detection() {
        let entries = vec![
            EntryHealthData {
                label: "breached_site".to_string(),
                password: "Str0ng!P@ssw0rd#2024".to_string(),
                has_username: true,
                has_url: true,
                has_totp: false,
                password_age_days: 10,
                days_until_expiry: None,
                is_expired: false,
                is_compromised: true,
            },
        ];
        let report = analyze_vault_health(&entries);
        assert!(report.issues.iter().any(|i| i.category == IssueCategory::Compromised));
        let issue = report.issues.iter().find(|i| i.category == IssueCategory::Compromised).unwrap();
        assert_eq!(issue.severity, IssueSeverity::Critical);
        assert_eq!(issue.affected_entries, vec!["breached_site"]);
    }

    #[test]
    fn test_incomplete_entry_detection() {
        let entries = vec![
            EntryHealthData {
                label: "no_username".to_string(),
                password: "Str0ng!P@ssw0rd#2024".to_string(),
                has_username: false,
                has_url: true,
                has_totp: false,
                password_age_days: 10,
                days_until_expiry: None,
                is_expired: false,
                is_compromised: false,
            },
        ];
        let report = analyze_vault_health(&entries);
        assert!(report.issues.iter().any(|i| i.category == IssueCategory::IncompleteEntry));
    }

    #[test]
    fn test_duplicate_detection() {
        let entries = vec![
            EntryHealthData {
                label: "entry1".to_string(),
                password: "same_password".to_string(),
                has_username: true,
                has_url: true,
                has_totp: false,
                password_age_days: 10,
                days_until_expiry: None,
                is_expired: false,
                is_compromised: false,
            },
            EntryHealthData {
                label: "entry2".to_string(),
                password: "same_password".to_string(),
                has_username: true,
                has_url: true,
                has_totp: false,
                password_age_days: 10,
                days_until_expiry: None,
                is_expired: false,
                is_compromised: false,
            },
        ];
        let duplicates = detect_duplicates(&entries);
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates.get("same_password").unwrap().len(), 2);
    }

    /// Helper to create an entry with strong defaults for health testing.
    fn make_healthy_entry(label: &str, password: &str) -> EntryHealthData {
        EntryHealthData {
            label: label.to_string(),
            password: password.to_string(),
            has_username: true,
            has_url: true,
            has_totp: true,
            password_age_days: 10,
            days_until_expiry: None,
            is_expired: false,
            is_compromised: false,
        }
    }

    #[test]
    fn test_health_score_calculation() {
        // Vault with a mix of issues: one weak password, one missing 2FA, one expired
        let entries = vec![
            EntryHealthData {
                label: "weak_entry".to_string(),
                password: "abc".to_string(), // VeryWeak
                has_username: true,
                has_url: true,
                has_totp: true,
                password_age_days: 10,
                days_until_expiry: None,
                is_expired: false,
                is_compromised: false,
            },
            EntryHealthData {
                label: "expired_entry".to_string(),
                password: "Str0ng!P@ssw0rd#2024".to_string(),
                has_username: true,
                has_url: true,
                has_totp: true,
                password_age_days: 200,
                days_until_expiry: Some(-10),
                is_expired: true,
                is_compromised: false,
            },
            make_healthy_entry("good_entry", "X9$kLm#pQ2wZ!nR7"),
        ];
        let report = analyze_vault_health(&entries);
        // Score should be less than 100 due to issues
        assert!(report.score.score < 100);
        // Score should be above 0 since some entries are healthy
        assert!(report.score.score > 0);
        // Breakdown components should all be <= 25
        assert!(report.score.breakdown.password_strength <= 25);
        assert!(report.score.breakdown.uniqueness <= 25);
        assert!(report.score.breakdown.freshness <= 25);
        assert!(report.score.breakdown.two_factor <= 25);
    }

    #[test]
    fn test_expired_password_detection() {
        let entries = vec![
            EntryHealthData {
                label: "expired_account".to_string(),
                password: "Str0ng!P@ssw0rd#2024".to_string(),
                has_username: true,
                has_url: true,
                has_totp: true,
                password_age_days: 100,
                days_until_expiry: Some(-5),
                is_expired: true,
                is_compromised: false,
            },
        ];
        let report = analyze_vault_health(&entries);
        assert!(report.issues.iter().any(|i| i.category == IssueCategory::ExpiredPassword
            && i.severity == IssueSeverity::Critical));
        let issue = report
            .issues
            .iter()
            .find(|i| i.category == IssueCategory::ExpiredPassword && i.severity == IssueSeverity::Critical)
            .expect("should have expired password issue");
        assert!(issue.affected_entries.contains(&"expired_account".to_string()));
        assert_eq!(report.stats.expired_passwords, 1);
    }

    #[test]
    fn test_missing_2fa_detection() {
        let entries = vec![
            EntryHealthData {
                label: "no_2fa_site".to_string(),
                password: "Str0ng!P@ssw0rd#2024".to_string(),
                has_username: true,
                has_url: true,  // has URL, so Missing2FA should trigger
                has_totp: false,
                password_age_days: 10,
                days_until_expiry: None,
                is_expired: false,
                is_compromised: false,
            },
        ];
        let report = analyze_vault_health(&entries);
        assert!(report.issues.iter().any(|i| i.category == IssueCategory::Missing2FA));
        let issue = report
            .issues
            .iter()
            .find(|i| i.category == IssueCategory::Missing2FA)
            .expect("should have missing 2FA issue");
        assert!(issue.affected_entries.contains(&"no_2fa_site".to_string()));
        assert_eq!(report.stats.without_2fa, 1);
        assert_eq!(report.stats.with_2fa, 0);
    }

    #[test]
    fn test_health_grade_boundaries() {
        // Test exact boundary values
        assert_eq!(HealthGrade::from_score(100), HealthGrade::A);
        assert_eq!(HealthGrade::from_score(90), HealthGrade::A);
        assert_eq!(HealthGrade::from_score(89), HealthGrade::B);
        assert_eq!(HealthGrade::from_score(80), HealthGrade::B);
        assert_eq!(HealthGrade::from_score(79), HealthGrade::C);
        assert_eq!(HealthGrade::from_score(70), HealthGrade::C);
        assert_eq!(HealthGrade::from_score(69), HealthGrade::D);
        assert_eq!(HealthGrade::from_score(60), HealthGrade::D);
        assert_eq!(HealthGrade::from_score(59), HealthGrade::F);
        assert_eq!(HealthGrade::from_score(0), HealthGrade::F);

        // Verify descriptions are non-empty for all grades
        assert!(!HealthGrade::A.description().is_empty());
        assert!(!HealthGrade::B.description().is_empty());
        assert!(!HealthGrade::C.description().is_empty());
        assert!(!HealthGrade::D.description().is_empty());
        assert!(!HealthGrade::F.description().is_empty());

        // Verify colors are non-empty for all grades
        assert!(!HealthGrade::A.color().is_empty());
        assert!(!HealthGrade::B.color().is_empty());
        assert!(!HealthGrade::C.color().is_empty());
        assert!(!HealthGrade::D.color().is_empty());
        assert!(!HealthGrade::F.color().is_empty());
    }

    #[test]
    fn test_multiple_issues_same_entry() {
        // Entry with a very weak password AND no 2FA (and has URL so 2FA check applies)
        let entries = vec![
            EntryHealthData {
                label: "problematic_entry".to_string(),
                password: "bad".to_string(), // VeryWeak
                has_username: true,
                has_url: true,
                has_totp: false,
                password_age_days: 10,
                days_until_expiry: None,
                is_expired: false,
                is_compromised: false,
            },
        ];
        let report = analyze_vault_health(&entries);

        // Should have at least a WeakPassword issue and a Missing2FA issue
        let has_weak = report
            .issues
            .iter()
            .any(|i| i.category == IssueCategory::WeakPassword);
        let has_no_2fa = report
            .issues
            .iter()
            .any(|i| i.category == IssueCategory::Missing2FA);
        assert!(has_weak, "should detect weak password");
        assert!(has_no_2fa, "should detect missing 2FA");

        // Both issues should reference the same entry
        let weak_issue = report
            .issues
            .iter()
            .find(|i| i.category == IssueCategory::WeakPassword)
            .expect("weak issue");
        let no_2fa_issue = report
            .issues
            .iter()
            .find(|i| i.category == IssueCategory::Missing2FA)
            .expect("missing 2FA issue");
        assert!(weak_issue.affected_entries.contains(&"problematic_entry".to_string()));
        assert!(no_2fa_issue.affected_entries.contains(&"problematic_entry".to_string()));
    }

    #[test]
    fn test_health_with_all_strong() {
        // All entries have strong unique passwords, 2FA enabled, fresh passwords
        let entries = vec![
            make_healthy_entry("entry1", "X9$kLm#pQ2wZ!nR7"),
            make_healthy_entry("entry2", "Ht5@bN&vY8jF!cD3"),
            make_healthy_entry("entry3", "Qw7*eR#tY1uI!oP9"),
        ];
        let report = analyze_vault_health(&entries);

        // Score should be high (near or at 100)
        assert!(
            report.score.score >= 85,
            "score should be high for all-strong vault, got {}",
            report.score.score
        );

        // Should have no critical or warning issues
        assert_eq!(report.score.critical_issues, 0, "should have no critical issues");
        assert_eq!(report.score.warning_issues, 0, "should have no warning issues");

        // Stats should reflect all strong
        assert_eq!(report.stats.strong_passwords, 3);
        assert_eq!(report.stats.weak_passwords, 0);
        assert_eq!(report.stats.expired_passwords, 0);
        assert_eq!(report.stats.with_2fa, 3);
        assert_eq!(report.stats.unique_passwords, 3);
    }
}
