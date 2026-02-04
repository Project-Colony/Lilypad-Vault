//! Vault View
//!
//! Displays the list of credentials/entries with filtering and actions.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length, Padding};

use crate::message::Message;
use crate::state::{VaultEntry, VaultViewMode};
use crate::theme::{self, LilypadTheme};

/// Render the vault entries section
pub fn view(
    theme: LilypadTheme,
    entries: &[VaultEntry],
    search_query: &str,
    view_mode: VaultViewMode,
    show_add_entry: bool,
    edit_mode: bool,
    entry_title: &str,
    entry_username: &str,
    entry_password: &str,
    entry_url: &str,
    entry_notes: &str,
) -> Element<'static, Message> {
    let palette = theme.palette();

    // Filter entries based on search and view mode
    let filtered_entries: Vec<(usize, &VaultEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            // Search filter
            let matches_search = search_query.is_empty()
                || entry
                    .title
                    .to_lowercase()
                    .contains(&search_query.to_lowercase())
                || entry
                    .username
                    .to_lowercase()
                    .contains(&search_query.to_lowercase())
                || entry
                    .url
                    .to_lowercase()
                    .contains(&search_query.to_lowercase());

            // View mode filter
            let matches_mode = match view_mode {
                VaultViewMode::All => true,
                VaultViewMode::Favorites => entry.is_favorite,
                VaultViewMode::Recent => entry.last_accessed_at.is_some(),
                VaultViewMode::Weak => matches!(
                    entry.password_strength,
                    lilypad_common::PasswordStrength::VeryWeak
                        | lilypad_common::PasswordStrength::Weak
                ),
                VaultViewMode::Expired => entry.is_expired,
            };

            matches_search && matches_mode
        })
        .collect();

    // View mode tabs
    let view_tabs: Vec<Element<'static, Message>> = VaultViewMode::ALL
        .iter()
        .map(|mode| {
            let is_active = *mode == view_mode;
            button(
                text(mode.label())
                    .size(13)
                    .color(if is_active {
                        palette.primary
                    } else {
                        palette.text_muted
                    }),
            )
            .padding([8, 16])
            .style(move |_theme, status| {
                if is_active {
                    let mut style = theme::ghost_button(theme);
                    style.text_color = palette.primary;
                    style.border.color = palette.primary;
                    style.border.width = 0.0;
                    style.border.radius = 6.0.into();
                    style.background = Some(iced::Background::Color(palette.hover));
                    style
                } else {
                    match status {
                        button::Status::Hovered => theme::ghost_button_hovered(theme),
                        _ => theme::ghost_button(theme),
                    }
                }
            })
            .on_press(Message::SetViewMode(*mode))
            .into()
        })
        .collect();

    let tabs_row = row(view_tabs).spacing(4);

    // Entry count
    let count_text = text(format!(
        "{} credential{}",
        filtered_entries.len(),
        if filtered_entries.len() == 1 { "" } else { "s" }
    ))
    .size(13)
    .color(palette.text_muted);

    let header_row = row![tabs_row, Space::with_width(Length::Fill), count_text,]
        .align_y(Vertical::Center)
        .padding(Padding::new(0.0).bottom(16.0));

    // Entry list or empty state
    let entries_content: Element<'static, Message> = if filtered_entries.is_empty() {
        let empty_icon = text(if search_query.is_empty() { "🔐" } else { "🔍" }).size(48);

        let empty_text = text(if search_query.is_empty() {
            match view_mode {
                VaultViewMode::All => "No credentials yet",
                VaultViewMode::Favorites => "No favorite credentials",
                VaultViewMode::Recent => "No recently accessed credentials",
                VaultViewMode::Weak => "No weak passwords found",
                VaultViewMode::Expired => "No expired passwords",
            }
        } else {
            "No matching credentials"
        })
        .size(16)
        .color(palette.text_secondary);

        let empty_hint = text(if search_query.is_empty() && view_mode == VaultViewMode::All {
            "Click 'Add Entry' to create your first credential"
        } else {
            "Try adjusting your search or filter"
        })
        .size(13)
        .color(palette.text_muted);

        container(
            column![empty_icon, Space::with_height(16), empty_text, Space::with_height(8), empty_hint,]
                .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([60, 0])
        .align_x(Horizontal::Center)
        .into()
    } else {
        let entry_cards: Vec<Element<'static, Message>> = filtered_entries
            .iter()
            .map(|(index, entry)| entry_card(theme, *index, entry))
            .collect();

        scrollable(column(entry_cards).spacing(12))
            .height(Length::Fill)
            .style(move |_theme, _status| theme::scrollable_style(theme))
            .into()
    };

    // Add/Edit entry form
    let form_content = if show_add_entry {
        Some(entry_form(
            theme,
            edit_mode,
            entry_title,
            entry_username,
            entry_password,
            entry_url,
            entry_notes,
        ))
    } else {
        None
    };

    let main_content: Element<'static, Message> = if let Some(form) = form_content {
        column![form, Space::with_height(24), header_row, entries_content,]
            .spacing(0)
            .into()
    } else {
        column![header_row, entries_content,].spacing(0).into()
    };

    container(main_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(24)
        .into()
}

