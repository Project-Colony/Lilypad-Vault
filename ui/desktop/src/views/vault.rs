//! Vault View
//!
//! Displays the list of credentials/entries with filtering and actions.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length, Padding};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::state::{VaultEntry, VaultViewMode};
use crate::theme::{self, LilypadTheme};

/// Parameters for the vault view.
pub struct VaultViewParams<'a> {
    pub theme: LilypadTheme,
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
}

/// Render the vault entries section
pub fn view(params: VaultViewParams<'_>) -> Element<'static, Message> {
    let VaultViewParams {
        theme,
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
    } = params;
    let palette = theme.palette();

    // Filter entries based on search and view mode
    let filtered_entries: Vec<(usize, &VaultEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            // Search filter
            let query_lower = search_query.to_lowercase();
            let matches_search = search_query.is_empty()
                || entry.title.to_lowercase().contains(&query_lower)
                || entry.username.to_lowercase().contains(&query_lower)
                || entry.url.to_lowercase().contains(&query_lower)
                || entry.tags.iter().any(|t| t.to_lowercase().contains(&query_lower));

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

            // Folder filter
            let matches_folder = match folder_filter {
                Some(f) => entry.folder.as_deref() == Some(f),
                None => true,
            };

            matches_search && matches_mode && matches_folder
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

    // Folder filter chips
    let mut folders: Vec<String> = entries
        .iter()
        .filter_map(|e| e.folder.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    folders.sort();

    let folder_filter_owned: Option<String> = folder_filter.map(|s| s.to_string());
    let no_filter = folder_filter_owned.is_none();

    let folder_row: Option<Element<'static, Message>> = if !folders.is_empty() {
        let mut folder_chips = row![
            button(
                text("All folders")
                    .size(11)
                    .color(if no_filter {
                        palette.primary
                    } else {
                        palette.text_muted
                    }),
            )
            .padding([4, 10])
            .style(move |_theme, status| {
                if no_filter {
                    let mut style = theme::ghost_button(theme);
                    style.background = Some(iced::Background::Color(palette.hover));
                    style.text_color = palette.primary;
                    style
                } else {
                    match status {
                        button::Status::Hovered => theme::ghost_button_hovered(theme),
                        _ => theme::ghost_button(theme),
                    }
                }
            })
            .on_press(Message::FilterByFolder(None)),
        ]
        .spacing(4)
        .align_y(Vertical::Center);

        for folder in &folders {
            let folder_name = folder.clone();
            let is_active = folder_filter_owned.as_deref() == Some(folder.as_str());
            folder_chips = folder_chips.push(
                button(
                    row![
                        fonts::centered_icon(icons::FOLDER, 10.0),
                        Space::with_width(4),
                        text(folder_name.clone()).size(11),
                    ]
                    .align_y(Vertical::Center),
                )
                .padding([4, 10])
                .style(move |_theme, status| {
                    if is_active {
                        let mut style = theme::ghost_button(theme);
                        style.background = Some(iced::Background::Color(palette.hover));
                        style.text_color = palette.primary;
                        style
                    } else {
                        match status {
                            button::Status::Hovered => theme::ghost_button_hovered(theme),
                            _ => theme::ghost_button(theme),
                        }
                    }
                })
                .on_press(Message::FilterByFolder(Some(folder_name))),
            );
        }

        Some(folder_chips.into())
    } else {
        None
    };

    let header_row = row![tabs_row, Space::with_width(Length::Fill), count_text,]
        .align_y(Vertical::Center)
        .padding(Padding::new(0.0).bottom(16.0));

    // Entry list or empty state
    let entries_content: Element<'static, Message> = if filtered_entries.is_empty() {
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

        column(entry_cards).spacing(12).into()
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
            entry_email,
            entry_phone,
            entry_folder,
            entry_tags,
            entry_new_tag,
            entry_totp_secret,
            entry_custom_fields,
            entry_type,
            entry_attachments,
            show_advanced_fields,
        ))
    } else {
        None
    };

    let mut main_column = column![].spacing(0);

    if let Some(form) = form_content {
        main_column = main_column
            .push(form)
            .push(Space::with_height(24));
    }

    if let Some(folder_element) = folder_row {
        main_column = main_column
            .push(folder_element)
            .push(Space::with_height(8));
    }

    main_column = main_column
        .push(header_row)
        .push(entries_content);

    let main_content: Element<'static, Message> = scrollable(main_column)
        .height(Length::Fill)
        .style(move |_theme, _status| theme::scrollable_style(theme))
        .into();

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
        let color_rgb = theme::entry_color_to_iced(color);
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
    let star_color = if is_fav { palette.warning } else { palette.text_muted };
    let favorite_btn = button(
        fonts::centered_icon_colored(icons::STAR, 16.0, star_color),
    )
    .padding([4, 8])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
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
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(palette.surface_variant)),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    // Title and username
    let title_str = entry.title.clone();
    let username_str = entry.username.clone();
    let title_text = text(title_str)
        .size(15)
        .color(palette.text_primary);

    let username_text = text(username_str)
        .size(13)
        .color(palette.text_secondary);

    let mut info_column = column![title_text, username_text,].spacing(2);

    // Show tags as small badges and folder/TOTP/type indicators
    let has_tags = !entry.tags.is_empty();
    let has_folder = entry.folder.is_some();
    let has_totp = entry.totp_secret.is_some();
    let is_non_login = !matches!(entry.entry_type, lilypad_core::EntryType::Login);
    let has_attachments = !entry.attachments.is_empty();

    if has_tags || has_folder || has_totp || is_non_login || has_attachments {
        let mut badge_row = row![].spacing(4).align_y(Vertical::Center);

        // Entry type badge (only show for non-Login types)
        if is_non_login {
            let type_label = format!("{:?}", entry.entry_type);
            badge_row = badge_row.push(
                container(text(type_label).size(10).color(palette.text_secondary))
                    .padding([2, 6])
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(palette.surface_variant)),
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            );
        }

        if has_attachments {
            let count = entry.attachments.len();
            badge_row = badge_row.push(
                container(
                    row![
                        fonts::centered_icon(icons::SAVE, 9.0),
                        Space::with_width(3),
                        text(format!("{}", count)).size(10),
                    ]
                    .align_y(Vertical::Center),
                )
                .padding([2, 6])
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(palette.surface_variant)),
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
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
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(iced::Color {
                            a: 0.15,
                            ..palette.primary
                        })),
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            );
        }

        if let Some(ref folder) = entry.folder {
            let folder_str = folder.clone();
            badge_row = badge_row.push(
                text(format!("/{}", folder_str))
                    .size(10)
                    .color(palette.text_muted),
            );
        }

        for tag in &entry.tags {
            let tag_str = tag.clone();
            badge_row = badge_row.push(
                container(text(tag_str).size(10).color(palette.text_muted))
                    .padding([1, 5])
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(palette.surface_variant)),
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            );
        }

        info_column = info_column.push(Space::with_height(2));
        info_column = info_column.push(badge_row);
    }

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
    let copy_user_btn = button(
        fonts::centered_icon(icons::USER, 14.0),
    )
    .padding([6, 10])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
    })
    .on_press(Message::CopyUsername(index));

    let copy_pass_btn = button(
        fonts::centered_icon(icons::KEY, 14.0),
    )
    .padding([6, 10])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
    })
    .on_press(Message::CopyPassword(index));

    let has_url = !entry.url.is_empty();
    let open_url_btn: Option<Element<'static, Message>> = if has_url {
        Some(
            button(
                fonts::centered_icon(icons::EXTERNAL_LINK, 14.0),
            )
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

    let history_btn = button(
        fonts::centered_icon(icons::CLOCK, 14.0),
    )
    .padding([6, 10])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
    })
    .on_press(Message::ViewEntryHistory(index));

    let edit_btn = button(
        fonts::centered_icon(icons::EDIT, 14.0),
    )
    .padding([6, 10])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
    })
    .on_press(Message::EditEntry(index));

    let delete_btn = button(
        fonts::centered_icon(icons::TRASH, 14.0),
    )
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
    actions_row = actions_row
        .push(history_btn)
        .push(edit_btn)
        .push(delete_btn);

    let mut card_row = row![].align_y(Vertical::Center).padding(16);

    if let Some(indicator) = color_indicator {
        card_row = card_row.push(indicator);
        card_row = card_row.push(Space::with_width(12));
    }

    card_row = card_row
        .push(type_icon)
        .push(Space::with_width(10))
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
    icon: &'static str,
    title: &'static str,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let palette = theme.palette();
    let header = row![
        fonts::centered_icon_colored(icon, 13.0, palette.text_muted),
        Space::with_width(8),
        text(title).size(14).color(palette.text_primary),
    ]
    .align_y(Vertical::Center);

    // Separator line
    let separator = container(Space::new(Length::Fill, Length::Fixed(1.0)))
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(palette.surface_variant)),
            ..Default::default()
        });

    column![separator, Space::with_height(12), header, Space::with_height(12), content,]
        .into()
}

