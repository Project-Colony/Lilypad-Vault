//! The vault, its merge logic, deletion markers and the audit log.

use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BTreeSet;

use super::{
    current_timestamp, Entry, EntryColor, EntryMetadata, KeyMetadata, MAX_FOLDER_DEPTH,
    MAX_FOLDER_LENGTH, MAX_LABEL_LENGTH, MAX_TAGS_PER_ENTRY, MAX_TAG_LENGTH,
};
use crate::crypto::Ciphertext;
use crate::error::{CoreError, CoreResult as Result};

/// How far into the future a timestamp may claim to be before a merge clamps
/// it (see [`Vault::merge_from`]). Generous enough for ordinary clock drift,
/// small enough that a replica with a broken clock cannot poison a label
/// forever.
pub const MAX_CLOCK_SKEW_SECS: u64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vault {
    pub name: String,
    pub entries: Vec<Entry>,
    pub key_metadata: KeyMetadata,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub last_accessed_at: Option<u64>,
    #[serde(default)]
    pub audit_log: Vec<AuditEvent>,
    /// Deletion markers for entries that were permanently removed (or renamed
    /// away). Without these, a sync merge would resurrect a purged entry from
    /// any replica that still holds a live copy. Labels only - no secret data.
    #[serde(default)]
    pub tombstones: Vec<Tombstone>,
}

/// A deletion marker: entry `label` was removed at `deleted_at`. During a merge
/// the tombstone wins over any copy of the entry not updated after it.
///
/// `id` pins the marker to the specific entry that was deleted, so a DIFFERENT
/// entry that later comes to live under the same label (e.g. renamed onto it)
/// is not mistaken for the deleted one and destroyed by the marker. An empty
/// `id` (older data) conservatively matches any entry with the label.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tombstone {
    pub label: String,
    #[serde(default)]
    pub id: String,
    pub deleted_at: u64,
}

impl Tombstone {
    /// Whether this marker refers to the given entry: same label, and the
    /// entry identity matches (an empty id on either side - pre-id data -
    /// falls back to label-only matching).
    fn refers_to(&self, entry: &Entry) -> bool {
        self.label == entry.label
            && (self.id.is_empty() || entry.id.is_empty() || self.id == entry.id)
    }
}

impl Vault {
    pub fn new(name: impl Into<String>, key_metadata: KeyMetadata) -> Self {
        let now = current_timestamp();
        Self {
            name: name.into(),
            entries: Vec::new(),
            key_metadata,
            description: None,
            created_at: now,
            updated_at: now,
            last_accessed_at: None,
            audit_log: Vec::new(),
            tombstones: Vec::new(),
        }
    }

    /// Upserts a deletion marker for the entry `id` that lived under `label`,
    /// stamped at the current time.
    fn record_tombstone(&mut self, label: &str, id: &str) {
        let now = current_timestamp();
        match self
            .tombstones
            .iter_mut()
            .find(|t| t.label == label && t.id == id)
        {
            Some(t) => t.deleted_at = t.deleted_at.max(now),
            None => self.tombstones.push(Tombstone {
                label: label.to_string(),
                id: id.to_string(),
                deleted_at: now,
            }),
        }
    }

    pub fn set_description(&mut self, description: Option<String>) {
        self.description = description;
        self.touch();
        self.record_event(AuditEvent::new("vault_description_updated", None));
    }

    pub fn record_access(&mut self) {
        self.last_accessed_at = Some(current_timestamp());
        self.record_event(AuditEvent::new("vault_accessed", None));
    }

