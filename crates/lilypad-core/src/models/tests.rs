use crate::crypto::{encrypt, CryptoAlgorithm, KeyMaterial};

use super::{Entry, EntryMetadata, EntrySecret, EntryType, KeyMetadata, Tombstone, Vault};

#[test]
fn vault_starts_empty() {
    let key = KeyMaterial::generate();
    let metadata = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
    let vault = Vault::new("primary", metadata);
    assert_eq!(vault.entries.len(), 0);
    assert!(vault.created_at > 0);
    assert_eq!(vault.created_at, vault.updated_at);
}

#[test]
fn entry_roundtrip() {
    let key = KeyMaterial::generate();
    let secret = EntrySecret::new("vault-secret");
    let payload = serde_json::to_vec(&secret).expect("serialize");
    let ciphertext = encrypt(&key, &payload).expect("encrypt");
    let metadata = EntryMetadata {
        username: Some("name".to_string()),
        url: Some("https://example.com".to_string()),
        tags: vec!["personal".to_string()],
        folder: Some("accounts".to_string()),
        entry_type: EntryType::Login,
    };
    let entry = Entry::new_with_metadata("email", metadata, ciphertext);
    assert_eq!(entry.label, "email");
    assert_eq!(entry.metadata.username.as_deref(), Some("name"));
}

#[test]
fn vault_crud() {
    let key = KeyMaterial::generate();
    let metadata = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
    let mut vault = Vault::new("primary", metadata);
    let ciphertext = encrypt(&key, b"value").expect("encrypt");
    vault
        .add_entry(Entry::new("email", ciphertext))
        .expect("add");
    assert!(vault.find_entry("email").is_some());
    let updated = encrypt(&key, b"new").expect("encrypt");
    vault.update_entry("email", updated).expect("update");
    vault.add_entry_tag("email", "personal").expect("tag add");
    vault
        .set_entry_folder("email", Some("accounts".to_string()))
        .expect("folder update");
    vault.rename_entry("email", "primary").expect("rename");
    assert_eq!(vault.list_tags(), vec!["personal".to_string()]);
    assert_eq!(vault.list_folders(), vec!["accounts".to_string()]);
    let removed = vault.remove_entry("primary").expect("remove");
    assert_eq!(removed.label, "primary");
    assert!(vault.find_entry("primary").is_none());
}

// ==================== merge + tombstone tests ====================

fn merge_fixture() -> (KeyMaterial, Vault, Vault) {
    let key = KeyMaterial::generate();
    let metadata = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
    let local = Vault::new("v", metadata.clone());
    let remote = Vault::new("v", metadata);
    (key, local, remote)
}

fn entry_at(key: &KeyMaterial, label: &str, updated_at: u64) -> Entry {
    let ciphertext = encrypt(key, b"secret").expect("encrypt");
    let mut entry = Entry::new(label, ciphertext);
    entry.updated_at = updated_at;
    entry
}

#[test]
fn merge_does_not_resurrect_locally_purged_entry() {
    let (key, mut local, mut remote) = merge_fixture();
    remote.entries.push(entry_at(&key, "dead", 100));
    local.tombstones.push(Tombstone {
        label: "dead".to_string(),
        id: String::new(),
        deleted_at: 200,
    });

    let result = local.merge_from(&remote);

    assert!(local.find_entry("dead").is_none());
    assert_eq!(result.suppressed_by_tombstone, vec!["dead".to_string()]);
    assert!(result.added_from_remote.is_empty());
    // The remote still holds the live copy: the merged vault (carrying the
    // tombstone) must be pushed back so the deletion propagates.
    assert!(result.remote_is_stale());
    assert!(local.tombstones.iter().any(|t| t.label == "dead"));
}

#[test]
fn merge_applies_remote_deletion_to_local_copy() {
    let (key, mut local, mut remote) = merge_fixture();
    local.entries.push(entry_at(&key, "gone", 100));
    remote.tombstones.push(Tombstone {
        label: "gone".to_string(),
        id: String::new(),
        deleted_at: 200,
    });

    let result = local.merge_from(&remote);

    assert!(local.find_entry("gone").is_none());
    assert_eq!(result.removed_by_tombstone, vec!["gone".to_string()]);
    assert!(result.has_changes());
    // The tombstone is persisted so a three-way future merge stays correct.
    assert!(local
        .tombstones
        .iter()
        .any(|t| t.label == "gone" && t.deleted_at == 200));
}

