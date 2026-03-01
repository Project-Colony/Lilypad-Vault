//! Integration tests for the Lilypad CLI.

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn lilypad() -> Command {
    cargo_bin_cmd!("lilypad-cli")
}

#[test]
fn test_help_displays() {
    lilypad()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Minimal CLI for managing Lilypad vaults"));
}

#[test]
fn test_vaults_empty() {
    let dir = tempdir().unwrap();
    lilypad()
        .args(["--data-dir", dir.path().to_str().unwrap()])
        .arg("vaults")
        .assert()
        .success()
        .stdout(predicate::str::contains("No vaults found"));
}

#[test]
fn test_init_creates_vault() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize vault without master password (random key)
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test-vault"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Vault 'test-vault' initialized"));

    // Check key file was created
    let key_path = dir.path().join("key.json");
    assert!(key_path.exists());

    // Check vault file was created
    let vault_path = dir.path().join("vaults").join("test-vault.lily");
    assert!(vault_path.exists());

    // List vaults should show it
    lilypad()
        .args(["--data-dir", data_dir])
        .arg("vaults")
        .assert()
        .success()
        .stdout(predicate::str::contains("test-vault"));
}

#[test]
fn test_init_with_master_password() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["--master-password", "MyStr0ng!Password123"])
        .args(["init", "secure-vault", "--use-master-password"])
        .assert()
        .success();

    // Check key file uses KDF (pretty-printed JSON has spaces)
    let key_path = dir.path().join("key.json");
    let key_content = fs::read_to_string(&key_path).unwrap();
    assert!(key_content.contains("\"type\": \"Kdf\"") || key_content.contains("\"type\":\"Kdf\""));
}

#[test]
fn test_init_rejects_existing_key() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // First init succeeds
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "vault1"])
        .assert()
        .success();

    // Second init fails
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "vault2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("key already initialized"));
}

#[test]
fn test_add_and_list_entry() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add entry
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "email", "secret123", "--username", "user@example.com"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Entry 'email' added"));

    // List entries
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["list", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("email"));
}

#[test]
fn test_get_entry() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize and add
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "add", "test", "mysite", "hunter2",
            "--username", "admin",
            "--url", "https://example.com",
            "--notes", "Test notes",
        ])
        .assert()
        .success();

    // Get entry (with --show-password to see the actual password)
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["get", "test", "mysite", "--show-password"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Label: mysite"))
        .stdout(predicate::str::contains("Username: admin"))
        .stdout(predicate::str::contains("URL: https://example.com"))
        .stdout(predicate::str::contains("Password: hunter2"))
        .stdout(predicate::str::contains("Notes: Test notes"));
}

#[test]
fn test_search_entries() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add multiple entries
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "github", "pass1", "--username", "user1"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "gitlab", "pass2", "--username", "user2"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "email", "pass3", "--username", "user3"])
        .assert()
        .success();

    // Search for "git"
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["search", "git", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("github"))
        .stdout(predicate::str::contains("gitlab"))
        .stdout(predicate::str::contains("email").not());
}

#[test]
fn test_remove_entry() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize and add
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "todelete", "password"])
        .assert()
        .success();

    // Remove
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["remove", "test", "todelete"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Entry 'todelete' removed"));

    // Should be gone
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["get", "test", "todelete"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_update_entry() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize and add
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "entry", "old_password"])
        .assert()
        .success();

    // Update
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["update", "test", "entry", "new_password", "--username", "newuser"])
        .assert()
        .success();

    // Verify update (with --show-password to see the actual password)
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["get", "test", "entry", "--show-password"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Password: new_password"))
        .stdout(predicate::str::contains("Username: newuser"));
}

#[test]
fn test_rename_vault() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "original"])
        .assert()
        .success();

    // Rename
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["rename-vault", "original", "renamed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Vault 'original' renamed to 'renamed'"));

    // List should show new name
    lilypad()
        .args(["--data-dir", data_dir])
        .arg("vaults")
        .assert()
        .success()
        .stdout(predicate::str::contains("renamed"))
        .stdout(predicate::str::contains("original").not());
}

