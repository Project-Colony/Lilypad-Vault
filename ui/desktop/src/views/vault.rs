//! Vault View
//!
//! Displays the list of credentials/entries with filtering and actions.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input, Space};
use iced::{Element, Length, Padding};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::state::{VaultEntry, VaultViewMode};
use crate::theme::{self, LilypadTheme, UiVariation};
use crate::views::common::{labeled_input, separator};

/// Parameters for the vault view.
pub struct VaultViewParams<'a> {
    pub theme: LilypadTheme,
    pub variation: UiVariation,
    pub entries: &'a [VaultEntry],
    pub search_query: &'a str,
    pub view_mode: VaultViewMode,
    pub show_add_entry: bool,
    pub edit_mode: bool,
    pub entry_title: &'a str,
    pub entry_username: &'a str,
    pub entry_password: &'a str,
    pub entry_url: &'a str,
    pub entry_notes: &'a str,
    pub entry_email: &'a str,
    pub entry_phone: &'a str,
    pub entry_folder: &'a str,
    pub entry_tags: &'a [String],
    pub entry_new_tag: &'a str,
    pub entry_totp_secret: &'a str,
    pub entry_custom_fields: &'a [(String, String)],
    pub folder_filter: Option<&'a str>,
    pub entry_type: &'a str,
    pub entry_attachments: &'a [(String, String)],
    pub show_advanced_fields: bool,
    pub hovered_entry_index: Option<usize>,
}

/// Parameters for the entry add/edit form.
struct EntryFormParams<'a> {
    theme: LilypadTheme,
    v: UiVariation,
    edit_mode: bool,
    title: &'a str,
    username: &'a str,
    password: &'a str,
    url: &'a str,
    notes: &'a str,
    email: &'a str,
    phone: &'a str,
    folder: &'a str,
    tags: &'a [String],
    new_tag: &'a str,
    totp_secret: &'a str,
    custom_fields: &'a [(String, String)],
    entry_type: &'a str,
    attachments: &'a [(String, String)],
    show_advanced: bool,
}

/// Render the vault entries section
pub fn view(params: VaultViewParams<'_>) -> Element<'static, Message> {
    let VaultViewParams {
        theme,
        variation: v,
        entries,
        search_query,
        view_mode,
        show_add_entry,
        edit_mode,
        entry_title,
        entry_username,
        entry_password,
        entry_url,
        entry_notes,
        entry_email,
        entry_phone,
        entry_folder,
        entry_tags,
        entry_new_tag,
        entry_totp_secret,
        entry_custom_fields,
        folder_filter,
        entry_type,
        entry_attachments,
        show_advanced_fields,
        hovered_entry_index,
    } = params;

    // Filter entries based on search and view mode
    let filtered_entries = filter_entries(entries, search_query, view_mode, folder_filter);

    let tabs_row = view_mode_tabs(theme, v, view_mode);

    let count_text = text(format!(
        "{} credential{}",
        filtered_entries.len(),
        if filtered_entries.len() == 1 { "" } else { "s" }
    ))
    .size(13)
    .color(theme.palette().text_muted);

    let folder_row = folder_filter_chips(theme, v, entries, folder_filter);

    let header_row = row![tabs_row, Space::new().width(Length::Fill), count_text,]
        .align_y(Vertical::Center)
        .padding(Padding::new(0.0).bottom(16.0));

    let entries_content = entries_list(
        theme, v, &filtered_entries, search_query, view_mode, hovered_entry_index,
    );

    // Add/Edit entry form
    let form_content = if show_add_entry {
        Some(entry_form(EntryFormParams {
            theme,
            v,
            edit_mode,
            title: entry_title,
            username: entry_username,
            password: entry_password,
            url: entry_url,
            notes: entry_notes,
            email: entry_email,
            phone: entry_phone,
            folder: entry_folder,
            tags: entry_tags,
            new_tag: entry_new_tag,
            totp_secret: entry_totp_secret,
            custom_fields: entry_custom_fields,
            entry_type,
            attachments: entry_attachments,
            show_advanced: show_advanced_fields,
        }))
    } else {
        None
    };

    let mut main_column = column![].spacing(0);

    if let Some(form) = form_content {
        main_column = main_column
            .push(form)
            .push(Space::new().height(24));
    }

    if let Some(folder_element) = folder_row {
        main_column = main_column
            .push(folder_element)
            .push(Space::new().height(8));
    }

    main_column = main_column
        .push(header_row)
        .push(entries_content);

    let main_content: Element<'static, Message> = scrollable(main_column)
        .height(Length::Fill)
        .style(move |_theme, _status| theme::scrollable_style(theme, v))
        .into();

    container(main_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(24)
        .into()
}

/// Filter entries by search query, view mode, and folder.
fn filter_entries<'a>(
    entries: &'a [VaultEntry],
    search_query: &str,
    view_mode: VaultViewMode,
    folder_filter: Option<&str>,
) -> Vec<(usize, &'a VaultEntry)> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            let query_lower = search_query.to_lowercase();
            let matches_search = search_query.is_empty()
                || entry.title.to_lowercase().contains(&query_lower)
                || entry.username.to_lowercase().contains(&query_lower)
                || entry.url.to_lowercase().contains(&query_lower)
                || entry.tags.iter().any(|t| t.to_lowercase().contains(&query_lower));

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

            let matches_folder = match folder_filter {
                Some(f) => entry.folder.as_deref() == Some(f),
                None => true,
            };

            matches_search && matches_mode && matches_folder
        })
        .collect()
}