#[test]
fn merge_keeps_entry_edited_after_remote_deletion() {
    let (key, mut local, mut remote) = merge_fixture();
    local.entries.push(entry_at(&key, "kept", 300));
    remote.tombstones.push(Tombstone {
        label: "kept".to_string(),
        id: String::new(),
        deleted_at: 200,
    });

    let result = local.merge_from(&remote);

    assert!(local.find_entry("kept").is_some());
    assert!(result.removed_by_tombstone.is_empty());
    // The beaten tombstone is pruned: the label has been reclaimed by data.
    assert!(local.tombstones.iter().all(|t| t.label != "kept"));
}

#[test]
fn merge_after_rename_does_not_duplicate_entry() {
    let (key, mut local, mut remote) = merge_fixture();
    // Replicas share the entry (same id): remote still holds the
    // pre-rename copy while local has renamed it.
    let shared = entry_at(&key, "old", 100);
    remote.entries.push(shared.clone());
    local.add_entry(shared).expect("add");
    local.rename_entry("old", "new").expect("rename");

    let result = local.merge_from(&remote);

    assert!(
        local.find_entry("old").is_none(),
        "pre-rename copy came back"
    );
    assert!(local.find_entry("new").is_some());
    assert_eq!(local.entries.len(), 1);
    assert_eq!(result.suppressed_by_tombstone, vec!["old".to_string()]);
}

#[test]
fn merge_rename_onto_deleted_label_survives_concurrent_deletion() {
    // The red-team scenario: replicas hold P and Q. Local deletes Q, then
    // renames P -> Q (the entry now living under label Q is P). Remote
    // INDEPENDENTLY deletes its own Q. Label-only tombstones would let
    // remote's newer "Q" marker destroy the renamed P; id-pinned markers
    // must not.
    let (key, mut local, mut remote) = merge_fixture();
    let p = entry_at(&key, "P", 100);
    let q = entry_at(&key, "Q", 100);
    remote.entries.push(p.clone());
    remote.entries.push(q.clone());
    local.add_entry(p).expect("add P");
    local.add_entry(q).expect("add Q");

    // Local: delete Q, rename P -> Q. Pin explicit times so the remote
    // deletion below is strictly newest.
    local.remove_entry("Q").expect("remove Q");
    local.rename_entry("P", "Q").expect("rename P->Q");
    for t in &mut local.tombstones {
        t.deleted_at = 110;
    }
    local.entries[0].updated_at = 110;

    // Remote: delete its own Q later than everything local.
    let removed = remote.remove_entry("Q").expect("remote remove Q");
    remote
        .tombstones
        .iter_mut()
        .find(|t| t.id == removed.id)
        .unwrap()
        .deleted_at = 120;

    let result = local.merge_from(&remote);

    // The renamed entry (P living under label Q) must survive: remote's
    // marker refers to the ORIGINAL Q's id, not P's.
    let survivor = local.find_entry("Q").expect("renamed entry destroyed");
    assert_eq!(survivor.updated_at, 110);
    assert!(result.removed_by_tombstone.is_empty());
    // Remote's stale P copy is suppressed by the rename tombstone, and
    // the deletion markers for the original Q are merged (newest wins).
    assert_eq!(result.suppressed_by_tombstone, vec!["P".to_string()]);
    assert!(local
        .tombstones
        .iter()
        .any(|t| t.label == "Q" && t.deleted_at == 120));
}

#[test]
fn merge_tombstone_only_change_still_needs_saving() {
    let (_key, mut local, mut remote) = merge_fixture();
    remote.tombstones.push(Tombstone {
        label: "elsewhere".to_string(),
        id: String::new(),
        deleted_at: 100,
    });

    let result = local.merge_from(&remote);

    assert!(result.tombstones_changed);
    assert!(result.has_changes());
    assert!(local.tombstones.iter().any(|t| t.label == "elsewhere"));
}

