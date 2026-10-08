// Categorized settings page for LilypadApp, included from app.rs (same module).
//
// A full-screen preferences surface modeled on Colony's settings menu: a left
// category rail and a right pane of collapsible sections. Every control here is
// wired to a real `lilypad-app` setting (theme, density, auto-lock, clipboard,
// default view, master password) so the page is genuinely functional, not
// decorative.

/// The settings categories: (icon glyph, title, one-line description).
fn settings_categories() -> [(&'static str, &'static str, &'static str); 6] {
    use fonts::icons::*;
    [
        (COG, "General", "How Lilypad behaves day to day."),
        (LEAF, "Appearance", "Make Lilypad yours."),
        (SHIELD, "Security", "Protect what is in your vault."),
        (VAULT, "Vault", "What is inside and where it lives."),
        (SYNC, "Sync", "Back your vaults up to GitHub."),
        (INFO, "About", "Version, sync, and gestures."),
    ]
}

impl LilypadApp {
    // -- Page shell ----------------------------------------------------------

    fn view_settings(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);

        let header = row![
            self.icon(fonts::icons::COG, 18.0, p.primary),
            hgap(10.0),
            text("Settings").size(22).font(fonts::FONT_BOLD).color(p.text_primary),
            hfill(),
            button(
                row![
                    self.icon(fonts::icons::CLOSE, 12.0, p.text_muted),
                    hgap(6.0),
                    text("Close").size(13),
                ]
                .align_y(Alignment::Center)
            )
            .on_press(Message::CloseSettings)
            .padding([6, 12])
            .style(theme::ghost_style(t, d)),
        ]
        .align_y(Alignment::Center);

        // Category rail
        let mut nav = column![].spacing(3);
        for (i, (icon, title, _desc)) in settings_categories().into_iter().enumerate() {
            let active = self.settings_category == i;
            let style: BtnStyle = if active {
                boxed_btn(move |_, _| theme::nav_button_active(t, d))
            } else {
                boxed_btn(move |_, s| match s {
                    button::Status::Hovered => theme::nav_button_hovered(t, d),
                    button::Status::Pressed => theme::nav_button_pressed(t, d),
                    _ => theme::nav_button(t, d),
                })
            };
            nav = nav.push(
                button(
                    row![
                        self.icon(icon, 14.0, if active { p.primary } else { p.text_muted }),
                        hgap(10.0),
                        text(title).size(13),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press(Message::SettingsCategory(i))
                .width(Length::Fill)
                .padding([9, 12])
                .style(style),
            );
        }
        let nav_col = container(nav).width(Length::Fixed(188.0));

        let content: Element<Message> = match self.settings_category {
            0 => self.settings_general(),
            1 => self.settings_appearance(),
            2 => self.settings_security(),
            3 => self.settings_vault(),
            4 => self.settings_sync(),
            _ => self.settings_about(),
        };
        let content_area = container(
            scrollable(
                container(content).padding(iced::Padding {
                    top: 0.0,
                    right: 24.0,
                    bottom: 24.0,
                    left: 0.0,
                }),
            )
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        let body = row![nav_col, hgap(12.0), content_area].height(Length::Fill);

        // Status/error line (e.g. sync feedback) must be visible here too - the
        // buttons that produce it live on this page.
        let page = column![header, self.status_inline(), vgap(12.0), body]
            .padding(28)
            .width(Length::Fill)
            .height(Length::Fill);

        container(page)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| theme::app_container(t, d))
            .into()
    }

    // -- Categories ----------------------------------------------------------

    fn settings_general(&self) -> Element<'static, Message> {
        let default_pick = self.s_pick(
            "On unlock, show",
            vec![("all".into(), "All items".into()), ("fav".into(), "Favorites".into())],
            if self.default_filter == 1 { "fav" } else { "all" },
            |k| Message::SetDefaultFilter(if k == "fav" { 1 } else { 0 }),
        );

        let restore = self.s_toggle(
            "Reopen last vault",
            "Preselect the vault you used last time on the unlock screen.",
            self.restore_last_vault,
            Message::ToggleRestoreLastVault,
        );

        column![
            self.s_header("General", "How Lilypad behaves day to day."),
            self.s_section(
                "default_view",
                "Default view",
                column![
                    self.s_desc("Which list Lilypad opens on when you unlock a vault."),
                    vgap(10.0),
                    default_pick,
                ]
                .into(),
            ),
            vgap(6.0),
            self.s_section("startup", "Startup", column![restore].into()),
        ]
        .into()
    }

    fn settings_appearance(&self) -> Element<'static, Message> {
        column![
            self.s_header("Appearance", "Make Lilypad yours."),
            self.s_section("theme", "Theme", self.theme_grid()),
            vgap(6.0),
            self.s_section("density", "Interface density", self.density_list()),
            vgap(6.0),
            self.s_section("preview", "Preview", self.appearance_preview()),
        ]
        .into()
    }

    fn settings_security(&self) -> Element<'static, Message> {
        let auto = self.s_pick(
            "Lock after",
            vec![
                ("0".into(), "Never".into()),
                ("1".into(), "1 minute".into()),
                ("5".into(), "5 minutes".into()),
                ("15".into(), "15 minutes".into()),
                ("30".into(), "30 minutes".into()),
                ("60".into(), "1 hour".into()),
            ],
            &self.auto_lock_minutes.to_string(),
            |k| Message::SetAutoLock(k.parse().unwrap_or(5)),
        );
        let clip = self.s_pick(
            "Clear clipboard after",
            vec![
                ("10".into(), "10 seconds".into()),
                ("20".into(), "20 seconds".into()),
                ("30".into(), "30 seconds".into()),
                ("45".into(), "45 seconds".into()),
                ("60".into(), "60 seconds".into()),
            ],
            &self.clipboard_secs.to_string(),
            |k| Message::SetClipboardSecs(k.parse().unwrap_or(20)),
        );

        column![
            self.s_header("Security", "Protect what is in your vault."),
            self.s_section(
                "autolock",
                "Auto-lock",
                column![
                    self.s_desc("Lock the vault automatically after a period of inactivity."),
                    vgap(10.0),
                    auto,
                ]
                .into(),
            ),
            vgap(6.0),
            self.s_section(
                "clipboard",
                "Clipboard",
                column![
                    self.s_desc("A copied password is wiped from the clipboard after this delay."),
                    vgap(10.0),
                    clip,
                ]
                .into(),
            ),
            vgap(6.0),
            self.s_section("master", "Master password", self.change_master_body()),
        ]
        .into()
    }

    fn settings_vault(&self) -> Element<'static, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let mut col = column![self.s_header("Vault", "What is inside and where it lives.")];

        if let Some(session) = self.session.as_ref() {
            let folders = session.vault().list_folder_tree().len();
            let tags = session.vault().list_tags().len();
            let favs = self.entries.iter().filter(|e| e.is_favorite).count();
            let stats = column![
                self.s_info_row(fonts::icons::VAULT, "Entries", self.entries.len().to_string()),
                self.s_divider(),
                self.s_info_row(fonts::icons::FOLDER, "Folders", folders.to_string()),
                self.s_divider(),
                self.s_info_row(fonts::icons::TAG, "Tags", tags.to_string()),
                self.s_divider(),
                self.s_info_row(fonts::icons::STAR, "Favorites", favs.to_string()),
                self.s_divider(),
                self.s_info_row(fonts::icons::TRASH, "In Trash", self.trashed.len().to_string()),
            ];
            col = col.push(self.s_section(
                "stats",
                "Contents",
                column![self.s_desc(&format!("Vault '{}'.", session.name())), vgap(8.0), stats].into(),
            ));
            col = col.push(vgap(6.0));

            if let Some(report) = &self.health {
                let letter = grade_letter(report.score.grade);
                let display = grade_display(report.score.score, report.score.grade);
                let gc = theme::health_grade_color(letter, &p);
                let badge = row![
                    container(
                        text(display).size(14).font(fonts::FONT_BOLD).color(p.background),
                    )
                    .padding([2, 9])
                    .style(move |_| container::Style {
                        background: Some(Background::Color(gc)),
                        border: iced::Border { radius: 6.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                    hgap(10.0),
                    text(format!("{}/100 - {} issue(s)", report.score.score, report.issues.len()))
                        .size(13)
                        .color(p.text_secondary),
                ]
                .align_y(Alignment::Center);
                let mut hbody = column![badge].spacing(6);
                for issue in report.issues.iter().take(4) {
                    let sev = match issue.severity {
                        lilypad_app::IssueSeverity::Critical => p.danger,
                        lilypad_app::IssueSeverity::Warning => p.warning,
                        lilypad_app::IssueSeverity::Info => p.primary,
                    };
                    hbody = hbody.push(
                        row![
                            self.icon(fonts::icons::CIRCLE, 7.0, sev),
                            hgap(8.0),
                            text(issue.title.clone()).size(12).color(p.text_muted),
                        ]
                        .align_y(Alignment::Center),
                    );
                }
                hbody = hbody.push(vgap(8.0));
                hbody = hbody.push(self.s_desc(
                    "Check your passwords against known breaches (Have-I-Been-Pwned). \
                     Only the first 5 characters of each password's SHA-1 hash are \
                     sent - passwords never leave this machine.",
                ));
                hbody = hbody.push(vgap(6.0));
                hbody = hbody.push(
                    button(
                        row![
                            self.icon(fonts::icons::SHIELD, 12.0, p.background),
                            hgap(8.0),
                            text(if self.breach_busy {
                                "Checking..."
                            } else {
                                "Check for breaches"
                            })
                            .size(13),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .on_press_maybe((!self.breach_busy).then_some(Message::BreachCheckRun))
                    .padding([8, 14])
                    .style(theme::primary_style(t, d)),
                );
                col = col.push(self.s_section("health", "Health (Watchtower)", hbody.into()));
                col = col.push(vgap(6.0));
            }
        } else {
            let mut vlist = column![].spacing(2);
            for v in &self.vaults {
                vlist = vlist.push(self.s_info_row(fonts::icons::VAULT, v, String::new()));
            }
            let body = column![
                self.s_desc("Unlock a vault to see its contents and health."),
                vgap(8.0),
                vlist,
            ];
            col = col.push(self.s_section(
                "vaults",
                &format!("Vaults ({})", self.vaults.len()),
                body.into(),
            ));
            col = col.push(vgap(6.0));
        }

        if self.session.is_some() {
            col = col.push(self.s_section(
                "import_export",
                "Import / Export",
                self.import_export_body(),
            ));
            col = col.push(vgap(6.0));
        }

        let location = self
            .data_dir
            .as_ref()
            .map(|d| d.display().to_string())
            .unwrap_or_else(|| "Default Colony/Lilypad directory".to_string());
        col = col.push(self.s_section(
            "location",
            "Storage",
            column![
                self.s_desc("Vault files and settings are stored here, with 0600 permissions."),
                vgap(8.0),
                self.s_info_row(fonts::icons::FOLDER, "Location", location),
            ]
            .into(),
        ));

        col.into()
    }

    fn import_export_body(&self) -> Element<'static, Message> {
        let (t, d) = (self.theme, self.density);
        let p = self.theme.palette();
        column![
            self.s_desc(
                "Import from another password manager - the format is auto-detected: \
                 LastPass, Bitwarden (CSV/JSON), KeePassXC, 1Password, Safari, \
                 Chrome/Edge, Firefox, Proton Pass, Dashlane, or a Lilypad CSV. \
                 Existing entries are never overwritten.",
            ),
            vgap(8.0),
            row![
                text_input("Path to the exported file", &self.import_path)
                    .on_input(Message::ImportPathChanged)
                    .on_submit(Message::ImportRun)
                    .padding(10)
                    .style(theme::text_input_style_closure(t, d)),
                hgap(8.0),
                button(
                    row![
                        self.icon(fonts::icons::DOWNLOAD, 12.0, p.background),
                        hgap(8.0),
                        text("Import").size(13),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press(Message::ImportRun)
                .padding([8, 14])
                .style(theme::primary_style(t, d)),
            ]
            .align_y(Alignment::Center),
            vgap(16.0),
            self.s_desc(
                "Export this vault as PLAINTEXT CSV (every secret revealed) - for \
                 backups or moving away. Delete the file when you are done with it.",
            ),
            vgap(8.0),
            row![
                text_input("Destination path (.csv)", &self.export_path)
                    .on_input(Message::ExportPathChanged)
                    .on_submit(Message::ExportRun)
                    .padding(10)
                    .style(theme::text_input_style_closure(t, d)),
                hgap(8.0),
                button(
                    row![
                        self.icon(fonts::icons::UPLOAD, 12.0, p.text_secondary),
                        hgap(8.0),
                        text("Export").size(13),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press(Message::ExportRun)
                .padding([8, 14])
                .style(theme::secondary_style(t, d)),
            ]
            .align_y(Alignment::Center),
        ]
        .into()
    }

    fn settings_sync(&self) -> Element<'static, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let vault = self.active_vault_name();

        // -- Account section --
        let account: Element<Message> = match (&self.authed, &self.sync_login) {
            (_, Some(login)) => {
                // Device-flow in progress: surface the code the user must type.
                column![
                    self.s_desc("Authorize Lilypad in the browser tab that just opened, then enter this code:"),
                    vgap(10.0),
                    container(
                        text(login.user_code.clone())
                            .size(26)
                            .font(fonts::FONT_BOLD)
                            .color(p.primary),
                    )
                    .padding([10, 18])
                    .style(move |_| theme::elevated_container(t, d)),
                    vgap(8.0),
                    text(login.verification_uri.clone()).size(12).color(p.text_muted),
                    vgap(8.0),
                    row![
                        text("Waiting for authorization...").size(12).color(p.text_secondary),
                        hgap(12.0),
                        button(text("Cancel").size(12))
                            .on_press(Message::SyncLoginCancel)
                            .padding([4, 10])
                            .style(theme::ghost_style(t, d)),
                    ]
                    .align_y(Alignment::Center),
                ]
                .into()
            }
            (Some(user), None) => column![
                row![
                    self.icon(fonts::icons::CIRCLE_CHECK, 13.0, p.success),
                    hgap(8.0),
                    text(format!("Connected as {user}")).size(13).color(p.text_primary),
                    hfill(),
                    button(text("Sign out").size(12))
                        .on_press_maybe((!self.sync_busy).then_some(Message::SyncLogout))
                        .padding([5, 10])
                        .style(theme::ghost_style(t, d)),
                ]
                .align_y(Alignment::Center),
            ]
            .into(),
            (None, None) => column![
                self.s_desc("Sign in with GitHub to push encrypted vaults to a private repository. Only the encrypted file ever leaves this machine."),
                vgap(10.0),
                button(
                    row![
                        self.icon(fonts::icons::GITHUB, 13.0, p.background),
                        hgap(8.0),
                        text(if self.sync_busy { "Contacting GitHub..." } else { "Sign in with GitHub" }).size(13),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press_maybe((!self.sync_busy).then_some(Message::SyncLoginStart))
                .padding([8, 14])
                .style(theme::primary_style(t, d)),
            ]
            .into(),
        };

        let mut col = column![
            self.s_header("Sync", "Back your vaults up to GitHub."),
            self.s_section("sync_account", "GitHub account", account),
        ];

        // -- Status + transfer (only meaningful once signed in) --
        if self.authed.is_some() {
            let vault_label = vault.clone().unwrap_or_else(|| "(no vault)".to_string());

            let (status_text, status_hint, status_color) = match &self.sync_status {
                Some(lilypad_app::SyncStatusView::InSync) => {
                    ("In sync", "Nothing to do.", p.success)
                }
                Some(lilypad_app::SyncStatusView::LocalAhead) => {
                    ("Local ahead", "Push to publish your changes.", p.warning)
                }
                Some(lilypad_app::SyncStatusView::RemoteAhead) => {
                    ("Remote ahead", "Pull to get the latest.", p.warning)
                }
                Some(lilypad_app::SyncStatusView::Conflict) => (
                    "Conflict",
                    "Both sides changed. Sync combines them entry-by-entry (a safety backup is kept).",
                    p.danger,
                ),
                Some(lilypad_app::SyncStatusView::NoRemote) => {
                    ("No remote", "Push to create it.", p.text_muted)
                }
                Some(lilypad_app::SyncStatusView::NoBaseline) => (
                    "No baseline",
                    "Push or pull once to establish one.",
                    p.text_muted,
                ),
                None => ("Unknown", "Check the status to compare with GitHub.", p.text_muted),
            };

            let status_body = column![
                row![
                    self.icon(fonts::icons::CIRCLE, 9.0, status_color),
                    hgap(8.0),
                    text(status_text).size(13).font(fonts::FONT_SEMIBOLD).color(p.text_primary),
                    hgap(10.0),
                    text(format!("vault '{vault_label}'")).size(12).color(p.text_muted),
                ]
                .align_y(Alignment::Center),
                vgap(4.0),
                self.s_desc(status_hint),
                vgap(10.0),
                button(
                    text(if self.sync_busy { "Checking..." } else { "Check status" }).size(12),
                )
                .on_press_maybe((!self.sync_busy).then_some(Message::SyncRefreshStatus))
                .padding([6, 12])
                .style(theme::secondary_style(t, d)),
            ];
            col = col.push(vgap(6.0));
            col = col.push(self.s_section("sync_status", "Status", status_body.into()));

            let merge_body = column![
                self.s_desc("Merges the remote vault into this one entry-by-entry: the newest version of each entry wins, deletions and renames propagate, and the merged vault is pushed back if the remote is missing local data. Works from any state, including conflicts (a safety backup is kept; the app locks during the sync)."),
                vgap(8.0),
                // No on_submit here on purpose: this field is shared by Sync
                // AND Pull, and both can push/replace data - an accidental
                // Enter must never fire either. The explicit button is the
                // only trigger.
                text_input("Master password (used by Sync and Pull)", &self.sync_pull_password)
                    .on_input(Message::SyncPullPasswordChanged)
                    .secure(true)
                    .padding(10)
                    .style(theme::text_input_style_closure(t, d)),
                vgap(8.0),
                button(
                    row![
                        self.icon(fonts::icons::REFRESH, 12.0, p.background),
                        hgap(8.0),
                        text(if self.sync_busy { "Working..." } else { "Sync now" }).size(13),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press_maybe((!self.sync_busy).then_some(Message::SyncMerge))
                .padding([8, 14])
                .style(theme::primary_style(t, d)),
            ];
            col = col.push(vgap(6.0));
            col = col.push(self.s_section("sync_merge", "Sync (recommended)", merge_body.into()));

            let transfer_body = column![
                self.s_desc("Push uploads the encrypted vault; it refuses to overwrite a remote that moved since your last sync."),
                vgap(8.0),
                button(
                    row![
                        self.icon(fonts::icons::UPLOAD, 12.0, p.background),
                        hgap(8.0),
                        text(if self.sync_busy { "Working..." } else { "Push to GitHub" }).size(13),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press_maybe((!self.sync_busy).then_some(Message::SyncPush))
                .padding([8, 14])
                .style(theme::secondary_style(t, d)),
                vgap(16.0),
                self.s_desc("Pull downloads and validates the remote vault, then REPLACES the local one with it (a safety backup is kept). Uses the master password entered in the Sync section above; the app locks after a pull."),
                vgap(8.0),
                button(
                    row![
                        self.icon(fonts::icons::DOWNLOAD, 12.0, p.text_secondary),
                        hgap(8.0),
                        text(if self.sync_busy { "Working..." } else { "Pull from GitHub" }).size(13),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press_maybe((!self.sync_busy).then_some(Message::SyncPull))
                .padding([8, 14])
                .style(theme::secondary_style(t, d)),
            ];
            col = col.push(vgap(6.0));
            col = col.push(self.s_section(
                "sync_transfer",
                "Push / Pull (advanced)",
                transfer_body.into(),
            ));
        }

        col.into()
    }

    fn settings_about(&self) -> Element<'static, Message> {
        let sync = self
            .authed
            .clone()
            .map(|u| format!("Connected as {u}"))
            .unwrap_or_else(|| "Not connected".to_string());

        let version = column![
            self.s_info_row(fonts::icons::LEAF, "Lilypad", format!("v{}", env!("CARGO_PKG_VERSION"))),
            self.s_divider(),
            self.s_info_row(fonts::icons::GITHUB, "Repository", "Project-Colony/Lilypad-Vault".to_string()),
            self.s_divider(),
            self.s_info_row(fonts::icons::SYNC, "GitHub sync", sync),
        ];

        let tips = [
            "Hold the password field to reveal it while pressed.",
            "Hover a list row for one-click copy of user, password, or TOTP.",
            "Click the star to favorite an entry.",
            "Deleting moves an entry to Trash first; delete again to purge it.",
        ];
        let mut tipcol = column![].spacing(6);
        for tip in tips {
            tipcol = tipcol.push(
                row![
                    self.icon(fonts::icons::CIRCLE_CHECK, 11.0, self.theme.palette().primary),
                    hgap(8.0),
                    text(tip).size(12).color(self.theme.palette().text_muted),
                ]
                .align_y(Alignment::Center),
            );
        }

        column![
            self.s_header("About", "Lilypad is a thin client over the lilypad-app service layer."),
            self.s_section("version", "Version", version.into()),
            vgap(6.0),
            self.s_section("tips", "Tips & gestures", tipcol.into()),
        ]
        .into()
    }

    // -- Appearance building blocks -----------------------------------------

    /// A 3-column grid of theme preview cards.
    fn theme_grid(&self) -> Element<'static, Message> {
        let mut grid = column![].spacing(8);
        for chunk in LilypadTheme::ALL.chunks(3) {
            let mut r = row![].spacing(8);
            for th in chunk {
                r = r.push(self.theme_card(*th));
            }
            for _ in chunk.len()..3 {
                r = r.push(container(Space::new()).width(Length::Fill));
            }
            grid = grid.push(r);
        }
        grid.into()
    }

    /// A single theme card: a mini preview painted in that theme's own colors.
    fn theme_card(&self, th: LilypadTheme) -> Element<'static, Message> {
        let p = self.theme.palette();
        let pal = th.palette();
        let selected = th == self.theme;

        let dot = container(Space::new())
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(14.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(pal.primary)),
                border: iced::Border { radius: 7.0.into(), ..Default::default() },
                ..Default::default()
            });
        let bar1 = container(Space::new())
            .width(Length::Fixed(54.0))
            .height(Length::Fixed(5.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(pal.text_secondary)),
                border: iced::Border { radius: 2.5.into(), ..Default::default() },
                ..Default::default()
            });
        let bar2 = container(Space::new())
            .width(Length::Fixed(38.0))
            .height(Length::Fixed(5.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(pal.surface_variant)),
                border: iced::Border { radius: 2.5.into(), ..Default::default() },
                ..Default::default()
            });
        let preview = container(
            row![dot, hgap(8.0), column![bar1, vgap(5.0), bar2]].align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fixed(44.0))
        .padding(10)
        .style(move |_| container::Style {
            background: Some(Background::Color(pal.background)),
            border: iced::Border { radius: 8.0.into(), color: pal.border, width: 1.0 },
            ..Default::default()
        });

        let check: Element<Message> = if selected {
            self.icon(fonts::icons::CHECK, 9.0, p.primary)
        } else {
            Space::new().into()
        };
        let name_row = row![
            text(th.name()).size(11).color(if selected { p.text_primary } else { p.text_muted }),
            hfill(),
            check,
        ]
        .align_y(Alignment::Center);

        button(column![preview, vgap(6.0), name_row].width(Length::Fill))
            .on_press(Message::ThemeSelected(th))
            .padding(5)
            .width(Length::Fill)
            .style(move |_, status| {
                let border = if selected {
                    p.primary
                } else {
                    match status {
                        button::Status::Hovered => p.text_muted,
                        _ => p.border,
                    }
                };
                button::Style {
                    background: Some(Background::Color(p.surface)),
                    text_color: p.text_primary,
                    border: iced::Border {
                        color: border,
                        width: if selected { 2.0 } else { 1.0 },
                        radius: 10.0.into(),
                    },
                    ..Default::default()
                }
            })
            .into()
    }

    fn density_list(&self) -> Element<'static, Message> {
        let mut col = column![].spacing(8);
        for v in UiVariation::ALL {
            col = col.push(self.density_card(v));
        }
        col.into()
    }

    fn density_card(&self, v: UiVariation) -> Element<'static, Message> {
        let p = self.theme.palette();
        let selected = v == self.density;
        let check: Element<Message> = if selected {
            self.icon(fonts::icons::CHECK, 11.0, p.primary)
        } else {
            Space::new().into()
        };
        button(
            row![
                column![
                    text(v.name())
                        .size(13)
                        .font(fonts::FONT_SEMIBOLD)
                        .color(if selected { p.text_primary } else { p.text_secondary }),
                    text(v.description()).size(11).color(p.text_muted),
                ]
                .spacing(2),
                hfill(),
                check,
            ]
            .align_y(Alignment::Center),
        )
        .on_press(Message::DensitySelected(v))
        .width(Length::Fill)
        .padding([10, 12])
        .style(move |_, status| {
            let border = if selected {
                p.primary
            } else {
                match status {
                    button::Status::Hovered => p.text_muted,
                    _ => p.border,
                }
            };
            button::Style {
                background: Some(Background::Color(p.surface)),
                text_color: p.text_primary,
                border: iced::Border {
                    color: border,
                    width: if selected { 2.0 } else { 1.0 },
                    radius: 10.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
    }

    /// A live sample entry rendered in the current theme, so tweaks are visible.
    fn appearance_preview(&self) -> Element<'static, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let sample = container(
            row![
                container(self.icon(fonts::icons::KEY, 15.0, p.primary))
                    .center(Length::Fixed(32.0))
                    .style(move |_| theme::icon_badge_container(t, d)),
                hgap(10.0),
                column![
                    text("Example entry").size(14).font(fonts::FONT_SEMIBOLD).color(p.text_primary),
                    text("user@example.com").size(12).color(p.text_muted),
                ]
                .spacing(2),
                hfill(),
                container(Space::new())
                    .width(Length::Fixed(8.0))
                    .height(Length::Fixed(8.0))
                    .style(move |_| theme::dot_indicator(p.success)),
            ]
            .align_y(Alignment::Center),
        )
        .padding(12)
        .width(Length::Fill)
        .style(move |_| theme::card_container(t, d));

        column![
            self.s_desc(&format!("{} - {}", self.theme.name(), self.density.name())),
            vgap(10.0),
            sample,
        ]
        .into()
    }

    // -- Security building blocks -------------------------------------------

    fn change_master_body(&self) -> Element<'static, Message> {
        let (t, d) = (self.theme, self.density);
        if self.session.is_none() {
            return column![self.s_desc("Unlock a vault to change its master password.")].into();
        }
        column![
            self.s_desc("Re-encrypts every entry under a new key. At least 8 characters."),
            vgap(10.0),
            text_input("New master password", &self.cm_new)
                .on_input(Message::CmNewChanged)
                .secure(true)
                .padding(10)
                .style(theme::text_input_style_closure(t, d)),
            vgap(6.0),
            text_input("Confirm new password", &self.cm_confirm)
                .on_input(Message::CmConfirmChanged)
                .on_submit(Message::ChangeMasterSubmit)
                .secure(true)
                .padding(10)
                .style(theme::text_input_style_closure(t, d)),
            vgap(8.0),
            button(text("Change password").size(13))
                .on_press(Message::ChangeMasterSubmit)
                .padding([8, 14])
                .style(theme::primary_style(t, d)),
        ]
        .into()
    }

    // -- Shared building blocks ---------------------------------------------

    fn s_header(&self, title: &str, desc: &str) -> Element<'static, Message> {
        let p = self.theme.palette();
        column![
            text(title.to_string()).size(18).font(fonts::FONT_BOLD).color(p.text_primary),
            vgap(4.0),
            text(desc.to_string()).size(12).color(p.text_muted),
            vgap(18.0),
        ]
        .into()
    }

    fn s_desc(&self, s: &str) -> Element<'static, Message> {
        text(s.to_string()).size(12).color(self.theme.palette().text_muted).into()
    }

    fn s_divider(&self) -> Element<'static, Message> {
        let c = theme::with_alpha(self.theme.palette().border, 0.7);
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(c)),
                ..Default::default()
            })
            .into()
    }

    /// A collapsible section: a header that toggles its body's visibility.
    fn s_section<'a>(
        &self,
        key: &str,
        title: &str,
        content: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let expanded = self.settings_sections.contains(key);
        let chevron = if expanded {
            fonts::icons::CHEVRON_DOWN
        } else {
            fonts::icons::CHEVRON_RIGHT
        };
        let key_owned = key.to_string();
        let header = button(
            row![
                text(title.to_string()).size(15).font(fonts::FONT_SEMIBOLD).color(p.text_primary),
                hfill(),
                self.icon(chevron, 9.0, p.text_muted),
            ]
            .align_y(Alignment::Center),
        )
        .on_press(Message::SettingsToggleSection(key_owned))
        .width(Length::Fill)
        .padding([12, 6])
        .style(move |_, status| match status {
            button::Status::Hovered => theme::ghost_button_hovered(t, d),
            _ => {
                let mut st = theme::ghost_button(t, d);
                st.text_color = p.text_primary;
                st
            }
        });

        if expanded {
            column![
                header,
                self.s_divider(),
                container(content)
                    .padding(iced::Padding { top: 12.0, right: 6.0, bottom: 6.0, left: 6.0 })
                    .width(Length::Fill),
            ]
            .spacing(0)
            .into()
        } else {
            header.into()
        }
    }

    /// A labeled on/off row with a custom-drawn toggle switch.
    fn s_toggle(&self, title: &str, desc: &str, on: bool, msg: Message) -> Element<'static, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let track = if on { p.primary } else { p.surface_variant };
        let knob_offset = if on { 16.0 } else { 2.0 };
        let knob = container(Space::new())
            .width(Length::Fixed(14.0))
            .height(Length::Fixed(14.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::WHITE)),
                border: iced::Border { radius: 7.0.into(), ..Default::default() },
                ..Default::default()
            });
        let switch = container(
            container(knob).padding(iced::Padding {
                top: 1.0,
                right: 0.0,
                bottom: 0.0,
                left: knob_offset,
            }),
        )
        .width(Length::Fixed(34.0))
        .height(Length::Fixed(18.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(track)),
            border: iced::Border { radius: 9.0.into(), ..Default::default() },
            ..Default::default()
        });

        button(
            row![
                column![
                    text(title.to_string()).size(13).color(p.text_primary),
                    text(desc.to_string()).size(11).color(p.text_muted),
                ]
                .spacing(2),
                hfill(),
                switch,
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .on_press(msg)
        .padding([6, 4])
        .width(Length::Fill)
        .style(theme::ghost_style(t, d))
        .into()
    }

    /// A labeled dropdown row. `options` are (key, display-label) pairs; the
    /// selection callback receives the chosen key.
    fn s_pick(
        &self,
        title: &str,
        options: Vec<(String, String)>,
        selected_key: &str,
        on_select: impl Fn(String) -> Message + 'static,
    ) -> Element<'static, Message> {
        let p = self.theme.palette();
        let labels: Vec<String> = options.iter().map(|(_, l)| l.clone()).collect();
        let selected_label = options
            .iter()
            .find(|(k, _)| k == selected_key)
            .map(|(_, l)| l.clone());
        let keys: Vec<String> = options.iter().map(|(k, _)| k.clone()).collect();
        let labels_map = labels.clone();

        let pl = pick_list(labels, selected_label, move |chosen: String| {
            let idx = labels_map.iter().position(|l| *l == chosen).unwrap_or(0);
            on_select(keys.get(idx).cloned().unwrap_or_default())
        })
        .text_size(12)
        .padding([5, 10])
        .font(fonts::FONT_REGULAR)
        .style(move |_, status| {
            let bg = match status {
                pick_list::Status::Active => p.surface_variant,
                _ => theme::lighten_color(p.surface_variant, 0.05),
            };
            pick_list::Style {
                text_color: p.text_primary,
                placeholder_color: p.text_muted,
                handle_color: p.text_muted,
                background: Background::Color(bg),
                border: iced::Border { color: p.border, width: 1.0, radius: 8.0.into() },
            }
        })
        .menu_style(move |_| overlay_menu::Style {
            background: Background::Color(p.surface),
            border: iced::Border { color: p.border, width: 1.0, radius: 8.0.into() },
            text_color: p.text_primary,
            selected_text_color: p.text_primary,
            selected_background: Background::Color(p.primary),
            shadow: iced::Shadow::default(),
        });

        row![
            text(title.to_string()).size(13).color(p.text_primary),
            hfill(),
            pl,
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }

    fn s_info_row(&self, icon: &str, label: &str, value: String) -> Element<'static, Message> {
        let p = self.theme.palette();
        row![
            self.icon(icon, 13.0, p.primary),
            hgap(10.0),
            text(label.to_string()).size(13).color(p.text_muted),
            hfill(),
            text(value).size(13).color(p.text_primary),
        ]
        .align_y(Alignment::Center)
        .padding([6, 0])
        .into()
    }
}
