//! The entry detail pane, the edit and add forms, and the overlays.

use super::{
    boxed_btn, entry_color, expiry_label_for, hfill, hgap, strength_level, strength_score, tint,
    type_icon, type_label, vgap, BtnStyle, Ring, ALL_TYPES, EXPIRY_CHOICES,
};
use crate::fonts;
use crate::message::{CharSet, FormField, Message};
use crate::state::LilypadApp;
use crate::theme::{self};
use iced::widget::{
    button, column, container, mouse_area, pick_list, row, scrollable, slider, text, text_input,
    Canvas, Space,
};
use iced::{Alignment, Background, Color, Element, Length};
use lilypad_core::EntryType;

impl LilypadApp {
    pub fn detail_pane(&self) -> Element<'_, Message> {
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

    pub fn detail_view(&self, label: &str) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let view = self.entries.iter().find(|e| e.label == label);
        let entry_type = view
            .map(|v| v.entry_type.clone())
            .unwrap_or(EntryType::Login);

        let header = row![
            container(self.icon(type_icon(&entry_type), 20.0, p.primary))
                .center(Length::Fixed(40.0))
                .style(move |_| theme::icon_badge_container(t, d)),
            hgap(12.0),
            text(label.to_string())
                .size(20)
                .font(fonts::FONT_BOLD)
                .color(p.text_primary),
            hgap(8.0),
            button(self.icon(
                fonts::icons::STAR,
                15.0,
                if view.map(|v| v.is_favorite).unwrap_or(false) {
                    p.warning
                } else {
                    p.text_muted
                },
            ))
            .on_press(Message::ToggleFavorite(label.to_string()))
            .style(theme::ghost_style(t, d)),
            hfill(),
            button(
                row![
                    self.icon(fonts::icons::EDIT, 13.0, p.text_secondary),
                    hgap(6.0),
                    text("Edit").size(13)
                ]
                .align_y(Alignment::Center)
            )
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
                let ring_color = if self.totp_left <= 5 {
                    p.warning
                } else {
                    p.primary
                };
                let ring = Canvas::new(Ring {
                    fraction: frac,
                    color: ring_color,
                    track: p.surface_variant,
                })
                .width(Length::Fixed(30.0))
                .height(Length::Fixed(30.0));
                body = body.push(
                    self.field_row(
                        "TOTP",
                        row![
                            ring,
                            hgap(12.0),
                            text(grouped)
                                .size(18)
                                .font(fonts::FONT_SEMIBOLD)
                                .color(p.text_primary),
                            hgap(8.0),
                            text(format!("{}s", self.totp_left))
                                .size(12)
                                .color(p.text_muted),
                        ]
                        .align_y(Alignment::Center)
                        .into(),
                        button(self.icon(fonts::icons::COPY, 13.0, p.text_secondary))
                            .on_press(Message::CopyTotp)
                            .style(theme::ghost_style(t, d))
                            .into(),
                    ),
                );
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
            for c in lilypad_core::EntryColor::all() {
                let c = *c;
                let active = v.color == Some(c);
                let fill = entry_color(c);
                let ring = if active {
                    p.text_primary
                } else {
                    Color::TRANSPARENT
                };
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
                    .saturating_sub(lilypad_core::current_timestamp())
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
                &lilypad_core::format_timestamp_relative(v.updated_at),
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
                            lilypad_core::IssueSeverity::Critical => p.danger,
                            lilypad_core::IssueSeverity::Warning => p.warning,
                            lilypad_core::IssueSeverity::Info => p.primary,
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
                            border: iced::Border {
                                radius: 8.0.into(),
                                color: bd,
                                width: 1.0,
                            },
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

    pub fn strength_bar(&self, pw: &str) -> Element<'static, Message> {
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

    pub fn field(
        &self,
        label: &str,
        value: &str,
        copy: Option<Message>,
        open: Option<Message>,
    ) -> Element<'static, Message> {
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
        self.field_row(
            label,
            text(value.to_string())
                .size(14)
                .color(p.text_primary)
                .into(),
            actions.into(),
        )
    }

    pub fn field_row(
        &self,
        label: &str,
        value: Element<'static, Message>,
        actions: Element<'static, Message>,
    ) -> Element<'static, Message> {
        let p = self.theme.palette();
        column![
            text(label.to_uppercase()).size(10).color(p.text_muted),
            row![value, hfill(), actions].align_y(Alignment::Center),
        ]
        .spacing(3)
        .padding([9, 0])
        .into()
    }

