//! Header View
//!
//! The top bar containing vault selector, search, and action buttons.

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::theme::{self, LilypadTheme};

/// Render the header bar
pub fn view(
    theme: LilypadTheme,
    active_vault: &str,
    available_vaults: &[String],
    search_query: &str,
    show_vault_selector: bool,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let active_vault_owned = active_vault.to_string();
    let search_query_owned = search_query.to_string();

    // Vault selector button
    let vault_btn = button(
        row![
            text("🔐").size(16),
            Space::with_width(8),
            text(active_vault_owned.clone()).size(14).color(palette.text_primary),
            Space::with_width(8),
            text("▼").size(10).color(palette.text_muted),
        ]
        .align_y(Vertical::Center),
    )
    .padding([8, 12])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::ghost_button_hovered(theme),
        _ => theme::ghost_button(theme),
    })
    .on_press(if show_vault_selector {
        Message::HideVaultSelector
    } else {
        Message::ShowVaultSelector
    });

    // Vault selector dropdown (if visible)
    let vault_dropdown: Option<Element<'static, Message>> = if show_vault_selector {
        let active_vault_for_dropdown = active_vault_owned.clone();
        let mut items: Vec<Element<'static, Message>> = available_vaults
            .iter()
            .map(|name| {
                let is_active = name == &active_vault_for_dropdown;
                let name_owned = name.clone();
                button(
                    row![
                        text(if is_active { "●" } else { "" })
                            .size(8)
                            .color(palette.primary)
                            .width(Length::Fixed(16.0)),
                        text(name.clone()).size(14).color(palette.text_primary),
                    ]
                    .align_y(Vertical::Center),
                )
                .width(Length::Fill)
                .padding([10, 12])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::ghost_button_hovered(theme),
                    _ => theme::ghost_button(theme),
                })
                .on_press(Message::SelectVault(name_owned))
                .into()
            })
            .collect();

        // Add divider
        items.push(
            container(
                container(Space::new(Length::Fill, Length::Fixed(1.0)))
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(palette.border)),
                        ..Default::default()
                    }),
            )
            .padding([8, 0])
            .into(),
        );

        // Add "Create new vault" option
        items.push(
            button(
                row![
                    text("+").size(14).color(palette.primary),
                    Space::with_width(8),
                    text("Create new vault")
                        .size(14)
                        .color(palette.primary),
                ]
                .align_y(Vertical::Center),
            )
            .width(Length::Fill)
            .padding([10, 12])
            .style(move |_theme, status| match status {
                button::Status::Hovered => theme::ghost_button_hovered(theme),
                _ => theme::ghost_button(theme),
            })
            .on_press(Message::ShowNewVaultModal)
            .into(),
        );

        let dropdown_content = column(items).spacing(0);

        Some(
            container(dropdown_content)
                .style(move |_| theme::card_container(theme))
                .width(Length::Fixed(200.0))
                .into(),
        )
    } else {
        None
    };

    // Search bar
    let search_input = text_input("Search credentials...", &search_query_owned)
        .padding([10, 14])
        .size(14)
        .on_input(Message::SearchChanged)
        .style(move |_theme, status| match status {
            text_input::Status::Focused => theme::text_input_focused(theme),
            _ => theme::text_input_style(theme),
        })
        .width(Length::Fixed(280.0));

    let has_search = !search_query.is_empty();
    let clear_search_btn: Option<Element<'static, Message>> = if has_search {
        Some(
            button(text("✕").size(12).color(palette.text_muted))
                .padding([8, 10])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::icon_button_hovered(theme),
                    _ => theme::icon_button(theme),
                })
                .on_press(Message::ClearSearch)
                .into(),
        )
    } else {
        None
    };

    let mut search_row = row![search_input,]
        .align_y(Vertical::Center)
        .spacing(4);

    if let Some(btn) = clear_search_btn {
        search_row = search_row.push(btn);
    }

    // Action buttons
    let add_entry_btn = button(
        row![
            text("+").size(16),
            Space::with_width(6),
            text("Add Entry").size(14),
        ]
        .align_y(Vertical::Center),
    )
    .padding([10, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme),
        _ => theme::primary_button(theme),
    })
    .on_press(Message::ShowAddEntry);

    let settings_btn = button(text("⚙").size(18).color(palette.text_secondary))
        .padding([10, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme),
            _ => theme::icon_button(theme),
        })
        .on_press(Message::ShowSettings);

    let lock_btn = button(text("🔒").size(18).color(palette.text_secondary))
        .padding([10, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme),
            _ => theme::icon_button(theme),
        })
        .on_press(Message::LockVault);

    // Main header row
    let mut vault_section = column![vault_btn,];
    if let Some(dropdown) = vault_dropdown {
        vault_section = vault_section.push(dropdown);
    }

    let header_content = row![
        vault_section,
        Space::with_width(Length::Fill),
        search_row,
        Space::with_width(Length::Fill),
        add_entry_btn,
        Space::with_width(8),
        settings_btn,
        Space::with_width(4),
        lock_btn,
    ]
    .align_y(Vertical::Center)
    .padding([12, 20]);

    container(header_content)
        .width(Length::Fill)
        .style(move |_| theme::header_container(theme))
        .into()
}