    pub fn add_entry(&mut self, entry: Entry) -> Result<()> {
        if entry.label.trim().is_empty() {
            return Err(CoreError::InvalidInput(
                "entry label cannot be empty".to_string(),
            ));
        }
        if entry.label.len() > MAX_LABEL_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "entry label too long ({} chars, max {} chars)",
                entry.label.len(),
                MAX_LABEL_LENGTH
            )));
        }
        if self
            .entries
            .iter()
            .any(|existing| existing.label == entry.label)
        {
            return Err(CoreError::AlreadyExists(format!(
                "entry label '{}' already exists",
                entry.label
            )));
        }
        // Validate entry metadata
        entry.metadata.validate()?;
        // Re-creating a previously removed label lifts its deletion marker,
        // otherwise a later merge could treat the new entry as already deleted.
        self.tombstones.retain(|t| t.label != entry.label);
        self.entries.push(entry);
        self.touch();
        self.record_event(AuditEvent::new("entry_added", None));
        Ok(())
    }

    pub fn update_entry(&mut self, label: &str, ciphertext: Ciphertext) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.ciphertext = ciphertext;
        entry.updated_at = current_timestamp();
        self.touch();
        self.record_event(AuditEvent::new("entry_updated", Some(label)));
        Ok(())
    }

    pub fn remove_entry(&mut self, label: &str) -> Result<Entry> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        let removed = self.entries.remove(index);
        self.record_tombstone(label, &removed.id);
        self.touch();
        self.record_event(AuditEvent::new("entry_removed", Some(label)));
        Ok(removed)
    }

    pub fn rename_entry(&mut self, label: &str, new_label: impl Into<String>) -> Result<()> {
        let new_label = new_label.into();
        if new_label.trim().is_empty() {
            return Err(CoreError::InvalidInput(
                "entry label cannot be empty".to_string(),
            ));
        }
        if new_label.len() > MAX_LABEL_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "entry label too long ({} chars, max {} chars)",
                new_label.len(),
                MAX_LABEL_LENGTH
            )));
        }
        if label == new_label {
            return Ok(());
        }
        if self
            .entries
            .iter()
            .any(|existing| existing.label == new_label)
        {
            return Err(CoreError::AlreadyExists(format!(
                "entry label '{}' already exists",
                new_label
            )));
        }
        let (event_label, entry_id) = {
            let entry = self
                .entries
                .iter_mut()
                .find(|entry| entry.label == label)
                .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
            entry.label = new_label;
            entry.updated_at = current_timestamp();
            (entry.label.clone(), entry.id.clone())
        };
        // The old label no longer names a live entry: without a tombstone, a
        // merge with a replica that still holds the pre-rename copy would
        // duplicate the entry under both labels. The marker carries the
        // entry's id, so it only ever kills copies of THIS entry - never an
        // unrelated future entry that happens to reuse the label. (For the
        // same reason, a pre-existing tombstone on the destination label is
        // deliberately left in place: it refers to a different id and is
        // still needed to kill stale copies of the previously deleted entry
        // on other replicas.)
        self.record_tombstone(label, &entry_id);
        self.touch();
        self.record_event(AuditEvent::new("entry_renamed", Some(&event_label)));
        Ok(())
    }

    pub fn update_entry_metadata(&mut self, label: &str, metadata: EntryMetadata) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.metadata = metadata;
        entry.updated_at = current_timestamp();
        self.touch();
        self.record_event(AuditEvent::new("entry_metadata_updated", Some(label)));
        Ok(())
    }

    pub fn set_entry_folder(&mut self, label: &str, folder: Option<String>) -> Result<()> {
        if let Some(ref f) = folder {
            Self::validate_folder_path(f)?;
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.metadata.folder = folder;
        entry.updated_at = current_timestamp();
        self.touch();
        self.record_event(AuditEvent::new("entry_folder_updated", Some(label)));
        Ok(())
    }

    pub fn add_entry_tag(&mut self, label: &str, tag: impl Into<String>) -> Result<()> {
        let tag = tag.into();
        if tag.trim().is_empty() {
            return Err(CoreError::InvalidInput("tag cannot be empty".to_string()));
        }
        if tag.len() > MAX_TAG_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "tag too long ({} chars, max {} chars)",
                tag.len(),
                MAX_TAG_LENGTH
            )));
        }
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        if entry.metadata.tags.len() >= MAX_TAGS_PER_ENTRY {
            return Err(CoreError::InvalidInput(format!(
                "too many tags ({}, max {})",
                entry.metadata.tags.len(),
                MAX_TAGS_PER_ENTRY
            )));
        }
        if !entry.metadata.tags.iter().any(|existing| existing == &tag) {
            entry.metadata.tags.push(tag);
            entry.updated_at = current_timestamp();
            self.touch();
            self.record_event(AuditEvent::new("entry_tag_added", Some(label)));
        }
        Ok(())
    }

    pub fn remove_entry_tag(&mut self, label: &str, tag: &str) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        let index = entry
            .metadata
            .tags
            .iter()
            .position(|existing| existing == tag)
            .ok_or_else(|| CoreError::NotFound(format!("tag '{tag}'")))?;
        entry.metadata.tags.remove(index);
        entry.updated_at = current_timestamp();
        self.touch();
        self.record_event(AuditEvent::new("entry_tag_removed", Some(label)));
        Ok(())
    }

    pub fn find_entry(&self, label: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.label == label)
    }

    pub fn search_entries(&self, query: &str) -> Vec<&Entry> {
        let needle = query.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| entry.matches_query(&needle))
            .collect()
    }

    pub fn entries_by_folder(&self, folder: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.metadata.folder.as_deref() == Some(folder))
            .collect()
    }

    pub fn entries_by_tag(&self, tag: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.metadata.tags.iter().any(|value| value == tag))
            .collect()
    }

    pub fn list_folders(&self) -> Vec<String> {
        let mut folders = BTreeSet::new();
        for entry in &self.entries {
            if let Some(folder) = entry.metadata.folder.as_deref() {
                if !folder.trim().is_empty() {
                    folders.insert(folder.to_string());
                }
            }
        }
        folders.into_iter().collect()
    }

    pub fn list_tags(&self) -> Vec<String> {
        let mut tags = BTreeSet::new();
        for entry in &self.entries {
            for tag in &entry.metadata.tags {
                if !tag.trim().is_empty() {
                    tags.insert(tag.to_string());
                }
            }
        }
        tags.into_iter().collect()
    }

    /// Returns all favorite entries.
    pub fn favorites(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.is_favorite)
            .collect()
    }

    /// Returns recently used entries, sorted by last access time (most recent first).
    pub fn recently_used(&self, limit: usize) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|entry| entry.last_accessed_at.is_some())
            .collect();
        entries.sort_by(|a, b| {
            b.last_accessed_at
                .unwrap_or(0)
                .cmp(&a.last_accessed_at.unwrap_or(0))
        });
        entries.into_iter().take(limit).collect()
    }

    /// Returns most frequently used entries, sorted by access count (highest first).
    pub fn most_used(&self, limit: usize) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|entry| entry.access_count > 0)
            .collect();
        entries.sort_by_key(|entry| Reverse(entry.access_count));
        entries.into_iter().take(limit).collect()
    }

    /// Sets an entry as favorite or not.
    pub fn set_entry_favorite(&mut self, label: &str, favorite: bool) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.set_favorite(favorite);
        self.touch();
        Ok(())
    }

    /// Records that an entry was accessed.
    pub fn record_entry_access(&mut self, label: &str) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.record_access();
        Ok(())
    }

    /// Sets the color for an entry.
    pub fn set_entry_color(&mut self, label: &str, color: Option<EntryColor>) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.set_color(color);
        self.touch();
        Ok(())
    }

    /// Sets the icon for an entry.
    pub fn set_entry_icon(&mut self, label: &str, icon: Option<String>) -> Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.label == label)
            .ok_or_else(|| CoreError::NotFound(format!("entry '{label}'")))?;
        entry.set_icon(icon);
        self.touch();
        Ok(())
    }

    /// Lists all unique parent folders in the folder tree.
    /// For nested folders like "Work/Email/Google", this returns:
    /// ["Work", "Work/Email", "Work/Email/Google"]
    pub fn list_folder_tree(&self) -> Vec<String> {
        let mut folders = BTreeSet::new();
        for entry in &self.entries {
            if let Some(folder) = entry.metadata.folder.as_deref() {
                if !folder.trim().is_empty() {
                    // Add the full path and all parent paths
                    let parts: Vec<&str> = folder.split('/').collect();
                    let mut path = String::new();
                    for (i, part) in parts.iter().enumerate() {
                        if i > 0 {
                            path.push('/');
                        }
                        path.push_str(part);
                        folders.insert(path.clone());
                    }
                }
            }
        }
        folders.into_iter().collect()
    }

    /// Returns entries in a folder, including entries in nested subfolders.
    pub fn entries_in_folder_recursive(&self, folder: &str) -> Vec<&Entry> {
        let prefix = if folder.ends_with('/') {
            folder.to_string()
        } else {
            format!("{}/", folder)
        };
        self.entries
            .iter()
            .filter(|entry| {
                if let Some(f) = &entry.metadata.folder {
                    f == folder || f.starts_with(&prefix)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Returns only entries directly in a folder (not in subfolders).
    pub fn entries_in_folder_direct(&self, folder: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.metadata.folder.as_deref() == Some(folder))
            .collect()
    }

    /// Returns immediate child folders of a given folder.
    pub fn child_folders(&self, parent: &str) -> Vec<String> {
        let mut children = BTreeSet::new();
        let prefix = if parent.is_empty() {
            String::new()
        } else if parent.ends_with('/') {
            parent.to_string()
        } else {
            format!("{}/", parent)
        };

        for entry in &self.entries {
            if let Some(folder) = &entry.metadata.folder {
                let relative = if parent.is_empty() {
                    folder.as_str()
                } else if folder.starts_with(&prefix) {
                    &folder[prefix.len()..]
                } else {
                    continue;
                };

                // Get the first component of the relative path
                if let Some(child) = relative.split('/').next() {
                    if !child.is_empty() {
                        let full_path = if parent.is_empty() {
                            child.to_string()
                        } else {
                            format!("{}/{}", parent.trim_end_matches('/'), child)
                        };
                        children.insert(full_path);
                    }
                }
            }
        }
        children.into_iter().collect()
    }

    /// Returns entries that have no folder assigned.
    pub fn entries_without_folder(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| {
                entry.metadata.folder.is_none() || entry.metadata.folder.as_deref() == Some("")
            })
            .collect()
    }

    /// Validates a folder path for nested folder support.
    pub fn validate_folder_path(path: &str) -> Result<()> {
        if path.is_empty() {
            return Ok(());
        }
        if path.len() > MAX_FOLDER_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "folder path too long ({} chars, max {} chars)",
                path.len(),
                MAX_FOLDER_LENGTH
            )));
        }
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() > MAX_FOLDER_DEPTH {
            return Err(CoreError::InvalidInput(format!(
                "folder path too deep ({} levels, max {} levels)",
                parts.len(),
                MAX_FOLDER_DEPTH
            )));
        }
        for part in &parts {
            if part.is_empty() {
                return Err(CoreError::InvalidInput(
                    "folder path contains empty components".to_string(),
                ));
            }
            if part.contains("..") {
                return Err(CoreError::InvalidInput(
                    "folder path contains invalid characters".to_string(),
                ));
            }
        }
        Ok(())
    }

    // ==================== Bulk Operations ====================

    /// Deletes multiple entries at once. Returns the number of entries deleted.
    pub fn bulk_delete(&mut self, labels: &[&str]) -> Result<usize> {
        let mut deleted = 0;
        for label in labels {
            if let Some(pos) = self.entries.iter().position(|e| e.label == *label) {
                self.entries.remove(pos);
                self.record_event(AuditEvent::new("entry_removed", Some(label)));
                deleted += 1;
            }
        }
        if deleted > 0 {
            self.touch();
        }
        Ok(deleted)
    }

    /// Moves multiple entries to a folder. Returns the number of entries moved.
    pub fn bulk_move_to_folder(
        &mut self,
        labels: &[&str],
        folder: Option<String>,
    ) -> Result<usize> {
        if let Some(ref f) = folder {
            Self::validate_folder_path(f)?;
        }
        let mut moved = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str()) {
                entry.metadata.folder = folder.clone();
                entry.updated_at = current_timestamp();
                moved += 1;
            }
        }
        if moved > 0 {
            self.touch();
            self.record_event(AuditEvent::new("bulk_folder_updated", None));
        }
        Ok(moved)
    }

    /// Adds a tag to multiple entries. Returns the number of entries modified.
    pub fn bulk_add_tag(&mut self, labels: &[&str], tag: impl Into<String>) -> Result<usize> {
        let tag = tag.into();
        if tag.trim().is_empty() {
            return Err(CoreError::InvalidInput("tag cannot be empty".to_string()));
        }
        if tag.len() > MAX_TAG_LENGTH {
            return Err(CoreError::InvalidInput(format!(
                "tag too long ({} chars, max {} chars)",
                tag.len(),
                MAX_TAG_LENGTH
            )));
        }
        let mut modified = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str())
                && entry.metadata.tags.len() < MAX_TAGS_PER_ENTRY
                && !entry.metadata.tags.iter().any(|t| t == &tag)
            {
                entry.metadata.tags.push(tag.clone());
                entry.updated_at = current_timestamp();
                modified += 1;
            }
        }
        if modified > 0 {
            self.touch();
            self.record_event(AuditEvent::new("bulk_tag_added", None));
        }
        Ok(modified)
    }

    /// Removes a tag from multiple entries. Returns the number of entries modified.
    pub fn bulk_remove_tag(&mut self, labels: &[&str], tag: &str) -> Result<usize> {
        let mut modified = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str()) {
                if let Some(pos) = entry.metadata.tags.iter().position(|t| t == tag) {
                    entry.metadata.tags.remove(pos);
                    entry.updated_at = current_timestamp();
                    modified += 1;
                }
            }
        }
        if modified > 0 {
            self.touch();
            self.record_event(AuditEvent::new("bulk_tag_removed", None));
        }
        Ok(modified)
    }

    /// Sets favorite status for multiple entries. Returns the number of entries modified.
    pub fn bulk_set_favorite(&mut self, labels: &[&str], favorite: bool) -> Result<usize> {
        let mut modified = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str()) && entry.is_favorite != favorite {
                entry.is_favorite = favorite;
                entry.updated_at = current_timestamp();
                modified += 1;
            }
        }
        if modified > 0 {
            self.touch();
        }
        Ok(modified)
    }

    /// Sets color for multiple entries. Returns the number of entries modified.
    pub fn bulk_set_color(&mut self, labels: &[&str], color: Option<EntryColor>) -> Result<usize> {
        let mut modified = 0;
        for entry in &mut self.entries {
            if labels.contains(&entry.label.as_str()) && entry.color != color {
                entry.color = color;
                entry.updated_at = current_timestamp();
                modified += 1;
            }
        }
        if modified > 0 {
            self.touch();
        }
        Ok(modified)
    }

    /// Returns entries matching the given labels.
    pub fn get_entries(&self, labels: &[&str]) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| labels.contains(&e.label.as_str()))
            .collect()
    }

    /// Returns mutable entries matching the given labels.
    pub fn get_entries_mut(&mut self, labels: &[&str]) -> Vec<&mut Entry> {
        self.entries
            .iter_mut()
            .filter(|e| labels.contains(&e.label.as_str()))
            .collect()
    }

    /// Merges entries from a remote vault into this (local) vault.
    ///
    /// Conflict resolution strategy (per-entry, last-modified-wins):
    /// - Entries only in local → kept, unless a remote tombstone is newer
    /// - Entries only in remote → added, unless a local tombstone is newer
    /// - Entries in both with same `updated_at` → kept as-is (in sync)
    /// - Entries in both with different `updated_at` → newer version wins
    ///
    /// Tombstones (recorded by [`Self::remove_entry`] and [`Self::rename_entry`])
    /// are merged too (per-label, newest wins), so a permanent deletion or a
    /// rename on one replica propagates instead of the dead copy resurrecting
    /// from the other. A tombstone only beats an entry that was NOT updated
    /// after it: an edit or re-creation newer than the deletion survives, so a
    /// timestamp tie always favors keeping data.
    ///
    /// Entries are matched by label. The remote entries' ciphertexts are copied
    /// as-is: the caller must ensure they are sealed under THIS vault's key
    /// (re-encrypting them first if the replicas use different KDF salts).
    /// Returns a summary of the merge.
    pub fn merge_from(&mut self, remote: &Vault) -> MergeResult {
        use std::collections::HashMap;

        let mut result = MergeResult::default();

        // Clock-skew guard: a replica with a wildly wrong clock must not be
        // able to poison the merge forever (a far-future tombstone would kill
        // every re-creation of its label; a far-future entry could never be
        // edited over). Timestamps are clamped to "now + allowance" on BOTH
        // sides before any comparison, bounding the damage to the allowance
        // window instead of eternity.
        let cap = current_timestamp() + MAX_CLOCK_SKEW_SECS;
        for e in &mut self.entries {
            if e.updated_at > cap {
                e.updated_at = cap;
                result.timestamps_sanitized = true;
            }
        }
        for t in &mut self.tombstones {
            if t.deleted_at > cap {
                t.deleted_at = cap;
                result.timestamps_sanitized = true;
            }
        }
        let clamp_entry = |e: &Entry| {
            let mut e = e.clone();
            e.updated_at = e.updated_at.min(cap);
            e
        };
        let clamp_tombstone = |t: &Tombstone| {
            let mut t = t.clone();
            t.deleted_at = t.deleted_at.min(cap);
            t
        };

        // Merged deletion markers: per (label, id) pair, the newest deletion
        // wins. Distinct ids under the same label stay distinct markers - a
        // marker must only ever kill copies of the entry it refers to.
        let mut dead: Vec<Tombstone> = self.tombstones.clone();
        for t in remote.tombstones.iter().map(&clamp_tombstone) {
            match dead.iter_mut().find(|d| d.label == t.label && d.id == t.id) {
                Some(d) => d.deleted_at = d.deleted_at.max(t.deleted_at),
                None => dead.push(t),
            }
        }
        // The newest merged marker referring to a given entry (label + id).
        let dead_at = |tombs: &[Tombstone], entry: &Entry| -> Option<u64> {
            tombs
                .iter()
                .filter(|t| t.refers_to(entry))
                .map(|t| t.deleted_at)
                .max()
        };

        // Build a lookup of local entries by label → (index, updated_at)
        let local_map: HashMap<String, (usize, u64)> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.label.clone(), (i, e.updated_at)))
            .collect();

        // Collect updates and additions separately to avoid borrow conflicts
        let mut updates: Vec<(usize, Entry)> = Vec::new();
        let mut additions: Vec<Entry> = Vec::new();

        for remote_entry in remote.entries.iter().map(&clamp_entry) {
            match local_map.get(&remote_entry.label) {
                Some(&(idx, local_updated_at)) => {
                    if local_updated_at == remote_entry.updated_at {
                        result.unchanged.push(remote_entry.label.clone());
                    } else if remote_entry.updated_at > local_updated_at {
                        result.updated_from_remote.push(remote_entry.label.clone());
                        updates.push((idx, remote_entry));
                    } else {
                        result.kept_local.push(remote_entry.label.clone());
                    }
                }
                None => {
                    // A remote-only entry that our own tombstone post-dates was
                    // deleted here: suppress the resurrection instead of adding.
                    let deleted_here = dead_at(&dead, &remote_entry)
                        .is_some_and(|at| at > remote_entry.updated_at);
                    if deleted_here {
                        result
                            .suppressed_by_tombstone
                            .push(remote_entry.label.clone());
                    } else {
                        result.added_from_remote.push(remote_entry.label.clone());
                        additions.push(remote_entry);
                    }
                }
            }
        }

        // Apply updates
        for (idx, entry) in updates {
            self.entries[idx] = entry;
        }

        // Apply additions
        self.entries.extend(additions);

        // A remote tombstone for this entry, newer than our copy, means it was
        // permanently removed (or renamed away) on the other replica after our
        // last change to it: drop it here too. A local edit newer than the
        // deletion wins and keeps the entry, and a marker for a DIFFERENT id
        // that merely shares the label never fires.
        let remote_tombstones: Vec<Tombstone> =
            remote.tombstones.iter().map(&clamp_tombstone).collect();
        let mut removed = Vec::new();
        self.entries.retain(|e| {
            let purged_remotely =
                dead_at(&remote_tombstones, e).is_some_and(|at| at > e.updated_at);
            if purged_remotely {
                removed.push(e.label.clone());
            }
            !purged_remotely
        });
        result.removed_by_tombstone = removed;

        // Entries only in local are implicitly kept (no action needed).
        let remote_labels: std::collections::HashSet<&str> =
            remote.entries.iter().map(|e| e.label.as_str()).collect();
        for entry in &self.entries {
            if !remote_labels.contains(entry.label.as_str())
                && !result.added_from_remote.contains(&entry.label)
            {
                result.local_only.push(entry.label.clone());
            }
        }

        // Persist the merged tombstones, minus any beaten by a surviving entry
        // (re-created or edited after the deletion): keeping those would make a
        // later merge second-guess an entry the data has already reclaimed.
        let sort_key = |t: &Tombstone| (t.label.clone(), t.id.clone());
        let mut merged_tombstones: Vec<Tombstone> = dead
            .into_iter()
            .filter(|t| {
                !self
                    .entries
                    .iter()
                    .any(|e| t.refers_to(e) && e.updated_at >= t.deleted_at)
            })
            .collect();
        merged_tombstones.sort_by_key(&sort_key);
        let mut previous = self.tombstones.clone();
        previous.sort_by_key(&sort_key);
        result.tombstones_changed = merged_tombstones != previous;

        // The push-back decision needs to know whether the REMOTE side is
        // missing deletion markers: without this, a purge whose entry the
        // remote never held would silently stay local and a third replica
        // could resurrect it later.
        let mut remote_sorted = remote_tombstones;
        remote_sorted.sort_by_key(&sort_key);
        result.remote_tombstones_stale = merged_tombstones != remote_sorted;

        self.tombstones = merged_tombstones;

        if result.has_changes() {
            self.touch();
            self.record_event(AuditEvent::new("vault_merged", None));
        }

        result
    }

    fn record_event(&mut self, mut event: AuditEvent) {
        if event.timestamp == 0 {
            event.timestamp = current_timestamp();
        }
        self.audit_log.push(event);
    }

    /// Records an externally-performed mutation in the audit log and bumps the
    /// vault's `updated_at`. For callers (like the service layer) that mutate
    /// entry fields directly and would otherwise leave no audit trail.
    pub fn record_mutation(&mut self, action: &str, entry_label: Option<&str>) {
        self.touch();
        self.record_event(AuditEvent::new(action, entry_label));
    }

    fn touch(&mut self) {
        self.updated_at = current_timestamp();
    }
}