/// Render a single entry card
fn entry_card(theme: LilypadTheme, index: usize, entry: &VaultEntry) -> Element<'static, Message> {
    let palette = theme.palette();

    // Color indicator
    let color_indicator: Option<Element<'static, Message>> = if let Some(color) = &entry.color {
        let color_rgb = match color {
            lilypad_core::EntryColor::Red => iced::Color::from_rgb8(239, 68, 68),
            lilypad_core::EntryColor::Orange => iced::Color::from_rgb8(249, 115, 22),
            lilypad_core::EntryColor::Yellow => iced::Color::from_rgb8(234, 179, 8),
            lilypad_core::EntryColor::Green => iced::Color::from_rgb8(34, 197, 94),
            lilypad_core::EntryColor::Blue => iced::Color::from_rgb8(59, 130, 246),
            lilypad_core::EntryColor::Purple => iced::Color::from_rgb8(139, 92, 246),
            lilypad_core::EntryColor::Pink => iced::Color::from_rgb8(236, 72, 153),
            lilypad_core::EntryColor::Gray => iced::Color::from_rgb8(107, 114, 128),
        };
        Some(
            container(Space::new(Length::Fixed(4.0), Length::Fixed(50.0)))
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(color_rgb)),
                    ..Default::default()
                })
                .into(),
        )
    } else {
        None
    };

    // Favorite star
    let is_fav = entry.is_favorite;
    let favorite_btn = button(
        text(if is_fav { "★" } else { "☆" })
            .size(16)
            .color(if is_fav {
                palette.warning
            } else {
                palette.text_muted
            }),
    )
    .padding([4, 8])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
    })
    .on_press(Message::ToggleFavorite(index));

    // Title and username
    let title_str = entry.title.clone();
    let username_str = entry.username.clone();
    let title_text = text(title_str)
        .size(15)
        .color(palette.text_primary);

    let username_text = text(username_str)
        .size(13)
        .color(palette.text_secondary);

    let info_column = column![title_text, username_text,].spacing(2);

    // Strength indicator
    let strength_color = match entry.password_strength {
        lilypad_common::PasswordStrength::VeryWeak => palette.danger,
        lilypad_common::PasswordStrength::Weak => iced::Color::from_rgb8(249, 115, 22),
        lilypad_common::PasswordStrength::Fair => palette.warning,
        lilypad_common::PasswordStrength::Strong => iced::Color::from_rgb8(132, 204, 22),
        lilypad_common::PasswordStrength::VeryStrong => palette.success,
    };

    let strength_dot = container(Space::new(Length::Fixed(8.0), Length::Fixed(8.0)))
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(strength_color)),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    // Last updated
    let updated_str = entry.last_updated.clone();
    let updated_text = text(updated_str)
        .size(11)
        .color(palette.text_muted);

    // Action buttons
    let copy_user_btn = button(text("👤").size(14))
        .padding([6, 10])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme),
            _ => theme::icon_button(theme),
        })
        .on_press(Message::CopyUsername(index));

    let copy_pass_btn = button(text("🔑").size(14))
        .padding([6, 10])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme),
            _ => theme::icon_button(theme),
        })
        .on_press(Message::CopyPassword(index));

    let has_url = !entry.url.is_empty();
    let open_url_btn: Option<Element<'static, Message>> = if has_url {
        Some(
            button(text("🔗").size(14))
                .padding([6, 10])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::icon_button_hovered(theme),
                    _ => theme::icon_button(theme),
                })
                .on_press(Message::OpenUrl(index))
                .into(),
        )
    } else {
        None
    };

    let edit_btn = button(text("✏").size(14))
        .padding([6, 10])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme),
            _ => theme::icon_button(theme),
        })
        .on_press(Message::EditEntry(index));

    let delete_btn = button(text("🗑").size(14))
        .padding([6, 10])
        .style(move |_theme, status| match status {
            button::Status::Hovered => {
                let mut style = theme::icon_button_hovered(theme);
                style.text_color = palette.danger;
                style
            }
            _ => theme::icon_button(theme),
        })
        .on_press(Message::DeleteEntry(index));

    let mut actions_row = row![copy_user_btn, copy_pass_btn,]
        .spacing(4)
        .align_y(Vertical::Center);

    if let Some(url_btn) = open_url_btn {
        actions_row = actions_row.push(url_btn);
    }
    actions_row = actions_row.push(edit_btn).push(delete_btn);

    let has_color = entry.color.is_some();
    let mut card_row = row![].align_y(Vertical::Center).padding(16);

    if let Some(indicator) = color_indicator {
        card_row = card_row.push(indicator);
        card_row = card_row.push(Space::with_width(12));
    }

    card_row = card_row
        .push(favorite_btn)
        .push(Space::with_width(8))
        .push(info_column)
        .push(Space::with_width(Length::Fill))
        .push(strength_dot)
        .push(Space::with_width(12))
        .push(updated_text)
        .push(Space::with_width(16))
        .push(actions_row);

    container(card_row)
        .width(Length::Fill)
        .style(move |_| theme::card_container(theme))
        .into()
}

