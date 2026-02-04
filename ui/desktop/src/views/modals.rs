//! Modal Dialogs
//!
//! Reusable modal dialog components.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::theme::{self, LilypadTheme};

/// Render the settings modal
pub fn settings_modal<'a>(theme: LilypadTheme) -> Element<'a, Message> {
    let current_theme = theme;
    let palette = theme.palette();

    let title = text("Settings")
        .size(20)
        .color(palette.text_primary);

    let close_btn = button(text("✕").size(16).color(palette.text_muted))
        .padding([8, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme),
            _ => theme::icon_button(theme),
        })
        .on_press(Message::HideSettings);

    let header = row![title, Space::with_width(Length::Fill), close_btn,].align_y(Vertical::Center);

    // Theme selection
    let theme_label = text("Theme")
        .size(14)
        .color(palette.text_secondary);

    let theme_buttons: Vec<Element<'a, Message>> = LilypadTheme::ALL
        .iter()
        .map(|t| {
            let is_active = *t == current_theme;
            button(
                text(t.name())
                    .size(14)
                    .color(if is_active {
                        palette.primary
                    } else {
                        palette.text_primary
                    }),
            )
            .padding([10, 16])
            .style(move |_theme, status| {
                if is_active {
                    let mut style = theme::secondary_button(theme);
                    style.background = Some(iced::Background::Color(palette.hover));
                    style
                } else {
                    match status {
                        button::Status::Hovered => theme::ghost_button_hovered(theme),
                        _ => theme::ghost_button(theme),
                    }
                }
            })
            .on_press(Message::ChangeTheme(*t))
            .into()
        })
        .collect();

    let theme_row = row(theme_buttons).spacing(8);

    let content = column![
        header,
        Space::with_height(24),
        theme_label,
        Space::with_height(8),
        theme_row,
        Space::with_height(24),
        button(
            container(text("Close").size(14))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::secondary_button_hovered(theme),
            _ => theme::secondary_button(theme),
        })
        .on_press(Message::HideSettings),
    ]
    .padding(24)
    .width(Length::Fixed(400.0));

    let modal_content = container(content).style(move |_| theme::modal_container(theme));

    let centered = container(modal_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

    container(centered)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| theme::modal_overlay(theme))
        .into()
}

/// Render the new vault modal
pub fn new_vault_modal<'a>(theme: LilypadTheme, vault_name: &str) -> Element<'a, Message> {
    let palette = theme.palette();

    let title = text("Create New Vault")
        .size(20)
        .color(palette.text_primary);

    let close_btn = button(text("✕").size(16).color(palette.text_muted))
        .padding([8, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme),
            _ => theme::icon_button(theme),
        })
        .on_press(Message::HideNewVaultModal);

    let header = row![title, Space::with_width(Length::Fill), close_btn,].align_y(Vertical::Center);

    let name_label = text("Vault Name")
        .size(14)
        .color(palette.text_secondary);

    let name_input = text_input("e.g., Personal, Work", vault_name)
        .padding(14)
        .size(14)
        .on_input(Message::NewVaultNameChanged)
        .on_submit(Message::CreateVault)
        .style(move |_theme, status| match status {
            text_input::Status::Focused => theme::text_input_focused(theme),
            _ => theme::text_input_style(theme),
        });

    let cancel_btn = button(text("Cancel").size(14).color(palette.text_secondary))
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::ghost_button_hovered(theme),
            _ => theme::ghost_button(theme),
        })
        .on_press(Message::HideNewVaultModal);

    let create_btn = button(text("Create Vault").size(14))
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::primary_button_hovered(theme),
            button::Status::Disabled => {
                let mut style = theme::primary_button(theme);
                style.background = Some(iced::Background::Color(palette.border));
                style
            }
            _ => theme::primary_button(theme),
        })
        .on_press_maybe(if vault_name.trim().is_empty() {
            None
        } else {
            Some(Message::CreateVault)
        });

    let actions = row![Space::with_width(Length::Fill), cancel_btn, Space::with_width(12), create_btn,]
        .align_y(Vertical::Center);

    let content = column![
        header,
        Space::with_height(24),
        name_label,
        Space::with_height(8),
        name_input,
        Space::with_height(24),
        actions,
    ]
    .padding(24)
    .width(Length::Fixed(420.0));

    let modal_content = container(content).style(move |_| theme::modal_container(theme));

    let centered = container(modal_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

    container(centered)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| theme::modal_overlay(theme))
        .into()
}