#[test]
fn merge_recreated_entry_wins_over_stale_remote_copy() {
    let (key, mut local, mut remote) = merge_fixture();
    // Purged then re-created locally: add_entry lifts the tombstone.
    local.tombstones.push(Tombstone {
        label: "reborn".to_string(),
        id: String::new(),
        deleted_at: 200,
    });
    local.add_entry(entry_at(&key, "reborn", 300)).expect("add");
    remote.entries.push(entry_at(&key, "reborn", 100));

    let result = local.merge_from(&remote);

    assert!(local.find_entry("reborn").is_some());
    assert_eq!(result.kept_local, vec!["reborn".to_string()]);
    assert!(local.tombstones.is_empty());
}

#[test]
fn merge_clamps_far_future_timestamps() {
    let (key, mut local, mut remote) = merge_fixture();
    let far_future = super::current_timestamp() + 1_000_000;
    // A broken-clock replica pushes a far-future tombstone and a
    // far-future entry.
    remote.tombstones.push(Tombstone {
        label: "poisoned".to_string(),
        id: String::new(),
        deleted_at: far_future,
    });
    remote.entries.push(entry_at(&key, "eternal", far_future));

    let result = local.merge_from(&remote);

    let cap = super::current_timestamp() + super::MAX_CLOCK_SKEW_SECS + 1;
    assert!(result.timestamps_sanitized || result.has_changes());
    assert!(local.tombstones.iter().all(|t| t.deleted_at <= cap));
    assert!(local.entries.iter().all(|e| e.updated_at <= cap));
}

#[test]
fn merge_reports_remote_missing_tombstones() {
    // Local purged an entry the remote never held: nothing is suppressed
    // or removed, but the marker itself must still reach the remote or a
    // third replica could resurrect the entry later.
    let (key, mut local, _remote) = merge_fixture();
    let (_key2, _l2, remote) = merge_fixture();
    local.add_entry(entry_at(&key, "ghost", 100)).expect("add");
    local.remove_entry("ghost").expect("remove");

    let result = local.merge_from(&remote);

    assert!(result.remote_tombstones_stale);
    assert!(result.remote_is_stale());
}

#[test]
fn merge_timestamp_tie_favors_keeping_data_over_tombstone() {
    let (key, mut local, mut remote) = merge_fixture();
    local.entries.push(entry_at(&key, "tie", 200));
    remote.tombstones.push(Tombstone {
        label: "tie".to_string(),
        id: String::new(),
        deleted_at: 200,
    });

    let result = local.merge_from(&remote);

    // deleted_at == updated_at: the tombstone must NOT win (a deletion only
    // beats an entry strictly older than it).
    assert!(local.find_entry("tie").is_some());
    assert!(result.removed_by_tombstone.is_empty());
}

#[test]
fn remove_and_rename_record_tombstones() {
    let (key, mut vault, _remote) = merge_fixture();
    vault.add_entry(entry_at(&key, "a", 100)).expect("add");
    vault.add_entry(entry_at(&key, "b", 100)).expect("add");

    vault.remove_entry("a").expect("remove");
    vault.rename_entry("b", "c").expect("rename");

    assert!(vault.tombstones.iter().any(|t| t.label == "a"));
    assert!(vault.tombstones.iter().any(|t| t.label == "b"));
    assert!(vault.tombstones.iter().all(|t| t.label != "c"));

    // Re-creating a purged label lifts its marker.
    vault.add_entry(entry_at(&key, "a", 200)).expect("re-add");
    assert!(vault.tombstones.iter().all(|t| t.label != "a"));
}

// ==================== EntrySecret tests ====================

#[test]
fn test_entry_secret_new() {
    let secret = EntrySecret::new("hunter2");
    assert_eq!(secret.password, "hunter2");
    assert!(secret.notes.is_none());
    assert!(secret.email.is_none());
    assert!(secret.phone.is_none());
    assert!(secret.totp_secret.is_none());
    assert!(secret.attachments.is_empty());
    assert!(secret.totp_backup_codes.is_empty());
    assert!(secret.custom_fields.is_empty());
}