/// Render the view mode tab bar (All, Favorites, Recent, Weak, Expired).
fn view_mode_tabs(
    theme: LilypadTheme,
    v: UiVariation,
    view_mode: VaultViewMode,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let tabs: Vec<Element<'static, Message>> = VaultViewMode::ALL
        .iter()
        .map(|mode| {
            let is_active = *mode == view_mode;
            button(
                text(mode.label())
                    .size(13)
                    .color(if is_active { palette.primary } else { palette.text_muted }),
            )
            .padding([8, 16])
            .style(move |_theme, status| {
                if is_active {
                    let mut style = theme::ghost_button(theme, v);
                    style.text_color = palette.primary;
                    style.border.color = palette.primary;
                    style.border.width = 0.0;
                    style.border.radius = 6.0.into();
                    style.background = Some(iced::Background::Color(palette.hover));
                    style
                } else {
                    match status {
                        button::Status::Hovered => theme::ghost_button_hovered(theme, v),
                        button::Status::Pressed => theme::ghost_button_pressed(theme, v),
                        _ => theme::ghost_button(theme, v),
                    }
                }
            })
            .on_press(Message::SetViewMode(*mode))
            .into()
        })
        .collect();

    row(tabs).spacing(4).into()
}

/// Render folder filter chip bar. Returns `None` if there are no folders.
fn folder_filter_chips(
    theme: LilypadTheme,
    v: UiVariation,
    entries: &[VaultEntry],
    folder_filter: Option<&str>,
) -> Option<Element<'static, Message>> {
    let palette = theme.palette();
    let mut folders: Vec<String> = entries
        .iter()
        .filter_map(|e| e.folder.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    folders.sort();

    if folders.is_empty() {
        return None;
    }

    let folder_filter_owned: Option<String> = folder_filter.map(|s| s.to_string());
    let no_filter = folder_filter_owned.is_none();

    let mut chips = row![
        button(
            text("All folders")
                .size(11)
                .color(if no_filter { palette.primary } else { palette.text_muted }),
        )
        .padding([4, 10])
        .style(theme::ghost_active_style(theme, v, no_filter))
        .on_press(Message::FilterByFolder(None)),
    ]
    .spacing(4)
    .align_y(Vertical::Center);

    for folder in &folders {
        let folder_name = folder.clone();
        let is_active = folder_filter_owned.as_deref() == Some(folder.as_str());
        chips = chips.push(
            button(
                row![
                    fonts::centered_icon(icons::FOLDER, 10.0),
                    Space::new().width(4),
                    text(folder_name.clone()).size(11),
                ]
                .align_y(Vertical::Center),
            )
            .padding([4, 10])
            .style(theme::ghost_active_style(theme, v, is_active))
            .on_press(Message::FilterByFolder(Some(folder_name))),
        );
    }

    Some(chips.into())
}

/// Render the entry list, or an empty state message if no entries match.
fn entries_list(
    theme: LilypadTheme,
    v: UiVariation,
    filtered_entries: &[(usize, &VaultEntry)],
    search_query: &str,
    view_mode: VaultViewMode,
    hovered_entry_index: Option<usize>,
) -> Element<'static, Message> {
    let palette = theme.palette();
    if filtered_entries.is_empty() {
        let empty_icon = text(if search_query.is_empty() {
            icons::VAULT
        } else {
            icons::SEARCH
        })
        .size(48)
        .font(fonts::FONT_REGULAR);

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
            column![empty_icon, Space::new().height(16), empty_text, Space::new().height(8), empty_hint,]
                .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([60, 0])
        .align_x(Horizontal::Center)
        .into()
    } else {
        let grid_rows: Vec<Element<'static, Message>> = filtered_entries
            .chunks(2)
            .map(|pair| {
                let mut r = row![].spacing(12);
                for (index, entry) in pair {
                    let is_hovered = hovered_entry_index == Some(*index);
                    r = r.push(entry_card(theme, v, *index, entry, is_hovered));
                }
                if pair.len() == 1 {
                    r = r.push(Space::new().width(Length::FillPortion(1)));
                }
                r.into()
            })
            .collect();

        column(grid_rows).spacing(12).into()
    }
}