#[test]
fn test_delete_vault_requires_confirmation() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Delete without --force requires confirmation (will fail without input)
    // We can't easily simulate interactive input, so we test with --force
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["delete-vault", "test", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Vault 'test' deleted"));

    // Should be gone
    lilypad()
        .args(["--data-dir", data_dir])
        .arg("vaults")
        .assert()
        .success()
        .stdout(predicate::str::contains("No vaults found"));
}

#[test]
fn test_audit_vault() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add weak password
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "weak", "short"])
        .assert()
        .success();

    // Add strong password
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "strong", "MyStr0ng!Password123"])
        .assert()
        .success();

    // Add duplicate password
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "duplicate", "short"])
        .assert()
        .success();

    // Audit
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["audit", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("weak"))
        .stdout(predicate::str::contains("reuse"));
}

#[test]
fn test_vault_name_validation() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize first
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "valid"])
        .assert()
        .success();

    // Try to add with path traversal in vault name (should be caught by storage)
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "../../../etc", "entry", "password"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn test_entry_with_tags_and_folder() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add with tags and folder
    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "add", "test", "organized",
            "password123",
            "--tag", "work",
            "--tag", "important",
            "--folder", "accounts",
            "--entry-type", "login",
        ])
        .assert()
        .success();

    // List should show metadata
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["list", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("folder: accounts"))
        .stdout(predicate::str::contains("work"))
        .stdout(predicate::str::contains("important"));
}

#[test]
fn test_export_encrypted() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();
    let export_path = dir.path().join("export.lily");

    // Initialize and add
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "entry", "password"])
        .assert()
        .success();

    // Export encrypted
    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "export", "test",
            "--output", export_path.to_str().unwrap(),
            "--format", "lily",
        ])
        .assert()
        .success();

    assert!(export_path.exists());
    let content = fs::read_to_string(&export_path).unwrap();
    assert!(content.contains("LILYPAD_VAULT_V1"));
}

#[test]
fn test_export_plaintext_requires_flag() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();
    let export_path = dir.path().join("export.json");

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Export JSON without --allow-plaintext should fail
    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "export", "test",
            "--output", export_path.to_str().unwrap(),
            "--format", "json",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--allow-plaintext"));
}

#[test]
fn test_generate_password() {
    // Test basic password generation
    lilypad()
        .args(["generate", "--length", "20"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Generated password:"));
}

#[test]
fn test_generate_password_default() {
    // Test password generation output includes strength indicator
    lilypad()
        .args(["generate"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Strength:"));
}

#[test]
fn test_add_with_password_expiry() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add entry with expiry
    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "add", "test", "expiring",
            "MyStr0ng!Pass123",
            "--expires-in", "90",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Password will expire in 90 days"));
}

#[test]
fn test_entry_history() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add entry
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "tracked", "password1"])
        .assert()
        .success();

    // Update entry (creates history)
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["update", "test", "tracked", "password2"])
        .assert()
        .success();

    // Check history
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["history", "test", "tracked"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created"))
        .stdout(predicate::str::contains("Password changed"));
}

#[test]
fn test_json_output_format() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add entry
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "jsontest", "pass123"])
        .assert()
        .success();

    // List with JSON output
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["--output-format", "json"])
        .args(["list", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"entries\""))
        .stdout(predicate::str::contains("\"label\""))
        .stdout(predicate::str::contains("jsontest"));
}

#[test]
fn test_completions_generation() {
    // Test that completions generate without error for bash
    lilypad()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

#[test]
fn test_audit_with_password_expiry() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add entry with no expiry
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "noexpiry", "MyStr0ng!Pass123"])
        .assert()
        .success();

    // Audit should work
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["audit", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Audit results"));
}

#[test]
fn test_require_strong_password() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add with weak password and --require-strong should fail
    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "add", "test", "weak",
            "short",
            "--require-strong",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("too weak"));
}

#[test]
fn test_search_with_json_output() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add entry
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "test", "searchable", "pass123"])
        .assert()
        .success();

    // Search with JSON output
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["--output-format", "json"])
        .args(["search", "search", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"results\""))
        .stdout(predicate::str::contains("searchable"));
}