/// Render the delete confirmation modal
pub fn delete_confirm_modal<'a>(theme: LilypadTheme, entry_title: &str) -> Element<'a, Message> {
    let palette = theme.palette();

    let icon = text("⚠").size(48).color(palette.warning);

    let title = text("Delete Entry?")
        .size(20)
        .color(palette.text_primary);

    let message = text(format!(
        "Are you sure you want to delete \"{}\"? This action cannot be undone.",
        entry_title
    ))
    .size(14)
    .color(palette.text_secondary);

    let cancel_btn = button(
        container(text("Cancel").size(14).color(palette.text_primary))
            .width(Length::Fill)
            .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 24])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::ghost_button_hovered(theme),
        _ => theme::ghost_button(theme),
    })
    .on_press(Message::CancelDelete);

    let delete_btn = button(
        container(text("Delete").size(14))
            .width(Length::Fill)
            .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 24])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::danger_button_hovered(theme),
        _ => theme::danger_button(theme),
    })
    .on_press(Message::ConfirmDelete);

    let actions = row![cancel_btn, Space::with_width(12), delete_btn,];

    let content = column![
        icon,
        Space::with_height(16),
        title,
        Space::with_height(12),
        message,
        Space::with_height(24),
        actions,
    ]
    .align_x(Horizontal::Center)
    .padding(32)
    .width(Length::Fixed(400.0));

    let modal_content = container(content).style(move |_| theme::modal_container(theme));

    let centered = container(modal_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

    container(centered)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| theme::modal_overlay(theme))
        .into()
}

/// Render the re-authentication modal
pub fn reauth_modal<'a>(theme: LilypadTheme, reauth_password: &str) -> Element<'a, Message> {
    let palette = theme.palette();

    let title = text("Confirm Your Identity")
        .size(20)
        .color(palette.text_primary);

    let message = text("Enter your master password to copy this password.")
        .size(14)
        .color(palette.text_secondary);

    let password_input = text_input("Master password", reauth_password)
        .padding(14)
        .size(14)
        .secure(true)
        .on_input(Message::ReauthPasswordChanged)
        .on_submit(Message::ConfirmReauth)
        .style(move |_theme, status| match status {
            text_input::Status::Focused => theme::text_input_focused(theme),
            _ => theme::text_input_style(theme),
        });

    let cancel_btn = button(text("Cancel").size(14).color(palette.text_secondary))
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::ghost_button_hovered(theme),
            _ => theme::ghost_button(theme),
        })
        .on_press(Message::CancelReauth);

    let confirm_btn = button(text("Confirm").size(14))
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::primary_button_hovered(theme),
            button::Status::Disabled => {
                let mut style = theme::primary_button(theme);
                style.background = Some(iced::Background::Color(palette.border));
                style
            }
            _ => theme::primary_button(theme),
        })
        .on_press_maybe(if reauth_password.is_empty() {
            None
        } else {
            Some(Message::ConfirmReauth)
        });

    let actions = row![Space::with_width(Length::Fill), cancel_btn, Space::with_width(12), confirm_btn,]
        .align_y(Vertical::Center);

    let content = column![
        text("🔐").size(48),
        Space::with_height(16),
        title,
        Space::with_height(8),
        message,
        Space::with_height(24),
        password_input,
        Space::with_height(24),
        actions,
    ]
    .align_x(Horizontal::Center)
    .padding(32)
    .width(Length::Fixed(400.0));

    let modal_content = container(content).style(move |_| theme::modal_container(theme));

    let centered = container(modal_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

    container(centered)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| theme::modal_overlay(theme))
        .into()
}
