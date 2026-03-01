//! Advanced search functionality for vault entries.
//!
//! This module provides powerful search and filtering capabilities
//! for finding entries in a vault.

use serde::{Deserialize, Serialize};

/// Sort order for search results.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Sort in ascending order (A-Z, oldest first, etc.).
    #[default]
    Ascending,
    /// Sort in descending order (Z-A, newest first, etc.).
    Descending,
}

/// Fields that can be used for sorting search results.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SortField {
    /// Sort by entry label (alphabetically).
    #[default]
    Label,
    /// Sort by username.
    Username,
    /// Sort by creation date.
    CreatedAt,
    /// Sort by last update date.
    UpdatedAt,
    /// Sort by last access date.
    LastAccessed,
    /// Sort by access count (frequency).
    AccessCount,
    /// Sort by password age.
    PasswordAge,
}

/// Comprehensive search filter for vault entries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilter {
    /// Text query to search in label, username, URL, and notes.
    pub query: Option<String>,

    /// Filter by entry type (Login, Card, etc.).
    pub entry_types: Vec<String>,

    /// Filter by tags (entries must have at least one of these tags).
    pub tags: Vec<String>,

    /// Filter by folder (exact match).
    pub folder: Option<String>,

    /// Include entries in subfolders when filtering by folder.
    pub include_subfolders: bool,

    /// Only show favorite entries.
    pub favorites_only: bool,

    /// Only show entries with TOTP enabled.
    pub has_totp: Option<bool>,

    /// Only show entries with attachments.
    pub has_attachments: Option<bool>,

    /// Only show entries with weak passwords.
    pub weak_passwords_only: bool,

    /// Only show entries with expired passwords.
    pub expired_only: bool,

    /// Only show entries with passwords expiring within N days.
    pub expiring_within_days: Option<u32>,

    /// Only show entries older than N days (by password age).
    pub password_older_than_days: Option<u32>,

    /// Only show entries with a specific color.
    pub color: Option<String>,

    /// Only show entries created after this timestamp.
    pub created_after: Option<u64>,

    /// Only show entries created before this timestamp.
    pub created_before: Option<u64>,

    /// Only show entries updated after this timestamp.
    pub updated_after: Option<u64>,

    /// Only show entries updated before this timestamp.
    pub updated_before: Option<u64>,

    /// Sort field.
    pub sort_by: SortField,

    /// Sort order.
    pub sort_order: SortOrder,

    /// Maximum number of results to return (0 = no limit).
    pub limit: usize,

    /// Number of results to skip (for pagination).
    pub offset: usize,
}

impl SearchFilter {
    /// Creates a new empty search filter.
    ///
    /// The returned filter matches all entries and can be refined with
    /// the builder methods.
    ///
    /// # Examples
    ///
    /// ```
    /// use lilypad_common::SearchFilter;
    ///
    /// let filter = SearchFilter::new()
    ///     .with_query("github")
    ///     .with_tag("dev")
    ///     .favorites_only();
    ///
    /// assert_eq!(filter.query.as_deref(), Some("github"));
    /// assert!(filter.favorites_only);
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the text query.
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Adds an entry type filter.
    pub fn with_entry_type(mut self, entry_type: impl Into<String>) -> Self {
        self.entry_types.push(entry_type.into());
        self
    }

    /// Adds a tag filter.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Sets the folder filter.
    pub fn with_folder(mut self, folder: impl Into<String>, include_subfolders: bool) -> Self {
        self.folder = Some(folder.into());
        self.include_subfolders = include_subfolders;
        self
    }

    /// Filters to favorites only.
    pub fn favorites_only(mut self) -> Self {
        self.favorites_only = true;
        self
    }

    /// Filters to entries with TOTP.
    pub fn with_totp(mut self, has_totp: bool) -> Self {
        self.has_totp = Some(has_totp);
        self
    }

    /// Filters to entries with weak passwords.
    pub fn weak_passwords_only(mut self) -> Self {
        self.weak_passwords_only = true;
        self
    }

    /// Filters to expired entries.
    pub fn expired_only(mut self) -> Self {
        self.expired_only = true;
        self
    }

    /// Filters to entries expiring within N days.
    pub fn expiring_within(mut self, days: u32) -> Self {
        self.expiring_within_days = Some(days);
        self
    }

