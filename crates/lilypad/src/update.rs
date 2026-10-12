//! The update loop: every message the app receives is handled here.

use crate::message::{CharSet, Filter, FormField, Message, QuickKind};
use crate::state::{FormBuffers, LilypadApp, Overlay, Screen};
use colony_ui::preferences::Change;
use iced::Task;
use lilypad_core::i18n::{t, tf};
use lilypad_core::{
    App as Service, EntryMetadata, EntrySecret, EntryType, KeyMaterial, OpenOptions,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

impl LilypadApp {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                if let Some(at) = self.status_at {
                    if at.elapsed() > Duration::from_secs(4) {
                        self.status = None;
                        self.status_at = None;
                    }
                }
                if let Some(at) = self.clipboard_clear_at {
                    if Instant::now() >= at {
                        let _ = lilypad_core::clipboard::copy_to_clipboard("");
                        self.clipboard_clear_at = None;
                    }
                }
                if self.session.as_ref().is_some_and(|s| s.is_expired()) {
                    self.lock();
                    self.set_status(t("status_locked_idle"));
                }
                if self.selected_secret.is_some() {
                    self.refresh_totp();
                }
                Task::none()
            }
            Message::Dismiss => {
                self.status = None;
                Task::none()
            }

            Message::DensitySelected(d) => {
                self.density = d;
                self.persist_settings();
                Task::none()
            }

            Message::TogglePreferences => {
                self.preferences_open = !self.preferences_open;
                self.preferences_category = 0;
                if !self.preferences_open {
                    self.close_preferences();
                    return Task::none();
                }
                self.overlay = Overlay::None;
                // Populate the GitHub auth state from the local token store
                // (fast disk read, no network) so the Sync category is correct
                // even before the first unlock.
                if self.authed.is_none() {
                    self.authed =
                        match lilypad_core::sync::net::auth_status(&self.service.token_file()) {
                            Ok(lilypad_core::sync::net::AuthState::Authenticated { username }) => {
                                Some(username)
                            }
                            _ => None,
                        };
                }
                Task::none()
            }
            Message::PreferencesCategory(i) => {
                self.preferences_category = i;
                Task::none()
            }
            Message::ToggleSection(key) => {
                if !self.expanded.remove(key) {
                    self.expanded.insert(key.to_string());
                }
                Task::none()
            }
            Message::Preference(change) => {
                // Applies at once (no Save button) and is saved right away.
                self.prefs.update(change);
                self.persist_settings();
                if matches!(change, Change::Theme { .. }) {
                    self.notice = Some(t("theme_applied"));
                }
                Task::none()
            }
            Message::SetLanguage(locale) => {
                lilypad_core::i18n::set_locale(locale);
                self.language = Some(locale.as_str().to_string());
                self.persist_settings();
                Task::none()
            }
            Message::DismissNotice => {
                self.notice = None;
                Task::none()
            }
            Message::SetAutoLock(minutes) => {
                self.auto_lock_minutes = minutes;
                // Apply to the live session so the change takes effect now, not
                // only on the next unlock.
                let dur = self.auto_lock_duration();
                if let Some(session) = self.session.as_mut() {
                    session.set_auto_lock(dur);
                }
                self.persist_settings();
                self.set_status(if minutes == 0 {
                    t("status_auto_lock_off").to_string()
                } else {
                    tf("status_auto_lock_set", &[("minutes", &minutes)])
                });
                Task::none()
            }
            Message::SetClipboardSecs(secs) => {
                self.clipboard_secs = secs;
                self.persist_settings();
                self.set_status(tf("status_clipboard_set", &[("secs", &secs)]));
                Task::none()
            }
            Message::SetDefaultFilter(f) => {
                self.default_filter = f;
                self.persist_settings();
                Task::none()
            }
            Message::ToggleRestoreLastVault => {
                self.restore_last_vault = !self.restore_last_vault;
                self.persist_settings();
                Task::none()
            }
            Message::CmNewChanged(v) => {
                self.cm_new = v;
                Task::none()
            }
            Message::CmConfirmChanged(v) => {
                self.cm_confirm = v;
                Task::none()
            }
            Message::ChangeMasterSubmit => {
                if self.cm_new.len() < 8 {
                    self.set_status(t("status_master_too_short"));
                    return Task::none();
                }
                if self.cm_new != self.cm_confirm {
                    self.set_status(t("status_passwords_differ"));
                    return Task::none();
                }
                // Zeroizing: this working copy of the new master password is
                // wiped when it drops, on every path.
                let new = zeroize::Zeroizing::new(self.cm_new.clone());
                let result = match self.session.as_mut() {
                    Some(session) => {
                        lilypad_core::change_master_password(&self.service, session, &new)
                    }
                    None => {
                        self.set_status(t("status_unlock_first"));
                        return Task::none();
                    }
                };
                match result {
                    Ok(()) => {
                        self.cm_new.zeroize();
                        self.cm_new.clear();
                        self.cm_confirm.zeroize();
                        self.cm_confirm.clear();
                        self.set_status(t("status_master_changed"));
                    }
                    Err(e) => self.set_status(e.localized()),
                }
                Task::none()
            }

            Message::ImportPathChanged(p) => {
                self.import_path = p;
                Task::none()
            }
            Message::ImportRun => {
                let path = self.import_path.trim().to_string();
                if path.is_empty() {
                    self.set_status(t("status_import_path_needed"));
                    return Task::none();
                }
                if self.session.is_none() {
                    self.set_status(t("status_unlock_first"));
                    return Task::none();
                }
                let result = std::fs::read(&path)
                    .map_err(|e| tf("status_read_failed", &[("path", &path), ("detail", &e)]))
                    .and_then(|bytes| {
                        let format =
                            lilypad_core::detect_format(&bytes).map_err(|e| e.localized())?;
                        let parsed = lilypad_core::parse_import(&bytes, format)
                            .map_err(|e| e.localized())?;
                        Ok((format, parsed))
                    });
                match result {
                    Ok((format, parsed)) => {
                        let warnings = parsed.warnings.len();
                        let session = self.session.as_mut().expect("checked above");
                        match lilypad_core::import_entries(&self.service, session, parsed) {
                            Ok(report) => {
                                self.refresh_entries();
                                self.import_path.clear();
                                let mut msg = vec![tf(
                                    "status_imported",
                                    &[("count", &report.added), ("format", &format.name())],
                                )];
                                if !report.renamed.is_empty() {
                                    msg.push(tf(
                                        "status_import_renamed",
                                        &[("count", &report.renamed.len())],
                                    ));
                                }
                                if report.skipped > 0 || warnings > 0 {
                                    msg.push(tf(
                                        "status_import_skipped",
                                        &[("count", &(report.skipped + warnings))],
                                    ));
                                }
                                msg.push(t("status_import_delete_source").to_string());
                                self.set_status(msg.join(" "));
                            }
                            Err(e) => self.set_status(e.localized()),
                        }
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::ExportPathChanged(p) => {
                self.export_path = p;
                Task::none()
            }
            Message::ExportRun => {
                let path = self.export_path.trim().to_string();
                if path.is_empty() {
                    self.set_status(t("status_export_path_needed"));
                    return Task::none();
                }
                let Some(session) = self.session.as_ref() else {
                    self.set_status(t("status_unlock_first"));
                    return Task::none();
                };
                match lilypad_core::export_csv(session) {
                    Ok(csv) => {
                        let write = std::fs::write(&path, csv.as_bytes());
                        #[cfg(unix)]
                        if write.is_ok() {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = std::fs::set_permissions(
                                &path,
                                std::fs::Permissions::from_mode(0o600),
                            );
                        }
                        match write {
                            Ok(()) => {
                                self.export_path.clear();
                                self.set_status(tf("status_exported", &[("path", &path)]));
                            }
                            Err(e) => self.set_status(tf(
                                "status_write_failed",
                                &[("path", &path), ("detail", &e)],
                            )),
                        }
                    }
                    Err(e) => self.set_status(e.localized()),
                }
                Task::none()
            }

            Message::BreachCheckRun => {
                if self.breach_busy {
                    return Task::none();
                }
                let Some(session) = self.session.as_ref() else {
                    self.set_status(t("status_unlock_first"));
                    return Task::none();
                };
                // Hashing is local and fast (UI thread); only the k-anonymity
                // range queries go to the background task. No plaintext and no
                // session crosses the thread boundary - only SHA-1 fragments.
                let hashes = lilypad_core::collect_hashes(session);
                self.breach_busy = true;
                self.set_status(t("status_breach_checking"));
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            lilypad_core::query_hashes(hashes)
                                .map(|r| {
                                    (
                                        r.unique_checked,
                                        r.breached
                                            .into_iter()
                                            .map(|b| (b.label, b.count))
                                            .collect::<Vec<_>>(),
                                    )
                                })
                                .map_err(|e| e.localized())
                        })
                        .await
                        .unwrap_or_else(|e| Err(tf("error_background", &[("detail", &e)])))
                    },
                    Message::BreachChecked,
                )
            }
            Message::BreachChecked(result) => {
                self.breach_busy = false;
                match result {
                    Ok((unique, breached)) => {
                        let n = breached.len();
                        self.breached = breached.into_iter().map(|(label, _)| label).collect();
                        // Recompute health so Compromised findings appear in the
                        // grade, the sidebar badge, and the per-entry banners.
                        self.refresh_entries();
                        self.set_status(if n == 0 {
                            tf("status_breach_none", &[("count", &unique)])
                        } else {
                            tf("status_breach_found", &[("count", &n)])
                        });
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }

            Message::SyncLoginStart => {
                if self.sync_busy {
                    return Task::none();
                }
                self.sync_busy = true;
                Task::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            lilypad_core::sync::net::begin_login().map_err(|e| e.localized())
                        })
                        .await
                        .unwrap_or_else(|e| Err(tf("error_background", &[("detail", &e)])))
                    },
                    Message::SyncLoginStarted,
                )
            }
            Message::SyncLoginStarted(result) => match result {
                Ok(login) => {
                    // Show the code in the UI, open the verification page, and
                    // poll in the background until the user authorizes.
                    let _ = webbrowser::open(&login.verification_uri);
                    self.sync_login = Some(login.clone());
                    let token_file = self.service.token_file();
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                lilypad_core::sync::net::complete_login(&token_file, &login)
                                    .map_err(|e| e.localized())
                            })
                            .await
                            .unwrap_or_else(|e| Err(tf("error_background", &[("detail", &e)])))
                        },
                        Message::SyncLoginDone,
                    )
                }
                Err(e) => {
                    self.sync_busy = false;
                    self.set_status(e);
                    Task::none()
                }
            },
            Message::SyncLoginCancel => {
                // Re-enables the sync UI; the abandoned poll either expires
                // server-side or completes and is still recorded harmlessly.
                self.sync_busy = false;
                self.sync_login = None;
                self.set_status(t("status_signin_cancelled"));
                Task::none()
            }
            Message::SyncLoginDone(result) => {
                let was_cancelled = self.sync_login.is_none() && !self.sync_busy;
                self.sync_busy = false;
                self.sync_login = None;
                match result {
                    Ok(username) => {
                        self.authed = Some(username.clone());
                        self.set_status(tf("status_signed_in", &[("user", &username)]));
                    }
                    // A cancelled flow's eventual expiry error is not news.
                    Err(_) if was_cancelled => {}
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::SyncLogout => {
                if self.sync_busy {
                    return Task::none();
                }
                // Local token removal only; no network round-trip.
                match lilypad_core::sync::net::logout(&self.service.token_file()) {
                    Ok(()) => {
                        self.authed = None;
                        self.sync_status = None;
                        self.set_status(t("status_signed_out"));
                    }
                    Err(e) => self.set_status(e.localized()),
                }
                Task::none()
            }
            Message::SyncPush => {
                let Some(vault) = self.active_vault_name() else {
                    self.set_status(t("status_no_vault"));
                    return Task::none();
                };
                if self.sync_busy {
                    return Task::none();
                }
                self.sync_busy = true;
                let data_dir = self.data_dir.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let app = Service::open(OpenOptions {
                                data_dir,
                                auto_lock_after: None,
                            })
                            .map_err(|e| e.localized())?;
                            lilypad_core::sync::net::push(&app, &vault, false)
                                .map(|_| vault)
                                .map_err(|e| e.localized())
                        })
                        .await
                        .unwrap_or_else(|e| Err(tf("error_background", &[("detail", &e)])))
                    },
                    Message::SyncPushDone,
                )
            }
            Message::SyncPushDone(result) => {
                self.sync_busy = false;
                match result {
                    Ok(vault) => {
                        // Only mark in-sync if that vault is still the active one.
                        if self.active_vault_name().as_deref() == Some(vault.as_str()) {
                            self.sync_status = Some(lilypad_core::SyncStatusView::InSync);
                        }
                        self.set_status(tf("status_pushed", &[("vault", &vault)]));
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::SyncPullPasswordChanged(p) => {
                self.sync_pull_password = p;
                Task::none()
            }
            Message::SyncPull => {
                if self.sync_busy {
                    return Task::none();
                }
                let Some(vault) = self.active_vault_name() else {
                    self.set_status(t("status_no_vault"));
                    return Task::none();
                };
                if self.sync_pull_password.is_empty() {
                    self.set_status(t("status_pull_password_needed"));
                    return Task::none();
                }
                self.sync_busy = true;
                let data_dir = self.data_dir.clone();
                let pw = std::mem::take(&mut self.sync_pull_password);
                // Lock BEFORE the pull runs, not after: otherwise the vault
                // stays editable while the remote payload replaces the file,
                // and an edit landing in that window is silently clobbered.
                if self.session.is_some() {
                    self.lock();
                    self.set_status(t("status_locked_for_pull"));
                }
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            // Owns the password for the duration of the pull,
                            // wiping it when the task ends.
                            let pw = zeroize::Zeroizing::new(pw);
                            let app = Service::open(OpenOptions {
                                data_dir,
                                auto_lock_after: None,
                            })
                            .map_err(|e| e.localized())?;
                            lilypad_core::sync::net::pull(&app, &vault, &pw)
                                .map_err(|e| e.localized())
                        })
                        .await
                        .unwrap_or_else(|e| Err(tf("error_background", &[("detail", &e)])))
                    },
                    Message::SyncPullDone,
                )
            }
            Message::SyncPullDone(result) => {
                self.sync_busy = false;
                match result {
                    Ok(lilypad_core::sync::net::PullOutcome::Applied) => {
                        // The on-disk vault was replaced (validated, safety
                        // backup taken) and may be keyed differently: lock so
                        // the next unlock reads the fresh state.
                        if self.session.is_some() {
                            self.lock();
                        }
                        self.sync_status = Some(lilypad_core::SyncStatusView::InSync);
                        self.set_status(t("status_pulled"));
                    }
                    Ok(lilypad_core::sync::net::PullOutcome::NoRemote) => {
                        self.set_status(t("status_no_remote"))
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::SyncMerge => {
                if self.sync_busy {
                    return Task::none();
                }
                let Some(vault) = self.active_vault_name() else {
                    self.set_status(t("status_no_vault"));
                    return Task::none();
                };
                if self.sync_pull_password.is_empty() {
                    self.set_status(t("status_sync_password_needed"));
                    return Task::none();
                }
                self.sync_busy = true;
                let data_dir = self.data_dir.clone();
                let pw = std::mem::take(&mut self.sync_pull_password);
                // Same discipline as pull: the merge rewrites the vault file,
                // so lock BEFORE it runs - an edit landing mid-merge would
                // otherwise be silently clobbered.
                if self.session.is_some() {
                    self.lock();
                    self.set_status(t("status_locked_for_sync"));
                }
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            // Owns the password for the duration of the merge,
                            // wiping it when the task ends.
                            let pw = zeroize::Zeroizing::new(pw);
                            let app = Service::open(OpenOptions {
                                data_dir,
                                auto_lock_after: None,
                            })
                            .map_err(|e| e.localized())?;
                            match lilypad_core::sync::net::sync_merge(&app, &vault, &pw)
                                .map_err(|e| e.localized())?
                            {
                                lilypad_core::sync::net::MergeOutcome::NoRemote => {
                                    Ok((vault, None))
                                }
                                lilypad_core::sync::net::MergeOutcome::Merged {
                                    report,
                                    pushed,
                                } => Ok((vault, Some((report.localized_summary(), pushed)))),
                            }
                        })
                        .await
                        .unwrap_or_else(|e| Err(tf("error_background", &[("detail", &e)])))
                    },
                    Message::SyncMergeDone,
                )
            }
            Message::SyncMergeDone(result) => {
                self.sync_busy = false;
                match result {
                    Ok((_, None)) => self.set_status(t("status_no_remote_push_first")),
                    Ok((vault, Some((summary, pushed)))) => {
                        // The on-disk vault was merged (safety backup taken):
                        // lock so the next unlock reads the fresh state - but
                        // only a session on the MERGED vault; a session on a
                        // different vault was never touched by this merge.
                        if self
                            .session
                            .as_ref()
                            .is_some_and(|s| s.name() == vault.as_str())
                        {
                            self.lock();
                        }
                        if self.active_vault_name().as_deref() == Some(vault.as_str()) {
                            self.sync_status = Some(lilypad_core::SyncStatusView::InSync);
                        }
                        let key = if pushed {
                            "status_synced_pushed"
                        } else {
                            "status_synced"
                        };
                        self.set_status(tf(key, &[("vault", &vault), ("summary", &summary)]));
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }
            Message::SyncRefreshStatus => {
                let Some(vault) = self.active_vault_name() else {
                    self.set_status(t("status_no_vault"));
                    return Task::none();
                };
                if self.sync_busy {
                    return Task::none();
                }
                self.sync_busy = true;
                let data_dir = self.data_dir.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let app = Service::open(OpenOptions {
                                data_dir,
                                auto_lock_after: None,
                            })
                            .map_err(|e| e.localized())?;
                            lilypad_core::sync::net::remote_status(&app, &vault)
                                .map(|s| (vault, s))
                                .map_err(|e| e.localized())
                        })
                        .await
                        .unwrap_or_else(|e| Err(tf("error_background", &[("detail", &e)])))
                    },
                    Message::SyncStatusLoaded,
                )
            }
            Message::SyncStatusLoaded(result) => {
                self.sync_busy = false;
                match result {
                    // Apply only if the status still describes the active vault
                    // (the user may have switched while the request ran).
                    Ok((vault, status)) => {
                        if self.active_vault_name().as_deref() == Some(vault.as_str()) {
                            self.sync_status = Some(status);
                        }
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }

            Message::VaultSelected(v) => {
                self.leave_preferences();
                // Picked in the sidebar: show its unlock card, also from the
                // create-vault card.
                if self.session.is_none() {
                    self.screen = Screen::Unlock;
                }
                self.selected_vault = Some(v);
                // The last fetched sync status described the previous vault.
                self.sync_status = None;
                Task::none()
            }
            Message::PasswordChanged(p) => {
                self.password = p;
                Task::none()
            }
            Message::UnlockPressed => {
                // A sync (pull or merge) rewrites the vault file: opening a
                // session mid-flight would hand the user a stale vault whose
                // edits raced the rewrite.
                if self.sync_busy {
                    self.set_status(t("status_sync_running"));
                    return Task::none();
                }
                let Some(vault) = self.selected_vault.clone() else {
                    self.set_status(t("status_no_vault"));
                    return Task::none();
                };
                self.pending_vault = vault.clone();
                self.unlocking = true;
                derive_key_task(self.data_dir.clone(), vault, self.password.clone())
            }
            Message::KeyDerived(result) => {
                self.unlocking = false;
                // Same guard as UnlockPressed, for a derive that was already
                // in flight when the sync started.
                if self.sync_busy {
                    self.password.zeroize();
                    self.password.clear();
                    self.set_status(t("status_sync_running"));
                    return Task::none();
                }
                match result {
                    Ok(key) => {
                        let vault = self.pending_vault.clone();
                        match self.service.open_with_key(&vault, &key) {
                            Ok(mut session) => {
                                // Apply the current auto-lock policy so it is
                                // correct regardless of what the service was
                                // opened with (settings may have changed since).
                                session.set_auto_lock(self.auto_lock_duration());
                                self.session = Some(session);
                                self.password.zeroize();
                                self.password.clear();
                                self.new_password.zeroize();
                                self.new_password.clear();
                                self.search.clear();
                                self.filter = if self.default_filter == 1 {
                                    Filter::Favorites
                                } else {
                                    Filter::All
                                };
                                self.selected = None;
                                self.selected_secret = None;
                                self.screen = Screen::Vault;
                                self.overlay = Overlay::None;
                                self.selected_vault = Some(vault.clone());
                                if !self.vaults.contains(&vault) {
                                    self.vaults.push(vault);
                                }
                                self.refresh_entries();
                                self.persist_settings();
                                self.authed = match lilypad_core::sync::net::auth_status(
                                    &self.service.token_file(),
                                ) {
                                    Ok(lilypad_core::sync::net::AuthState::Authenticated {
                                        username,
                                    }) => Some(username),
                                    _ => None,
                                };
                            }
                            Err(e) => self.set_status(e.localized()),
                        }
                    }
                    Err(e) => self.set_status(e),
                }
                Task::none()
            }

            Message::ShowCreate => {
                self.leave_preferences();
                self.screen = Screen::Create;
                Task::none()
            }
            Message::ShowUnlock => {
                self.screen = Screen::Unlock;
                Task::none()
            }
            Message::NewNameChanged(n) => {
                self.new_name = n;
                Task::none()
            }
            Message::NewPasswordChanged(p) => {
                self.new_password = p;
                Task::none()
            }
            Message::CreatePressed => {
                let name = self.new_name.trim().to_string();
                if name.is_empty() {
                    self.set_status(t("status_vault_name_needed"));
                    return Task::none();
                }
                if self.new_password.is_empty() {
                    self.set_status(t("status_master_needed"));
                    return Task::none();
                }
                self.pending_vault = name.clone();
                self.unlocking = true;
                create_vault_task(self.data_dir.clone(), name, self.new_password.clone())
            }

            Message::FilterSelected(f) => {
                self.leave_preferences();
                self.filter = f;
                self.selected = None;
                self.selected_secret = None;
                self.editing = false;
                Task::none()
            }
            Message::SearchChanged(q) => {
                self.search = q;
                Task::none()
            }

            Message::SelectEntry(label) => {
                self.selected = Some(label);
                self.editing = false;
                self.reveal = false;
                self.reveal_hold = false;
                self.reveal_selected();
                self.refresh_totp();
                Task::none()
            }
            Message::ToggleReveal => {
                self.reveal = !self.reveal;
                Task::none()
            }
            Message::RevealHold(held) => {
                self.reveal_hold = held;
                Task::none()
            }
            Message::RowHovered(label) => {
                self.hovered = label;
                Task::none()
            }
            Message::QuickCopy(label, kind) => {
                let value: Option<(String, &'static str)> =
                    self.session.as_ref().and_then(|session| match kind {
                        QuickKind::Username => self
                            .entries
                            .iter()
                            .find(|e| e.label == label)
                            .and_then(|e| e.username.clone())
                            .map(|u| (u, "copied_username")),
                        QuickKind::Password => lilypad_core::reveal_secret(session, &label)
                            .ok()
                            .map(|s| (s.password.clone(), "copied_password")),
                        QuickKind::Totp => lilypad_core::reveal_secret(session, &label)
                            .ok()
                            .and_then(|s| s.totp_secret.clone())
                            .and_then(|ts| lilypad_core::totp::code_for_secret(&ts).ok())
                            .map(|c| ((*c).clone(), "copied_totp")),
                    });
                match value {
                    Some((v, what)) => self.copy(&v, what),
                    None => self.set_status(t("status_nothing_to_copy")),
                }
                Task::none()
            }
            Message::ClipboardCancel => {
                let _ = lilypad_core::clipboard::copy_to_clipboard("");
                self.clipboard_clear_at = None;
                self.set_status(t("status_clipboard_cleared"));
                Task::none()
            }
            Message::CopyPassword => {
                if let Some(pw) = self.selected_secret.as_ref().map(|s| s.password.clone()) {
                    self.copy(&pw, "copied_password");
                }
                Task::none()
            }
            Message::CopyUsername => {
                let user = self
                    .selected
                    .as_ref()
                    .and_then(|label| self.entries.iter().find(|e| &e.label == label))
                    .and_then(|e| e.username.clone());
                match user {
                    Some(u) => self.copy(&u, "copied_username"),
                    None => self.set_status(t("status_no_username")),
                }
                Task::none()
            }
            Message::CopyTotp => {
                let ts = self
                    .selected_secret
                    .as_ref()
                    .and_then(|s| s.totp_secret.clone());
                match ts {
                    Some(ts) => match lilypad_core::totp::code_for_secret(&ts) {
                        Ok(code) => self.copy(&code, "copied_totp"),
                        Err(e) => self.set_status(e.localized()),
                    },
                    None => self.set_status(t("status_no_totp")),
                }
                Task::none()
            }
            Message::OpenUrl(url) => {
                let _ = webbrowser::open(&url);
                Task::none()
            }
            Message::ToggleFavorite(label) => {
                let new_state = !self
                    .entries
                    .iter()
                    .find(|e| e.label == label)
                    .map(|e| e.is_favorite)
                    .unwrap_or(false);
                if let Some(session) = self.session.as_mut() {
                    if let Err(e) =
                        lilypad_core::set_favorite(&self.service, session, &label, new_state)
                    {
                        self.set_status(e.localized());
                    } else {
                        self.refresh_entries();
                    }
                }
                Task::none()
            }
            Message::SetColor(label, color) => {
                if let Some(session) = self.session.as_mut() {
                    let _ = lilypad_core::set_color(&self.service, session, &label, color);
                    self.refresh_entries();
                }
                Task::none()
            }
            Message::Lock => {
                self.lock();
                self.set_status(t("status_locked"));
                Task::none()
            }

            Message::ShowAddForm => {
                self.form = FormBuffers::default();
                self.overlay = Overlay::AddForm;
                Task::none()
            }
            Message::StartEdit => {
                if let (Some(label), Some(secret)) =
                    (self.selected.clone(), self.selected_secret.as_ref())
                {
                    let view = self.entries.iter().find(|e| e.label == label).cloned();
                    if let Some(view) = view {
                        self.form = FormBuffers {
                            editing: Some(label.clone()),
                            label,
                            username: view.username.unwrap_or_default(),
                            url: view.url.unwrap_or_default(),
                            password: secret.password.clone(),
                            notes: secret.notes.clone().unwrap_or_default(),
                            totp: secret.totp_secret.clone().unwrap_or_default(),
                            tags: view.tags.join(", "),
                            folder: view.folder.unwrap_or_default(),
                            entry_type: Some(view.entry_type),
                            expiry_days: None, // keep current unless changed
                        };
                        self.editing = true;
                    }
                }
                Task::none()
            }
            Message::FormChanged(field, value) => {
                match field {
                    FormField::Label => self.form.label = value,
                    FormField::Username => self.form.username = value,
                    FormField::Url => self.form.url = value,
                    FormField::Password => self.form.password = value,
                    FormField::Notes => self.form.notes = value,
                    FormField::Totp => self.form.totp = value,
                    FormField::Tags => self.form.tags = value,
                    FormField::Folder => self.form.folder = value,
                }
                Task::none()
            }
            Message::FormTypeSelected(entry_type) => {
                self.form.entry_type = Some(entry_type);
                Task::none()
            }
            Message::FormExpirySelected(days) => {
                self.form.expiry_days = days;
                Task::none()
            }
            Message::FormCancel => {
                self.editing = false;
                self.overlay = Overlay::None;
                Task::none()
            }
            Message::FormSave => {
                self.save_form();
                Task::none()
            }

            Message::DeleteRequested(label) => {
                self.overlay = Overlay::ConfirmDelete(label);
                Task::none()
            }
            Message::DeleteConfirmed => {
                if let Overlay::ConfirmDelete(label) = &self.overlay {
                    let label = label.clone();
                    let is_trashed = self.trashed.iter().any(|e| e.label == label);
                    if let Some(session) = self.session.as_mut() {
                        let result = if is_trashed {
                            lilypad_core::delete_entry(&self.service, session, &label)
                        } else {
                            lilypad_core::soft_delete(&self.service, session, &label)
                        };
                        match result {
                            Ok(()) => {
                                self.refresh_entries();
                                if self.selected.as_deref() == Some(label.as_str()) {
                                    self.selected = None;
                                    self.selected_secret = None;
                                }
                                self.set_status(if is_trashed {
                                    tf("status_deleted", &[("label", &label)])
                                } else {
                                    tf("status_trashed", &[("label", &label)])
                                });
                            }
                            Err(e) => self.set_status(e.localized()),
                        }
                    }
                }
                self.overlay = Overlay::None;
                Task::none()
            }
            Message::DeleteCancelled => {
                self.overlay = Overlay::None;
                Task::none()
            }
            Message::RestoreEntry(label) => {
                if let Some(session) = self.session.as_mut() {
                    match lilypad_core::restore(&self.service, session, &label) {
                        Ok(()) => {
                            self.refresh_entries();
                            self.set_status(tf("status_restored", &[("label", &label)]));
                        }
                        Err(e) => self.set_status(e.localized()),
                    }
                }
                Task::none()
            }

            Message::ShowGenerator => {
                if self.gen_result.is_empty() {
                    self.regen();
                }
                self.overlay = Overlay::Generator;
                Task::none()
            }
            Message::GenLength(v) => {
                self.gen_length = v as usize;
                self.regen();
                Task::none()
            }
            Message::GenToggle(set) => {
                match set {
                    CharSet::Upper => self.gen_upper = !self.gen_upper,
                    CharSet::Lower => self.gen_lower = !self.gen_lower,
                    CharSet::Digits => self.gen_digits = !self.gen_digits,
                    CharSet::Symbols => self.gen_symbols = !self.gen_symbols,
                }
                self.regen();
                Task::none()
            }
            Message::GenRegenerate => {
                self.regen();
                Task::none()
            }
            Message::GenUse => {
                let r = self.gen_result.clone();
                if !r.is_empty() {
                    self.copy(&r, "copied_generated");
                }
                Task::none()
            }
            Message::CloseGenerator => {
                self.overlay = Overlay::None;
                Task::none()
            }
        }
    }

    /// Picking a destination in the sidebar while Preferences is open goes
    /// there: Preferences closes.
    fn leave_preferences(&mut self) {
        if self.preferences_open {
            self.preferences_open = false;
            self.close_preferences();
        }
    }

    /// Leaving Preferences wipes the secrets typed there: the new master
    /// password and the sync password.
    fn close_preferences(&mut self) {
        self.cm_new.zeroize();
        self.cm_new.clear();
        self.cm_confirm.zeroize();
        self.cm_confirm.clear();
        self.sync_pull_password.zeroize();
        self.sync_pull_password.clear();
    }

    pub fn save_form(&mut self) {
        if self.form.label.trim().is_empty() {
            self.set_status(t("status_label_needed"));
            return;
        }
        let label = self.form.label.trim().to_string();
        let editing = self.form.editing.clone();
        let username = opt(&self.form.username);
        let url = opt(&self.form.url);
        let notes = opt(&self.form.notes);
        let totp = opt(&self.form.totp);
        let entry_type = self.form.entry_type.clone().unwrap_or(EntryType::Login);
        let tags: Vec<String> = self
            .form
            .tags
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(String::from)
            .collect();
        let folder = opt(&self.form.folder);
        let expiry = self.form.expiry_days;

        let Some(session) = self.session.as_mut() else {
            return;
        };
        let result = match &editing {
            Some(original) => {
                // Field-preserving edit: attachments, backup codes, email,
                // phone and custom fields are kept; the form now owns tags and
                // folder too, and a password change enters history.
                lilypad_core::edit_entry(
                    &self.service,
                    session,
                    original,
                    lilypad_core::EntryEdit {
                        new_label: label.clone(),
                        username,
                        url,
                        entry_type,
                        password: self.form.password.clone(),
                        notes,
                        totp_secret: totp,
                        tags: Some(tags),
                        folder: Some(folder),
                    },
                )
            }
            None => {
                let mut secret = EntrySecret::new(self.form.password.clone());
                secret.notes = notes;
                secret.totp_secret = totp;
                let metadata = EntryMetadata {
                    username,
                    url,
                    entry_type,
                    tags,
                    folder,
                };
                lilypad_core::add_entry(&self.service, session, &label, metadata, &secret)
            }
        };
        // Apply the expiry choice ("keep" = None skips this entirely).
        let result = result.and_then(|()| match expiry {
            Some(days) => {
                let session = self.session.as_mut().expect("checked above");
                lilypad_core::set_expiry_days(&self.service, session, &label, days)
            }
            None => Ok(()),
        });
        match result {
            Ok(()) => {
                self.editing = false;
                self.overlay = Overlay::None;
                self.refresh_entries();
                self.selected = Some(label.clone());
                self.reveal = false;
                self.reveal_selected();
                self.set_status(tf("status_saved", &[("label", &label)]));
            }
            Err(e) => self.set_status(e.localized()),
        }
    }
}

pub fn opt(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

pub fn derive_key_task(
    data_dir: Option<PathBuf>,
    vault: String,
    password: String,
) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(move || {
                let app = Service::open(OpenOptions {
                    data_dir,
                    auto_lock_after: None,
                })
                .map_err(|e| e.localized())?;
                app.derive_unlock_key(&vault, &password)
                    .map_err(|e| e.localized())
            })
            .await
            {
                Ok(res) => res,
                Err(e) => Err(tf("error_background", &[("detail", &e)])),
            }
        },
        Message::KeyDerived,
    )
}

pub fn create_vault_task(
    data_dir: Option<PathBuf>,
    vault: String,
    password: String,
) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(move || {
                let app = Service::open(OpenOptions {
                    data_dir,
                    auto_lock_after: None,
                })
                .map_err(|e| e.localized())?;
                let session = app
                    .create_vault(&vault, &password)
                    .map_err(|e| e.localized())?;
                Ok::<KeyMaterial, String>(session.key().clone())
            })
            .await
            {
                Ok(res) => res,
                Err(e) => Err(tf("error_background", &[("detail", &e)])),
            }
        },
        Message::KeyDerived,
    )
}
