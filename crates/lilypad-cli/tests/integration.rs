//! End-to-end tests for the CLI, driving the real binary against a temp data
//! dir. The master password is passed via the flag so the tests are scripted
//! (interactive prompts are covered by the io module's stdin fallback).

#![allow(deprecated)] // Command::cargo_bin is fine for our fixed build dir.

use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::TempDir;

fn lily(dir: &TempDir, password: &str) -> Command {
    let mut cmd = Command::cargo_bin("lilypad-cli").unwrap();
    cmd.arg("--data-dir")
        .arg(dir.path())
        .arg("--master-password")
        .arg(password);
    cmd
}

#[test]
fn init_add_list_get_roundtrip() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "gh", "--username", "alice"])
        .write_stdin("s3cret")
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["list", "v"])
        .assert()
        .success()
        .stdout(contains("gh").and(contains("alice")));
    lily(&dir, "pw")
        .args(["get", "v", "gh", "--show-password"])
        .assert()
        .success()
        .stdout(contains("s3cret"));
}

#[test]
fn get_hides_password_by_default() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "gh"])
        .write_stdin("s3cret")
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["get", "v", "gh"])
        .assert()
        .success()
        .stdout(contains("s3cret").not());
}

#[test]
fn wrong_password_is_rejected_and_vault_is_preserved() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "right").args(["init", "v"]).assert().success();
    lily(&dir, "right")
        .args(["add", "v", "gh"])
        .write_stdin("s")
        .assert()
        .success();
    // Wrong password must fail...
    lily(&dir, "wrong")
        .args(["get", "v", "gh", "--show-password"])
        .assert()
        .failure()
        .stderr(contains("incorrect master password"));
    // ...and must NOT have destroyed the vault.
    lily(&dir, "right")
        .args(["list", "v"])
        .assert()
        .success()
        .stdout(contains("gh"));
}

#[test]
fn change_master_password_rekeys() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "old").args(["init", "v"]).assert().success();
    lily(&dir, "old")
        .args(["add", "v", "gh"])
        .write_stdin("s")
        .assert()
        .success();
    lily(&dir, "old")
        .args(["change-master-password", "v"])
        .write_stdin("new")
        .assert()
        .success();
    // Old password no longer works.
    lily(&dir, "old").args(["list", "v"]).assert().failure();
    // New password works and the entry survived.
    lily(&dir, "new")
        .args(["list", "v"])
        .assert()
        .success()
        .stdout(contains("gh"));
}

#[test]
fn remove_and_rename_entry() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "one"])
        .write_stdin("s")
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["rename-entry", "v", "one", "two"])
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["list", "v"])
        .assert()
        .success()
        .stdout(contains("two").and(contains("one").not()));
    lily(&dir, "pw")
        .args(["remove", "v", "two"])
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["list", "v"])
        .assert()
        .success()
        .stdout(contains("no entries"));
}

#[test]
fn remove_soft_deletes_and_restore_brings_it_back() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "gh"])
        .write_stdin("s")
        .assert()
        .success();
    // Remove moves to Trash by default...
    lily(&dir, "pw")
        .args(["remove", "v", "gh"])
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["list", "v"])
        .assert()
        .success()
        .stdout(contains("no entries"));
    lily(&dir, "pw")
        .args(["trash", "v"])
        .assert()
        .success()
        .stdout(contains("gh"));
    // ...and restore brings it back to the live list.
    lily(&dir, "pw")
        .args(["restore", "v", "gh"])
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["list", "v"])
        .assert()
        .success()
        .stdout(contains("gh"));
}

#[test]
fn remove_purge_permanently_deletes() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "gh"])
        .write_stdin("s")
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["remove", "v", "gh", "--purge"])
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["trash", "v"])
        .assert()
        .success()
        .stdout(contains("empty"));
}

#[test]
fn audit_reports_a_health_grade() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "gh"])
        .write_stdin("weak")
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["audit", "v"])
        .assert()
        .success()
        .stdout(contains("Vault health"));
}

#[test]
fn update_records_password_history_and_preserves_tags() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "gh", "--tag", "work"])
        .write_stdin("old-pw")
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["update", "v", "gh", "new-pw"])
        .assert()
        .success();
    // The password change entered the entry's history...
    lily(&dir, "pw")
        .args(["history", "v", "gh"])
        .assert()
        .success()
        .stdout(contains("password-changed"));
    // ...and the untouched tag survived the edit.
    lily(&dir, "pw")
        .args(["list", "v", "--output-format", "json"])
        .assert()
        .success()
        .stdout(contains("work"));
}