    /// Sets the sort field and order.
    pub fn sorted_by(mut self, field: SortField, order: SortOrder) -> Self {
        self.sort_by = field;
        self.sort_order = order;
        self
    }

    /// Sets pagination.
    pub fn paginate(mut self, limit: usize, offset: usize) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }
}

/// Entry data for search (simplified for filtering).
#[derive(Debug, Clone)]
pub struct SearchableEntry {
    /// Entry label.
    pub label: String,
    /// Username if set.
    pub username: Option<String>,
    /// URL if set.
    pub url: Option<String>,
    /// Entry type as string.
    pub entry_type: String,
    /// Tags.
    pub tags: Vec<String>,
    /// Folder path.
    pub folder: Option<String>,
    /// Is favorite.
    pub is_favorite: bool,
    /// Has TOTP enabled.
    pub has_totp: bool,
    /// Has attachments.
    pub has_attachments: bool,
    /// Notes (for search).
    pub notes: Option<String>,
    /// Created timestamp.
    pub created_at: u64,
    /// Updated timestamp.
    pub updated_at: u64,
    /// Last accessed timestamp.
    pub last_accessed_at: Option<u64>,
    /// Access count.
    pub access_count: u64,
    /// Password age in days.
    pub password_age_days: u64,
    /// Days until password expires (None if no expiry).
    pub days_until_expiry: Option<i64>,
    /// Is password expired.
    pub is_expired: bool,
    /// Is password weak.
    pub is_weak_password: bool,
    /// Color label.
    pub color: Option<String>,
}

/// Result of a search operation.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Entry labels that match the filter.
    pub entries: Vec<String>,
    /// Total count before pagination.
    pub total_count: usize,
    /// Whether there are more results.
    pub has_more: bool,
}

/// Result of a fuzzy match attempt.
#[derive(Debug, Clone)]
pub struct FuzzyMatchResult {
    /// The label of the matched entry.
    pub label: String,
    /// Relevance score (higher = better match). 0 means no match.
    pub score: u32,
}

/// Performs a fuzzy match of `query` against `text`.
///
/// Characters from the query must appear in `text` in order but not necessarily
/// consecutively. Returns a relevance score (higher = better). A score of 0
/// means the query does not match the text at all.
///
/// Scoring:
/// - Base points for each matched character.
/// - Bonus for consecutive character matches.
/// - Bonus for matching at the start of the text.
/// - Bonus for matching at the start of a word (after a non-alphanumeric char).
/// - Penalty for large gaps between matched characters.
///
/// # Examples
///
/// ```
/// use lilypad_common::fuzzy_match;
///
/// // Exact prefix match scores high
/// let score = fuzzy_match("git", "github.com");
/// assert!(score > 0);
///
/// // Non-matching query returns 0
/// assert_eq!(fuzzy_match("xyz", "github"), 0);
///
/// // Empty query returns 0
/// assert_eq!(fuzzy_match("", "anything"), 0);
///
/// // Case-insensitive matching
/// assert!(fuzzy_match("GIT", "github") > 0);
/// ```
pub fn fuzzy_match(query: &str, text: &str) -> u32 {
    let query_lower: Vec<char> = query.to_lowercase().chars().collect();
    let text_lower: Vec<char> = text.to_lowercase().chars().collect();

    if query_lower.is_empty() {
        return 0;
    }

    if text_lower.is_empty() {
        return 0;
    }

    let mut query_idx = 0;
    let mut score: u32 = 0;
    let mut prev_match_idx: Option<usize> = None;
    let mut first_match = true;

    for (text_idx, &ch) in text_lower.iter().enumerate() {
        if query_idx >= query_lower.len() {
            break;
        }

        if ch == query_lower[query_idx] {
            // Base score for each matched character
            score += 10;

            // Bonus for matching at start of text
            if text_idx == 0 && first_match {
                score += 15;
            }

            // Bonus for matching at start of a word (after space, hyphen, underscore, etc.)
            if text_idx > 0 && !text_lower[text_idx - 1].is_alphanumeric() {
                score += 10;
            }

            // Bonus for consecutive matches
            if let Some(prev) = prev_match_idx {
                if text_idx == prev + 1 {
                    score += 15;
                } else {
                    // Penalty for gap between matches (larger gap = larger penalty)
                    let gap = (text_idx - prev - 1) as u32;
                    score = score.saturating_sub(gap.min(5));
                }
            }

            prev_match_idx = Some(text_idx);
            query_idx += 1;
            first_match = false;
        }
    }

    // If we did not match all query characters, no match
    if query_idx < query_lower.len() {
        return 0;
    }

    // Bonus for shorter texts (tighter match)
    if text_lower.len() < 20 {
        score += (20 - text_lower.len() as u32) / 2;
    }

    score
}