#[test]
fn test_entry_secret_fields() {
    let mut secret = EntrySecret::new("p@ssw0rd!");
    secret.notes = Some("My important notes".to_string());
    secret.email = Some("user@example.com".to_string());
    secret.phone = Some("+1234567890".to_string());
    secret.totp_secret = Some("JBSWY3DPEHPK3PXP".to_string());

    assert_eq!(secret.password, "p@ssw0rd!");
    assert_eq!(secret.notes.as_deref(), Some("My important notes"));
    assert_eq!(secret.email.as_deref(), Some("user@example.com"));
    assert_eq!(secret.phone.as_deref(), Some("+1234567890"));
    assert_eq!(secret.totp_secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));
}

#[test]
fn test_entry_secret_serialization() {
    let mut secret = EntrySecret::new("serialize-me");
    secret.notes = Some("some notes".to_string());
    secret.email = Some("test@test.com".to_string());
    secret.phone = Some("+9876543210".to_string());
    secret.totp_secret = Some("TOTP123".to_string());

    let json = serde_json::to_string(&secret).expect("serialize");
    let restored: EntrySecret = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.password, "serialize-me");
    assert_eq!(restored.notes.as_deref(), Some("some notes"));
    assert_eq!(restored.email.as_deref(), Some("test@test.com"));
    assert_eq!(restored.phone.as_deref(), Some("+9876543210"));
    assert_eq!(restored.totp_secret.as_deref(), Some("TOTP123"));
    assert!(restored.attachments.is_empty());
    assert!(restored.custom_fields.is_empty());
}

#[test]
fn test_entry_secret_default_fields() {
    // Minimal JSON with only the required "password" field
    let json = r#"{"password":"minimal"}"#;
    let secret: EntrySecret = serde_json::from_str(json).expect("deserialize minimal");

    assert_eq!(secret.password, "minimal");
    assert!(secret.notes.is_none());
    assert!(secret.totp_secret.is_none());
    assert!(secret.attachments.is_empty());
    assert!(secret.totp_backup_codes.is_empty());
    assert!(secret.email.is_none());
    assert!(secret.phone.is_none());
    assert!(secret.custom_fields.is_empty());
}

// ==================== Entry history tests ====================

#[test]
fn test_entry_history_record_new() {
    use super::EntryChangeType;
    use super::EntryHistoryRecord;

    let record = EntryHistoryRecord::new(EntryChangeType::Created);
    assert!(record.timestamp > 0);
    assert_eq!(record.change_type, EntryChangeType::Created);
    assert!(record.description.is_none());
    assert!(record.previous_ciphertext.is_none());
}

#[test]
fn test_entry_history_with_description() {
    use super::EntryChangeType;
    use super::EntryHistoryRecord;

    let record =
        EntryHistoryRecord::with_description(EntryChangeType::MetadataUpdated, "Updated username");
    assert!(record.timestamp > 0);
    assert_eq!(record.change_type, EntryChangeType::MetadataUpdated);
    assert_eq!(record.description.as_deref(), Some("Updated username"));
}

#[test]
fn test_entry_password_change_history() {
    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"initial-password").expect("encrypt");
    let mut entry = Entry::new("test-entry", ciphertext);

    // New entry starts with 1 history record (Created)
    let initial_history_len = entry.history.len();
    assert_eq!(initial_history_len, 1);

    // Record a password change without previous ciphertext
    entry.record_password_change_with_history(None);
    assert_eq!(entry.history.len(), initial_history_len + 1);

    let last = entry.history.last().expect("history not empty");
    assert_eq!(last.change_type, super::EntryChangeType::PasswordChanged);
    assert!(last.previous_ciphertext.is_none());

    // Record a password change with previous ciphertext
    let prev_ct = encrypt(&key, b"old-password").expect("encrypt");
    entry.record_password_change_with_history(Some(prev_ct));
    assert_eq!(entry.history.len(), initial_history_len + 2);

    let last = entry.history.last().expect("history not empty");
    assert_eq!(last.change_type, super::EntryChangeType::PasswordChanged);
    assert!(last.previous_ciphertext.is_some());
}