/// Render a single entry card
fn entry_card(theme: LilypadTheme, v: UiVariation, index: usize, entry: &VaultEntry, is_hovered: bool) -> Element<'static, Message> {
    let palette = theme.palette();

    // Color indicator
    let color_indicator: Option<Element<'static, Message>> = if let Some(color) = &entry.color {
        let color_rgb = theme::entry_color_to_iced(color);
        Some(
            container(Space::new().width(Length::Fixed(4.0)).height(Length::Fixed(50.0)))
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
    let star_color = if is_fav { palette.warning } else { palette.text_muted };
    let favorite_btn = button(
        fonts::centered_icon_colored(icons::STAR, 16.0, star_color),
    )
    .padding([4, 8])
    .style(move |_theme, status| match status {
        button::Status::Hovered => {
            let mut s = theme::icon_button_hovered(theme, v);
            s.text_color = palette.warning;
            s
        }
        button::Status::Pressed => {
            let mut s = theme::icon_button_pressed(theme, v);
            s.text_color = palette.warning;
            s.background = Some(iced::Background::Color(iced::Color {
                a: 0.2,
                ..palette.warning
            }));
            s
        }
        _ => theme::icon_button(theme, v),
    })
    .on_press(Message::ToggleFavorite(index));

    // Type icon
    let type_icon_str = match entry.entry_type {
        lilypad_core::EntryType::Login => icons::KEY,
        lilypad_core::EntryType::Card => icons::CREDIT_CARD,
        lilypad_core::EntryType::Identity => icons::ID_CARD,
        lilypad_core::EntryType::SecureNote => icons::STICKY_NOTE,
        lilypad_core::EntryType::SoftwareLicense => icons::CERTIFICATE,
        lilypad_core::EntryType::Wifi => icons::WIFI,
        lilypad_core::EntryType::Server => icons::SERVER,
        lilypad_core::EntryType::Custom => icons::ELLIPSIS,
    };
    let type_icon = container(
        fonts::centered_icon_colored(type_icon_str, 18.0, palette.text_muted),
    )
    .width(Length::Fixed(36.0))
    .height(Length::Fixed(36.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| theme::icon_badge_container(theme, v));

    // Title and username
    let title_str = entry.title.clone();
    let username_str = entry.username.clone();
    let title_text = text(title_str)
        .size(15)
        .color(palette.text_primary);

    let username_text = text(username_str)
        .size(13)
        .color(palette.text_secondary);

    // Strength indicator
    let strength_color = match entry.password_strength {
        lilypad_common::PasswordStrength::VeryWeak => palette.danger,
        lilypad_common::PasswordStrength::Weak => iced::Color::from_rgb8(249, 115, 22),
        lilypad_common::PasswordStrength::Fair => palette.warning,
        lilypad_common::PasswordStrength::Strong => iced::Color::from_rgb8(132, 204, 22),
        lilypad_common::PasswordStrength::VeryStrong => palette.success,
    };

    let strength_dot = container(Space::new().width(Length::Fixed(8.0)).height(Length::Fixed(8.0)))
        .style(move |_| theme::dot_indicator(strength_color));

    // Last updated
    let updated_str = entry.last_updated.clone();
    let updated_text = text(updated_str)
        .size(11)
        .color(palette.text_muted);

    let actions_row = entry_action_buttons(theme, v, index, &entry.url);

    // Row 1: icon + star + title + strength dot
    let top_row = row![
        type_icon,
        Space::new().width(8),
        favorite_btn,
        Space::new().width(6),
        title_text,
        Space::new().width(Length::Fill),
        strength_dot,
    ]
    .align_y(Vertical::Center);

    // Info section: username + badges, indented under title
    // Indent = icon(36) + space(8) + star_btn(~32) + space(6) = 82px
    let mut info_col: iced::widget::Column<'static, Message> = column![].spacing(2);
    // Always push username (or empty space) so all cards have uniform height
    info_col = info_col.push(username_text);
    if let Some(badges) = entry_badge_row(theme, v, entry) {
        info_col = info_col.push(badges);
    }
    let info_section = container(info_col).padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 82.0 });

    // Row 2: action buttons + date
    let bottom_row = row![
        actions_row,
        Space::new().width(Length::Fill),
        updated_text,
    ]
    .align_y(Vertical::Center);

    // Assemble vertical card layout
    let card_col = column![top_row, info_section, bottom_row].spacing(4);

    // Wrap with optional color indicator
    let inner: Element<'static, Message> = if let Some(indicator) = color_indicator {
        row![indicator, Space::new().width(8), card_col]
            .width(Length::Fill)
            .into()
    } else {
        card_col.into()
    };

    let card = container(inner)
        .width(Length::FillPortion(1))
        .padding(14)
        .style(move |_| {
            if is_hovered {
                theme::card_container_hovered(theme, v)
            } else {
                theme::card_container(theme, v)
            }
        });

    mouse_area(card)
        .on_enter(Message::EntryCardHovered(index))
        .on_exit(Message::EntryCardUnhovered)
        .into()
}