/// Summary of a vault merge operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MergeResult {
    /// Entries that existed in both vaults with the same timestamp.
    pub unchanged: Vec<String>,
    /// Entries updated to the remote version (remote was newer).
    pub updated_from_remote: Vec<String>,
    /// Entries where the local version was kept (local was newer).
    pub kept_local: Vec<String>,
    /// New entries added from the remote vault.
    pub added_from_remote: Vec<String>,
    /// Entries that existed only locally.
    pub local_only: Vec<String>,
    /// Local entries removed because a newer remote tombstone marked them
    /// permanently deleted (or renamed away) on the other replica.
    #[serde(default)]
    pub removed_by_tombstone: Vec<String>,
    /// Remote entries NOT added because a newer local tombstone marks them
    /// deleted here; the remote replica still holds a live copy.
    #[serde(default)]
    pub suppressed_by_tombstone: Vec<String>,
    /// Whether the merged tombstone list differs from the local one (the vault
    /// needs saving even if no entry changed).
    #[serde(default)]
    pub tombstones_changed: bool,
    /// Whether the merged tombstone list differs from the REMOTE one (the
    /// remote is missing deletion markers and must be pushed, or a third
    /// replica could resurrect a purged entry later).
    #[serde(default)]
    pub remote_tombstones_stale: bool,
    /// Whether any timestamp had to be clamped for claiming to be further in
    /// the future than the clock-skew allowance.
    #[serde(default)]
    pub timestamps_sanitized: bool,
    /// Entries where both replicas were edited within the same second with
    /// DIFFERENT content, resolved deterministically in favor of the LOCAL
    /// version (detected by the service layer, which can decrypt; the losing
    /// version survives in the pre-merge safety backup).
    #[serde(default)]
    pub tie_conflicts_local_won: Vec<String>,
    /// Same-second conflicting edits resolved in favor of the REMOTE version.
    #[serde(default)]
    pub tie_conflicts_remote_won: Vec<String>,
}

