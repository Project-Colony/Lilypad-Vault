//! Lilypad CLI - a secure password manager.
//!
//! A thin command-line frontend over `lilypad-core`: it parses arguments,
//! prompts for secrets, and prints results. All vault logic (crypto, storage,
//! validated sync, locking) lives in the service layer.

mod args;
mod cmd;
mod io;

use anyhow::Result;
use args::{Cli, Commands, SyncCommands};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use lilypad_core::{App, OpenOptions};
use std::io as stdio;
use std::path::PathBuf;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.master_password.is_some() {
        eprintln!("WARNING: --master-password is visible in `ps` and shell history; prefer interactive entry.");
    }

    let master_password = cli
        .master_password
        .clone()
        .or_else(|| std::env::var("LILYPAD_MASTER_PASSWORD").ok());
    let mp = master_password.as_deref();
    let fmt = cli.output_format;

    let app = App::open(OpenOptions {
        data_dir: cli.data_dir.clone().map(PathBuf::from),
        auto_lock_after: None,
    })?;

    // Older CLI builds defaulted to `./.lilypad`; point at those vaults
    // instead of silently showing an empty store.
    if cli.data_dir.is_none()
        && std::env::var_os("LILYPAD_DATA_DIR").is_none()
        && app.list_vaults().is_ok_and(|v| v.is_empty())
        && std::fs::read_dir(".lilypad/vaults").is_ok_and(|mut d| {
            d.any(|e| e.is_ok_and(|e| e.path().extension().is_some_and(|x| x == "lily")))
        })
    {
        eprintln!("hint: ./.lilypad holds vaults from an older Lilypad CLI; pass --data-dir .lilypad (or set LILYPAD_DATA_DIR) to use them.");
    }

    match cli.command {
        Commands::Init { vault } => cmd::init(&app, &vault, mp),
        Commands::Vaults => cmd::vaults(&app, fmt),
        Commands::RenameVault { from, to } => cmd::rename_vault(&app, &from, &to),
        Commands::DeleteVault { vault, force } => cmd::delete_vault(&app, &vault, force),

        Commands::Add {
            vault,
            label,
            value,
            username,
            url,
            notes,
            tags,
            folder,
            entry_type,
            totp_secret,
            attachments,
        } => cmd::add(
            &app,
            &vault,
            &label,
            value,
            username,
            url,
            notes,
            tags,
            folder,
            entry_type,
            totp_secret,
            attachments,
            mp,
        ),
        Commands::List { vault } => cmd::list(&app, &vault, fmt, mp),
        Commands::Get {
            vault,
            label,
            copy,
            clipboard_timeout,
            show_password,
        } => cmd::get(
            &app,
            &vault,
            &label,
            copy,
            clipboard_timeout,
            show_password,
            mp,
        ),
        Commands::Update {
            vault,
            label,
            value,
            username,
            url,
            notes,
            tags,
            remove_tags,
            clear_tags,
            folder,
            entry_type,
            totp_secret,
        } => cmd::update(
            &app,
            &vault,
            &label,
            value,
            username,
            url,
            notes,
            tags,
            remove_tags,
            clear_tags,
            folder,
            entry_type,
            totp_secret,
            mp,
        ),
        Commands::Remove {
            vault,
            label,
            purge,
        } => cmd::remove(&app, &vault, &label, purge, mp),
        Commands::Trash { vault } => cmd::trash(&app, &vault, fmt, mp),
        Commands::Restore { vault, label } => cmd::restore(&app, &vault, &label, mp),
        Commands::Audit { vault } => cmd::audit(&app, &vault, fmt, mp),
        Commands::BreachCheck { vault } => cmd::breach_check(&app, &vault, fmt, mp),
        Commands::AuditLog { vault } => cmd::audit_log(&app, &vault, fmt, mp),
        Commands::History { vault, label } => cmd::history(&app, &vault, &label, fmt, mp),
        Commands::Import {
            vault,
            file,
            dry_run,
        } => cmd::import(&app, &vault, &file, dry_run, mp),
        Commands::Export { vault, file, force } => cmd::export(&app, &vault, &file, force, mp),
        Commands::RenameEntry {
            vault,
            label,
            new_label,
        } => cmd::rename_entry(&app, &vault, &label, &new_label, mp),
        Commands::Search { vault, query } => cmd::search(&app, &vault, &query, fmt, mp),
        Commands::Generate {
            length,
            uppercase,
            lowercase,
            digits,
            symbols,
            copy,
            clipboard_timeout,
        } => cmd::generate(
            length,
            uppercase,
            lowercase,
            digits,
            symbols,
            copy,
            clipboard_timeout,
        ),
        Commands::Totp { vault, label } => cmd::totp(&app, &vault, &label, mp),
        Commands::BackupCodes {
            vault,
            label,
            generate,
            verify,
        } => cmd::backup_codes(&app, &vault, &label, generate, verify.as_deref(), mp),
        Commands::ChangeMasterPassword { vault } => cmd::change_master_password(&app, &vault, mp),

        Commands::Backup { vault } => cmd::backup(&app, &vault),
        Commands::ListBackups { vault } => cmd::list_backups(&app, &vault),
        Commands::RestoreBackup { backup, force } => cmd::restore_backup(&app, &backup, force),
        Commands::PruneBackups { vault, keep } => cmd::prune_backups(&app, &vault, keep),

        Commands::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            generate(shell, &mut command, name, &mut stdio::stdout());
            Ok(())
        }

        Commands::Login => cmd::login(&app),
        Commands::Logout => cmd::logout(&app),
        Commands::AuthStatus => cmd::auth_status(&app),
        Commands::Sync(sync) => match sync {
            SyncCommands::Push { vault, force } => cmd::sync_push(&app, &vault, force),
            SyncCommands::Pull { vault } => cmd::sync_pull(&app, &vault, mp),
            SyncCommands::Merge { vault } => cmd::sync_merge(&app, &vault, mp),
            SyncCommands::Status { vault } => cmd::sync_status(&app, &vault),
        },
    }
}