#[test]
fn test_entry_get_history_reverse_order() {
    use super::{EntryChangeType, EntryHistoryRecord};

    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"secret").expect("encrypt");
    let mut entry = Entry::new("history-test", ciphertext);

    // Entry starts with 1 Created record; add more records
    entry.record_change(EntryHistoryRecord::new(EntryChangeType::MetadataUpdated));
    std::thread::sleep(std::time::Duration::from_millis(10));
    entry.record_change(EntryHistoryRecord::new(EntryChangeType::NotesUpdated));
    std::thread::sleep(std::time::Duration::from_millis(10));
    entry.record_change(EntryHistoryRecord::new(EntryChangeType::PasswordChanged));

    let history: Vec<_> = entry.get_history().collect();
    // Most recent first
    assert_eq!(history[0].change_type, EntryChangeType::PasswordChanged);
    assert_eq!(history[1].change_type, EntryChangeType::NotesUpdated);
    assert_eq!(history[2].change_type, EntryChangeType::MetadataUpdated);
    assert_eq!(history[3].change_type, EntryChangeType::Created);
}

#[test]
fn test_entry_password_change_count() {
    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"pw").expect("encrypt");
    let mut entry = Entry::new("count-test", ciphertext);

    assert_eq!(entry.password_change_count(), 0);

    entry.record_password_change();
    assert_eq!(entry.password_change_count(), 1);

    entry.record_password_change();
    entry.record_password_change();
    assert_eq!(entry.password_change_count(), 3);
}

// ==================== Entry metadata tests ====================

#[test]
fn test_entry_metadata_default() {
    let meta = EntryMetadata::default();
    assert!(meta.username.is_none());
    assert!(meta.url.is_none());
    assert!(meta.tags.is_empty());
    assert!(meta.folder.is_none());
    assert_eq!(meta.entry_type, EntryType::Login);
}

#[test]
fn test_entry_new_with_metadata() {
    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"data").expect("encrypt");
    let metadata = EntryMetadata {
        username: Some("admin".to_string()),
        url: Some("https://admin.example.com".to_string()),
        tags: vec!["work".to_string(), "admin".to_string()],
        folder: Some("Work/Admin".to_string()),
        entry_type: EntryType::Login,
    };

    let entry = Entry::new_with_metadata("admin-panel", metadata, ciphertext);

    assert_eq!(entry.label, "admin-panel");
    assert_eq!(entry.metadata.username.as_deref(), Some("admin"));
    assert_eq!(
        entry.metadata.url.as_deref(),
        Some("https://admin.example.com")
    );
    assert_eq!(entry.metadata.tags.len(), 2);
    assert_eq!(entry.metadata.tags[0], "work");
    assert_eq!(entry.metadata.tags[1], "admin");
    assert_eq!(entry.metadata.folder.as_deref(), Some("Work/Admin"));
}

#[test]
fn test_entry_set_favorite() {
    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"fav").expect("encrypt");
    let mut entry = Entry::new("fav-test", ciphertext);

    assert!(!entry.is_favorite);

    entry.set_favorite(true);
    assert!(entry.is_favorite);

    entry.set_favorite(false);
    assert!(!entry.is_favorite);
}

#[test]
fn test_entry_record_access() {
    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"access").expect("encrypt");
    let mut entry = Entry::new("access-test", ciphertext);

    assert!(entry.last_accessed_at.is_none());
    assert_eq!(entry.access_count, 0);

    entry.record_access();
    assert!(entry.last_accessed_at.is_some());
    assert!(entry.last_accessed_at.unwrap() > 0);
    assert_eq!(entry.access_count, 1);

    entry.record_access();
    assert_eq!(entry.access_count, 2);

    entry.record_access();
    assert_eq!(entry.access_count, 3);
}

// ==================== Entry expiration tests ====================

#[test]
fn test_entry_no_expiry_by_default() {
    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"noexpiry").expect("encrypt");
    let entry = Entry::new("no-expiry", ciphertext);

    assert_eq!(entry.password_expires_at, 0);
    assert!(!entry.is_password_expired());
}

#[test]
fn test_entry_password_expiry() {
    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"expired").expect("encrypt");
    let mut entry = Entry::new("expiry-test", ciphertext);

    // Set expiry to a timestamp in the past (1 second after epoch)
    entry.set_password_expiry(1);
    assert!(entry.is_password_expired());
}