/// Build the badge row (type, attachments, TOTP, folder, tags) for an entry card.
/// Returns `None` if there are no badges to show.
fn entry_badge_row(
    theme: LilypadTheme,
    v: UiVariation,
    entry: &VaultEntry,
) -> Option<Element<'static, Message>> {
    let palette = theme.palette();
    let has_tags = !entry.tags.is_empty();
    let has_folder = entry.folder.is_some();
    let has_totp = entry.totp_secret.is_some();
    let is_non_login = !matches!(entry.entry_type, lilypad_core::EntryType::Login);
    let has_attachments = !entry.attachments.is_empty();

    if !(has_tags || has_folder || has_totp || is_non_login || has_attachments) {
        return None;
    }

    let mut badge_row = row![].spacing(4).align_y(Vertical::Center);

    if is_non_login {
        let type_label = format!("{:?}", entry.entry_type);
        badge_row = badge_row.push(
            container(text(type_label).size(10).color(palette.text_secondary))
                .padding([2, 6])
                .style(move |_| theme::badge_container(theme, v)),
        );
    }

    if has_attachments {
        let count = entry.attachments.len();
        badge_row = badge_row.push(
            container(
                row![
                    fonts::centered_icon(icons::SAVE, 9.0),
                    Space::new().width(3),
                    text(format!("{}", count)).size(10),
                ]
                .align_y(Vertical::Center),
            )
            .padding([2, 6])
            .style(move |_| theme::badge_container(theme, v)),
        );
    }

    if has_totp {
        let totp_label = if let Some(ref code) = entry.totp_code {
            format!("TOTP: {}", code)
        } else {
            "TOTP".to_string()
        };
        badge_row = badge_row.push(
            container(text(totp_label).size(10).color(palette.primary))
                .padding([2, 6])
                .style(move |_| theme::primary_badge_container(theme, v)),
        );
    }

    if let Some(ref folder) = entry.folder {
        badge_row = badge_row.push(
            text(format!("/{}", folder))
                .size(10)
                .color(palette.text_muted),
        );
    }

    for tag in &entry.tags {
        let tag_str = tag.clone();
        badge_row = badge_row.push(
            container(text(tag_str).size(10).color(palette.text_muted))
                .padding([1, 5])
                .style(move |_| theme::badge_container(theme, v)),
        );
    }

    Some(badge_row.into())
}

/// Build the action button row for an entry card.
fn entry_action_buttons(
    theme: LilypadTheme,
    v: UiVariation,
    index: usize,
    url: &str,
) -> Element<'static, Message> {
    let palette = theme.palette();

    let copy_user_btn = button(fonts::centered_icon(icons::USER, 14.0))
        .padding([6, 10])
        .style(theme::icon_style(theme, v))
        .on_press(Message::CopyUsername(index));

    let copy_pass_btn = button(fonts::centered_icon(icons::KEY, 14.0))
        .padding([6, 10])
        .style(theme::icon_style(theme, v))
        .on_press(Message::CopyPassword(index));

    let history_btn = button(fonts::centered_icon(icons::CLOCK, 14.0))
        .padding([6, 10])
        .style(theme::icon_style(theme, v))
        .on_press(Message::ViewEntryHistory(index));

    let edit_btn = button(fonts::centered_icon(icons::EDIT, 14.0))
        .padding([6, 10])
        .style(theme::icon_style(theme, v))
        .on_press(Message::EditEntry(index));

    let delete_btn = button(fonts::centered_icon(icons::TRASH, 14.0))
        .padding([6, 10])
        .style(move |_theme, status| match status {
            button::Status::Hovered => {
                let mut style = theme::icon_button_hovered(theme, v);
                style.text_color = palette.danger;
                style.border.color = iced::Color { a: 0.3, ..palette.danger };
                style.background = Some(iced::Background::Color(iced::Color {
                    a: 0.1,
                    ..palette.danger
                }));
                style
            }
            button::Status::Pressed => {
                let mut style = theme::icon_button_pressed(theme, v);
                style.text_color = palette.danger;
                style.background = Some(iced::Background::Color(iced::Color {
                    a: 0.2,
                    ..palette.danger
                }));
                style
            }
            _ => theme::icon_button(theme, v),
        })
        .on_press(Message::DeleteEntry(index));

    let mut actions = row![copy_user_btn, copy_pass_btn,]
        .spacing(4)
        .align_y(Vertical::Center);

    if !url.is_empty() {
        actions = actions.push(
            button(fonts::centered_icon(icons::EXTERNAL_LINK, 14.0))
                .padding([6, 10])
                .style(theme::icon_style(theme, v))
                .on_press(Message::OpenUrl(index)),
        );
    }

    actions
        .push(history_btn)
        .push(edit_btn)
        .push(delete_btn)
        .into()
}

/// Field visibility per entry type
struct FieldVisibility {
    username: bool,
    email: bool,
    password: bool,
    url: bool,
    phone: bool,
    totp: bool,
    notes: bool,
}

