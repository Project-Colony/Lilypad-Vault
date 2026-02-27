//! Header View
//!
//! The top bar containing vault selector, search, and action buttons.

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::theme::{self, LilypadTheme, UiVariation};

/// Render the header bar
pub fn view(
    theme: LilypadTheme,
    v: UiVariation,
    active_vault: &str,
    _available_vaults: &[String], // Dropdown uses vault_dropdown_overlay instead
    search_query: &str,
    show_vault_selector: bool,
    github_authenticated: bool,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let active_vault_owned = active_vault.to_string();
    let search_query_owned = search_query.to_string();

    // Vault selector button
    let vault_btn = button(
        row![
            fonts::centered_icon_colored(icons::VAULT, 14.0, palette.primary),
            Space::new().width(8),
            text(active_vault_owned).size(14).font(fonts::FONT_MEDIUM).color(palette.text_primary),
            Space::new().width(8),
            fonts::centered_icon_colored(icons::CHEVRON_DOWN, 10.0, palette.text_muted),
        ]
        .align_y(Vertical::Center),
    )
    .padding([8, 12])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::ghost_button_hovered(theme, v),
        _ => theme::ghost_button(theme, v),
    })
    .on_press(if show_vault_selector {
        Message::HideVaultSelector
    } else {
        Message::ShowVaultSelector
    });

    // Search bar
    let search_input = text_input("Search credentials...", &search_query_owned)
        .padding([10, 14])
        .size(14)
        .on_input(Message::SearchChanged)
        .style(move |_theme, status| match status {
            text_input::Status::Focused { .. } => theme::text_input_focused(theme, v),
            _ => theme::text_input_style(theme, v),
        })
        .width(Length::Fixed(280.0));

    let has_search = !search_query.is_empty();
    let clear_search_btn: Option<Element<'static, Message>> = if has_search {
        Some(
            button(fonts::centered_icon_colored(icons::CLOSE, 12.0, palette.text_muted))
                .padding([8, 10])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::icon_button_hovered(theme, v),
                    _ => theme::icon_button(theme, v),
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
            fonts::centered_icon(icons::PLUS, 14.0),
            Space::new().width(6),
            text("Add Entry").size(14).font(fonts::FONT_SEMIBOLD),
        ]
        .align_y(Vertical::Center),
    )
    .padding([10, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme, v),
        _ => theme::primary_button(theme, v),
    })
    .on_press(Message::ShowAddEntry);

    // GitHub sync status indicator
    let sync_indicator: Element<'static, Message> = if github_authenticated {
        fonts::centered_icon_colored(icons::CIRCLE, 10.0, palette.success)
    } else {
        fonts::centered_icon_colored(icons::CIRCLE, 10.0, palette.text_muted)
    };

    let settings_btn = button(fonts::centered_icon_colored(icons::COG, 16.0, palette.text_secondary))
        .padding([10, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme, v),
            _ => theme::icon_button(theme, v),
        })
        .on_press(Message::ShowSettings);

    let lock_btn = button(fonts::centered_icon_colored(icons::LOCK, 16.0, palette.text_secondary))
        .padding([10, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme, v),
            _ => theme::icon_button(theme, v),
        })
        .on_press(Message::LockVault);

    // Main header row (dropdown is rendered as overlay in app.rs)
    let header_content = row![
        vault_btn,
        Space::new().width(Length::Fill),
        search_row,
        Space::new().width(Length::Fill),
        add_entry_btn,
        Space::new().width(8),
        sync_indicator,
        Space::new().width(8),
        settings_btn,
        Space::new().width(4),
        lock_btn,
    ]
    .align_y(Vertical::Center)
    .padding([12, 20]);

    container(header_content)
        .width(Length::Fill)
        .style(move |_| theme::header_container(theme, v))
        .into()
}

/// Render the vault selector dropdown as an overlay
pub fn vault_dropdown_overlay(
    theme: LilypadTheme,
    v: UiVariation,
    active_vault: &str,
    available_vaults: &[String],
) -> Element<'static, Message> {
    let palette = theme.palette();
    let active_vault_owned = active_vault.to_string();

    let mut items: Vec<Element<'static, Message>> = available_vaults
        .iter()
        .map(|name| {
            let is_active = name == &active_vault_owned;
            let name_for_select = name.clone();
            let name_for_rename = name.clone();
            button(
                row![
                    text(if is_active { icons::CIRCLE } else { "" })
                        .size(8)
                        .font(fonts::FONT_REGULAR)
                        .color(palette.primary)
                        .width(Length::Fixed(16.0)),
                    text(name.clone()).size(14).font(fonts::FONT_REGULAR).color(palette.text_primary),
                    Space::new().width(Length::Fill),
                    button(
                        fonts::centered_icon_colored(icons::EDIT, 12.0, palette.text_muted),
                    )
                    .padding([4, 6])
                    .style(move |_theme, status| match status {
                        button::Status::Hovered => theme::icon_button_hovered(theme, v),
                        _ => theme::icon_button(theme, v),
                    })
                    .on_press(Message::StartRenameVault(name_for_rename)),
                ]
                .align_y(Vertical::Center),
            )
            .width(Length::Fill)
            .padding([10, 12])
            .style(move |_theme, status| match status {
                button::Status::Hovered => theme::ghost_button_hovered(theme, v),
                _ => theme::ghost_button(theme, v),
            })
            .on_press(Message::SelectVault(name_for_select))
            .into()
        })
        .collect();

    // Add divider
    items.push(
        container(
            container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
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
            container(
                row![
                    fonts::centered_icon_colored(icons::PLUS, 14.0, palette.primary),
                    Space::new().width(8),
                    text("Create new vault")
                        .size(14)
                        .font(fonts::FONT_MEDIUM)
                        .color(palette.primary),
                ]
                .align_y(Vertical::Center),
            )
            .width(Length::Fill)
            .center_y(Length::Shrink),
        )
        .width(Length::Fill)
        .padding([10, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::ghost_button_hovered(theme, v),
            _ => theme::ghost_button(theme, v),
        })
        .on_press(Message::ShowNewVaultModal)
        .into(),
    );

    let dropdown_content = column(items).spacing(0);

    let dropdown_card = container(dropdown_content)
        .style(move |_| theme::card_container(theme, v))
        .width(Length::Fixed(200.0));

    // Position the dropdown at the top-left below the header button
    let positioned = column![
        Space::new().height(56), // Offset below header
        row![
            Space::new().width(20), // Left margin
            dropdown_card,
        ],
    ];

    container(positioned)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
