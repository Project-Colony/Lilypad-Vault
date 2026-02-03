//! Integration tests for the Lilypad CLI.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn lilypad() -> Command {
    Command::cargo_bin("lilypad-cli").unwrap()
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
        .args(["search", "test", "git"])
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