fn fields_for_type(entry_type: &str) -> FieldVisibility {
    match entry_type {
        "Login" => FieldVisibility {
            username: true, email: true, password: true, url: true,
            phone: false, totp: true, notes: true,
        },
        "Card" => FieldVisibility {
            username: true, email: false, password: true, url: false,
            phone: true, totp: false, notes: true,
        },
        "Identity" => FieldVisibility {
            username: true, email: true, password: false, url: false,
            phone: true, totp: false, notes: true,
        },
        "SecureNote" => FieldVisibility {
            username: false, email: false, password: false, url: false,
            phone: false, totp: false, notes: true,
        },
        "SoftwareLicense" => FieldVisibility {
            username: true, email: false, password: true, url: true,
            phone: false, totp: false, notes: true,
        },
        "Wifi" => FieldVisibility {
            username: true, email: false, password: true, url: false,
            phone: false, totp: false, notes: false,
        },
        "Server" => FieldVisibility {
            username: true, email: false, password: true, url: true,
            phone: false, totp: true, notes: true,
        },
        _ => FieldVisibility { // Custom
            username: true, email: true, password: true, url: true,
            phone: true, totp: true, notes: true,
        },
    }
}

/// Type-aware labels and placeholders for fields
fn type_labels(entry_type: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    // Returns (username_label, username_placeholder, password_label, url_label)
    match entry_type {
        "Card" => ("Cardholder Name", "Name on card", "PIN / CVV", ""),
        "Identity" => ("Full Name", "First and last name", "", ""),
        "SoftwareLicense" => ("License Key", "XXXX-XXXX-XXXX", "Activation Code", "Vendor Website"),
        "Wifi" => ("Network Name (SSID)", "e.g., MyWiFi", "WiFi Password", ""),
        "Server" => ("Username", "e.g., root", "Password", "Host / IP"),
        _ => ("Username", "e.g., john_doe", "Password", "URL"),
    }
}

/// Get icon for entry type
fn type_icon(entry_type: &str) -> &'static str {
    match entry_type {
        "Login" => icons::KEY,
        "Card" => icons::CREDIT_CARD,
        "Identity" => icons::ID_CARD,
        "SecureNote" => icons::STICKY_NOTE,
        "SoftwareLicense" => icons::CERTIFICATE,
        "Wifi" => icons::WIFI,
        "Server" => icons::SERVER,
        _ => icons::ELLIPSIS,
    }
}

/// Helper to build a form section with title and icon (no card, just a labeled group)
fn form_section<'a>(
    theme: LilypadTheme,
    v: UiVariation,
    icon: &'static str,
    title: &'static str,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let palette = theme.palette();
    let header = row![
        fonts::centered_icon_colored(icon, 13.0, palette.text_muted),
        Space::new().width(8),
        text(title).size(14).color(palette.text_primary),
    ]
    .align_y(Vertical::Center);

    // Separator line
    let sep = separator(theme, v);

    column![sep, Space::new().height(12), header, Space::new().height(12), content,]
        .into()
}

