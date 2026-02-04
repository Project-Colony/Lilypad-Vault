//! Welcome Screen View
//!
//! Displays the initial welcome/onboarding modal for new users.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::theme::{self, LilypadTheme};

/// Render the welcome modal
pub fn view(theme: LilypadTheme) -> Element<'static, Message> {
    let palette = theme.palette();

    let title = text("Welcome to Lilypad")
        .size(28)
        .color(palette.text_primary);

    let subtitle = text("A Colony Project")
        .size(16)
        .color(palette.primary);

    let description = text(
        "Lilypad is a free companion tool in the Colony ecosystem. \
         It's still in active development, so your feedback is essential \
         to help improve it over time.",
    )
    .size(14)
    .color(palette.text_secondary);

    let description2 = text(
        "You can follow the project and share feedback via the Colony \
         repository on GitHub.",
    )
    .size(14)
    .color(palette.text_secondary);

    let github_btn = button(
        text("Visit GitHub")
            .size(14)
            .color(palette.text_primary),
    )
    .padding([10, 20])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::secondary_button_hovered(theme),
        _ => theme::secondary_button(theme),
    })
    .on_press(Message::OpenExternalLink(
        "https://github.com/MotherSphere/Colony".to_string(),
    ));

    let continue_btn = button(
        text("Get Started")
            .size(14),
    )
    .padding([12, 32])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme),
        _ => theme::primary_button(theme),
    })
    .on_press(Message::AcknowledgeWelcome);

    let content = column![
        title,
        Space::with_height(4),
        subtitle,
        Space::with_height(24),
        description,
        Space::with_height(12),
        description2,
        Space::with_height(32),
        row![github_btn, Space::with_width(12), continue_btn]
            .align_y(Vertical::Center),
    ]
    .spacing(0)
    .padding(40)
    .width(Length::Fixed(480.0));

    let modal_content = container(content)
        .style(move |_| theme::modal_container(theme));

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