#[test]
fn test_entry_days_until_expires_none() {
    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"nodays").expect("encrypt");
    let entry = Entry::new("no-days", ciphertext);

    assert!(entry.days_until_password_expires().is_none());
}

#[test]
fn test_entry_days_until_expires_future() {
    let key = KeyMaterial::generate();
    let ciphertext = encrypt(&key, b"future").expect("encrypt");
    let mut entry = Entry::new("future-expiry", ciphertext);

    // Set expiry 30 days from now
    entry.set_password_expiry_days(30);

    let days = entry.days_until_password_expires();
    assert!(days.is_some());
    let days = days.unwrap();
    // Should be approximately 29-30 days (slight timing variance)
    assert!(days >= 29, "expected >= 29 days, got {days}");
    assert!(days <= 30, "expected <= 30 days, got {days}");
    assert!(!entry.is_password_expired());
}

// ==================== Vault tests ====================

#[test]
fn test_vault_remove_entry() {
    let key = KeyMaterial::generate();
    let km = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
    let mut vault = Vault::new("test-vault", km);

    let ct = encrypt(&key, b"remove-me").expect("encrypt");
    vault.add_entry(Entry::new("to-remove", ct)).expect("add");
    assert!(vault.find_entry("to-remove").is_some());

    let removed = vault.remove_entry("to-remove").expect("remove");
    assert_eq!(removed.label, "to-remove");
    assert!(vault.find_entry("to-remove").is_none());
    assert_eq!(vault.entries.len(), 0);
}

#[test]
fn test_vault_get_entry() {
    let key = KeyMaterial::generate();
    let km = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
    let mut vault = Vault::new("test-vault", km);

    let ct = encrypt(&key, b"get-me").expect("encrypt");
    vault.add_entry(Entry::new("target", ct)).expect("add");

    let found = vault.find_entry("target");
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.label, "target");

    // Non-existent entry should return None
    assert!(vault.find_entry("nonexistent").is_none());
}

#[test]
fn test_vault_update_entry() {
    let key = KeyMaterial::generate();
    let km = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
    let mut vault = Vault::new("test-vault", km);

    let ct = encrypt(&key, b"original").expect("encrypt");
    vault.add_entry(Entry::new("updatable", ct)).expect("add");

    // Rename the entry (update label)
    vault
        .rename_entry("updatable", "updated-label")
        .expect("rename");

    assert!(vault.find_entry("updatable").is_none());
    let found = vault.find_entry("updated-label");
    assert!(found.is_some());
    assert_eq!(found.unwrap().label, "updated-label");
}

#[test]
fn test_vault_entry_count() {
    let key = KeyMaterial::generate();
    let km = KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305);
    let mut vault = Vault::new("test-vault", km);

    assert_eq!(vault.entries.len(), 0);

    let ct1 = encrypt(&key, b"one").expect("encrypt");
    vault.add_entry(Entry::new("entry-1", ct1)).expect("add");
    assert_eq!(vault.entries.len(), 1);

    let ct2 = encrypt(&key, b"two").expect("encrypt");
    vault.add_entry(Entry::new("entry-2", ct2)).expect("add");
    assert_eq!(vault.entries.len(), 2);

    let ct3 = encrypt(&key, b"three").expect("encrypt");
    vault.add_entry(Entry::new("entry-3", ct3)).expect("add");
    assert_eq!(vault.entries.len(), 3);

    vault.remove_entry("entry-2").expect("remove");
    assert_eq!(vault.entries.len(), 2);
}

// ==================== EntryColor / EntryChangeType tests ====================

#[test]
fn test_entry_change_type_variants() {
    use super::EntryChangeType;

    let created = EntryChangeType::Created;
    let password_changed = EntryChangeType::PasswordChanged;
    let metadata_updated = EntryChangeType::MetadataUpdated;
    let notes_updated = EntryChangeType::NotesUpdated;
    let renamed = EntryChangeType::Renamed;
    let totp_updated = EntryChangeType::TotpUpdated;
    let attachments_updated = EntryChangeType::AttachmentsUpdated;

    // Verify each variant can be compared with itself
    assert_eq!(created, EntryChangeType::Created);
    assert_eq!(password_changed, EntryChangeType::PasswordChanged);
    assert_eq!(metadata_updated, EntryChangeType::MetadataUpdated);
    assert_eq!(notes_updated, EntryChangeType::NotesUpdated);
    assert_eq!(renamed, EntryChangeType::Renamed);
    assert_eq!(totp_updated, EntryChangeType::TotpUpdated);
    assert_eq!(attachments_updated, EntryChangeType::AttachmentsUpdated);

    // Verify different variants are not equal
    assert_ne!(created, password_changed);
    assert_ne!(metadata_updated, notes_updated);
    assert_ne!(renamed, totp_updated);
}

