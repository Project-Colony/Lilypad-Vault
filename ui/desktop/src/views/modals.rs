//! Modal Dialogs
//!
//! Reusable modal dialog components.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::theme::{self, LilypadTheme, UiVariation};
use crate::views::common::{modal_header, modal_header_with_icon, padded_separator};

/// Compute hue (0..360) from an iced Color for sorting themes by color.
fn color_hue(c: iced::Color) -> f32 {
    let r = c.r;
    let g = c.g;
    let b = c.b;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    if delta < 0.001 {
        return 0.0; // achromatic
    }
    let hue = if (max - r).abs() < 0.001 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 0.001 {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    if hue < 0.0 { hue + 360.0 } else { hue }
}

/// Render the settings modal
pub fn settings_modal<'a>(theme: LilypadTheme, v: UiVariation, sort_by_color: bool) -> Element<'a, Message> {
    let current_theme = theme;
    let palette = theme.palette();

    let header = modal_header(theme, v, "Settings", Message::HideSettings);

    // Theme label + sort toggle
    let theme_label = text("Theme")
        .size(14)
        .color(palette.text_secondary);

    let sort_label = if sort_by_color { "Color" } else { "A-Z" };
    let sort_btn = button(
        row![
            fonts::centered_icon(icons::SORT, 12.0),
            Space::new().width(6),
            text(sort_label).size(12).color(palette.text_secondary),
        ]
        .align_y(Vertical::Center),
    )
    .padding([6, 10])
    .style(theme::ghost_style(theme, v))
    .on_press(Message::ToggleThemeSort);

    let theme_header = row![
        theme_label,
        Space::new().width(Length::Fill),
        sort_btn,
    ]
    .align_y(Vertical::Center);

    // Sort themes
    let mut themes: Vec<LilypadTheme> = LilypadTheme::ALL.to_vec();
    if sort_by_color {
        themes.sort_by(|a, b| {
            let ha = color_hue(a.palette().primary);
            let hb = color_hue(b.palette().primary);
            ha.partial_cmp(&hb).unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        themes.sort_by(|a, b| a.name().cmp(b.name()));
    }

    // Build theme grid: 2 per row
    let mut theme_rows: Vec<Element<'a, Message>> = Vec::new();
    for pair in themes.chunks(2) {
        let mut r = Vec::new();
        for t in pair {
            let is_active = *t == current_theme;
            let t_palette = t.palette();
            let theme_btn = button(
                row![
                    container(Space::new().width(Length::Fixed(12.0)).height(Length::Fixed(12.0)))
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(t_palette.primary)),
                            border: iced::Border {
                                radius: 6.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                    Space::new().width(10),
                    text(t.name())
                        .size(13)
                        .color(if is_active {
                            palette.primary
                        } else {
                            palette.text_primary
                        }),
                ]
                .align_y(Vertical::Center),
            )
            .width(Length::FillPortion(1))
            .padding([10, 14])
            .style(move |_theme, status| {
                if is_active {
                    let mut style = theme::secondary_button(theme, v);
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
            .on_press(Message::ChangeTheme(*t));
            r.push(theme_btn.into());
            r.push(Space::new().width(8).into());
        }
        // If odd number, add spacer for second column
        if pair.len() == 1 {
            r.push(Space::new().width(Length::FillPortion(1)).into());
            r.push(Space::new().width(8).into());
        }
        theme_rows.push(row(r).into());
        theme_rows.push(Space::new().height(8).into());
    }

    // ── UI Variation picker ───────────────────────────────────────────
    let variation_divider = padded_separator(theme, v);

    let variation_label = text("UI Style")
        .size(14)
        .color(palette.text_secondary);

    let current_variation = v;
    let mut variation_rows: Vec<Element<'a, Message>> = Vec::new();
    for pair in UiVariation::ALL.chunks(2) {
        let mut r = Vec::new();
        for variation in pair {
            let is_active = *variation == current_variation;
            let var_name = variation.name();
            let var_desc = variation.description();
            let var_copy = *variation;
            let variation_btn = button(
                column![
                    text(var_name)
                        .size(13)
                        .color(if is_active {
                            palette.primary
                        } else {
                            palette.text_primary
                        }),
                    text(var_desc)
                        .size(10)
                        .color(palette.text_muted),
                ]
                .spacing(2),
            )
            .width(Length::FillPortion(1))
            .padding([10, 14])
            .style(move |_theme, status| {
                if is_active {
                    let mut style = theme::secondary_button(theme, v);
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
            .on_press(Message::ChangeUiVariation(var_copy));
            r.push(variation_btn.into());
            r.push(Space::new().width(8).into());
        }
        if pair.len() == 1 {
            r.push(Space::new().width(Length::FillPortion(1)).into());
            r.push(Space::new().width(8).into());
        }
        variation_rows.push(row(r).into());
        variation_rows.push(Space::new().height(8).into());
    }

    let variation_grid = column(variation_rows);

    // Single scrollable containing both themes and UI style
    let combined_grid = scrollable(
        column![
            column(theme_rows),
            variation_divider,
            Space::new().height(8),
            variation_label,
            Space::new().height(8),
            variation_grid,
        ],
    )
    .height(Length::Fixed(320.0))
    .style(move |_theme, _status| theme::scrollable_style(theme, v));

    let content = column![
        header,
        Space::new().height(24),
        theme_header,
        Space::new().height(8),
        combined_grid,
        Space::new().height(16),
        button(
            container(text("Close").size(14))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([12, 24])
        .style(theme::secondary_style(theme, v))
        .on_press(Message::HideSettings),
    ]
    .padding(24)
    .width(Length::Fixed(440.0));

    wrap_modal(theme, v, content)
}

/// Render the new vault modal
pub fn new_vault_modal<'a>(theme: LilypadTheme, v: UiVariation, vault_name: &str) -> Element<'a, Message> {
    let palette = theme.palette();

    let header = modal_header(theme, v, "Create New Vault", Message::HideNewVaultModal);

    let name_label = text("Vault Name")
        .size(14)
        .color(palette.text_secondary);

    let name_input = text_input("e.g., Personal, Work", vault_name)
        .padding(14)
        .size(14)
        .on_input(Message::NewVaultNameChanged)
        .on_submit(Message::CreateVault)
        .style(theme::text_input_style_closure(theme, v));

    let cancel_btn = button(text("Cancel").size(14).color(palette.text_secondary))
        .padding([12, 24])
        .style(theme::ghost_style(theme, v))
        .on_press(Message::HideNewVaultModal);

    let create_btn = button(text("Create Vault").size(14))
        .padding([12, 24])
        .style(theme::primary_disabled_style(theme, v))
        .on_press_maybe(if vault_name.trim().is_empty() {
            None
        } else {
            Some(Message::CreateVault)
        });

    let actions = row![Space::new().width(Length::Fill), cancel_btn, Space::new().width(12), create_btn,]
        .align_y(Vertical::Center);

    let content = column![
        header,
        Space::new().height(24),
        name_label,
        Space::new().height(8),
        name_input,
        Space::new().height(24),
        actions,
    ]
    .padding(24)
    .width(Length::Fixed(420.0));

    wrap_modal(theme, v, content)
}

/// Render the rename vault modal
pub fn rename_vault_modal<'a>(theme: LilypadTheme, v: UiVariation, vault_name: &str) -> Element<'a, Message> {
    let palette = theme.palette();

    let header = modal_header(theme, v, "Rename Vault", Message::CancelRenameVault);

    let name_label = text("New Name")
        .size(14)
        .color(palette.text_secondary);

    let name_input = text_input("Enter new vault name...", vault_name)
        .padding(14)
        .size(14)
        .on_input(Message::RenameVaultNameChanged)
        .on_submit(Message::ConfirmRenameVault)
        .style(theme::text_input_style_closure(theme, v));

    let cancel_btn = button(text("Cancel").size(14).color(palette.text_secondary))
        .padding([12, 24])
        .style(theme::ghost_style(theme, v))
        .on_press(Message::CancelRenameVault);

    let rename_btn = button(text("Rename").size(14))
        .padding([12, 24])
        .style(theme::primary_disabled_style(theme, v))
        .on_press_maybe(if vault_name.trim().is_empty() {
            None
        } else {
            Some(Message::ConfirmRenameVault)
        });

    let actions = row![Space::new().width(Length::Fill), cancel_btn, Space::new().width(12), rename_btn,]
        .align_y(Vertical::Center);

    let content = column![
        header,
        Space::new().height(24),
        name_label,
        Space::new().height(8),
        name_input,
        Space::new().height(24),
        actions,
    ]
    .padding(24)
    .width(Length::Fixed(420.0));

    wrap_modal(theme, v, content)
}

/// Render the delete confirmation modal
pub fn delete_confirm_modal<'a>(theme: LilypadTheme, v: UiVariation, entry_title: &str) -> Element<'a, Message> {
    let palette = theme.palette();

    let icon = fonts::centered_icon_colored(icons::TRIANGLE_EXCLAMATION, 48.0, palette.warning);

    let title = text("Delete Entry?")
        .size(20)
        .color(palette.text_primary);

    let message = text(format!(
        "Are you sure you want to delete \"{}\"? This action cannot be undone.",
        entry_title
    ))
    .size(14)
    .color(palette.text_secondary);

    let cancel_btn = button(
        container(text("Cancel").size(14).color(palette.text_primary))
            .width(Length::Fill)
            .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 24])
    .style(theme::ghost_style(theme, v))
    .on_press(Message::CancelDelete);

    let delete_btn = button(
        container(text("Delete").size(14))
            .width(Length::Fill)
            .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 24])
    .style(theme::danger_style(theme, v))
    .on_press(Message::ConfirmDelete);

    let actions = row![cancel_btn, Space::new().width(12), delete_btn,];

    let content = column![
        icon,
        Space::new().height(16),
        title,
        Space::new().height(12),
        message,
        Space::new().height(24),
        actions,
    ]
    .align_x(Horizontal::Center)
    .padding(32)
    .width(Length::Fixed(400.0));

    wrap_modal(theme, v, content)
}

/// Render the re-authentication modal
pub fn reauth_modal<'a>(theme: LilypadTheme, v: UiVariation, reauth_password: &str) -> Element<'a, Message> {
    let palette = theme.palette();

    let title = text("Confirm Your Identity")
        .size(20)
        .color(palette.text_primary);

    let message = text("Enter your master password to copy this password.")
        .size(14)
        .color(palette.text_secondary);

    let password_input = text_input("Master password", reauth_password)
        .padding(14)
        .size(14)
        .secure(true)
        .on_input(Message::ReauthPasswordChanged)
        .on_submit(Message::ConfirmReauth)
        .style(theme::text_input_style_closure(theme, v));

    let cancel_btn = button(text("Cancel").size(14).color(palette.text_secondary))
        .padding([12, 24])
        .style(theme::ghost_style(theme, v))
        .on_press(Message::CancelReauth);

    let confirm_btn = button(text("Confirm").size(14))
        .padding([12, 24])
        .style(theme::primary_disabled_style(theme, v))
        .on_press_maybe(if reauth_password.is_empty() {
            None
        } else {
            Some(Message::ConfirmReauth)
        });

    let actions = row![Space::new().width(Length::Fill), cancel_btn, Space::new().width(12), confirm_btn,]
        .align_y(Vertical::Center);

    let content = column![
        fonts::centered_icon(icons::VAULT, 48.0),
        Space::new().height(16),
        title,
        Space::new().height(8),
        message,
        Space::new().height(24),
        password_input,
        Space::new().height(24),
        actions,
    ]
    .align_x(Horizontal::Center)
    .padding(32)
    .width(Length::Fixed(400.0));

    wrap_modal(theme, v, content)
}

/// Render the change master password modal
pub fn change_password_modal<'a>(
    theme: LilypadTheme,
    v: UiVariation,
    new_password: &str,
) -> Element<'a, Message> {
    let palette = theme.palette();

    let icon = fonts::centered_icon_colored(icons::KEY, 48.0, palette.primary);

    let title = text("Change Master Password")
        .size(20)
        .color(palette.text_primary);

    let message = text("Enter a new master password. All entries will be re-encrypted with the new key.")
        .size(14)
        .color(palette.text_secondary);

    let warning = row![
        fonts::centered_icon_colored(icons::TRIANGLE_EXCLAMATION, 13.0, palette.warning),
        Space::new().width(8),
        text("Make sure you remember this password. If you forget it, your vault data cannot be recovered.")
            .size(12)
            .color(palette.warning),
    ]
    .align_y(Vertical::Center);

    let password_input = text_input("New master password", new_password)
        .padding(14)
        .size(14)
        .secure(true)
        .on_input(Message::NewMasterPasswordChanged)
        .on_submit(Message::ConfirmChangeMasterPassword)
        .style(theme::text_input_style_closure(theme, v));

    let cancel_btn = button(text("Cancel").size(14).color(palette.text_secondary))
        .padding([12, 24])
        .style(theme::ghost_style(theme, v))
        .on_press(Message::CancelChangeMasterPassword);

    let confirm_btn = button(text("Change Password").size(14))
        .padding([12, 24])
        .style(theme::danger_disabled_style(theme, v))
        .on_press_maybe(if new_password.trim().len() < 4 {
            None
        } else {
            Some(Message::ConfirmChangeMasterPassword)
        });

    let actions = row![Space::new().width(Length::Fill), cancel_btn, Space::new().width(12), confirm_btn,]
        .align_y(Vertical::Center);

    let content = column![
        icon,
        Space::new().height(16),
        title,
        Space::new().height(8),
        message,
        Space::new().height(16),
        warning,
        Space::new().height(16),
        password_input,
        Space::new().height(24),
        actions,
    ]
    .align_x(Horizontal::Center)
    .padding(32)
    .width(Length::Fixed(440.0));

    wrap_modal(theme, v, content)
}

/// Render the entry history overlay
pub fn entry_history_modal<'a>(
    theme: LilypadTheme,
    v: UiVariation,
    history_entries: &[(u64, String)],
) -> Element<'a, Message> {
    let palette = theme.palette();

    let header = modal_header_with_icon(theme, v, icons::CLOCK, 32.0, "Entry History", Message::CloseEntryHistory);

    let history_content: Element<'a, Message> = if history_entries.is_empty() {
        container(
            column![
                fonts::centered_icon_colored(icons::CLOCK, 32.0, palette.text_muted),
                Space::new().height(12),
                text("No history available")
                    .size(14)
                    .color(palette.text_muted),
            ]
            .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding(32)
        .align_x(Horizontal::Center)
        .into()
    } else {
        let items: Vec<Element<'a, Message>> = history_entries
            .iter()
            .enumerate()
            .map(|(i, (timestamp, password_masked))| {
                let ts = *timestamp;
                let pwd = password_masked.clone();
                let relative_time =
                    lilypad_common::time::format_timestamp_relative(ts);
                let is_latest = i == 0;

                container(
                    row![
                        column![
                            row![
                                text(if is_latest { "Current" } else { "Previous" })
                                    .size(13)
                                    .color(if is_latest {
                                        palette.primary
                                    } else {
                                        palette.text_secondary
                                    }),
                                Space::new().width(Length::Fill),
                                text(relative_time)
                                    .size(12)
                                    .color(palette.text_muted),
                            ],
                            Space::new().height(6),
                            text(pwd).size(13).color(palette.text_muted),
                        ]
                        .width(Length::Fill),
                    ]
                    .padding(12),
                )
                .width(Length::Fill)
                .style(move |_| theme::elevated_container(theme, v))
                .into()
            })
            .collect();

        scrollable(column(items).spacing(8))
            .height(Length::Fixed(300.0))
            .style(move |_theme, _status| theme::scrollable_style(theme, v))
            .into()
    };

    let hint = text(format!(
        "{} version{}",
        history_entries.len(),
        if history_entries.len() == 1 { "" } else { "s" }
    ))
    .size(12)
    .color(palette.text_muted);

    let content = column![
        header,
        Space::new().height(16),
        hint,
        Space::new().height(12),
        history_content,
        Space::new().height(16),
        button(
            container(text("Close").size(14))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([12, 24])
        .style(theme::secondary_style(theme, v))
        .on_press(Message::CloseEntryHistory),
    ]
    .padding(24)
    .width(Length::Fixed(480.0));

    wrap_modal(theme, v, content)
}

/// Render the audit log modal
pub fn audit_log_modal<'a>(
    theme: LilypadTheme,
    v: UiVariation,
    audit_events: &[(u64, String, Option<String>)],
) -> Element<'a, Message> {
    let palette = theme.palette();

    let header = modal_header_with_icon(theme, v, icons::SHIELD, 32.0, "Audit Log", Message::CloseAuditLog);

    let log_content: Element<'a, Message> = if audit_events.is_empty() {
        container(
            column![
                fonts::centered_icon_colored(icons::SHIELD, 32.0, palette.text_muted),
                Space::new().height(12),
                text("No events recorded yet")
                    .size(14)
                    .color(palette.text_muted),
            ]
            .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding(32)
        .align_x(Horizontal::Center)
        .into()
    } else {
        let items: Vec<Element<'a, Message>> = audit_events
            .iter()
            .map(|(timestamp, action, entry_label)| {
                let ts = *timestamp;
                let action_display = format_audit_action(action);
                let relative_time = lilypad_common::time::format_timestamp_relative(ts);

                let mut info_col = column![
                    row![
                        text(action_display)
                            .size(13)
                            .color(palette.text_primary),
                        Space::new().width(Length::Fill),
                        text(relative_time)
                            .size(11)
                            .color(palette.text_muted),
                    ],
                ];

                if let Some(label) = entry_label {
                    info_col = info_col.push(
                        text(label.clone())
                            .size(11)
                            .color(palette.text_muted),
                    );
                }

                container(info_col.spacing(2).padding(10))
                    .width(Length::Fill)
                    .style(move |_| theme::elevated_container(theme, v))
                    .into()
            })
            .collect();

        scrollable(column(items).spacing(6))
            .height(Length::Fixed(400.0))
            .style(move |_theme, _status| theme::scrollable_style(theme, v))
            .into()
    };

    let count_text = text(format!(
        "{} event{}",
        audit_events.len(),
        if audit_events.len() == 1 { "" } else { "s" }
    ))
    .size(12)
    .color(palette.text_muted);

    let has_events = !audit_events.is_empty();

    let export_json_btn = button(
        row![
            fonts::centered_icon(icons::DOWNLOAD, 12.0),
            Space::new().width(6),
            text("JSON").size(12),
        ]
        .align_y(Vertical::Center),
    )
    .padding([8, 14])
    .style(theme::ghost_style(theme, v))
    .on_press_maybe(if has_events {
        Some(Message::ExportAuditLogJson)
    } else {
        None
    });

    let export_csv_btn = button(
        row![
            fonts::centered_icon(icons::DOWNLOAD, 12.0),
            Space::new().width(6),
            text("CSV").size(12),
        ]
        .align_y(Vertical::Center),
    )
    .padding([8, 14])
    .style(theme::ghost_style(theme, v))
    .on_press_maybe(if has_events {
        Some(Message::ExportAuditLogCsv)
    } else {
        None
    });

    let export_txt_btn = button(
        row![
            fonts::centered_icon(icons::DOWNLOAD, 12.0),
            Space::new().width(6),
            text("Text").size(12),
        ]
        .align_y(Vertical::Center),
    )
    .padding([8, 14])
    .style(theme::ghost_style(theme, v))
    .on_press_maybe(if has_events {
        Some(Message::ExportAuditLogText)
    } else {
        None
    });

    let export_label = text("Export:")
        .size(12)
        .color(palette.text_muted);

    let export_row = row![
        export_label,
        Space::new().width(8),
        export_json_btn,
        Space::new().width(4),
        export_csv_btn,
        Space::new().width(4),
        export_txt_btn,
    ]
    .align_y(Vertical::Center);

    let content = column![
        header,
        Space::new().height(12),
        row![count_text, Space::new().width(Length::Fill), export_row,].align_y(Vertical::Center),
        Space::new().height(12),
        log_content,
        Space::new().height(16),
        button(
            container(text("Close").size(14))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([12, 24])
        .style(theme::secondary_style(theme, v))
        .on_press(Message::CloseAuditLog),
    ]
    .padding(24)
    .width(Length::Fixed(560.0));

    wrap_modal(theme, v, content)
}

/// Format audit action strings for display
fn format_audit_action(action: &str) -> String {
    const LABELS: &[(&str, &str)] = &[
        ("entry_added", "Entry added"),
        ("entry_updated", "Entry updated"),
        ("entry_removed", "Entry removed"),
        ("entry_renamed", "Entry renamed"),
        ("entry_metadata_updated", "Entry metadata updated"),
        ("entry_folder_updated", "Entry folder updated"),
        ("entry_tag_added", "Tag added"),
        ("entry_tag_removed", "Tag removed"),
        ("vault_accessed", "Vault accessed"),
        ("vault_description_updated", "Vault description updated"),
        ("bulk_folder_updated", "Bulk folder update"),
        ("bulk_tag_added", "Bulk tag added"),
        ("bulk_tag_removed", "Bulk tag removed"),
    ];
    LABELS
        .iter()
        .find(|(k, _)| *k == action)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| action.replace('_', " "))
}

/// Helper: wrap content in a centered modal overlay
fn wrap_modal<'a>(
    theme: LilypadTheme,
    v: UiVariation,
    content: iced::widget::Column<'a, Message>,
) -> Element<'a, Message> {
    let modal_content = container(content).style(move |_| theme::modal_container(theme, v));

    let centered = container(modal_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

    container(centered)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| theme::modal_overlay(theme, v))
        .into()
}
