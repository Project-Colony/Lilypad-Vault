//! Unlock Screen View
//!
//! Dual-mode screen: Create Vault (when no vault exists) or Unlock Vault (when one does).

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::state::{LockoutState, UnlockMode};
use crate::theme::{self, LilypadTheme};

/// Render the unlock/create screen
pub fn view(
    theme: LilypadTheme,
    master_password: &str,
    confirm_password: &str,
    lockout_state: &LockoutState,
    error_message: Option<&str>,
    unlock_mode: UnlockMode,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let master_password_owned = master_password.to_string();
    let confirm_password_owned = confirm_password.to_string();
    let error_message_owned = error_message.map(|s| s.to_string());

    // Logo/Brand
    let logo: Element<'static, Message> = container(fonts::centered_icon_colored(icons::SHIELD, 56.0, palette.primary))
        .width(Length::Fill)
        .align_x(Horizontal::Center)
        .into();

    let title = text("Lilypad")
        .size(36)
        .font(fonts::FONT_BOLD)
        .color(palette.text_primary)
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
            .font(fonts::FONT_SEMIBOLD)
            .color(palette.danger);

        let lockout_msg = text(format!(
            "Please wait {} seconds before trying again.",
            remaining
        ))
        .size(14)
        .font(fonts::FONT_REGULAR)
        .color(palette.text_secondary);

        let attempts_info = text(format!("{} failed attempts", failed_attempts))
            .size(12)
            .font(fonts::FONT_REGULAR)
            .color(palette.text_muted);

        column![
            logo,
            Space::with_height(16),
            title,
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
        match unlock_mode {
            UnlockMode::Create => build_create_form(
                theme,
                palette,
                logo,
                title,
                &master_password_owned,
                &confirm_password_owned,
                error_message_owned,
            ),
            UnlockMode::Unlock => build_unlock_form(
                theme,
                palette,
                logo,
                title,
                &master_password_owned,
                error_message_owned,
                failed_attempts,
            ),
        }
    };

    // Footer (only in Unlock mode)
    let footer: iced::widget::Column<'static, Message> = match unlock_mode {
        UnlockMode::Unlock if !is_locked => {
            column![row![
                text("New here?")
                    .size(13)
                    .font(fonts::FONT_REGULAR)
                    .color(palette.text_muted),
                Space::with_width(4),
                button(
                    text("Create a vault")
                        .size(13)
                        .font(fonts::FONT_MEDIUM)
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
            .align_y(Vertical::Center)]
        }
        _ => column![],
    };

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
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| theme::app_container(theme))
        .into()
}