#[test]
fn test_entry_color_variants() {
    use super::EntryColor;

    let all_colors = EntryColor::all();
    assert_eq!(all_colors.len(), 8);

    // Verify all variants exist and can be created
    let red = EntryColor::Red;
    let orange = EntryColor::Orange;
    let yellow = EntryColor::Yellow;
    let green = EntryColor::Green;
    let blue = EntryColor::Blue;
    let purple = EntryColor::Purple;
    let pink = EntryColor::Pink;
    let gray = EntryColor::Gray;

    assert_eq!(red, EntryColor::Red);
    assert_eq!(orange, EntryColor::Orange);
    assert_eq!(yellow, EntryColor::Yellow);
    assert_eq!(green, EntryColor::Green);
    assert_eq!(blue, EntryColor::Blue);
    assert_eq!(purple, EntryColor::Purple);
    assert_eq!(pink, EntryColor::Pink);
    assert_eq!(gray, EntryColor::Gray);

    // Verify hex codes are non-empty
    for color in all_colors {
        assert!(!color.to_hex().is_empty());
        assert!(color.to_hex().starts_with('#'));
    }

    // Verify different colors are not equal
    assert_ne!(red, blue);
    assert_ne!(green, purple);
}

// ==================== EmbeddedKdfParams tests ====================

#[test]
fn test_embedded_kdf_roundtrip() {
    use super::EmbeddedKdfParams;
    use crate::crypto::KeyDerivationParams;

    let params = KeyDerivationParams::generate();
    let embedded = EmbeddedKdfParams::from_kdf_params(&params);

    // Serialize and deserialize
    let json = serde_json::to_string(&embedded).expect("serialize");
    let restored: EmbeddedKdfParams = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.algorithm, "argon2id");
    assert_eq!(restored.salt, params.salt);
    assert_eq!(restored.memory_kib, params.memory_kib);
    assert_eq!(restored.iterations, params.iterations);
    assert_eq!(restored.parallelism, params.parallelism);

    // Convert back to KDF params and verify
    let kdf_restored = restored.to_kdf_params();
    assert_eq!(kdf_restored.salt, params.salt);
    assert_eq!(kdf_restored.memory_kib, params.memory_kib);
    assert_eq!(kdf_restored.iterations, params.iterations);
    assert_eq!(kdf_restored.parallelism, params.parallelism);
}

#[test]
fn test_key_metadata_v1_compat() {
    // V1 JSON (no kdf_params field) should still deserialize
    let json = r#"{
        "key_id": "abc123",
        "algorithm": "XChaCha20Poly1305"
    }"#;
    let metadata: KeyMetadata = serde_json::from_str(json).expect("deserialize V1 metadata");
    assert_eq!(metadata.key_id, "abc123");
    assert!(metadata.kdf_params.is_none());
    assert!(metadata.kdf.is_none());
}

#[test]
fn test_key_metadata_with_embedded_kdf() {
    use crate::crypto::KeyDerivationParams;

    let key = KeyMaterial::generate();
    let params = KeyDerivationParams::generate();
    let metadata =
        KeyMetadata::new(&key, CryptoAlgorithm::XChaCha20Poly1305).with_embedded_kdf(&params);

    assert!(metadata.kdf_params.is_some());
    assert_eq!(metadata.kdf.as_deref(), Some("argon2id"));

    // Serialize and verify it roundtrips
    let json = serde_json::to_string(&metadata).expect("serialize");
    let embedded = metadata.kdf_params.unwrap();
    assert_eq!(embedded.salt, params.salt);
    let restored: KeyMetadata = serde_json::from_str(&json).expect("deserialize");
    assert!(restored.kdf_params.is_some());
}