/// Render the add/edit entry form
#[allow(clippy::too_many_arguments)]
fn entry_form(
    theme: LilypadTheme,
    edit_mode: bool,
    entry_title: &str,
    entry_username: &str,
    entry_password: &str,
    entry_url: &str,
    entry_notes: &str,
    entry_email: &str,
    entry_phone: &str,
    entry_folder: &str,
    entry_tags: &[String],
    entry_new_tag: &str,
    entry_totp_secret: &str,
    entry_custom_fields: &[(String, String)],
    entry_type: &str,
    entry_attachments: &[(String, String)],
    show_advanced: bool,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let vis = fields_for_type(entry_type);
    let (username_label, username_placeholder, password_label, url_label) = type_labels(entry_type);

    // ── Header ──────────────────────────────────────────────────────────
    let form_title = row![
        text(type_icon(entry_type)).size(20).font(fonts::FONT_REGULAR).color(palette.primary),
        Space::with_width(10),
        text(if edit_mode { "Edit Entry" } else { "New Entry" })
            .size(18)
            .color(palette.text_primary),
        Space::with_width(Length::Fill),
        button(
            fonts::centered_icon_colored(icons::CLOSE, 14.0, palette.text_muted),
        )
        .padding([6, 10])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::icon_button_hovered(theme),
            _ => theme::icon_button(theme),
        })
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
                    Space::with_width(5),
                    text(display).size(12),
                ]
                .align_y(Vertical::Center),
            )
            .padding([6, 10])
            .style(move |_theme, status| {
                if is_active {
                    let mut style = theme::ghost_button(theme);
                    style.background = Some(iced::Background::Color(palette.hover));
                    style.text_color = palette.primary;
                    style
                } else {
                    match status {
                        button::Status::Hovered => theme::ghost_button_hovered(theme),
                        _ => theme::ghost_button(theme),
                    }
                }
            })
            .on_press(Message::EntryTypeChanged(type_str))
            .into()
        })
        .collect();

    let type_row = row(type_buttons).spacing(4);

    // ── Section 1: General ──────────────────────────────────────────────
    let title_input = labeled_input(theme, "Title", "e.g., GitHub", entry_title.to_string(), |s| {
        Message::EntryTitleChanged(s)
    });

    let general_section = form_section(theme, icons::INFO, "General", title_input);

    // ── Section 2: Credentials ──────────────────────────────────────────
    let has_credentials = vis.username || vis.email || vis.password;
    let credentials_section: Option<Element<'static, Message>> = if has_credentials {
        let mut creds_col = column![].spacing(12);

        // Row 1: Username + Email side by side (if both visible)
        if vis.username && vis.email {
            creds_col = creds_col.push(
                row![
                    column![labeled_input(
                        theme, username_label, username_placeholder,
                        entry_username.to_string(), Message::EntryUsernameChanged,
                    )]
                    .width(Length::Fill),
                    Space::with_width(12),
                    column![labeled_input(
                        theme, "Email", "e.g., john@example.com",
                        entry_email.to_string(), Message::EntryEmailChanged,
                    )]
                    .width(Length::Fill),
                ],
            );
        } else if vis.username {
            creds_col = creds_col.push(labeled_input(
                theme, username_label, username_placeholder,
                entry_username.to_string(), Message::EntryUsernameChanged,
            ));
        } else if vis.email {
            creds_col = creds_col.push(labeled_input(
                theme, "Email", "e.g., john@example.com",
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
                            row![
                                fonts::centered_icon(icons::DICE, 14.0),
                                Space::with_width(6),
                                text("Generate").size(13),
                            ]
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
                .align_y(Vertical::Bottom),
            );
        }

        Some(form_section(theme, icons::KEY, "Credentials", creds_col.into()))
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
                        theme, url_lbl, "e.g., https://github.com",
                        entry_url.to_string(), Message::EntryUrlChanged,
                    )]
                    .width(Length::Fill),
                    Space::with_width(12),
                    column![labeled_input(
                        theme, "Phone", "e.g., +33 6 12 34 56 78",
                        entry_phone.to_string(), Message::EntryPhoneChanged,
                    )]
                    .width(Length::Fill),
                ],
            );
        } else if vis.url {
            let url_lbl = if url_label.is_empty() { "URL" } else { url_label };
            details_col = details_col.push(labeled_input(
                theme, url_lbl, "e.g., https://github.com",
                entry_url.to_string(), Message::EntryUrlChanged,
            ));
        } else if vis.phone {
            details_col = details_col.push(labeled_input(
                theme, "Phone", "e.g., +33 6 12 34 56 78",
                entry_phone.to_string(), Message::EntryPhoneChanged,
            ));
        }

        if vis.totp {
            details_col = details_col.push(labeled_input(
                theme, "TOTP Secret", "e.g., JBSWY3DPEHPK3PXP",
                entry_totp_secret.to_string(), Message::EntryTotpSecretChanged,
            ));
        }

        Some(form_section(theme, icons::GLOBE, "Details", details_col.into()))
    } else {
        None
    };

    // ── Organization + Notes content (moved into Advanced) ────────────
    let mut org_col = column![].spacing(12);
    org_col = org_col.push(labeled_input(
        theme, "Folder", "e.g., Work/Email",
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
                    Space::with_width(4),
                    button(fonts::centered_icon_colored(icons::CLOSE, 9.0, palette.text_muted))
                        .padding([2, 4])
                        .style(move |_theme, status| match status {
                            button::Status::Hovered => theme::icon_button_hovered(theme),
                            _ => theme::icon_button(theme),
                        })
                        .on_press(Message::RemoveEntryTag(i)),
                ]
                .align_y(Vertical::Center),
            )
            .padding([3, 8])
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(palette.surface_variant)),
                border: iced::Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            }),
        );
    }

    let tag_input_row = row![
        text_input("Add tag...", entry_new_tag)
            .padding(10)
            .size(13)
            .on_input(Message::EntryNewTagChanged)
            .on_submit(Message::AddEntryTag)
            .style(move |_theme, status| match status {
                text_input::Status::Focused => theme::text_input_focused(theme),
                _ => theme::text_input_style(theme),
            }),
        Space::with_width(8),
        button(
            row![
                fonts::centered_icon(icons::PLUS, 11.0),
                Space::with_width(4),
                text("Add").size(12),
            ]
            .align_y(Vertical::Center),
        )
        .padding([8, 12])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::secondary_button_hovered(theme),
            _ => theme::secondary_button(theme),
        })
        .on_press(Message::AddEntryTag),
    ]
    .align_y(Vertical::Center);

    org_col = org_col.push(
        column![
            text("Tags").size(13).color(palette.text_secondary),
            Space::with_height(6),
            tags_row,
            Space::with_height(6),
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
            Space::with_width(8),
            fonts::centered_icon_colored(icons::COG, 14.0, palette.text_muted),
            Space::with_width(8),
            text("Advanced").size(14).color(palette.text_primary),
            Space::with_width(Length::Fill),
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
            let mut s = theme::ghost_button_hovered(theme);
            s.border.radius = 8.0.into();
            s
        }
        _ => {
            let mut s = theme::ghost_button(theme);
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
                .style(move |_theme, status| match status {
                    text_input::Status::Focused => theme::text_input_focused(theme),
                    _ => theme::text_input_style(theme),
                });
            advanced_col = advanced_col.push(
                column![
                    row![
                        fonts::centered_icon_colored(icons::EDIT, 13.0, palette.text_muted),
                        Space::with_width(6),
                        text("Notes").size(13).color(palette.text_secondary),
                    ]
                    .align_y(Vertical::Center),
                    Space::with_height(6),
                    notes_input,
                ],
            );
        }

        // Organization (Folder + Tags)
        advanced_col = advanced_col.push(
            column![
                row![
                    fonts::centered_icon_colored(icons::FOLDER, 13.0, palette.text_muted),
                    Space::with_width(6),
                    text("Organization").size(13).color(palette.text_secondary),
                ]
                .align_y(Vertical::Center),
                Space::with_height(6),
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
                        .style(move |_theme, status| match status {
                            text_input::Status::Focused => theme::text_input_focused(theme),
                            _ => theme::text_input_style(theme),
                        }),
                    Space::with_width(8),
                    text_input("Value", &value_owned)
                        .padding(10)
                        .size(13)
                        .on_input(move |s| Message::CustomFieldValueChanged(idx, s))
                        .style(move |_theme, status| match status {
                            text_input::Status::Focused => theme::text_input_focused(theme),
                            _ => theme::text_input_style(theme),
                        }),
                    Space::with_width(8),
                    button(
                        fonts::centered_icon_colored(icons::CLOSE, 12.0, palette.danger),
                    )
                    .padding([6, 10])
                    .style(move |_theme, status| match status {
                        button::Status::Hovered => theme::icon_button_hovered(theme),
                        _ => theme::icon_button(theme),
                    })
                    .on_press(Message::RemoveCustomField(i)),
                ]
                .align_y(Vertical::Center),
            );
        }

        let add_field_btn = button(
            row![
                fonts::centered_icon(icons::PLUS, 12.0),
                Space::with_width(6),
                text("Add Custom Field").size(13),
            ]
            .align_y(Vertical::Center),
        )
        .padding([8, 14])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::secondary_button_hovered(theme),
            _ => theme::secondary_button(theme),
        })
        .on_press(Message::AddCustomField);

        advanced_col = advanced_col.push(
            column![
                text("Custom Fields").size(13).color(palette.text_secondary),
                Space::with_height(6),
                custom_fields_col,
                Space::with_height(6),
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
                        Space::with_width(8),
                        text(filename_str).size(13).color(palette.text_primary),
                        Space::with_width(8),
                        text(size_display).size(11).color(palette.text_muted),
                        Space::with_width(Length::Fill),
                        button(
                            fonts::centered_icon_colored(icons::CLOSE, 12.0, palette.danger),
                        )
                        .padding([4, 8])
                        .style(move |_theme, status| match status {
                            button::Status::Hovered => theme::icon_button_hovered(theme),
                            _ => theme::icon_button(theme),
                        })
                        .on_press(Message::RemoveAttachment(i)),
                    ]
                    .align_y(Vertical::Center),
                )
                .padding([6, 10])
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(palette.surface_variant)),
                    border: iced::Border { radius: 4.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            );
        }

        let add_attachment_btn = button(
            row![
                fonts::centered_icon(icons::PLUS, 12.0),
                Space::with_width(6),
                text("Add Attachment").size(13),
            ]
            .align_y(Vertical::Center),
        )
        .padding([8, 14])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::secondary_button_hovered(theme),
            _ => theme::secondary_button(theme),
        })
        .on_press(Message::AddAttachment);

        advanced_col = advanced_col.push(
            column![
                text("Attachments").size(13).color(palette.text_secondary),
                Space::with_height(6),
                attachments_col,
                Space::with_height(6),
                add_attachment_btn,
            ],
        );

        let separator = container(Space::new(Length::Fill, Length::Fixed(1.0)))
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(palette.surface_variant)),
                ..Default::default()
            });

        column![
            separator,
            Space::with_height(8),
            advanced_header_btn,
            advanced_col,
        ]
        .into()
    } else {
        let separator = container(Space::new(Length::Fill, Length::Fixed(1.0)))
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(palette.surface_variant)),
                ..Default::default()
            });

        column![separator, Space::with_height(8), advanced_header_btn,].into()
    };

    // ── Actions ─────────────────────────────────────────────────────────
    let cancel_btn = button(text("Cancel").size(14).color(palette.text_secondary))
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::ghost_button_hovered(theme),
            _ => theme::ghost_button(theme),
        })
        .on_press(Message::HideAddEntry);

    let save_btn = button(
        row![
            fonts::centered_icon(icons::CHECK, 14.0),
            Space::with_width(6),
            text(if edit_mode { "Update" } else { "Save" }).size(14),
        ]
        .align_y(Vertical::Center),
    )
    .padding([12, 24])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme),
        _ => theme::primary_button(theme),
    })
    .on_press(Message::SaveEntry);

    let actions_row = row![Space::with_width(Length::Fill), cancel_btn, Space::with_width(12), save_btn,]
        .align_y(Vertical::Center);

    // ── Assemble form ───────────────────────────────────────────────────
    let mut form_col = column![
        form_title,
        Space::with_height(12),
        type_row,
        Space::with_height(16),
        general_section,
    ]
    .spacing(0);

    if let Some(creds) = credentials_section {
        form_col = form_col
            .push(Space::with_height(12))
            .push(creds);
    }

    if let Some(details) = details_section {
        form_col = form_col
            .push(Space::with_height(12))
            .push(details);
    }

    form_col = form_col
        .push(Space::with_height(12))
        .push(advanced_section)
        .push(Space::with_height(16))
        .push(actions_row);

    container(form_col.padding(24))
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