    pub fn detail_edit(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let title = if self.form.editing.is_some() {
            "Edit entry"
        } else {
            "New entry"
        };
        let inp = |ph: &'static str, value: &str, field: FormField, secure: bool| {
            text_input(ph, value)
                .on_input(move |v| Message::FormChanged(field, v))
                .secure(secure)
                .padding(10)
                .style(theme::text_input_style_closure(t, d))
        };
        let small = |s: &'static str| text(s).size(10).color(p.text_muted);

        // Entry-type picker.
        let type_labels: Vec<String> = ALL_TYPES
            .iter()
            .map(|ty| type_label(ty).to_string())
            .collect();
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
            text(title)
                .size(20)
                .font(fonts::FONT_BOLD)
                .color(p.text_primary),
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
            inp(
                "TOTP secret (base32 or otpauth://)",
                &self.form.totp,
                FormField::Totp,
                false
            ),
            inp(
                "Tags (comma-separated)",
                &self.form.tags,
                FormField::Tags,
                false
            ),
            inp(
                "Folder (e.g. Work/Email)",
                &self.form.folder,
                FormField::Folder,
                false
            ),
            vgap(4.0),
            row![
                button(text("Save").size(14))
                    .on_press(Message::FormSave)
                    .style(theme::primary_style(t, d)),
                button(text("Cancel").size(14))
                    .on_press(Message::FormCancel)
                    .style(theme::ghost_style(t, d)),
            ]
            .spacing(8),
        ]
        .spacing(10)
        .into()
    }

    // -- Overlays ------------------------------------------------------------

    pub fn view_add_form(&self) -> Element<'_, Message> {
        self.detail_edit()
    }

    pub fn view_confirm(&self, label: &str) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        column![
            text("Delete entry?")
                .size(20)
                .font(fonts::FONT_BOLD)
                .color(p.danger),
            text(format!("'{label}' will be permanently removed."))
                .size(14)
                .color(p.text_secondary),
            vgap(8.0),
            row![
                button(text("Delete").size(14))
                    .on_press(Message::DeleteConfirmed)
                    .style(theme::danger_style(t, d)),
                button(text("Cancel").size(14))
                    .on_press(Message::DeleteCancelled)
                    .style(theme::ghost_style(t, d)),
            ]
            .spacing(8),
        ]
        .spacing(10)
        .into()
    }

    pub fn view_generator(&self) -> Element<'_, Message> {
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
            text("Password generator")
                .size(20)
                .font(fonts::FONT_BOLD)
                .color(p.text_primary),
            text(self.gen_result.clone())
                .size(16)
                .color(p.primary)
                .font(fonts::FONT_SEMIBOLD),
            row![
                text(format!("Length: {}", self.gen_length))
                    .size(13)
                    .color(p.text_secondary),
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
                button(text("Regenerate").size(13))
                    .on_press(Message::GenRegenerate)
                    .style(theme::secondary_style(t, d)),
                button(text("Copy").size(13))
                    .on_press(Message::GenUse)
                    .style(theme::primary_style(t, d)),
                hfill(),
                button(text("Close").size(13))
                    .on_press(Message::CloseGenerator)
                    .style(theme::ghost_style(t, d)),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into()
    }

    // -- Shared chrome -------------------------------------------------------
}