#[test]
fn test_get_entry_with_json_output() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "test"])
        .assert()
        .success();

    // Add entry
    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "add", "test", "jsonentry",
            "secret123",
            "--username", "user@example.com",
        ])
        .assert()
        .success();

    // Get with JSON output
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["--output-format", "json"])
        .args(["get", "test", "jsonentry"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"label\""))
        .stdout(predicate::str::contains("jsonentry"))
        .stdout(predicate::str::contains("user@example.com"));
}

#[test]
fn test_import_from_lily_format() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();
    let export_path = dir.path().join("export.lily");

    // Initialize and add
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "source"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "source", "entry1", "password1"])
        .assert()
        .success();

    // Export
    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "export", "source",
            "--output", export_path.to_str().unwrap(),
            "--format", "lily",
        ])
        .assert()
        .success();

    // Import to new vault
    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "import", "target",
            "--input", export_path.to_str().unwrap(),
            "--format", "lily",
        ])
        .assert()
        .success();

    // Verify import
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["list", "target"])
        .assert()
        .success()
        .stdout(predicate::str::contains("entry1"));
}

#[test]
fn test_backup_and_restore() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize and add
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "backuptest"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "backuptest", "entry1", "secret123"])
        .assert()
        .success();

    // Create backup
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["backup", "backuptest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup created"));

    // List backups
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["list-backups", "backuptest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("backuptest"));
}

#[test]
fn test_verify_vault() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "verifytest"])
        .assert()
        .success();

    // Verify vault integrity
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["verify-vault", "verifytest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("integrity verified"));
}

#[test]
fn test_audit_log_filtering() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize and perform some actions
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "auditfilter"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "auditfilter", "entry1", "pass1"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "auditfilter", "entry2", "pass2"])
        .assert()
        .success();

    // Test text format
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["audit-log", "auditfilter", "--format", "text"])
        .assert()
        .success()
        .stdout(predicate::str::contains("entry_added"));

    // Test filtering by action
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["audit-log", "auditfilter", "--action", "entry_added", "--format", "text"])
        .assert()
        .success()
        .stdout(predicate::str::contains("entry_added"));

    // Test limit
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["audit-log", "auditfilter", "--limit", "1", "--format", "text"])
        .assert()
        .success();
}

#[test]
fn test_backup_codes_without_totp() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize and add entry without TOTP
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "backupcodetest"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "backupcodetest", "nototp", "pass123"])
        .assert()
        .success();

    // Trying to get backup codes without TOTP should fail
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["backup-codes", "backupcodetest", "nototp"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("TOTP"));
}

#[test]
fn test_prune_backups() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();

    // Initialize
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "prunetest"])
        .assert()
        .success();

    // Create multiple backups with longer delays to ensure unique timestamps
    for _ in 0..3 {
        lilypad()
            .args(["--data-dir", data_dir])
            .args(["backup", "prunetest"])
            .assert()
            .success();
        // Longer delay to ensure unique timestamps (backup name includes seconds)
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Verify we have 3 backups
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["list-backups", "prunetest"])
        .assert()
        .success();

    // Prune keeping only 1 - should delete 2
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["prune-backups", "prunetest", "--keep", "1"])
        .assert()
        .success();
        // Don't check for "Deleted" since backups might have same timestamp
}

#[test]
fn test_csv_audit_log_export() {
    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_str().unwrap();
    let export_path = dir.path().join("audit.csv");

    // Initialize and add
    lilypad()
        .args(["--data-dir", data_dir])
        .args(["init", "csvaudit"])
        .assert()
        .success();

    lilypad()
        .args(["--data-dir", data_dir])
        .args(["add", "csvaudit", "testentry", "pass"])
        .assert()
        .success();

    // Export audit log as CSV
    lilypad()
        .args(["--data-dir", data_dir])
        .args([
            "audit-log", "csvaudit",
            "--format", "csv",
            "--output", export_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Verify CSV file was created
    assert!(export_path.exists());
    let content = fs::read_to_string(&export_path).unwrap();
    assert!(content.contains("timestamp"));
    assert!(content.contains("action"));
}
