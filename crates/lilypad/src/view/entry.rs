//! The entry detail pane, the edit and add forms, and the dialogs.

use super::{
    hfill, hgap, strength_level, type_icon, type_label, vgap, Ring, ALL_TYPES, EXPIRY_CHOICES,
};
use crate::icons;
use crate::message::{CharSet, FormField, Message};
use crate::state::LilypadApp;
use crate::theme;
use colony_ui::theme::Palette;
use colony_ui::{sz, ui_font_bold, ui_font_medium, widgets, Typography};
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, slider, text, text_input, Canvas, Space,
};
use iced::{Alignment, Background, Color, Element, Length};
use lilypad_core::i18n::{t, tf};
use lilypad_core::EntryType;

impl LilypadApp {
    pub fn detail_pane(&self) -> Element<'_, Message> {
        let d = self.density;
        let inner: Element<Message> = if self.editing {
            self.detail_edit()
        } else if let Some(label) = &self.selected {
            self.detail_view(label)
        } else {
            container(
                column![
                    icons::icon(icons::VAULT, 40, Palette::TEXT_MUTED()),
                    vgap(10.0),
                    text(t("detail_select_entry"))
                        .size(sz(16))
                        .color(Palette::TEXT_MUTED()),
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
            .style(move |_| theme::card_container(d))
            .into()
    }

    pub fn detail_view(&self, label: &str) -> Element<'_, Message> {
        let d = self.density;
        let view = self.entries.iter().find(|e| e.label == label);
        let entry_type = view
            .map(|v| v.entry_type.clone())
            .unwrap_or(EntryType::Login);
        let favorite = view.is_some_and(|v| v.is_favorite);

        let header = row![
            container(icons::icon(type_icon(&entry_type), 20, Palette::ACCENT()))
                .center(Length::Fixed(sz(40)))
                .style(|_| theme::icon_badge_container()),
            hgap(12.0),
            text(label.to_string())
                .size(sz(20))
                .font(ui_font_bold())
                .color(Palette::TEXT_PRIMARY()),
            hgap(8.0),
            button(icons::icon(
                icons::STAR,
                15,
                if favorite {
                    Palette::WARNING()
                } else {
                    Palette::TEXT_MUTED()
                },
            ))
            .on_press(Message::ToggleFavorite(label.to_string()))
            .style(theme::ghost_style(d)),
            hfill(),
            button(
                row![
                    icons::icon(icons::EDIT, 13, Palette::ACCENT()),
                    hgap(6.0),
                    text(t("detail_edit")).size(sz(13))
                ]
                .align_y(Alignment::Center)
            )
            .on_press(Message::StartEdit)
            .style(theme::secondary_style(d)),
            button(icons::icon(icons::TRASH, 13, Palette::ERROR()))
                .on_press(Message::DeleteRequested(label.to_string()))
                .style(theme::ghost_style(d)),
        ]
        .align_y(Alignment::Center);

        let mut body = column![].spacing(2);
        if let Some(v) = view {
            if let Some(u) = &v.username {
                body = body.push(self.field(
                    t("field_username"),
                    u,
                    Some(Message::CopyUsername),
                    None,
                ));
            }
            if let Some(u) = &v.url {
                body = body.push(self.field(
                    t("field_url"),
                    u,
                    None,
                    Some(Message::OpenUrl(u.clone())),
                ));
            }
        }
        if let Some(secret) = &self.selected_secret {
            let shown = if self.revealed() {
                secret.password.clone()
            } else {
                "•".repeat(secret.password.chars().count().max(8))
            };
            let pw_area = mouse_area(text(shown).size(sz(14)).color(Palette::TEXT_PRIMARY()))
                .on_press(Message::RevealHold(true))
                .on_release(Message::RevealHold(false));
            body = body.push(
                self.field_row(
                    t("field_password"),
                    row![
                        pw_area,
                        hgap(10.0),
                        text(t("detail_hold_to_reveal"))
                            .size(sz(11))
                            .color(Palette::TEXT_MUTED()),
                    ]
                    .align_y(Alignment::Center)
                    .into(),
                    row![
                        button(
                            text(if self.revealed() {
                                t("detail_hide")
                            } else {
                                t("detail_show")
                            })
                            .size(sz(12))
                        )
                        .on_press(Message::ToggleReveal)
                        .style(theme::ghost_style(d)),
                        button(icons::icon(icons::COPY, 13, Palette::TEXT_SECONDARY()))
                            .on_press(Message::CopyPassword)
                            .style(theme::ghost_style(d)),
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
                let ring = Canvas::new(Ring {
                    fraction: self.totp_left as f32 / 30.0,
                    color: if self.totp_left <= 5 {
                        Palette::WARNING()
                    } else {
                        Palette::ACCENT()
                    },
                    track: Palette::BG_CARD_HOVER(),
                })
                .width(Length::Fixed(sz(30)))
                .height(Length::Fixed(sz(30)));
                body = body.push(
                    self.field_row(
                        t("field_totp"),
                        row![
                            ring,
                            hgap(12.0),
                            text(grouped)
                                .size(sz(18))
                                .font(ui_font_bold())
                                .color(Palette::TEXT_PRIMARY()),
                            hgap(8.0),
                            text(tf("detail_totp_seconds", &[("secs", &self.totp_left)]))
                                .size(sz(12))
                                .color(Palette::TEXT_MUTED()),
                        ]
                        .align_y(Alignment::Center)
                        .into(),
                        button(icons::icon(icons::COPY, 13, Palette::TEXT_SECONDARY()))
                            .on_press(Message::CopyTotp)
                            .style(theme::ghost_style(d))
                            .into(),
                    ),
                );
            }
            if let Some(n) = &secret.notes {
                body = body.push(self.field(t("field_notes"), n, None, None));
            }
        }
        if let Some(v) = view {
            if !v.tags.is_empty() {
                let chips = v.tags.iter().fold(row![].spacing(5), |r, tag| {
                    r.push(
                        container(
                            text(tag.clone())
                                .size(sz(11))
                                .color(Palette::TEXT_SECONDARY()),
                        )
                        .padding([1, 8])
                        .style(|_| theme::badge_container()),
                    )
                });
                body =
                    body.push(self.field_row(t("field_tags"), chips.into(), Space::new().into()));
            }
            // Colour label picker: 8 dots + clear. The active colour has a ring.
            let mut dots = row![].spacing(6).align_y(Alignment::Center);
            for c in lilypad_core::EntryColor::all() {
                let c = *c;
                let active = v.color == Some(c);
                let fill = theme::entry_color(c);
                let ring = if active {
                    Palette::TEXT_PRIMARY()
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
                    button(text(t("detail_color_clear")).size(sz(11)))
                        .on_press(Message::SetColor(label.to_string(), None))
                        .padding([2, 6])
                        .style(theme::ghost_style(d)),
                );
            }
            body = body.push(self.field_row(t("field_color"), dots.into(), Space::new().into()));

            if v.password_expires_at > 0 {
                let days = v
                    .password_expires_at
                    .saturating_sub(lilypad_core::current_timestamp())
                    / 86_400;
                body = body.push(self.field(
                    t("field_password_expires"),
                    &tf("detail_expires_in_days", &[("days", &days)]),
                    None,
                    None,
                ));
            }
            body = body.push(self.field(
                t("field_modified"),
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
                        let sev = severity_color(issue.severity);
                        container(
                            row![
                                icons::icon(icons::TRIANGLE_EXCLAMATION, 13, sev),
                                hgap(8.0),
                                text(issue.localized_title().to_string())
                                    .size(sz(12))
                                    .color(Palette::TEXT_PRIMARY()),
                            ]
                            .align_y(Alignment::Center),
                        )
                        .padding([8, 10])
                        .width(Length::Fill)
                        .style(move |_| theme::banner(sev))
                        .into()
                    })
            })
            .unwrap_or_else(|| vgap(0.0).into());

        column![
            header,
            vgap(12.0),
            banner,
            vgap(4.0),
            scrollable(container(body).max_width(720.0 * colony_ui::font_scale()))
                .height(Length::Fill)
        ]
        .spacing(4)
        .height(Length::Fill)
        .into()
    }

    pub fn strength_bar(&self, pw: &str) -> Element<'static, Message> {
        let level = strength_level(pw);
        let color = theme::strength_color(level);
        let mut r = row![].spacing(4).width(Length::Fixed(200.0));
        for i in 0..5u8 {
            let c = if i < level {
                color
            } else {
                Palette::BG_CARD_HOVER()
            };
            r = r.push(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fixed(5.0))
                    .style(move |_| theme::filled(c, 2.0)),
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
        let d = self.density;
        let mut actions = row![].spacing(4);
        if let Some(open) = open {
            actions = actions.push(
                button(icons::icon(icons::EXTERNAL_LINK, 13, Palette::ACCENT()))
                    .on_press(open)
                    .style(theme::ghost_style(d)),
            );
        }
        if let Some(copy) = copy {
            actions = actions.push(
                button(icons::icon(icons::COPY, 13, Palette::TEXT_SECONDARY()))
                    .on_press(copy)
                    .style(theme::ghost_style(d)),
            );
        }
        self.field_row(
            label,
            text(value.to_string())
                .size(sz(14))
                .color(Palette::TEXT_PRIMARY())
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
        column![
            text(label.to_uppercase())
                .size(sz(10))
                .font(ui_font_medium())
                .color(Palette::TEXT_MUTED()),
            row![value, hfill(), actions].align_y(Alignment::Center),
        ]
        .spacing(3)
        .padding([9, 0])
        .into()
    }

    pub fn detail_edit(&self) -> Element<'_, Message> {
        let d = self.density;
        let typo = Typography::current();
        let title = if self.form.editing.is_some() {
            t("form_edit_title")
        } else {
            t("form_new_title")
        };
        let inp = |placeholder: &str, value: &str, field: FormField, secure: bool| {
            text_input(placeholder, value)
                .on_input(move |v| Message::FormChanged(field, v))
                .secure(secure)
                .size(sz(13))
                .padding(10)
                .style(theme::text_input_style(d))
        };
        let small = |s: &str| {
            text(s.to_uppercase())
                .size(sz(10))
                .font(ui_font_medium())
                .color(Palette::TEXT_MUTED())
        };

        let type_pick = widgets::dropdown(
            &typo,
            ALL_TYPES
                .iter()
                .map(|ty| (ty.clone(), type_label(ty).to_string())),
            self.form.entry_type.as_ref().unwrap_or(&EntryType::Login),
            Message::FormTypeSelected,
        );
        let expiry_pick = widgets::dropdown(
            &typo,
            EXPIRY_CHOICES
                .iter()
                .map(|(key, days)| (*days, t(key).to_string())),
            &self.form.expiry_days,
            Message::FormExpirySelected,
        );

        column![
            text(title)
                .size(sz(20))
                .font(ui_font_bold())
                .color(Palette::TEXT_PRIMARY()),
            vgap(6.0),
            inp(t("field_label"), &self.form.label, FormField::Label, false),
            row![
                column![small(t("form_type")), type_pick].spacing(3),
                hgap(10.0),
                column![small(t("field_password_expires")), expiry_pick].spacing(3),
            ]
            .align_y(Alignment::End),
            inp(
                t("field_username"),
                &self.form.username,
                FormField::Username,
                false
            ),
            inp(t("field_url"), &self.form.url, FormField::Url, false),
            inp(
                t("field_password"),
                &self.form.password,
                FormField::Password,
                true
            ),
            inp(t("field_notes"), &self.form.notes, FormField::Notes, false),
            inp(
                t("form_totp_placeholder"),
                &self.form.totp,
                FormField::Totp,
                false
            ),
            inp(
                t("form_tags_placeholder"),
                &self.form.tags,
                FormField::Tags,
                false
            ),
            inp(
                t("form_folder_placeholder"),
                &self.form.folder,
                FormField::Folder,
                false
            ),
            vgap(4.0),
            row![
                button(text(t("form_save")).size(sz(14)))
                    .on_press(Message::FormSave)
                    .style(theme::primary_style(d)),
                button(text(t("form_cancel")).size(sz(14)))
                    .on_press(Message::FormCancel)
                    .style(theme::ghost_style(d)),
            ]
            .spacing(8),
        ]
        .spacing(10)
        .into()
    }

    pub fn view_add_form(&self) -> Element<'_, Message> {
        self.detail_edit()
    }

    pub fn view_confirm(&self, label: &str) -> Element<'_, Message> {
        let d = self.density;
        let in_trash = self.trashed.iter().any(|e| e.label == label);
        let (title, body) = if in_trash {
            ("confirm_delete_title", "confirm_delete_body")
        } else {
            ("confirm_trash_title", "confirm_trash_body")
        };
        column![
            text(t(title))
                .size(sz(20))
                .font(ui_font_bold())
                .color(Palette::ERROR()),
            text(tf(body, &[("label", &label)]))
                .size(sz(14))
                .color(Palette::TEXT_SECONDARY()),
            vgap(8.0),
            row![
                button(text(t("confirm_delete")).size(sz(14)))
                    .on_press(Message::DeleteConfirmed)
                    .style(theme::danger_style(d)),
                button(text(t("form_cancel")).size(sz(14)))
                    .on_press(Message::DeleteCancelled)
                    .style(theme::ghost_style(d)),
            ]
            .spacing(8),
        ]
        .spacing(10)
        .into()
    }