/// Advanced search engine for vault entries.
pub struct AdvancedSearch;

impl AdvancedSearch {
    /// Searches entries using the given filter.
    ///
    /// Applies every active predicate in the filter, sorts the results,
    /// and returns a paginated [`SearchResult`].
    ///
    /// # Examples
    ///
    /// ```
    /// use lilypad_common::search::{AdvancedSearch, SearchableEntry, SearchFilter};
    ///
    /// let entries = vec![
    ///     SearchableEntry {
    ///         label: "Gmail".to_string(),
    ///         username: Some("user@gmail.com".to_string()),
    ///         url: Some("https://mail.google.com".to_string()),
    ///         entry_type: "Login".to_string(),
    ///         tags: vec!["email".to_string()],
    ///         folder: None, is_favorite: false, has_totp: false,
    ///         has_attachments: false, notes: None,
    ///         created_at: 1000, updated_at: 2000,
    ///         last_accessed_at: None, access_count: 0,
    ///         password_age_days: 10, days_until_expiry: None,
    ///         is_expired: false, is_weak_password: false, color: None,
    ///     },
    ///     SearchableEntry {
    ///         label: "GitHub".to_string(),
    ///         username: Some("dev".to_string()),
    ///         url: Some("https://github.com".to_string()),
    ///         entry_type: "Login".to_string(),
    ///         tags: vec!["dev".to_string()],
    ///         folder: None, is_favorite: false, has_totp: false,
    ///         has_attachments: false, notes: None,
    ///         created_at: 1000, updated_at: 2000,
    ///         last_accessed_at: None, access_count: 0,
    ///         password_age_days: 10, days_until_expiry: None,
    ///         is_expired: false, is_weak_password: false, color: None,
    ///     },
    /// ];
    ///
    /// let filter = SearchFilter::new().with_query("git");
    /// let result = AdvancedSearch::search(&entries, &filter);
    /// assert_eq!(result.total_count, 1);
    /// assert_eq!(result.entries[0], "GitHub");
    /// ```
    pub fn search(entries: &[SearchableEntry], filter: &SearchFilter) -> SearchResult {
        let mut matching: Vec<&SearchableEntry> = entries
            .iter()
            .filter(|entry| Self::matches_filter(entry, filter))
            .collect();

        let total_count = matching.len();

        // Sort results
        Self::sort_entries(&mut matching, filter.sort_by, filter.sort_order);

        // Apply pagination
        let has_more = if filter.limit > 0 {
            filter.offset + filter.limit < total_count
        } else {
            false
        };

        let entries: Vec<String> = if filter.limit > 0 {
            matching
                .into_iter()
                .skip(filter.offset)
                .take(filter.limit)
                .map(|e| e.label.clone())
                .collect()
        } else {
            matching
                .into_iter()
                .skip(filter.offset)
                .map(|e| e.label.clone())
                .collect()
        };

        SearchResult {
            entries,
            total_count,
            has_more,
        }
    }

