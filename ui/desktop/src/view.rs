// View layer for LilypadApp, included from app.rs (same module).

impl LilypadApp {
    pub fn view(&self) -> Element<'_, Message> {
        // The settings page is a full-screen replacement, reachable while locked
        // or unlocked; it takes precedence over the normal screen.
        let base: Element<Message> = if self.settings_open {
            self.view_settings()
        } else {
            match self.screen {
                Screen::Unlock => self.view_unlock(),
                Screen::Create => self.view_create(),
                Screen::Vault => self.view_vault(),
            }
        };

        let mut content: Element<Message> = match &self.overlay {
            Overlay::None => base,
            Overlay::Generator => stack![base, self.modal(self.view_generator())].into(),
            Overlay::AddForm => stack![base, self.modal(self.view_add_form())].into(),
            Overlay::ConfirmDelete(label) => {
                stack![base, self.modal(self.view_confirm(label))].into()
            }
        };

        if let Some(at) = self.clipboard_clear_at {
            let left = at.saturating_duration_since(Instant::now()).as_secs();
            content = stack![content, self.copy_toast(left)].into();
        }

        let (t, d) = (self.theme, self.density);
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| theme::app_container(t, d))
            .into()
    }

    fn icon(&self, glyph: &str, size: f32, color: Color) -> Element<'static, Message> {
        // Nerd Font glyphs have an asymmetric side-bearing (they sit right of
        // center in their advance box). Compensate with a right nudge so icons
        // look centered - especially in icon-only buttons.
        fonts::centered_icon_colored(glyph, size, color)
    }

    // -- Unlock / Create -----------------------------------------------------

    fn view_unlock(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let vault_buttons = self.vaults.iter().fold(
            column![].spacing(6).width(Length::Fill),
            |col, name| {
                let selected = self.selected_vault.as_deref() == Some(name.as_str());
                let style: BtnStyle = if selected {
                    boxed_btn(theme::primary_style(t, d))
                } else {
                    boxed_btn(theme::secondary_style(t, d))
                };
                col.push(
                    button(text(name.clone()).size(15))
                        .on_press(Message::VaultSelected(name.clone()))
                        .width(Length::Fill)
                        .padding(10)
                        .style(style),
                )
            },
        );
        let unlock_btn = if self.unlocking {
            button(text("Unlocking…").size(15)).padding(12).width(Length::Fill)
        } else {
            button(text("Unlock").size(15))
                .on_press(Message::UnlockPressed)
                .padding(12)
                .width(Length::Fill)
                .style(theme::primary_style(t, d))
        };
        let card = column![
            text("🪷 Lilypad").size(30).font(fonts::FONT_BOLD).color(p.primary),
            text("Choose a vault and enter your master password").size(13).color(p.text_muted),
            vgap(8.0),
            scrollable(vault_buttons).height(Length::Fixed(160.0)),
            text_input("Master password", &self.password)
                .on_input(Message::PasswordChanged)
                .on_submit(Message::UnlockPressed)
                .secure(true)
                .padding(12)
                .style(theme::text_input_style_closure(t, d)),
            unlock_btn,
            self.status_inline(),
            row![
                button(text("Create a new vault").size(13))
                    .on_press(Message::ShowCreate)
                    .style(theme::ghost_style(t, d)),
                hfill(),
                button(text("Settings").size(13))
                    .on_press(Message::OpenSettings)
                    .style(theme::ghost_style(t, d)),
            ],
        ]
        .spacing(12);
        self.centered(card)
    }

    fn view_create(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let card = column![
            text("Create a vault").size(26).font(fonts::FONT_BOLD).color(p.primary),
            text("Pick a name and a strong master password").size(13).color(p.text_muted),
            vgap(8.0),
            text_input("Vault name", &self.new_name)
                .on_input(Message::NewNameChanged)
                .padding(12)
                .style(theme::text_input_style_closure(t, d)),
            text_input("Master password", &self.new_password)
                .on_input(Message::NewPasswordChanged)
                .on_submit(Message::CreatePressed)
                .secure(true)
                .padding(12)
                .style(theme::text_input_style_closure(t, d)),
            button(text(if self.unlocking { "Creating…" } else { "Create vault" }).size(15))
                .on_press(Message::CreatePressed)
                .padding(12)
                .width(Length::Fill)
                .style(theme::primary_style(t, d)),
            self.status_inline(),
            button(text("Back to unlock").size(13))
                .on_press(Message::ShowUnlock)
                .style(theme::ghost_style(t, d)),
        ]
        .spacing(12);
        self.centered(card)
    }

    // -- The 3-pane vault ----------------------------------------------------

    fn view_vault(&self) -> Element<'_, Message> {
        row![
            container(self.sidebar())
                .width(Length::Fixed(232.0))
                .height(Length::Fill),
            container(self.item_list())
                .width(Length::Fixed(330.0))
                .height(Length::Fill),
            container(self.detail_pane())
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .height(Length::Fill)
        .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let all = self.entries.len();
        let favs = self.entries.iter().filter(|e| e.is_favorite).count();

        let mut col = column![
            row![
                self.icon(fonts::icons::FLOWER, 16.0, p.primary),
                hgap(8.0),
                text(self.session.as_ref().map(|s| s.name().to_string()).unwrap_or_default())
                    .size(14)
                    .font(fonts::FONT_SEMIBOLD),
                hfill(),
                self.icon(fonts::icons::CIRCLE, 9.0, p.success),
            ]
            .align_y(Alignment::Center)
            .padding([4, 8]),
            vgap(6.0),
            self.nav_row(fonts::icons::VAULT, "All items", Some(all), self.filter == Filter::All, Filter::All),
            self.nav_row(fonts::icons::STAR, "Favorites", Some(favs), self.filter == Filter::Favorites, Filter::Favorites),
        ]
        .spacing(2);

        // Types present
        let mut types: Vec<EntryType> = Vec::new();
        for e in &self.entries {
            if !types.contains(&e.entry_type) {
                types.push(e.entry_type.clone());
            }
        }
        if !types.is_empty() {
            col = col.push(self.section_header("Types"));
            for ty in types {
                let count = self.entries.iter().filter(|e| e.entry_type == ty).count();
                let active = self.filter == Filter::Type(ty.clone());
                col = col.push(self.nav_row(type_icon(&ty), type_label(&ty), Some(count), active, Filter::Type(ty)));
            }
        }

        // Folders
        if let Some(session) = self.session.as_ref() {
            let folders = session.vault().list_folder_tree();
            if !folders.is_empty() {
                col = col.push(self.section_header("Folders"));
                for f in folders {
                    let depth = f.matches('/').count();
                    let leaf = f.rsplit('/').next().unwrap_or(&f).to_string();
                    let active = self.filter == Filter::Folder(f.clone());
                    let label = format!("{}{leaf}", "  ".repeat(depth));
                    col = col.push(self.nav_row_owned(fonts::icons::FOLDER, label, None, active, Filter::Folder(f)));
                }
            }
            let tags = session.vault().list_tags();
            if !tags.is_empty() {
                col = col.push(self.section_header("Tags"));
                for tag in tags {
                    let active = self.filter == Filter::Tag(tag.clone());
                    col = col.push(self.nav_row_owned(fonts::icons::TAG, tag.clone(), None, active, Filter::Tag(tag)));
                }
            }
        }

        // Footer status
        let lock_line = self
            .session
            .as_ref()
            .and_then(|s| s.time_until_lock())
            .map(|d| format!("Locks in {}:{:02}", d.as_secs() / 60, d.as_secs() % 60))
            .unwrap_or_else(|| "Unlocked".to_string());
        let mut footer = column![].spacing(6).padding([8, 8]);
        if let Some(report) = &self.health {
            let letter = grade_letter(report.score.grade);
            let display = grade_display(report.score.score, report.score.grade);
            let gc = theme::health_grade_color(letter, &p);
            footer = footer.push(
                row![
                    container(text(display).size(11).font(fonts::FONT_BOLD).color(p.background))
                        .padding([1, 7])
                        .style(move |_| container::Style {
                            background: Some(Background::Color(gc)),
                            border: iced::Border { radius: 5.0.into(), ..Default::default() },
                            ..Default::default()
                        }),
                    hgap(8.0),
                    text("Vault health").size(11).color(p.text_muted),
                ]
                .align_y(Alignment::Center),
            );
        }
        if !self.trashed.is_empty() {
            footer = footer.push(self.nav_row_owned(
                fonts::icons::TRASH,
                "Trash".to_string(),
                Some(self.trashed.len()),
                self.filter == Filter::Trash,
                Filter::Trash,
            ));
        }
        footer = footer.push(
            row![self.icon(fonts::icons::CIRCLE, 8.0, p.success), hgap(8.0), text(lock_line).size(11).color(p.text_muted)]
                .align_y(Alignment::Center),
        );
        if let Some(user) = &self.authed {
            footer = footer.push(
                row![self.icon(fonts::icons::SYNC, 10.0, p.primary), hgap(8.0), text(format!("GitHub · {user}")).size(11).color(p.text_muted)]
                    .align_y(Alignment::Center),
            );
        }
        footer = footer.push(
            row![
                button(row![self.icon(fonts::icons::COG, 12.0, p.text_muted), hgap(6.0), text("Settings").size(12)].align_y(Alignment::Center))
                    .on_press(Message::OpenSettings)
                    .style(theme::ghost_style(t, d)),
            ],
        );

        container(
            column![
                scrollable(col.padding([8, 6])).height(Length::Fill),
                container(footer).width(Length::Fill),
            ]
            .height(Length::Fill),
        )
        .style(move |_| theme::sidebar_container(t, d))
        .height(Length::Fill)
        .into()
    }

    fn section_header(&self, label: &str) -> Element<'_, Message> {
        let p = self.theme.palette();
        text(label.to_uppercase())
            .size(10)
            .color(p.text_muted)
            .font(fonts::FONT_MEDIUM)
            .into()
    }

    fn nav_row(&self, glyph: &str, label: &str, count: Option<usize>, active: bool, filter: Filter) -> Element<'_, Message> {
        self.nav_row_owned(glyph, label.to_string(), count, active, filter)
    }

    fn nav_row_owned(&self, glyph: &str, label: String, count: Option<usize>, active: bool, filter: Filter) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let mut r = row![
            self.icon(glyph, 14.0, if active { p.primary } else { p.text_secondary }),
            hgap(9.0),
            text(label).size(13),
            hfill(),
        ]
        .align_y(Alignment::Center);
        if let Some(c) = count {
            r = r.push(
                container(text(format!("{c}")).size(11).color(p.text_muted))
                    .padding([0, 6])
                    .style(move |_| theme::badge_container(t, d)),
            );
        }
        let style: BtnStyle = if active {
            boxed_btn(move |_, _| theme::nav_button_active(t, d))
        } else {
            boxed_btn(move |_, s| match s {
                button::Status::Hovered => theme::nav_button_hovered(t, d),
                button::Status::Pressed => theme::nav_button_pressed(t, d),
                _ => theme::nav_button(t, d),
            })
        };
        button(r)
            .on_press(Message::FilterSelected(filter))
            .width(Length::Fill)
            .padding([7, 9])
            .style(style)
            .into()
    }

    fn item_list(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let visible = self.visible();
        let search = text_input("Search", &self.search)
            .on_input(Message::SearchChanged)
            .padding(9)
            .style(theme::text_input_style_closure(t, d));

        let toolbar = row![
            search,
            hgap(8.0),
            button(self.icon(fonts::icons::PLUS, 14.0, p.text_primary))
                .on_press(Message::ShowAddForm)
                .padding(9)
                .style(theme::primary_style(t, d)),
            button(self.icon(fonts::icons::WAND, 14.0, p.text_secondary))
                .on_press(Message::ShowGenerator)
                .padding(9)
                .style(theme::secondary_style(t, d)),
            button(self.icon(fonts::icons::LOCK, 14.0, p.danger))
                .on_press(Message::Lock)
                .padding(9)
                .style(theme::ghost_style(t, d)),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let list: Element<Message> = if visible.is_empty() {
            container(text("No entries here.").size(13).color(p.text_muted))
                .padding(20)
                .into()
        } else {
            visible
                .iter()
                .fold(column![].spacing(4), |col, e| col.push(self.list_row(e)))
                .into()
        };

        column![
            container(toolbar).padding(10),
            scrollable(container(list).padding([0, 8])).height(Length::Fill),
        ]
        .height(Length::Fill)
        .into()
    }

    fn quick_btn(&self, glyph: &str, msg: Message) -> Element<'static, Message> {
        let (t, d) = (self.theme, self.density);
        let p = self.theme.palette();
        button(self.icon(glyph, 12.0, p.text_secondary))
            .on_press(msg)
            .padding(4)
            .style(theme::ghost_style(t, d))
            .into()
    }

    fn list_row(&self, e: &EntryView) -> Element<'static, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let label = e.label.clone();
        let selected = self.selected.as_deref() == Some(label.as_str());
        let hovered = self.hovered.as_deref() == Some(label.as_str());
        let (strength, has_totp) = self.derived.get(&label).copied().unwrap_or((0, false));

        let tile = container(self.icon(type_icon(&e.entry_type), 15.0, p.primary))
            .center(Length::Fixed(32.0))
            .style(move |_| theme::icon_badge_container(t, d));
        let subtitle = e
            .username
            .clone()
            .or_else(|| e.url.clone())
            .unwrap_or_else(|| type_label(&e.entry_type).to_string());

        let in_trash = self.filter == Filter::Trash;
        let mut trailing = row![].spacing(5).align_y(Alignment::Center);
        if in_trash {
            trailing = trailing.push(
                button(text("Restore").size(11))
                    .on_press(Message::RestoreEntry(label.clone()))
                    .padding([2, 8])
                    .style(theme::secondary_style(t, d)),
            );
        } else if hovered {
            if e.username.is_some() {
                trailing = trailing
                    .push(self.quick_btn(fonts::icons::USER, Message::QuickCopy(label.clone(), QuickKind::Username)));
            }
            trailing = trailing
                .push(self.quick_btn(fonts::icons::KEY, Message::QuickCopy(label.clone(), QuickKind::Password)));
            if has_totp {
                trailing = trailing
                    .push(self.quick_btn(fonts::icons::CLOCK, Message::QuickCopy(label.clone(), QuickKind::Totp)));
            }
            if let Some(url) = e.url.clone() {
                trailing = trailing.push(self.quick_btn(fonts::icons::EXTERNAL_LINK, Message::OpenUrl(url)));
            }
        }
        if e.is_favorite {
            trailing = trailing.push(self.icon(fonts::icons::STAR, 13.0, p.warning));
        }
        let dot_color = if strength == 0 {
            p.text_muted
        } else {
            theme::strength_color(strength_score(strength), &p)
        };
        trailing = trailing.push(
            container(Space::new())
                .width(Length::Fixed(8.0))
                .height(Length::Fixed(8.0))
                .style(move |_| theme::dot_indicator(dot_color)),
        );

        // Color label: a slim tinted bar on the row's leading edge.
        let color_bar: Element<'static, Message> = match e.color {
            Some(c) => {
                let fill = entry_color(c);
                container(Space::new())
                    .width(Length::Fixed(3.0))
                    .height(Length::Fixed(30.0))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(fill)),
                        border: iced::Border { radius: 2.0.into(), ..Default::default() },
                        ..Default::default()
                    })
                    .into()
            }
            None => hgap(3.0).into(),
        };

        let content = row![
            color_bar,
            hgap(6.0),
            tile,
            hgap(10.0),
            column![
                text(label.clone()).size(14).font(fonts::FONT_SEMIBOLD).color(p.text_primary),
                text(subtitle).size(12).color(p.text_muted),
            ]
            .spacing(2)
            .width(Length::Fill),
            trailing,
        ]
        .align_y(Alignment::Center);

        let sel_bg = p.surface_variant;
        let hov_bg = p.hover;
        let bg = move |_: &iced::Theme| container::Style {
            background: if selected {
                Some(Background::Color(sel_bg))
            } else if hovered {
                Some(Background::Color(hov_bg))
            } else {
                None
            },
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        mouse_area(container(content).width(Length::Fill).padding(9).style(bg))
            .on_press(Message::SelectEntry(label.clone()))
            .on_enter(Message::RowHovered(Some(label)))
            .on_exit(Message::RowHovered(None))
            .into()
    }

    fn detail_pane(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let inner: Element<Message> = if self.editing {
            self.detail_edit()
        } else if let Some(label) = &self.selected {
            self.detail_view(label)
        } else {
            container(
                column![
                    self.icon(fonts::icons::VAULT, 40.0, p.text_muted),
                    vgap(10.0),
                    text("Select an entry").size(16).color(p.text_muted),
                ]
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        };
        container(inner)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| theme::card_container(t, d))
            .into()
    }

    fn detail_view(&self, label: &str) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let view = self.entries.iter().find(|e| e.label == label);
        let entry_type = view.map(|v| v.entry_type.clone()).unwrap_or(EntryType::Login);

        let header = row![
            container(self.icon(type_icon(&entry_type), 20.0, p.primary))
                .center(Length::Fixed(40.0))
                .style(move |_| theme::icon_badge_container(t, d)),
            hgap(12.0),
            text(label.to_string()).size(20).font(fonts::FONT_BOLD).color(p.text_primary),
            hgap(8.0),
            button(self.icon(
                fonts::icons::STAR,
                15.0,
                if view.map(|v| v.is_favorite).unwrap_or(false) { p.warning } else { p.text_muted },
            ))
            .on_press(Message::ToggleFavorite(label.to_string()))
            .style(theme::ghost_style(t, d)),
            hfill(),
            button(row![self.icon(fonts::icons::EDIT, 13.0, p.text_secondary), hgap(6.0), text("Edit").size(13)].align_y(Alignment::Center))
                .on_press(Message::StartEdit)
                .style(theme::secondary_style(t, d)),
            button(self.icon(fonts::icons::TRASH, 13.0, p.danger))
                .on_press(Message::DeleteRequested(label.to_string()))
                .style(theme::ghost_style(t, d)),
        ]
        .align_y(Alignment::Center);

        let mut body = column![].spacing(2);
        if let Some(v) = view {
            if let Some(u) = &v.username {
                body = body.push(self.field("Username", u, Some(Message::CopyUsername), None));
            }
            if let Some(u) = &v.url {
                body = body.push(self.field("URL", u, None, Some(Message::OpenUrl(u.clone()))));
            }
        }
        if let Some(secret) = &self.selected_secret {
            let shown = if self.revealed() {
                secret.password.clone()
            } else {
                "•".repeat(secret.password.chars().count().max(8))
            };
            let pw_area = mouse_area(text(shown).size(14).color(p.text_primary))
                .on_press(Message::RevealHold(true))
                .on_release(Message::RevealHold(false));
            body = body.push(
                self.field_row(
                    "Password",
                    row![
                        pw_area,
                        hgap(10.0),
                        text("hold to reveal").size(11).color(p.text_muted),
                    ]
                    .align_y(Alignment::Center)
                    .into(),
                    row![
                        button(text(if self.revealed() { "Hide" } else { "Show" }).size(12))
                            .on_press(Message::ToggleReveal)
                            .style(theme::ghost_style(t, d)),
                        button(self.icon(fonts::icons::COPY, 13.0, p.text_secondary))
                            .on_press(Message::CopyPassword)
                            .style(theme::ghost_style(t, d)),
                    ]
                    .spacing(4)
                    .into(),
                ),
            );
            body = body.push(self.strength_bar(&secret.password));
            if secret.totp_secret.is_some() {
                let code = if self.totp_code.is_empty() {
                    "------".to_string()
                } else {
                    self.totp_code.clone()
                };
                let grouped = if code.len() == 6 {
                    format!("{} {}", &code[..3], &code[3..])
                } else {
                    code
                };
                let frac = self.totp_left as f32 / 30.0;
                let ring_color = if self.totp_left <= 5 { p.warning } else { p.primary };
                let ring = Canvas::new(Ring {
                    fraction: frac,
                    color: ring_color,
                    track: p.surface_variant,
                })
                .width(Length::Fixed(30.0))
                .height(Length::Fixed(30.0));
                body = body.push(self.field_row(
                    "TOTP",
                    row![
                        ring,
                        hgap(12.0),
                        text(grouped).size(18).font(fonts::FONT_SEMIBOLD).color(p.text_primary),
                        hgap(8.0),
                        text(format!("{}s", self.totp_left)).size(12).color(p.text_muted),
                    ]
                    .align_y(Alignment::Center)
                    .into(),
                    button(self.icon(fonts::icons::COPY, 13.0, p.text_secondary))
                        .on_press(Message::CopyTotp)
                        .style(theme::ghost_style(t, d))
                        .into(),
                ));
            }
            if let Some(n) = &secret.notes {
                body = body.push(self.field("Notes", n, None, None));
            }
        }
        if let Some(v) = view {
            if !v.tags.is_empty() {
                let chips = v.tags.iter().fold(row![].spacing(5), |r, tag| {
                    r.push(
                        container(text(tag.clone()).size(11).color(p.text_secondary))
                            .padding([1, 8])
                            .style(move |_| theme::badge_container(t, d)),
                    )
                });
                body = body.push(self.field_row("Tags", chips.into(), Space::new().into()));
            }
            // Color label picker: 8 dots + clear. The active color has a ring.
            let mut dots = row![].spacing(6).align_y(Alignment::Center);
            for c in lilypad_app::EntryColor::all() {
                let c = *c;
                let active = v.color == Some(c);
                let fill = entry_color(c);
                let ring = if active { p.text_primary } else { Color::TRANSPARENT };
                dots = dots.push(
                    button(Space::new())
                        .on_press(Message::SetColor(label.to_string(), Some(c)))
                        .width(Length::Fixed(18.0))
                        .height(Length::Fixed(18.0))
                        .padding(0)
                        .style(move |_, _| button::Style {
                            background: Some(Background::Color(fill)),
                            border: iced::Border {
                                color: ring,
                                width: if active { 2.0 } else { 0.0 },
                                radius: 9.0.into(),
                            },
                            ..Default::default()
                        }),
                );
            }
            if v.color.is_some() {
                dots = dots.push(
                    button(text("clear").size(11))
                        .on_press(Message::SetColor(label.to_string(), None))
                        .padding([2, 6])
                        .style(theme::ghost_style(t, d)),
                );
            }
            body = body.push(self.field_row("Color", dots.into(), Space::new().into()));

            if v.password_expires_at > 0 {
                let days = v
                    .password_expires_at
                    .saturating_sub(lilypad_common::current_timestamp())
                    / 86_400;
                body = body.push(self.field(
                    "Password expires",
                    &format!("in {days} day(s)"),
                    None,
                    None,
                ));
            }
            body = body.push(self.field(
                "Modified",
                &lilypad_common::format_timestamp_relative(v.updated_at),
                None,
                None,
            ));
        }

        let banner: Element<Message> = self
            .health
            .as_ref()
            .and_then(|report| {
                report
                    .issues
                    .iter()
                    .filter(|i| i.affected_entries.iter().any(|a| a == label))
                    .max_by_key(|i| i.severity)
                    .map(|issue| {
                        let sev = match issue.severity {
                            lilypad_app::IssueSeverity::Critical => p.danger,
                            lilypad_app::IssueSeverity::Warning => p.warning,
                            lilypad_app::IssueSeverity::Info => p.primary,
                        };
                        let bg = tint(sev, 0.14);
                        let bd = tint(sev, 0.4);
                        container(
                            row![
                                self.icon(fonts::icons::TRIANGLE_EXCLAMATION, 13.0, sev),
                                hgap(8.0),
                                text(issue.title.clone()).size(12).color(p.text_primary),
                            ]
                            .align_y(Alignment::Center),
                        )
                        .padding([8, 10])
                        .width(Length::Fill)
                        .style(move |_| container::Style {
                            background: Some(Background::Color(bg)),
                            border: iced::Border { radius: 8.0.into(), color: bd, width: 1.0 },
                            ..Default::default()
                        })
                        .into()
                    })
            })
            .unwrap_or_else(|| vgap(0.0).into());

        column![
            header,
            vgap(12.0),
            banner,
            vgap(4.0),
            scrollable(container(body).max_width(720)).height(Length::Fill)
        ]
        .spacing(4)
        .height(Length::Fill)
        .into()
    }

    fn strength_bar(&self, pw: &str) -> Element<'static, Message> {
        let p = self.theme.palette();
        let level = strength_level(pw);
        let color = theme::strength_color(strength_score(level), &p);
        let mut r = row![].spacing(4).width(Length::Fixed(200.0));
        for i in 0..5u8 {
            let c = if i < level { color } else { p.surface_variant };
            r = r.push(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fixed(5.0))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(c)),
                        border: iced::Border {
                            radius: 2.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            );
        }
        r.into()
    }

    fn field(&self, label: &str, value: &str, copy: Option<Message>, open: Option<Message>) -> Element<'static, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let mut actions = row![].spacing(4);
        if let Some(open) = open {
            actions = actions.push(
                button(self.icon(fonts::icons::EXTERNAL_LINK, 13.0, p.primary))
                    .on_press(open)
                    .style(theme::ghost_style(t, d)),
            );
        }
        if let Some(copy) = copy {
            actions = actions.push(
                button(self.icon(fonts::icons::COPY, 13.0, p.text_secondary))
                    .on_press(copy)
                    .style(theme::ghost_style(t, d)),
            );
        }
        self.field_row(label, text(value.to_string()).size(14).color(p.text_primary).into(), actions.into())
    }

    fn field_row(&self, label: &str, value: Element<'static, Message>, actions: Element<'static, Message>) -> Element<'static, Message> {
        let p = self.theme.palette();
        column![
            text(label.to_uppercase()).size(10).color(p.text_muted),
            row![value, hfill(), actions].align_y(Alignment::Center),
        ]
        .spacing(3)
        .padding([9, 0])
        .into()
    }

    fn detail_edit(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let title = if self.form.editing.is_some() { "Edit entry" } else { "New entry" };
        let inp = |ph: &'static str, value: &str, field: FormField, secure: bool| {
            text_input(ph, value)
                .on_input(move |v| Message::FormChanged(field, v))
                .secure(secure)
                .padding(10)
                .style(theme::text_input_style_closure(t, d))
        };
        let small = |s: &'static str| text(s).size(10).color(p.text_muted);

        // Entry-type picker.
        let type_labels: Vec<String> = ALL_TYPES.iter().map(|ty| type_label(ty).to_string()).collect();
        let type_selected = Some(
            type_label(self.form.entry_type.as_ref().unwrap_or(&EntryType::Login)).to_string(),
        );
        let type_pick = pick_list(type_labels, type_selected, Message::FormTypeSelected)
            .text_size(12)
            .padding([6, 10]);

        // Expiry picker ("Keep current" leaves the entry's setting untouched).
        let expiry_labels: Vec<String> =
            EXPIRY_CHOICES.iter().map(|(l, _)| l.to_string()).collect();
        let expiry_selected = Some(expiry_label_for(self.form.expiry_days).to_string());
        let expiry_pick = pick_list(expiry_labels, expiry_selected, Message::FormExpirySelected)
            .text_size(12)
            .padding([6, 10]);

        column![
            text(title).size(20).font(fonts::FONT_BOLD).color(p.text_primary),
            vgap(6.0),
            inp("Label", &self.form.label, FormField::Label, false),
            row![
                column![small("TYPE"), type_pick].spacing(3),
                hgap(10.0),
                column![small("PASSWORD EXPIRES"), expiry_pick].spacing(3),
            ]
            .align_y(Alignment::End),
            inp("Username", &self.form.username, FormField::Username, false),
            inp("URL", &self.form.url, FormField::Url, false),
            inp("Password", &self.form.password, FormField::Password, true),
            inp("Notes", &self.form.notes, FormField::Notes, false),
            inp("TOTP secret (base32 or otpauth://)", &self.form.totp, FormField::Totp, false),
            inp("Tags (comma-separated)", &self.form.tags, FormField::Tags, false),
            inp("Folder (e.g. Work/Email)", &self.form.folder, FormField::Folder, false),
            vgap(4.0),
            row![
                button(text("Save").size(14)).on_press(Message::FormSave).style(theme::primary_style(t, d)),
                button(text("Cancel").size(14)).on_press(Message::FormCancel).style(theme::ghost_style(t, d)),
            ]
            .spacing(8),
        ]
        .spacing(10)
        .into()
    }

    // -- Overlays ------------------------------------------------------------

    fn view_add_form(&self) -> Element<'_, Message> {
        self.detail_edit()
    }

    fn view_confirm(&self, label: &str) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        column![
            text("Delete entry?").size(20).font(fonts::FONT_BOLD).color(p.danger),
            text(format!("'{label}' will be permanently removed.")).size(14).color(p.text_secondary),
            vgap(8.0),
            row![
                button(text("Delete").size(14)).on_press(Message::DeleteConfirmed).style(theme::danger_style(t, d)),
                button(text("Cancel").size(14)).on_press(Message::DeleteCancelled).style(theme::ghost_style(t, d)),
            ]
            .spacing(8),
        ]
        .spacing(10)
        .into()
    }

    fn view_generator(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let toggle = |label: &'static str, on: bool, set: CharSet| {
            let style: BtnStyle = if on {
                boxed_btn(theme::primary_style(t, d))
            } else {
                boxed_btn(theme::secondary_style(t, d))
            };
            button(text(format!("{} {label}", if on { "☑" } else { "☐" })).size(13))
                .on_press(Message::GenToggle(set))
                .style(style)
        };
        column![
            text("Password generator").size(20).font(fonts::FONT_BOLD).color(p.text_primary),
            text(self.gen_result.clone()).size(16).color(p.primary).font(fonts::FONT_SEMIBOLD),
            row![
                text(format!("Length: {}", self.gen_length)).size(13).color(p.text_secondary),
                slider(4.0..=64.0, self.gen_length as f32, Message::GenLength),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            row![
                toggle("A-Z", self.gen_upper, CharSet::Upper),
                toggle("a-z", self.gen_lower, CharSet::Lower),
                toggle("0-9", self.gen_digits, CharSet::Digits),
                toggle("!@#", self.gen_symbols, CharSet::Symbols),
            ]
            .spacing(6),
            row![
                button(text("Regenerate").size(13)).on_press(Message::GenRegenerate).style(theme::secondary_style(t, d)),
                button(text("Copy").size(13)).on_press(Message::GenUse).style(theme::primary_style(t, d)),
                hfill(),
                button(text("Close").size(13)).on_press(Message::CloseGenerator).style(theme::ghost_style(t, d)),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into()
    }

    // -- Shared chrome -------------------------------------------------------

    fn copy_toast(&self, left: u64) -> Element<'static, Message> {
        let (t, d) = (self.theme, self.density);
        let p = self.theme.palette();
        let card = container(
            row![
                self.icon(fonts::icons::CIRCLE_CHECK, 13.0, p.success),
                hgap(8.0),
                text(format!("Copied · clears in {left}s")).size(12).color(p.text_secondary),
                hgap(10.0),
                button(text("Undo").size(12))
                    .on_press(Message::ClipboardCancel)
                    .style(theme::ghost_style(t, d)),
            ]
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(move |_| theme::modal_container(t, d));
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(18)
            .into()
    }

    fn status_inline(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        match &self.status {
            Some(msg) => text(msg.clone()).size(13).color(p.text_secondary).into(),
            None => vgap(0.0).into(),
        }
    }

    fn centered<'a>(&self, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let (t, d) = (self.theme, self.density);
        let card = container(content)
            .padding(28)
            .width(Length::Fill)
            .max_width(440)
            .style(move |_| theme::card_container(t, d));
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
    }

    fn modal<'a>(&self, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        let (t, d) = (self.theme, self.density);
        let card = container(content)
            .padding(24)
            .max_width(560)
            .style(move |_| theme::modal_container(t, d));
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.55 })),
                ..Default::default()
            })
            .into()
    }
}
