//! Shared view helpers
//!
//! Reusable UI components used across multiple views.

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::theme::{self, LilypadTheme, UiVariation};

/// Horizontal separator line using `surface_variant` color.
pub fn separator(theme: LilypadTheme, _v: UiVariation) -> Element<'static, Message> {
    let palette = theme.palette();
    container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(palette.surface_variant)),
            ..Default::default()
        })
        .into()
}

/// Horizontal separator using `border` color, with vertical padding.
pub fn padded_separator(theme: LilypadTheme, _v: UiVariation) -> Element<'static, Message> {
    let palette = theme.palette();
    container(
        container(Space::new().width(Length::Fill).height(Length::Fixed(1.0))).style(move |_| {
            container::Style {
                background: Some(iced::Background::Color(palette.border)),
                ..Default::default()
            }
        }),
    )
    .padding([8, 0])
    .into()
}

/// Labeled text input field with consistent styling.
pub fn labeled_input<F>(
    theme: LilypadTheme,
    v: UiVariation,
    label: &'static str,
    placeholder: &'static str,
    value: String,
    on_change: F,
) -> Element<'static, Message>
where
    F: 'static + Fn(String) -> Message,
{
    let palette = theme.palette();

    column![
        text(label).size(13).color(palette.text_secondary),
        Space::new().height(6),
        text_input(placeholder, &value)
            .padding(12)
            .size(14)
            .on_input(on_change)
            .style(theme::text_input_style_closure(theme, v)),
    ]
    .into()
}

/// Modal header row: title text + spacer + close button.
pub fn modal_header(
    theme: LilypadTheme,
    v: UiVariation,
    title_text: &str,
    close_msg: Message,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let title = text(title_text.to_string())
        .size(20)
        .color(palette.text_primary);
    let close_btn = button(fonts::centered_icon_colored(
        icons::CLOSE,
        16.0,
        palette.text_muted,
    ))
    .padding([8, 12])
    .style(theme::icon_style(theme, v))
    .on_press(close_msg);

    row![title, Space::new().width(Length::Fill), close_btn,]
        .align_y(Vertical::Center)
        .into()
}

/// Modal header row with an icon prefix.
pub fn modal_header_with_icon(
    theme: LilypadTheme,
    v: UiVariation,
    icon: &'static str,
    icon_size: f32,
    title_text: &str,
    close_msg: Message,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let icon_el = fonts::centered_icon_colored(icon, icon_size, palette.primary);
    let title = text(title_text.to_string())
        .size(20)
        .color(palette.text_primary);
    let close_btn = button(fonts::centered_icon_colored(
        icons::CLOSE,
        16.0,
        palette.text_muted,
    ))
    .padding([8, 12])
    .style(theme::icon_style(theme, v))
    .on_press(close_msg);

    row![
        icon_el,
        Space::new().width(12),
        title,
        Space::new().width(Length::Fill),
        close_btn,
    ]
    .align_y(Vertical::Center)
    .into()
}