/// Render the add/edit entry form
fn entry_form(
    theme: LilypadTheme,
    edit_mode: bool,
    entry_title: &str,
    entry_username: &str,
    entry_password: &str,
    entry_url: &str,
    entry_notes: &str,
) -> Element<'static, Message> {
    let palette = theme.palette();

    let form_title = text(if edit_mode { "Edit Entry" } else { "Add New Entry" })
        .size(18)
        .color(palette.text_primary);

    let title_input = labeled_input(theme, "Title", "e.g., GitHub", entry_title.to_string(), |s| {
        Message::EntryTitleChanged(s)
    });

    let username_input = labeled_input(
        theme,
        "Username / Email",
        "e.g., john@example.com",
        entry_username.to_string(),
        |s| Message::EntryUsernameChanged(s),
    );

    let password_row = row![
        column![
            text("Password").size(13).color(palette.text_secondary),
            Space::with_height(6),
            text_input("Enter password...", entry_password)
                .padding(12)
                .size(14)
                .secure(true)
                .on_input(Message::EntryPasswordChanged)
                .style(move |_theme, status| match status {
                    text_input::Status::Focused => theme::text_input_focused(theme),
                    _ => theme::text_input_style(theme),
                }),
        ]
        .width(Length::Fill),
        Space::with_width(12),
        column![
            text("").size(13),
            Space::with_height(6),
            button(
                row![text("🎲").size(14), Space::with_width(6), text("Generate").size(13),]
                    .align_y(Vertical::Center),
            )
            .padding([12, 16])
            .style(move |_theme, status| match status {
                button::Status::Hovered => theme::secondary_button_hovered(theme),
                _ => theme::secondary_button(theme),
            })
            .on_press(Message::UseGeneratedPassword),
        ],
    ]
    .align_y(Vertical::Bottom);

    let url_input = labeled_input(
        theme,
        "URL (optional)",
        "e.g., https://github.com",
        entry_url.to_string(),
        |s| Message::EntryUrlChanged(s),
    );

    let notes_label = text("Notes (optional)")
        .size(13)
        .color(palette.text_secondary);

    let notes_input = text_input("Additional notes...", entry_notes)
        .padding(12)
        .size(14)
        .on_input(Message::EntryNotesChanged)
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
        .on_press(Message::HideAddEntry);

    let save_btn = button(
        text(if edit_mode { "Update" } else { "Save" })
            .size(14),
    )
    .padding([12, 24])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme),
        _ => theme::primary_button(theme),
    })
    .on_press(Message::SaveEntry);

    let actions_row = row![Space::with_width(Length::Fill), cancel_btn, Space::with_width(12), save_btn,]
        .align_y(Vertical::Center);

    let form_content = column![
        form_title,
        Space::with_height(20),
        title_input,
        Space::with_height(16),
        username_input,
        Space::with_height(16),
        password_row,
        Space::with_height(16),
        url_input,
        Space::with_height(16),
        notes_label,
        Space::with_height(6),
        notes_input,
        Space::with_height(24),
        actions_row,
    ]
    .padding(24);

    container(form_content)
        .width(Length::Fill)
        .style(move |_| theme::card_container(theme))
        .into()
}

/// Helper to create a labeled text input
fn labeled_input<F>(
    theme: LilypadTheme,
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
        Space::with_height(6),
        text_input(placeholder, &value)
            .padding(12)
            .size(14)
            .on_input(on_change)
            .style(move |_theme, status| match status {
                text_input::Status::Focused => theme::text_input_focused(theme),
                _ => theme::text_input_style(theme),
            }),
    ]
    .into()
}