    /// Checks if an entry matches the filter.
    fn matches_filter(entry: &SearchableEntry, filter: &SearchFilter) -> bool {
        // Text query
        if let Some(ref query) = filter.query {
            let q = query.to_lowercase();
            let matches_label = entry.label.to_lowercase().contains(&q);
            let matches_username = entry
                .username
                .as_ref()
                .map(|u| u.to_lowercase().contains(&q))
                .unwrap_or(false);
            let matches_url = entry
                .url
                .as_ref()
                .map(|u| u.to_lowercase().contains(&q))
                .unwrap_or(false);
            let matches_notes = entry
                .notes
                .as_ref()
                .map(|n| n.to_lowercase().contains(&q))
                .unwrap_or(false);
            let matches_tags = entry.tags.iter().any(|t| t.to_lowercase().contains(&q));

            if !(matches_label || matches_username || matches_url || matches_notes || matches_tags)
            {
                return false;
            }
        }

        // Entry type filter
        if !filter.entry_types.is_empty()
            && !filter
                .entry_types
                .iter()
                .any(|t| t.eq_ignore_ascii_case(&entry.entry_type))
        {
            return false;
        }

        // Tag filter
        if !filter.tags.is_empty() {
            let has_matching_tag = filter.tags.iter().any(|filter_tag| {
                entry
                    .tags
                    .iter()
                    .any(|entry_tag| entry_tag.eq_ignore_ascii_case(filter_tag))
            });
            if !has_matching_tag {
                return false;
            }
        }

        // Folder filter
        if let Some(ref folder) = filter.folder {
            match &entry.folder {
                Some(entry_folder) => {
                    if filter.include_subfolders {
                        let prefix = format!("{}/", folder);
                        if entry_folder != folder && !entry_folder.starts_with(&prefix) {
                            return false;
                        }
                    } else if entry_folder != folder {
                        return false;
                    }
                }
                None => return false,
            }
        }

        // Favorites filter
        if filter.favorites_only && !entry.is_favorite {
            return false;
        }

        // TOTP filter
        if let Some(has_totp) = filter.has_totp {
            if entry.has_totp != has_totp {
                return false;
            }
        }

        // Attachments filter
        if let Some(has_attachments) = filter.has_attachments {
            if entry.has_attachments != has_attachments {
                return false;
            }
        }

        // Weak password filter
        if filter.weak_passwords_only && !entry.is_weak_password {
            return false;
        }

        // Expired filter
        if filter.expired_only && !entry.is_expired {
            return false;
        }

        // Expiring within N days filter
        if let Some(days) = filter.expiring_within_days {
            match entry.days_until_expiry {
                Some(d) if d >= 0 && d <= days as i64 => {}
                _ => return false,
            }
        }

        // Password older than N days filter
        if let Some(days) = filter.password_older_than_days {
            if entry.password_age_days < days as u64 {
                return false;
            }
        }

        // Color filter
        if let Some(ref color) = filter.color {
            match &entry.color {
                Some(c) if c.eq_ignore_ascii_case(color) => {}
                _ => return false,
            }
        }

        // Date range filters
        if let Some(after) = filter.created_after {
            if entry.created_at < after {
                return false;
            }
        }

        if let Some(before) = filter.created_before {
            if entry.created_at > before {
                return false;
            }
        }

        if let Some(after) = filter.updated_after {
            if entry.updated_at < after {
                return false;
            }
        }

        if let Some(before) = filter.updated_before {
            if entry.updated_at > before {
                return false;
            }
        }

        true
    }

