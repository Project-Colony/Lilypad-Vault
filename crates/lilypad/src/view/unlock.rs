//! The unlock and create-vault cards. The vaults themselves are listed in
//! the sidebar.

use super::{hfill, vgap};
use crate::message::Message;
use crate::state::LilypadApp;
use crate::theme;
use colony_ui::theme::Palette;
use colony_ui::{sz, ui_font_bold};
use iced::widget::{button, column, row, text, text_input};
use iced::{Element, Length};
use lilypad_core::i18n::{t, tf};

impl LilypadApp {
    pub fn view_unlock(&self) -> Element<'_, Message> {
        let d = self.density;
        let vault = self.selected_vault.clone().unwrap_or_default();
        let unlock_btn = if self.unlocking {
            button(text(t("unlock_busy")).size(sz(15)))
                .padding(12)
                .width(Length::Fill)
        } else {
            button(text(t("unlock_button")).size(sz(15)))
                .on_press(Message::UnlockPressed)
                .padding(12)
                .width(Length::Fill)
                .style(theme::primary_style(d))
        };
        let card = column![
            text(tf("unlock_title", &[("vault", &vault)]))
                .size(sz(26))
                .font(ui_font_bold())
                .color(Palette::TEXT_PRIMARY()),
            text(t("unlock_subtitle"))
                .size(sz(13))
                .color(Palette::TEXT_MUTED()),
            vgap(8.0),
            text_input(t("unlock_password_placeholder"), &self.password)
                .on_input(Message::PasswordChanged)
                .on_submit(Message::UnlockPressed)
                .secure(true)
                .size(sz(14))
                .padding(12)
                .style(theme::text_input_style(d)),
            unlock_btn,
            row![
                button(text(t("unlock_create_link")).size(sz(13)))
                    .on_press(Message::ShowCreate)
                    .style(theme::ghost_style(d)),
                hfill(),
            ],
        ]
        .spacing(12);
        self.centered(card)
    }

    pub fn view_create(&self) -> Element<'_, Message> {
        let d = self.density;
        let mut card = column![
            text(t("create_title"))
                .size(sz(26))
                .font(ui_font_bold())
                .color(Palette::TEXT_PRIMARY()),
            text(t("create_subtitle"))
                .size(sz(13))
                .color(Palette::TEXT_MUTED()),
            vgap(8.0),
            text_input(t("create_name_placeholder"), &self.new_name)
                .on_input(Message::NewNameChanged)
                .size(sz(14))
                .padding(12)
                .style(theme::text_input_style(d)),
            text_input(t("unlock_password_placeholder"), &self.new_password)
                .on_input(Message::NewPasswordChanged)
                .on_submit(Message::CreatePressed)
                .secure(true)
                .size(sz(14))
                .padding(12)
                .style(theme::text_input_style(d)),
            button(
                text(if self.unlocking {
                    t("create_busy")
                } else {
                    t("create_button")
                })
                .size(sz(15))
            )
            .on_press(Message::CreatePressed)
            .padding(12)
            .width(Length::Fill)
            .style(theme::primary_style(d)),
        ]
        .spacing(12);
        if !self.vaults.is_empty() {
            card = card.push(
                button(text(t("create_back")).size(sz(13)))
                    .on_press(Message::ShowUnlock)
                    .style(theme::ghost_style(d)),
            );
        }
        self.centered(card)
    }
}
