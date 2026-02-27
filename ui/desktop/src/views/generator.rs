//! Password Generator View
//!
//! UI for generating secure passwords with customizable options.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, checkbox, column, container, row, scrollable, slider, text, Space};
use iced::{Element, Length};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::theme::{self, LilypadTheme, UiVariation};

/// Parameters for the password generator view.
pub struct GeneratorViewParams<'a> {
    pub theme: LilypadTheme,
    pub variation: UiVariation,
    pub generated_password: &'a str,
    pub generator_length: usize,
    pub generator_lowercase: bool,
    pub generator_uppercase: bool,
    pub generator_digits: bool,
    pub generator_symbols: bool,
    pub exclude_ambiguous: bool,
}

/// Render the password generator section
#[allow(clippy::too_many_arguments)]
pub fn view(params: GeneratorViewParams<'_>) -> Element<'static, Message> {
    let GeneratorViewParams {
        theme,
        variation: v,
        generated_password,
        generator_length,
        generator_lowercase,
        generator_uppercase,
        generator_digits,
        generator_symbols,
        exclude_ambiguous,
    } = params;
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
        .style(move |_| theme::elevated_container(theme, v))
    } else {
        container(
            row![
                text(generated_password_owned.clone())
                    .size(18)
                    .color(palette.text_primary),
                Space::with_width(Length::Fill),
                button(fonts::centered_icon(icons::COPY, 16.0))
                    .padding([8, 12])
                    .style(move |_theme, status| match status {
                        button::Status::Hovered => theme::icon_button_hovered(theme, v),
                        _ => theme::icon_button(theme, v),
                    })
                    .on_press(Message::CopyGeneratedPassword),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .padding(20)
        .style(move |_| theme::elevated_container(theme, v))
    };

    // Generate button
    let generate_btn = button(
        container(
            row![
                fonts::centered_icon(icons::DICE, 16.0),
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
        button::Status::Hovered => theme::primary_button_hovered(theme, v),
        _ => theme::primary_button(theme, v),
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
    // Map score to category with label, color, and visual fill percentage
    // The bar fill is tied to the category, not the raw score, for consistent visual feedback
    let (strength_label, strength_color, fill_percent) = match strength_score {
        0..=20 => ("Very Weak", palette.danger, 20u16),
        21..=40 => ("Weak", iced::Color::from_rgb8(249, 115, 22), 40u16),
        41..=60 => ("Medium", palette.warning, 60u16),
        61..=80 => ("Strong", iced::Color::from_rgb8(132, 204, 22), 80u16),
        _ => ("Very Strong", palette.success, 100u16),
    };

    // Use category-based fill for consistent visual feedback
    let fill_portion = fill_percent;
    let empty_portion = 100u16.saturating_sub(fill_percent);

    // Build the bar - only include empty portion if there's any empty space
    let bar_content: Element<'static, Message> = if empty_portion == 0 {
        // Full bar - no empty portion needed
        container(Space::new(Length::Fill, Length::Fixed(8.0)))
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(strength_color)),
                ..Default::default()
            })
            .into()
    } else {
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
        .into()
    };

    let strength_section: Element<'static, Message> = column![
        row![
            text("Strength").size(14).color(palette.text_secondary),
            Space::with_width(Length::Fill),
            text(strength_label).size(14).color(strength_color),
        ],
        Space::with_height(8),
        container(bar_content)
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
        scrollable(
            container(content)
                .padding(32)
                .style(move |_| theme::card_container(theme, v)),
        )
        .height(Length::Fill)
        .style(move |_theme, _status| theme::scrollable_style(theme, v)),
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