#[test]
fn update_refuses_a_trashed_entry() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "gh"])
        .write_stdin("s")
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["remove", "v", "gh"])
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["update", "v", "gh", "new"])
        .assert()
        .failure()
        .stderr(contains("in Trash"));
}

#[test]
fn import_lastpass_csv_and_export_roundtrip() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();

    // A realistic LastPass export: a login with folder+favorite and a secure note.
    let src = dir.path().join("lastpass.csv");
    std::fs::write(
        &src,
        "url,username,password,totp,extra,name,grouping,fav\n\
         https://github.com,alice,gh-pw,,personal token in notes,GitHub,Dev\\Git,1\n\
         http://sn,,,,\"NoteType:Credit Card\nLanguage:en-US\nNumber:4111\nNotes:backup card\",Visa,,0\n",
    )
    .unwrap();

    lily(&dir, "pw")
        .args(["import", "v", src.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("LastPass").and(contains("Imported 2")));
    lily(&dir, "pw")
        .args(["list", "v"])
        .assert()
        .success()
        .stdout(contains("GitHub").and(contains("Visa")));
    lily(&dir, "pw")
        .args(["get", "v", "GitHub", "--show-password"])
        .assert()
        .success()
        .stdout(contains("gh-pw"));

    // Export round-trips through detection into a second vault.
    let out = dir.path().join("export.csv");
    lily(&dir, "pw")
        .args(["export", "v", out.to_str().unwrap(), "--force"])
        .assert()
        .success();
    lily(&dir, "pw").args(["init", "v2"]).assert().success();
    lily(&dir, "pw")
        .args(["import", "v2", out.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Lilypad CSV").and(contains("Imported 2")));
}

#[test]
fn import_dry_run_writes_nothing() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    let src = dir.path().join("chrome.csv");
    std::fs::write(
        &src,
        "name,url,username,password,note\nSite,https://s.com,u,p,\n",
    )
    .unwrap();
    lily(&dir, "pw")
        .args(["import", "v", src.to_str().unwrap(), "--dry-run"])
        .assert()
        .success()
        .stdout(contains("Dry run: 1"));
    lily(&dir, "pw")
        .args(["list", "v"])
        .assert()
        .success()
        .stdout(contains("no entries"));
}

#[test]
fn audit_log_records_mutations() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "gh"])
        .write_stdin("s")
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["audit-log", "v"])
        .assert()
        .success()
        .stdout(contains("entry_added"));
}

#[test]
fn generate_outputs_a_password_of_the_right_length() {
    let dir = TempDir::new().unwrap();
    let output = lily(&dir, "pw")
        .args(["generate", "--length", "24", "--symbols", "false"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    let password = text.lines().next().unwrap();
    assert_eq!(password.len(), 24);
    assert!(password.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn backup_creates_and_lists() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw").args(["backup", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["list-backups", "v"])
        .assert()
        .success()
        .stdout(contains(".backup"));
}

#[test]
fn vaults_lists_created_vaults() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "alpha"]).assert().success();
    lily(&dir, "pw").args(["init", "beta"]).assert().success();
    lily(&dir, "pw")
        .args(["vaults"])
        .assert()
        .success()
        .stdout(contains("alpha").and(contains("beta")));
}

#[test]
fn auth_status_reports_not_authenticated() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw")
        .args(["auth-status"])
        .assert()
        .success()
        .stdout(contains("Not authenticated"));
}

#[test]
fn backup_codes_generate_verify_once_and_report() {
    let dir = TempDir::new().unwrap();
    lily(&dir, "pw").args(["init", "v"]).assert().success();
    lily(&dir, "pw")
        .args(["add", "v", "gh", "--totp-secret", "JBSWY3DPEHPK3PXP"])
        .write_stdin("s3cret")
        .assert()
        .success();
    let out = lily(&dir, "pw")
        .args(["backup-codes", "v", "gh", "--generate"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out = String::from_utf8(out).unwrap();
    let code = out.lines().nth(1).unwrap().trim().to_string();
    lily(&dir, "pw")
        .args(["backup-codes", "v", "gh", "--verify", &code])
        .assert()
        .success();
    lily(&dir, "pw")
        .args(["backup-codes", "v", "gh", "--verify", &code])
        .assert()
        .failure();
    lily(&dir, "pw")
        .args(["backup-codes", "v", "gh"])
        .assert()
        .success()
        .stdout(contains("9 unused of 10"));
}
