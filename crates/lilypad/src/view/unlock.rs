//! The unlock and create-vault cards.

use super::{boxed_btn, hfill, vgap, BtnStyle};
use crate::fonts;
use crate::message::Message;
use crate::state::LilypadApp;
use crate::theme::{self};
use iced::widget::{button, column, row, scrollable, text, text_input};
use iced::{Element, Length};

impl LilypadApp {
    pub fn view_unlock(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let vault_buttons =
            self.vaults
                .iter()
                .fold(column![].spacing(6).width(Length::Fill), |col, name| {
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
                });
        let unlock_btn = if self.unlocking {
            button(text("Unlocking…").size(15))
                .padding(12)
                .width(Length::Fill)
        } else {
            button(text("Unlock").size(15))
                .on_press(Message::UnlockPressed)
                .padding(12)
                .width(Length::Fill)
                .style(theme::primary_style(t, d))
        };
        let card = column![
            text("🪷 Lilypad")
                .size(30)
                .font(fonts::FONT_BOLD)
                .color(p.primary),
            text("Choose a vault and enter your master password")
                .size(13)
                .color(p.text_muted),
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

    pub fn view_create(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let card = column![
            text("Create a vault")
                .size(26)
                .font(fonts::FONT_BOLD)
                .color(p.primary),
            text("Pick a name and a strong master password")
                .size(13)
                .color(p.text_muted),
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
            button(
                text(if self.unlocking {
                    "Creating…"
                } else {
                    "Create vault"
                })
                .size(15)
            )
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
}