impl MergeResult {
    /// Returns true if the merge made any changes to the local vault.
    pub fn has_changes(&self) -> bool {
        !self.updated_from_remote.is_empty()
            || !self.added_from_remote.is_empty()
            || !self.removed_by_tombstone.is_empty()
            || self.tombstones_changed
            || self.timestamps_sanitized
            || !self.tie_conflicts_remote_won.is_empty()
    }

    /// Returns true if the REMOTE replica is missing data the merged local
    /// vault holds (local-only entries, local-newer versions, deletions or
    /// deletion markers the remote has not applied, or a tie conflict the
    /// local version won): the merged vault should be pushed back.
    pub fn remote_is_stale(&self) -> bool {
        !self.local_only.is_empty()
            || !self.kept_local.is_empty()
            || !self.suppressed_by_tombstone.is_empty()
            || self.remote_tombstones_stale
            || !self.tie_conflicts_local_won.is_empty()
    }

    /// Returns a human-readable summary of the merge.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.added_from_remote.is_empty() {
            parts.push(format!(
                "{} added from remote",
                self.added_from_remote.len()
            ));
        }
        if !self.updated_from_remote.is_empty() {
            parts.push(format!(
                "{} updated from remote",
                self.updated_from_remote.len()
            ));
        }
        if !self.kept_local.is_empty() {
            parts.push(format!("{} kept local (newer)", self.kept_local.len()));
        }
        if !self.unchanged.is_empty() {
            parts.push(format!("{} unchanged", self.unchanged.len()));
        }
        if !self.local_only.is_empty() {
            parts.push(format!("{} local only", self.local_only.len()));
        }
        if !self.removed_by_tombstone.is_empty() {
            parts.push(format!(
                "{} removed (deleted remotely)",
                self.removed_by_tombstone.len()
            ));
        }
        if !self.suppressed_by_tombstone.is_empty() {
            parts.push(format!(
                "{} deletions kept (deleted locally)",
                self.suppressed_by_tombstone.len()
            ));
        }
        let ties = self.tie_conflicts_local_won.len() + self.tie_conflicts_remote_won.len();
        if ties > 0 {
            parts.push(format!(
                "{ties} conflicting simultaneous edit(s) resolved (other version kept in the safety backup)"
            ));
        }
        if parts.is_empty() {
            "No changes".to_string()
        } else {
            parts.join(", ")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub timestamp: u64,
    pub action: String,
    #[serde(default)]
    pub entry_label: Option<String>,
}

impl AuditEvent {
    pub fn new(action: impl Into<String>, entry_label: Option<&str>) -> Self {
        Self {
            timestamp: current_timestamp(),
            action: action.into(),
            entry_label: entry_label.map(|label| label.to_string()),
        }
    }
}