/// Render the add/edit entry form
fn entry_form(params: EntryFormParams<'_>) -> Element<'static, Message> {
    let EntryFormParams {
        theme, v, edit_mode,
        title: entry_title, username: entry_username, password: entry_password,
        url: entry_url, notes: entry_notes, email: entry_email,
        phone: entry_phone, folder: entry_folder, tags: entry_tags,
        new_tag: entry_new_tag, totp_secret: entry_totp_secret,
        custom_fields: entry_custom_fields, entry_type,
        attachments: entry_attachments, show_advanced,
    } = params;
    let palette = theme.palette();
    let vis = fields_for_type(entry_type);
    let (username_label, username_placeholder, password_label, url_label) = type_labels(entry_type);

    // ── Header ──────────────────────────────────────────────────────────
    let form_title = row![
        text(type_icon(entry_type)).size(20).font(fonts::FONT_REGULAR).color(palette.primary),
        Space::new().width(10),
        text(if edit_mode { "Edit Entry" } else { "New Entry" })
            .size(18)
            .color(palette.text_primary),
        Space::new().width(Length::Fill),
        button(
            fonts::centered_icon_colored(icons::CLOSE, 14.0, palette.text_muted),
        )
        .padding([6, 10])
        .style(theme::icon_style(theme, v))
        .on_press(Message::HideAddEntry),
    ]
    .align_y(Vertical::Center);

    // ── Type selector ───────────────────────────────────────────────────
    let entry_types = [
        ("Login", icons::KEY),
        ("Card", icons::CREDIT_CARD),
        ("Identity", icons::ID_CARD),
        ("SecureNote", icons::STICKY_NOTE),
        ("SoftwareLicense", icons::CERTIFICATE),
        ("Wifi", icons::WIFI),
        ("Server", icons::SERVER),
        ("Custom", icons::ELLIPSIS),
    ];

    let type_buttons: Vec<Element<'static, Message>> = entry_types
        .iter()
        .map(|(t, icon)| {
            let is_active = *t == entry_type;
            let type_str = (*t).to_string();
            let display = match *t {
                "SecureNote" => "Note",
                "SoftwareLicense" => "License",
                other => other,
            };
            button(
                row![
                    fonts::centered_icon(*icon, 12.0),
                    Space::new().width(5),
                    text(display).size(12),
                ]
                .align_y(Vertical::Center),
            )
            .padding([6, 10])
            .style(theme::ghost_active_style(theme, v, is_active))
            .on_press(Message::EntryTypeChanged(type_str))
            .into()
        })
        .collect();

    let type_row = row(type_buttons).spacing(4);

    // ── Section 1: General ──────────────────────────────────────────────
    let title_input = labeled_input(theme, v, "Title", "e.g., GitHub", entry_title.to_string(), |s| {
        Message::EntryTitleChanged(s)
    });

    let general_section = form_section(theme, v, icons::INFO, "General", title_input);

    // ── Section 2: Credentials ──────────────────────────────────────────
    let has_credentials = vis.username || vis.email || vis.password;
    let credentials_section: Option<Element<'static, Message>> = if has_credentials {
        let mut creds_col = column![].spacing(12);

        // Row 1: Username + Email side by side (if both visible)
        if vis.username && vis.email {
            creds_col = creds_col.push(
                row![
                    column![labeled_input(
                        theme, v, username_label, username_placeholder,
                        entry_username.to_string(), Message::EntryUsernameChanged,
                    )]
                    .width(Length::Fill),
                    Space::new().width(12),
                    column![labeled_input(
                        theme, v, "Email", "e.g., john@example.com",
                        entry_email.to_string(), Message::EntryEmailChanged,
                    )]
                    .width(Length::Fill),
                ],
            );
        } else if vis.username {
            creds_col = creds_col.push(labeled_input(
                theme, v, username_label, username_placeholder,
                entry_username.to_string(), Message::EntryUsernameChanged,
            ));
        } else if vis.email {
            creds_col = creds_col.push(labeled_input(
                theme, v, "Email", "e.g., john@example.com",
                entry_email.to_string(), Message::EntryEmailChanged,
            ));
        }

        // Row 2: Password + Generate button
        if vis.password {
            let pw_label = if password_label.is_empty() { "Password" } else { password_label };
            creds_col = creds_col.push(
                row![
                    column![
                        text(pw_label).size(13).color(palette.text_secondary),
                        Space::new().height(6),
                        text_input("Enter password...", entry_password)
                            .padding(12)
                            .size(14)
                            .secure(true)
                            .on_input(Message::EntryPasswordChanged)
                            .style(theme::text_input_style_closure(theme, v)),
                    ]
                    .width(Length::Fill),
                    Space::new().width(12),
                    column![
                        text("").size(13),
                        Space::new().height(6),
                        button(
                            row![
                                fonts::centered_icon(icons::DICE, 14.0),
                                Space::new().width(6),
                                text("Generate").size(13),
                            ]
                            .align_y(Vertical::Center),
                        )
                        .padding([12, 16])
                        .style(theme::secondary_style(theme, v))
                        .on_press(Message::UseGeneratedPassword),
                    ],
                ]
                .align_y(Vertical::Bottom),
            );
        }

        Some(form_section(theme, v, icons::KEY, "Credentials", creds_col.into()))
    } else {
        None
    };

    // ── Section 3: Details (URL, Phone, TOTP) ───────────────────────────
    let has_details = vis.url || vis.phone || vis.totp;
    let details_section: Option<Element<'static, Message>> = if has_details {
        let mut details_col = column![].spacing(12);

        // URL + Phone side by side if both visible
        if vis.url && vis.phone {
            let url_lbl = if url_label.is_empty() { "URL" } else { url_label };
            details_col = details_col.push(
                row![
                    column![labeled_input(
                        theme, v, url_lbl, "e.g., https://github.com",
                        entry_url.to_string(), Message::EntryUrlChanged,
                    )]
                    .width(Length::Fill),
                    Space::new().width(12),
                    column![labeled_input(
                        theme, v, "Phone", "e.g., +33 6 12 34 56 78",
                        entry_phone.to_string(), Message::EntryPhoneChanged,
                    )]
                    .width(Length::Fill),
                ],
            );
        } else if vis.url {
            let url_lbl = if url_label.is_empty() { "URL" } else { url_label };
            details_col = details_col.push(labeled_input(
                theme, v, url_lbl, "e.g., https://github.com",
                entry_url.to_string(), Message::EntryUrlChanged,
            ));
        } else if vis.phone {
            details_col = details_col.push(labeled_input(
                theme, v, "Phone", "e.g., +33 6 12 34 56 78",
                entry_phone.to_string(), Message::EntryPhoneChanged,
            ));
        }

        if vis.totp {
            details_col = details_col.push(labeled_input(
                theme, v, "TOTP Secret", "e.g., JBSWY3DPEHPK3PXP",
                entry_totp_secret.to_string(), Message::EntryTotpSecretChanged,
            ));
        }

        Some(form_section(theme, v, icons::GLOBE, "Details", details_col.into()))
    } else {
        None
    };

    // ── Organization + Notes content (moved into Advanced) ────────────
    let mut org_col = column![].spacing(12);
    org_col = org_col.push(labeled_input(
        theme, v, "Folder", "e.g., Work/Email",
        entry_folder.to_string(), Message::EntryFolderChanged,
    ));

    // Tags
    let mut tags_row = row![].spacing(6).align_y(Vertical::Center);
    for (i, tag) in entry_tags.iter().enumerate() {
        let tag_str = tag.clone();
        tags_row = tags_row.push(
            container(
                row![
                    text(tag_str).size(11).color(palette.text_primary),
                    Space::new().width(4),
                    button(fonts::centered_icon_colored(icons::CLOSE, 9.0, palette.text_muted))
                        .padding([2, 4])
                        .style(theme::icon_style(theme, v))
                        .on_press(Message::RemoveEntryTag(i)),
                ]
                .align_y(Vertical::Center),
            )
            .padding([3, 8])
            .style(move |_| theme::badge_container(theme, v)),
        );
    }

    let tag_input_row = row![
        text_input("Add tag...", entry_new_tag)
            .padding(10)
            .size(13)
            .on_input(Message::EntryNewTagChanged)
            .on_submit(Message::AddEntryTag)
            .style(theme::text_input_style_closure(theme, v)),
        Space::new().width(8),
        button(
            row![
                fonts::centered_icon(icons::PLUS, 11.0),
                Space::new().width(4),
                text("Add").size(12),
            ]
            .align_y(Vertical::Center),
        )
        .padding([8, 12])
        .style(theme::secondary_style(theme, v))
        .on_press(Message::AddEntryTag),
    ]
    .align_y(Vertical::Center);

    org_col = org_col.push(
        column![
            text("Tags").size(13).color(palette.text_secondary),
            Space::new().height(6),
            tags_row,
            Space::new().height(6),
            tag_input_row,
        ],
    );

    // ── Advanced section (collapsible) ────────────────────────────────
    let advanced_toggle_icon = if show_advanced {
        icons::CHEVRON_DOWN
    } else {
        icons::CHEVRON_RIGHT
    };

    let advanced_header_btn = button(
        row![
            fonts::centered_icon_colored(advanced_toggle_icon, 12.0, palette.text_muted),
            Space::new().width(8),
            fonts::centered_icon_colored(icons::COG, 14.0, palette.text_muted),
            Space::new().width(8),
            text("Advanced").size(14).color(palette.text_primary),
            Space::new().width(Length::Fill),
            text(format!(
                "{} field{}, {} file{}",
                entry_custom_fields.len(),
                if entry_custom_fields.len() == 1 { "" } else { "s" },
                entry_attachments.len(),
                if entry_attachments.len() == 1 { "" } else { "s" },
            ))
            .size(12)
            .color(palette.text_muted),
        ]
        .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding([12, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => {
            let mut s = theme::ghost_button_hovered(theme, v);
            s.border.radius = 8.0.into();
            s
        }
        button::Status::Pressed => {
            let mut s = theme::ghost_button_pressed(theme, v);
            s.border.radius = 8.0.into();
            s
        }
        _ => {
            let mut s = theme::ghost_button(theme, v);
            s.border.radius = 8.0.into();
            s
        }
    })
    .on_press(Message::ToggleAdvancedFields);

    let advanced_section: Element<'static, Message> = if show_advanced {
        let mut advanced_col = column![].spacing(16);

        // Notes
        if vis.notes {
            let notes_input = text_input("Additional notes...", entry_notes)
                .padding(12)
                .size(14)
                .on_input(Message::EntryNotesChanged)
                .style(theme::text_input_style_closure(theme, v));
            advanced_col = advanced_col.push(
                column![
                    row![
                        fonts::centered_icon_colored(icons::EDIT, 13.0, palette.text_muted),
                        Space::new().width(6),
                        text("Notes").size(13).color(palette.text_secondary),
                    ]
                    .align_y(Vertical::Center),
                    Space::new().height(6),
                    notes_input,
                ],
            );
        }

        // Organization (Folder + Tags)
        advanced_col = advanced_col.push(
            column![
                row![
                    fonts::centered_icon_colored(icons::FOLDER, 13.0, palette.text_muted),
                    Space::new().width(6),
                    text("Organization").size(13).color(palette.text_secondary),
                ]
                .align_y(Vertical::Center),
                Space::new().height(6),
                org_col,
            ],
        );

        // Custom fields
        let mut custom_fields_col = column![].spacing(8);
        for (i, (name, value)) in entry_custom_fields.iter().enumerate() {
            let name_owned = name.clone();
            let value_owned = value.clone();
            let idx = i;
            custom_fields_col = custom_fields_col.push(
                row![
                    text_input("Field name", &name_owned)
                        .padding(10)
                        .size(13)
                        .on_input(move |s| Message::CustomFieldNameChanged(idx, s))
                        .style(theme::text_input_style_closure(theme, v)),
                    Space::new().width(8),
                    text_input("Value", &value_owned)
                        .padding(10)
                        .size(13)
                        .on_input(move |s| Message::CustomFieldValueChanged(idx, s))
                        .style(theme::text_input_style_closure(theme, v)),
                    Space::new().width(8),
                    button(
                        fonts::centered_icon_colored(icons::CLOSE, 12.0, palette.danger),
                    )
                    .padding([6, 10])
                    .style(theme::icon_style(theme, v))
                    .on_press(Message::RemoveCustomField(i)),
                ]
                .align_y(Vertical::Center),
            );
        }

        let add_field_btn = button(
            row![
                fonts::centered_icon(icons::PLUS, 12.0),
                Space::new().width(6),
                text("Add Custom Field").size(13),
            ]
            .align_y(Vertical::Center),
        )
        .padding([8, 14])
        .style(theme::secondary_style(theme, v))
        .on_press(Message::AddCustomField);

        advanced_col = advanced_col.push(
            column![
                text("Custom Fields").size(13).color(palette.text_secondary),
                Space::new().height(6),
                custom_fields_col,
                Space::new().height(6),
                add_field_btn,
            ],
        );

        // Attachments
        let mut attachments_col = column![].spacing(6);
        for (i, (filename, data)) in entry_attachments.iter().enumerate() {
            let filename_str = filename.clone();
            let size_bytes = data.len() * 3 / 4;
            let size_display = if size_bytes > 1_048_576 {
                format!("{:.1} MB", size_bytes as f64 / 1_048_576.0)
            } else if size_bytes > 1024 {
                format!("{:.1} KB", size_bytes as f64 / 1024.0)
            } else {
                format!("{} B", size_bytes)
            };

            attachments_col = attachments_col.push(
                container(
                    row![
                        fonts::centered_icon_colored(icons::SAVE, 12.0, palette.text_muted),
                        Space::new().width(8),
                        text(filename_str).size(13).color(palette.text_primary),
                        Space::new().width(8),
                        text(size_display).size(11).color(palette.text_muted),
                        Space::new().width(Length::Fill),
                        button(
                            fonts::centered_icon_colored(icons::CLOSE, 12.0, palette.danger),
                        )
                        .padding([4, 8])
                        .style(theme::icon_style(theme, v))
                        .on_press(Message::RemoveAttachment(i)),
                    ]
                    .align_y(Vertical::Center),
                )
                .padding([6, 10])
                .style(move |_| theme::badge_container(theme, v)),
            );
        }

        let add_attachment_btn = button(
            row![
                fonts::centered_icon(icons::PLUS, 12.0),
                Space::new().width(6),
                text("Add Attachment").size(13),
            ]
            .align_y(Vertical::Center),
        )
        .padding([8, 14])
        .style(theme::secondary_style(theme, v))
        .on_press(Message::AddAttachment);

        advanced_col = advanced_col.push(
            column![
                text("Attachments").size(13).color(palette.text_secondary),
                Space::new().height(6),
                attachments_col,
                Space::new().height(6),
                add_attachment_btn,
            ],
        );

        let sep = separator(theme, v);

        column![
            sep,
            Space::new().height(8),
            advanced_header_btn,
            advanced_col,
        ]
        .into()
    } else {
        let sep = separator(theme, v);

        column![sep, Space::new().height(8), advanced_header_btn,].into()
    };

    // ── Actions ─────────────────────────────────────────────────────────
    let cancel_btn = button(text("Cancel").size(14).color(palette.text_secondary))
        .padding([12, 24])
        .style(theme::ghost_style(theme, v))
        .on_press(Message::HideAddEntry);

    let save_btn = button(
        row![
            fonts::centered_icon(icons::CHECK, 14.0),
            Space::new().width(6),
            text(if edit_mode { "Update" } else { "Save" }).size(14),
        ]
        .align_y(Vertical::Center),
    )
    .padding([12, 24])
    .style(theme::primary_style(theme, v))
    .on_press(Message::SaveEntry);

    let actions_row = row![Space::new().width(Length::Fill), cancel_btn, Space::new().width(12), save_btn,]
        .align_y(Vertical::Center);

    // ── Assemble form ───────────────────────────────────────────────────
    let mut form_col = column![
        form_title,
        Space::new().height(12),
        type_row,
        Space::new().height(16),
        general_section,
    ]
    .spacing(0);

    if let Some(creds) = credentials_section {
        form_col = form_col
            .push(Space::new().height(12))
            .push(creds);
    }

    if let Some(details) = details_section {
        form_col = form_col
            .push(Space::new().height(12))
            .push(details);
    }

    form_col = form_col
        .push(Space::new().height(12))
        .push(advanced_section)
        .push(Space::new().height(16))
        .push(actions_row);

    container(form_col.padding(24))
        .width(Length::Fill)
        .style(move |_| theme::card_container(theme, v))
        .into()
}

