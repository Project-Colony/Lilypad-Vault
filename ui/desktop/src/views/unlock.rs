//! Unlock Screen View
//!
//! The login screen where users enter their master password.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::state::LockoutState;
use crate::theme::{self, LilypadTheme};

/// Render the unlock screen
pub fn view(
    theme: LilypadTheme,
    master_password: &str,
    lockout_state: &LockoutState,
    error_message: Option<&str>,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let master_password_owned = master_password.to_string();
    let error_message_owned = error_message.map(|s| s.to_string());

    // Logo/Brand
    let logo = text("🌸")
        .size(64)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    let title = text("Lilypad")
        .size(36)
        .color(palette.text_primary)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    let subtitle = text("Password Manager")
        .size(16)
        .color(palette.text_secondary)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    // Check if locked out
    let is_locked = lockout_state.is_locked_out();
    let remaining = lockout_state.remaining_secs();
    let failed_attempts = lockout_state.failed_attempts;

    let content: iced::widget::Column<'static, Message> = if is_locked {
        // Lockout message
        let lockout_title = text("Too Many Attempts")
            .size(20)
            .color(palette.danger);

        let lockout_msg = text(format!(
            "Please wait {} seconds before trying again.",
            remaining
        ))
        .size(14)
        .color(palette.text_secondary);

        let attempts_info = text(format!(
            "{} failed attempts",
            failed_attempts
        ))
        .size(12)
        .color(palette.text_muted);

        column![
            logo,
            Space::with_height(16),
            title,
            Space::with_height(4),
            subtitle,
            Space::with_height(48),
            lockout_title,
            Space::with_height(8),
            lockout_msg,
            Space::with_height(8),
            attempts_info,
        ]
        .align_x(Horizontal::Center)
        .spacing(0)
        .width(Length::Fixed(360.0))
    } else {
        // Normal login form
        let password_label = text("Master Password")
            .size(14)
            .color(palette.text_secondary);

        let is_empty = master_password_owned.is_empty();
        let password_input = text_input("Enter your master password...", &master_password_owned)
            .id(text_input::Id::new("master_password"))
            .padding(14)
            .size(16)
            .secure(true)
            .on_input(Message::MasterPasswordChanged)
            .on_submit(Message::UnlockVault)
            .style(move |_theme, status| match status {
                text_input::Status::Focused => theme::text_input_focused(theme),
                _ => theme::text_input_style(theme),
            });

        let unlock_btn = button(
            container(
                text("Unlock Vault")
                    .size(15),
            )
            .width(Length::Fill)
            .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([14, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::primary_button_hovered(theme),
            button::Status::Disabled => {
                let mut style = theme::primary_button(theme);
                style.background = Some(iced::Background::Color(palette.border));
                style
            }
            _ => theme::primary_button(theme),
        })
        .on_press_maybe(if is_empty {
            None
        } else {
            Some(Message::UnlockVault)
        });

        let mut form = column![
            logo,
            Space::with_height(16),
            title,
            Space::with_height(4),
            subtitle,
            Space::with_height(48),
            password_label,
            Space::with_height(8),
            password_input,
            Space::with_height(24),
            unlock_btn,
        ]
        .align_x(Horizontal::Center)
        .spacing(0)
        .width(Length::Fixed(360.0));

        // Add error message if present
        if let Some(error) = error_message_owned {
            let error_text = text(error)
                .size(13)
                .color(palette.danger);

            form = form.push(Space::with_height(16));
            form = form.push(error_text);
        }

        // Add attempts warning if there have been failures
        if failed_attempts > 0 && failed_attempts < 5 {
            let attempts_remaining = 5 - failed_attempts;
            let warning = text(format!(
                "{} attempt{} remaining before lockout",
                attempts_remaining,
                if attempts_remaining == 1 { "" } else { "s" }
            ))
            .size(12)
            .color(palette.warning);

            form = form.push(Space::with_height(8));
            form = form.push(warning);
        }

        form
    };

    // Footer
    let footer = row![
        text("New here?")
            .size(13)
            .color(palette.text_muted),
        Space::with_width(4),
        button(
            text("Create a vault")
                .size(13)
                .color(palette.primary),
        )
        .padding(0)
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: palette.primary,
            ..Default::default()
        })
        .on_press(Message::ShowNewVaultModal),
    ]
    .align_y(Vertical::Center);

    let full_content = column![
        Space::with_height(Length::FillPortion(1)),
        content,
        Space::with_height(32),
        footer,
        Space::with_height(Length::FillPortion(1)),
    ]
    .align_x(Horizontal::Center);

    container(full_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| theme::app_container(theme))
        .into()
}