    /// Sorts entries by the specified field and order.
    fn sort_entries(
        entries: &mut [&SearchableEntry],
        field: SortField,
        order: SortOrder,
    ) {
        entries.sort_by(|a, b| {
            let cmp = match field {
                SortField::Label => a.label.to_lowercase().cmp(&b.label.to_lowercase()),
                SortField::Username => {
                    let a_user = a.username.as_deref().unwrap_or("");
                    let b_user = b.username.as_deref().unwrap_or("");
                    a_user.to_lowercase().cmp(&b_user.to_lowercase())
                }
                SortField::CreatedAt => a.created_at.cmp(&b.created_at),
                SortField::UpdatedAt => a.updated_at.cmp(&b.updated_at),
                SortField::LastAccessed => {
                    a.last_accessed_at.unwrap_or(0).cmp(&b.last_accessed_at.unwrap_or(0))
                }
                SortField::AccessCount => a.access_count.cmp(&b.access_count),
                SortField::PasswordAge => a.password_age_days.cmp(&b.password_age_days),
            };

            match order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
            }
        });
    }

    /// Performs a fuzzy search as a fallback when substring matching yields no results.
    ///
    /// First attempts the standard substring search. If no results are found and a
    /// text query is provided, falls back to fuzzy matching against the entry label,
    /// username, and URL. Results are sorted by fuzzy relevance score (best first).
    pub fn fuzzy_search(entries: &[SearchableEntry], filter: &SearchFilter) -> SearchResult {
        // First try the normal search
        let normal_result = Self::search(entries, filter);
        if normal_result.total_count > 0 {
            return normal_result;
        }

        // If there's no query, fuzzy search doesn't apply
        let query = match &filter.query {
            Some(q) if !q.is_empty() => q.clone(),
            _ => return normal_result,
        };

        // Build a filter without the text query so we can apply all other filters
        let mut filter_without_query = filter.clone();
        filter_without_query.query = None;

        // Collect entries that pass all non-query filters
        let candidates: Vec<&SearchableEntry> = entries
            .iter()
            .filter(|entry| Self::matches_filter(entry, &filter_without_query))
            .collect();

        // Score each candidate with fuzzy match
        let mut scored: Vec<(&SearchableEntry, u32)> = candidates
            .into_iter()
            .filter_map(|entry| {
                // Try fuzzy match against label, username, and URL; take best score
                let label_score = fuzzy_match(&query, &entry.label);
                let username_score = entry
                    .username
                    .as_ref()
                    .map(|u| fuzzy_match(&query, u))
                    .unwrap_or(0);
                let url_score = entry
                    .url
                    .as_ref()
                    .map(|u| fuzzy_match(&query, u))
                    .unwrap_or(0);

                let best_score = label_score.max(username_score).max(url_score);
                if best_score > 0 {
                    Some((entry, best_score))
                } else {
                    None
                }
            })
            .collect();

        // Sort by fuzzy score descending (best matches first)
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        let total_count = scored.len();

        let has_more = if filter.limit > 0 {
            filter.offset + filter.limit < total_count
        } else {
            false
        };

        let result_entries: Vec<String> = if filter.limit > 0 {
            scored
                .into_iter()
                .skip(filter.offset)
                .take(filter.limit)
                .map(|(e, _)| e.label.clone())
                .collect()
        } else {
            scored
                .into_iter()
                .skip(filter.offset)
                .map(|(e, _)| e.label.clone())
                .collect()
        };

        SearchResult {
            entries: result_entries,
            total_count,
            has_more,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(label: &str) -> SearchableEntry {
        SearchableEntry {
            label: label.to_string(),
            username: Some("user".to_string()),
            url: Some("https://example.com".to_string()),
            entry_type: "Login".to_string(),
            tags: vec!["test".to_string()],
            folder: Some("Work".to_string()),
            is_favorite: false,
            has_totp: false,
            has_attachments: false,
            notes: None,
            created_at: 1000,
            updated_at: 2000,
            last_accessed_at: Some(3000),
            access_count: 5,
            password_age_days: 30,
            days_until_expiry: None,
            is_expired: false,
            is_weak_password: false,
            color: None,
        }
    }

    #[test]
    fn test_query_search() {
        let entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
            create_test_entry("Facebook"),
        ];

        let filter = SearchFilter::new().with_query("git");
        let result = AdvancedSearch::search(&entries, &filter);

        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "GitHub");
    }

    #[test]
    fn test_favorites_filter() {
        let mut entries = vec![
            create_test_entry("Entry1"),
            create_test_entry("Entry2"),
        ];
        entries[0].is_favorite = true;

        let filter = SearchFilter::new().favorites_only();
        let result = AdvancedSearch::search(&entries, &filter);

        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "Entry1");
    }

    #[test]
    fn test_folder_filter() {
        let mut entries = vec![
            create_test_entry("Entry1"),
            create_test_entry("Entry2"),
        ];
        entries[0].folder = Some("Work".to_string());
        entries[1].folder = Some("Work/Projects".to_string());

        // Exact folder match
        let filter = SearchFilter::new().with_folder("Work", false);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 1);

        // Including subfolders
        let filter = SearchFilter::new().with_folder("Work", true);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 2);
    }

    #[test]
    fn test_sorting() {
        let mut entries = vec![
            create_test_entry("Zebra"),
            create_test_entry("Apple"),
            create_test_entry("Mango"),
        ];
        entries[0].access_count = 10;
        entries[1].access_count = 5;
        entries[2].access_count = 15;

        let filter = SearchFilter::new().sorted_by(SortField::Label, SortOrder::Ascending);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.entries, vec!["Apple", "Mango", "Zebra"]);

        let filter = SearchFilter::new().sorted_by(SortField::AccessCount, SortOrder::Descending);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.entries, vec!["Mango", "Zebra", "Apple"]);
    }

    #[test]
    fn test_pagination() {
        let entries: Vec<SearchableEntry> = (0..10)
            .map(|i| create_test_entry(&format!("Entry{}", i)))
            .collect();

        let filter = SearchFilter::new().paginate(3, 0);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.total_count, 10);
        assert!(result.has_more);

        let filter = SearchFilter::new().paginate(3, 9);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.entries.len(), 1);
        assert!(!result.has_more);
    }

    #[test]
    fn test_search_by_tag() {
        let mut entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
            create_test_entry("Facebook"),
        ];
        entries[0].tags = vec!["email".to_string(), "google".to_string()];
        entries[1].tags = vec!["dev".to_string(), "code".to_string()];
        entries[2].tags = vec!["social".to_string()];

        let filter = SearchFilter::new().with_tag("email");
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "Gmail");

        // Multiple tags: entries matching ANY of the tags
        let filter = SearchFilter::new().with_tag("email").with_tag("dev");
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 2);
        assert!(result.entries.contains(&"Gmail".to_string()));
        assert!(result.entries.contains(&"GitHub".to_string()));
    }

    #[test]
    fn test_search_case_insensitive() {
        let entries = vec![
            create_test_entry("Gmail Account"),
            create_test_entry("GitHub"),
        ];

        // Query with different case should still match
        let filter = SearchFilter::new().with_query("GMAIL");
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "Gmail Account");

        let filter = SearchFilter::new().with_query("github");
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "GitHub");

        // Mixed case
        let filter = SearchFilter::new().with_query("gMaIl");
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "Gmail Account");
    }

    #[test]
    fn test_search_by_url() {
        let mut entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
            create_test_entry("Facebook"),
        ];
        entries[0].url = Some("https://mail.google.com".to_string());
        entries[1].url = Some("https://github.com".to_string());
        entries[2].url = Some("https://facebook.com".to_string());

        let filter = SearchFilter::new().with_query("github.com");
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "GitHub");

        // Partial URL match
        let filter = SearchFilter::new().with_query("google");
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "Gmail");
    }

    #[test]
    fn test_search_empty_query() {
        let entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
            create_test_entry("Facebook"),
        ];

        // No query set (None) should return all entries
        let filter = SearchFilter::new();
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 3);
        assert_eq!(result.entries.len(), 3);

        // Empty string query should also return all (contains "" is always true)
        let filter = SearchFilter::new().with_query("");
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 3);
        assert_eq!(result.entries.len(), 3);
    }

    #[test]
    fn test_search_no_results() {
        let entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
            create_test_entry("Facebook"),
        ];

        let filter = SearchFilter::new().with_query("nonexistent_xyz_123");
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 0);
        assert!(result.entries.is_empty());
        assert!(!result.has_more);
    }

    #[test]
    fn test_sort_by_name() {
        let entries = vec![
            create_test_entry("Zebra"),
            create_test_entry("Apple"),
            create_test_entry("Mango"),
            create_test_entry("banana"), // lowercase to test case-insensitive sort
        ];

        let filter = SearchFilter::new().sorted_by(SortField::Label, SortOrder::Ascending);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.entries, vec!["Apple", "banana", "Mango", "Zebra"]);

        let filter = SearchFilter::new().sorted_by(SortField::Label, SortOrder::Descending);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.entries, vec!["Zebra", "Mango", "banana", "Apple"]);
    }

    #[test]
    fn test_sort_by_date() {
        let mut entries = vec![
            create_test_entry("Oldest"),
            create_test_entry("Middle"),
            create_test_entry("Newest"),
        ];
        entries[0].created_at = 1000;
        entries[1].created_at = 2000;
        entries[2].created_at = 3000;

        let filter = SearchFilter::new().sorted_by(SortField::CreatedAt, SortOrder::Ascending);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.entries, vec!["Oldest", "Middle", "Newest"]);

        let filter = SearchFilter::new().sorted_by(SortField::CreatedAt, SortOrder::Descending);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.entries, vec!["Newest", "Middle", "Oldest"]);

        // Also test UpdatedAt sorting
        entries[0].updated_at = 5000;
        entries[1].updated_at = 3000;
        entries[2].updated_at = 4000;

        let filter = SearchFilter::new().sorted_by(SortField::UpdatedAt, SortOrder::Ascending);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.entries, vec!["Middle", "Newest", "Oldest"]);
    }

    #[test]
    fn test_filter_favorites_only() {
        let mut entries = vec![
            create_test_entry("Favorite1"),
            create_test_entry("Normal1"),
            create_test_entry("Favorite2"),
            create_test_entry("Normal2"),
        ];
        entries[0].is_favorite = true;
        entries[2].is_favorite = true;

        let filter = SearchFilter::new().favorites_only();
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 2);
        assert!(result.entries.contains(&"Favorite1".to_string()));
        assert!(result.entries.contains(&"Favorite2".to_string()));
        assert!(!result.entries.contains(&"Normal1".to_string()));
        assert!(!result.entries.contains(&"Normal2".to_string()));
    }

    #[test]
    fn test_filter_by_folder() {
        let mut entries = vec![
            create_test_entry("Entry1"),
            create_test_entry("Entry2"),
            create_test_entry("Entry3"),
            create_test_entry("Entry4"),
        ];
        entries[0].folder = Some("Personal".to_string());
        entries[1].folder = Some("Work".to_string());
        entries[2].folder = Some("Personal/Banking".to_string());
        entries[3].folder = None;

        // Exact folder match
        let filter = SearchFilter::new().with_folder("Personal", false);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "Entry1");

        // With subfolders
        let filter = SearchFilter::new().with_folder("Personal", true);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 2);
        assert!(result.entries.contains(&"Entry1".to_string()));
        assert!(result.entries.contains(&"Entry3".to_string()));

        // Folder that doesn't exist
        let filter = SearchFilter::new().with_folder("Nonexistent", false);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 0);

        // Entry with no folder should not match any folder filter
        let filter = SearchFilter::new().with_folder("Work", false);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "Entry2");
    }

    #[test]
    fn test_pagination_bounds() {
        let entries: Vec<SearchableEntry> = (0..5)
            .map(|i| create_test_entry(&format!("Entry{}", i)))
            .collect();

        // Offset beyond available entries
        let filter = SearchFilter::new().paginate(10, 100);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 5);
        assert!(result.entries.is_empty());
        assert!(!result.has_more);

        // Limit of 0 means no limit
        let filter = SearchFilter::new().paginate(0, 0);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 5);
        assert_eq!(result.entries.len(), 5);
        assert!(!result.has_more);

        // Offset at exact boundary
        let filter = SearchFilter::new().paginate(3, 5);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 5);
        assert!(result.entries.is_empty());

        // Limit larger than remaining entries
        let filter = SearchFilter::new().paginate(10, 3);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 5);
        assert_eq!(result.entries.len(), 2);
        assert!(!result.has_more);

        // Single page exactly matching count
        let filter = SearchFilter::new().paginate(5, 0);
        let result = AdvancedSearch::search(&entries, &filter);
        assert_eq!(result.total_count, 5);
        assert_eq!(result.entries.len(), 5);
        assert!(!result.has_more);
    }

    // ========================================================================
    // Fuzzy match tests
    // ========================================================================

    #[test]
    fn test_fuzzy_match_basic() {
        // "gml" should match "Gmail" (g...m...l in order)
        let score = fuzzy_match("gml", "Gmail");
        assert!(score > 0, "gml should fuzzy-match Gmail, got score {}", score);
    }

    #[test]
    fn test_fuzzy_match_exact_substring() {
        // Exact substring should produce a high score
        let score = fuzzy_match("mail", "Gmail");
        assert!(score > 0);
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        // Characters not in order should not match
        let score = fuzzy_match("xyz", "Gmail");
        assert_eq!(score, 0, "xyz should not match Gmail");
    }

    #[test]
    fn test_fuzzy_match_empty_query() {
        let score = fuzzy_match("", "Gmail");
        assert_eq!(score, 0);
    }

    #[test]
    fn test_fuzzy_match_empty_text() {
        let score = fuzzy_match("gml", "");
        assert_eq!(score, 0);
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        let score_lower = fuzzy_match("gml", "gmail");
        let score_upper = fuzzy_match("GML", "Gmail");
        assert!(score_lower > 0);
        assert!(score_upper > 0);
        // Both should produce the same score since matching is case-insensitive
        assert_eq!(score_lower, score_upper);
    }

    #[test]
    fn test_fuzzy_match_consecutive_bonus() {
        // "git" in "GitHub" has consecutive g-i-t, should score higher than
        // "gib" in "GitHub" which has g-i then skips to b (not present -> 0)
        let score_consecutive = fuzzy_match("git", "GitHub");
        let score_sparse = fuzzy_match("ghb", "GitHub");
        assert!(score_consecutive > 0);
        assert!(score_sparse > 0);
        assert!(
            score_consecutive > score_sparse,
            "consecutive matches should score higher: {} vs {}",
            score_consecutive,
            score_sparse
        );
    }

    #[test]
    fn test_fuzzy_match_start_bonus() {
        // Matching at start of text should score higher
        let score_start = fuzzy_match("gi", "GitHub");
        let score_middle = fuzzy_match("hu", "GitHub");
        assert!(score_start > 0);
        assert!(score_middle > 0);
        assert!(
            score_start > score_middle,
            "start match should score higher: {} vs {}",
            score_start,
            score_middle
        );
    }

    #[test]
    fn test_fuzzy_match_query_longer_than_text() {
        let score = fuzzy_match("toolongquery", "Git");
        assert_eq!(score, 0, "query longer than text should not match");
    }

    #[test]
    fn test_fuzzy_search_fallback() {
        let entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
            create_test_entry("Facebook"),
        ];

        // "gml" does not substring-match anything, but fuzzy matches "Gmail"
        let filter = SearchFilter::new().with_query("gml");
        let result = AdvancedSearch::fuzzy_search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "Gmail");
    }

    #[test]
    fn test_fuzzy_search_prefers_exact_when_available() {
        let entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
            create_test_entry("Facebook"),
        ];

        // "Git" substring-matches "GitHub", so normal search should be used
        let filter = SearchFilter::new().with_query("Git");
        let result = AdvancedSearch::fuzzy_search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "GitHub");
    }

    #[test]
    fn test_fuzzy_search_no_match() {
        let entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
            create_test_entry("Facebook"),
        ];

        let filter = SearchFilter::new().with_query("zzzzz");
        let result = AdvancedSearch::fuzzy_search(&entries, &filter);
        assert_eq!(result.total_count, 0);
        assert!(result.entries.is_empty());
    }

    #[test]
    fn test_fuzzy_search_respects_other_filters() {
        let mut entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
        ];
        entries[0].is_favorite = true;
        entries[1].is_favorite = false;

        // "gml" fuzzy-matches "Gmail" but we also require favorites
        let filter = SearchFilter::new().with_query("gml").favorites_only();
        let result = AdvancedSearch::fuzzy_search(&entries, &filter);
        assert_eq!(result.total_count, 1);
        assert_eq!(result.entries[0], "Gmail");

        // "ghb" fuzzy-matches "GitHub" but GitHub is not a favorite
        let filter = SearchFilter::new().with_query("ghb").favorites_only();
        let result = AdvancedSearch::fuzzy_search(&entries, &filter);
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_fuzzy_search_sorted_by_relevance() {
        let entries = vec![
            create_test_entry("My Google Mail"),
            create_test_entry("Gmail"),
        ];

        // "gml" should match both, but "Gmail" should score higher (tighter match)
        let filter = SearchFilter::new().with_query("gml");
        let result = AdvancedSearch::fuzzy_search(&entries, &filter);
        assert_eq!(result.total_count, 2);
        assert_eq!(result.entries[0], "Gmail", "Gmail should rank first due to tighter match");
    }

    #[test]
    fn test_fuzzy_search_empty_query() {
        let entries = vec![
            create_test_entry("Gmail"),
            create_test_entry("GitHub"),
        ];

        // No query means return all (via normal search path)
        let filter = SearchFilter::new();
        let result = AdvancedSearch::fuzzy_search(&entries, &filter);
        assert_eq!(result.total_count, 2);
    }
}