    pub fn view_generator(&self) -> Element<'_, Message> {
        let d = self.density;
        let toggle = |label: &'static str, on: bool, set: CharSet| {
            let style = if on {
                theme::primary_style(d)
            } else {
                theme::secondary_style(d)
            };
            let ink = if on {
                colony_ui::contrast_on(Palette::ACCENT())
            } else {
                Palette::ACCENT()
            };
            button(
                row![
                    icons::icon(
                        if on {
                            icons::CHECKBOX_ON
                        } else {
                            icons::CHECKBOX_OFF
                        },
                        13,
                        ink
                    ),
                    text(label).size(sz(13)),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .on_press(Message::GenToggle(set))
            .style(style)
        };
        column![
            text(t("generator_title"))
                .size(sz(20))
                .font(ui_font_bold())
                .color(Palette::TEXT_PRIMARY()),
            text(self.gen_result.clone())
                .size(sz(16))
                .color(Palette::ACCENT())
                .font(ui_font_bold()),
            row![
                text(tf("generator_length", &[("length", &self.gen_length)]))
                    .size(sz(13))
                    .color(Palette::TEXT_SECONDARY()),
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
                button(text(t("generator_regenerate")).size(sz(13)))
                    .on_press(Message::GenRegenerate)
                    .style(theme::secondary_style(d)),
                button(text(t("generator_copy")).size(sz(13)))
                    .on_press(Message::GenUse)
                    .style(theme::primary_style(d)),
                hfill(),
                button(text(t("generator_close")).size(sz(13)))
                    .on_press(Message::CloseGenerator)
                    .style(theme::ghost_style(d)),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into()
    }
}

/// The colour of a health issue's severity.
pub fn severity_color(severity: lilypad_core::IssueSeverity) -> Color {
    match severity {
        lilypad_core::IssueSeverity::Critical => Palette::ERROR(),
        lilypad_core::IssueSeverity::Warning => Palette::WARNING(),
        lilypad_core::IssueSeverity::Info => Palette::ACCENT(),
    }
}