/// Build the "Create Your Vault" form.
fn build_create_form(
    theme: LilypadTheme,
    palette: crate::theme::LilypadPalette,
    logo: Element<'static, Message>,
    title: iced::widget::Text<'static>,
    master_password: &str,
    confirm_password: &str,
    error_message: Option<String>,
) -> iced::widget::Column<'static, Message> {
    let subtitle = text("Create Your Vault")
        .size(16)
        .font(fonts::FONT_LIGHT)
        .color(palette.text_secondary)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    let password_label = text("Master Password")
        .size(14)
        .font(fonts::FONT_MEDIUM)
        .color(palette.text_secondary);

    let password_input = text_input("Choose a strong password...", master_password)
        .id(text_input::Id::new("master_password"))
        .padding(14)
        .size(16)
        .font(fonts::FONT_REGULAR)
        .secure(true)
        .on_input(Message::MasterPasswordChanged)
        .style(move |_theme, status| match status {
            text_input::Status::Focused => theme::text_input_focused(theme),
            _ => theme::text_input_style(theme),
        });

    let confirm_label = text("Confirm Password")
        .size(14)
        .font(fonts::FONT_MEDIUM)
        .color(palette.text_secondary);

    let confirm_input = text_input("Re-enter your password...", confirm_password)
        .id(text_input::Id::new("confirm_password"))
        .padding(14)
        .size(16)
        .font(fonts::FONT_REGULAR)
        .secure(true)
        .on_input(Message::ConfirmPasswordChanged)
        .on_submit(Message::CreateVaultWithPassword)
        .style(move |_theme, status| match status {
            text_input::Status::Focused => theme::text_input_focused(theme),
            _ => theme::text_input_style(theme),
        });

    // Password strength indicator
    let strength_text = if master_password.is_empty() {
        None
    } else {
        let strength = lilypad_common::validation::validate_password_strength(master_password);
        let (label, color) = match strength {
            s if s.score() >= 80 => ("Strong", palette.success),
            s if s.score() >= 60 => ("Good", palette.primary),
            s if s.score() >= 40 => ("Fair", palette.warning),
            _ => ("Weak", palette.danger),
        };
        Some(
            text(label)
                .size(12)
                .font(fonts::FONT_MEDIUM)
                .color(color),
        )
    };

    let can_create =
        !master_password.is_empty() && !confirm_password.is_empty() && master_password == confirm_password;

    let create_btn = button(
        container(
            text("Create Vault")
                .size(15)
                .font(fonts::FONT_SEMIBOLD),
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
    .on_press_maybe(if can_create {
        Some(Message::CreateVaultWithPassword)
    } else {
        None
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
    ]
    .align_x(Horizontal::Center)
    .spacing(0)
    .width(Length::Fixed(360.0));

    // Strength indicator
    if let Some(strength) = strength_text {
        form = form.push(Space::with_height(4));
        form = form.push(strength);
    }

    form = form.push(Space::with_height(16));
    form = form.push(confirm_label);
    form = form.push(Space::with_height(8));
    form = form.push(confirm_input);
    form = form.push(Space::with_height(24));
    form = form.push(create_btn);

    // Error message
    if let Some(error) = error_message {
        let error_text = text(error)
            .size(13)
            .font(fonts::FONT_REGULAR)
            .color(palette.danger);
        form = form.push(Space::with_height(16));
        form = form.push(error_text);
    }

    form
}

/// Build the "Unlock Vault" form.
fn build_unlock_form(
    theme: LilypadTheme,
    palette: crate::theme::LilypadPalette,
    logo: Element<'static, Message>,
    title: iced::widget::Text<'static>,
    master_password: &str,
    error_message: Option<String>,
    failed_attempts: u32,
) -> iced::widget::Column<'static, Message> {
    let subtitle = text("Password Manager")
        .size(16)
        .font(fonts::FONT_LIGHT)
        .color(palette.text_secondary)
        .width(Length::Fill)
        .align_x(Horizontal::Center);

    let password_label = text("Master Password")
        .size(14)
        .font(fonts::FONT_MEDIUM)
        .color(palette.text_secondary);

    let is_empty = master_password.is_empty();
    let password_input = text_input("Enter your master password...", master_password)
        .id(text_input::Id::new("master_password"))
        .padding(14)
        .size(16)
        .font(fonts::FONT_REGULAR)
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
                .size(15)
                .font(fonts::FONT_SEMIBOLD),
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

    // Error message
    if let Some(error) = error_message {
        let error_text = text(error)
            .size(13)
            .font(fonts::FONT_REGULAR)
            .color(palette.danger);
        form = form.push(Space::with_height(16));
        form = form.push(error_text);
    }

    // Attempts warning
    if failed_attempts > 0 && failed_attempts < 5 {
        let attempts_remaining = 5 - failed_attempts;
        let warning = text(format!(
            "{} attempt{} remaining before lockout",
            attempts_remaining,
            if attempts_remaining == 1 { "" } else { "s" }
        ))
        .size(12)
        .font(fonts::FONT_REGULAR)
        .color(palette.warning);

        form = form.push(Space::with_height(8));
        form = form.push(warning);
    }

    form
}
