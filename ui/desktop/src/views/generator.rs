//! Password Generator View
//!
//! UI for generating secure passwords with customizable options.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, checkbox, column, container, row, slider, text, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::theme::{self, LilypadTheme};

/// Render the password generator section
pub fn view(
    theme: LilypadTheme,
    generated_password: &str,
    generator_length: usize,
    generator_lowercase: bool,
    generator_uppercase: bool,
    generator_digits: bool,
    generator_symbols: bool,
    exclude_ambiguous: bool,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let generated_password_owned = generated_password.to_string();

    let title = text("Password Generator")
        .size(24)
        .color(palette.text_primary);

    let subtitle = text("Generate strong, unique passwords for your accounts")
        .size(14)
        .color(palette.text_secondary);

    // Generated password display
    let password_display = if generated_password_owned.is_empty() {
        container(
            text("Click 'Generate' to create a password")
                .size(14)
                .color(palette.text_muted),
        )
        .width(Length::Fill)
        .padding(20)
        .style(move |_| theme::elevated_container(theme))
    } else {
        container(
            row![
                text(generated_password_owned.clone())
                    .size(18)
                    .color(palette.text_primary),
                Space::with_width(Length::Fill),
                button(text("📋").size(16))
                    .padding([8, 12])
                    .style(move |_theme, status| match status {
                        button::Status::Hovered => theme::icon_button_hovered(theme),
                        _ => theme::icon_button(theme),
                    })
                    .on_press(Message::CopyGeneratedPassword),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .padding(20)
        .style(move |_| theme::elevated_container(theme))
    };

    // Generate button
    let generate_btn = button(
        container(
            row![
                text("🎲").size(16),
                Space::with_width(8),
                text("Generate Password").size(15),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([14, 24])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme),
        _ => theme::primary_button(theme),
    })
    .on_press(Message::GeneratePassword);

    // Length slider
    let length_label = row![
        text("Length").size(14).color(palette.text_secondary),
        Space::with_width(Length::Fill),
        text(format!("{} characters", generator_length))
            .size(14)
            .color(palette.text_primary),
    ];

    let length_slider = slider(8..=64, generator_length as u32, |v| {
        Message::GeneratorLengthChanged(v as usize)
    })
    .width(Length::Fill);

    // Character options
    let options_title = text("Character Types")
        .size(14)
        .color(palette.text_secondary);

    let lowercase_check = checkbox("Lowercase (a-z)", generator_lowercase)
        .on_toggle(Message::ToggleLowercase)
        .text_size(14)
        .spacing(10);

    let uppercase_check = checkbox("Uppercase (A-Z)", generator_uppercase)
        .on_toggle(Message::ToggleUppercase)
        .text_size(14)
        .spacing(10);

    let digits_check = checkbox("Digits (0-9)", generator_digits)
        .on_toggle(Message::ToggleDigits)
        .text_size(14)
        .spacing(10);

    let symbols_check = checkbox("Symbols (!@#$%...)", generator_symbols)
        .on_toggle(Message::ToggleSymbols)
        .text_size(14)
        .spacing(10);

    let ambiguous_check = checkbox("Exclude ambiguous (0O, 1lI)", exclude_ambiguous)
        .on_toggle(Message::ToggleExcludeAmbiguous)
        .text_size(14)
        .spacing(10);

    // Strength indicator
    let options_count = [
        generator_lowercase,
        generator_uppercase,
        generator_digits,
        generator_symbols,
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    let strength_score = calculate_strength(generator_length, options_count);
    let (strength_label, strength_color) = match strength_score {
        0..=20 => ("Very Weak", palette.danger),
        21..=40 => ("Weak", iced::Color::from_rgb8(249, 115, 22)),
        41..=60 => ("Medium", palette.warning),
        61..=80 => ("Strong", iced::Color::from_rgb8(132, 204, 22)),
        _ => ("Very Strong", palette.success),
    };

    // Use a simple row with colored portions for the strength bar
    let fill_portion = (strength_score as u16).max(1);
    let empty_portion = (100u16.saturating_sub(strength_score as u16)).max(1);

    let strength_section: Element<'static, Message> = column![
        row![
            text("Strength").size(14).color(palette.text_secondary),
            Space::with_width(Length::Fill),
            text(strength_label).size(14).color(strength_color),
        ],
        Space::with_height(8),
        container(
            row![
                container(Space::new(Length::FillPortion(fill_portion), Length::Fixed(8.0)))
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(strength_color)),
                        ..Default::default()
                    }),
                container(Space::new(Length::FillPortion(empty_portion), Length::Fixed(8.0)))
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(palette.surface_variant)),
                        ..Default::default()
                    }),
            ]
        )
        .width(Length::Fill)
        .style(move |_| container::Style {
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }),
    ]
    .into();

    // Tips section
    let tips_title = text("Tips for Strong Passwords")
        .size(14)
        .color(palette.text_secondary);

    let tips = column![
        tip_item(&palette, "Use at least 16 characters"),
        tip_item(&palette, "Include all character types"),
        tip_item(&palette, "Avoid dictionary words"),
        tip_item(&palette, "Use unique passwords for each account"),
    ]
    .spacing(8);

    // Main content layout
    let content = column![
        title,
        Space::with_height(4),
        subtitle,
        Space::with_height(32),
        password_display,
        Space::with_height(16),
        generate_btn,
        Space::with_height(32),
        length_label,
        Space::with_height(8),
        length_slider,
        Space::with_height(24),
        options_title,
        Space::with_height(12),
        lowercase_check,
        Space::with_height(8),
        uppercase_check,
        Space::with_height(8),
        digits_check,
        Space::with_height(8),
        symbols_check,
        Space::with_height(8),
        ambiguous_check,
        Space::with_height(24),
        strength_section,
        Space::with_height(32),
        tips_title,
        Space::with_height(12),
        tips,
    ]
    .width(Length::Fixed(480.0));

    container(
        container(content)
            .padding(32)
            .style(move |_| theme::card_container(theme)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(24)
    .align_x(Horizontal::Center)
    .into()
}

/// Calculate password strength score (0-100)
fn calculate_strength(length: usize, options_count: usize) -> u8 {
    let length_score = ((length as f32 / 32.0) * 50.0).min(50.0) as u8;
    let variety_score = ((options_count as f32 / 4.0) * 50.0) as u8;
    length_score + variety_score
}

/// Render a tip item
fn tip_item<'a>(
    palette: &crate::theme::LilypadPalette,
    tip: &'static str,
) -> Element<'a, Message> {
    row![
        text("•").size(14).color(palette.primary),
        Space::with_width(8),
        text(tip).size(13).color(palette.text_muted),
    ]
    .align_y(Vertical::Center)
    .into()
}
