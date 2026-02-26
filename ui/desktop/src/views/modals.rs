//! Modal Dialogs
//!
//! Reusable modal dialog components.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::theme::{self, LilypadTheme};

/// Render the settings modal
pub fn settings_modal<'a>(theme: LilypadTheme) -> Element<'a, Message> {
    let current_theme = theme;
    let palette = theme.palette();

    let title = text("Settings")
        .size(20)
        .color(palette.text_primary);

    let close_btn = button(
        text(icons::CLOSE).size(16).font(fonts::FONT_REGULAR).color(palette.text_muted),
    )
    .padding([8, 12])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
    })
    .on_press(Message::HideSettings);

    let header = row![title, Space::with_width(Length::Fill), close_btn,].align_y(Vertical::Center);

    // Theme selection
    let theme_label = text("Theme")
        .size(14)
        .color(palette.text_secondary);

    let theme_buttons: Vec<Element<'a, Message>> = LilypadTheme::ALL
        .iter()
        .map(|t| {
            let is_active = *t == current_theme;
            button(
                text(t.name())
                    .size(14)
                    .color(if is_active {
                        palette.primary
                    } else {
                        palette.text_primary
                    }),
            )
            .padding([10, 16])
            .style(move |_theme, status| {
                if is_active {
                    let mut style = theme::secondary_button(theme);
                    style.background = Some(iced::Background::Color(palette.hover));
                    style
                } else {
                    match status {
                        button::Status::Hovered => theme::ghost_button_hovered(theme),
                        _ => theme::ghost_button(theme),
                    }
                }
            })
            .on_press(Message::ChangeTheme(*t))
            .into()
        })
        .collect();

    let theme_row = row(theme_buttons).spacing(8);

    let content = column![
        header,
        Space::with_height(24),
        theme_label,
        Space::with_height(8),
        theme_row,
        Space::with_height(24),
        button(
            container(text("Close").size(14))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::secondary_button_hovered(theme),
            _ => theme::secondary_button(theme),
        })
        .on_press(Message::HideSettings),
    ]
    .padding(24)
    .width(Length::Fixed(400.0));

    wrap_modal(theme, content)
}

/// Render the new vault modal
pub fn new_vault_modal<'a>(theme: LilypadTheme, vault_name: &str) -> Element<'a, Message> {
    let palette = theme.palette();

    let title = text("Create New Vault")
        .size(20)
        .color(palette.text_primary);

    let close_btn = button(
        text(icons::CLOSE).size(16).font(fonts::FONT_REGULAR).color(palette.text_muted),
    )
    .padding([8, 12])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
    })
    .on_press(Message::HideNewVaultModal);

    let header = row![title, Space::with_width(Length::Fill), close_btn,].align_y(Vertical::Center);

    let name_label = text("Vault Name")
        .size(14)
        .color(palette.text_secondary);

    let name_input = text_input("e.g., Personal, Work", vault_name)
        .padding(14)
        .size(14)
        .on_input(Message::NewVaultNameChanged)
        .on_submit(Message::CreateVault)
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
        .on_press(Message::HideNewVaultModal);

    let create_btn = button(text("Create Vault").size(14))
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::primary_button_hovered(theme),
            button::Status::Disabled => {
                let mut style = theme::primary_button(theme);
                style.background = Some(iced::Background::Color(palette.border));
                style
            }
            _ => theme::primary_button(theme),
        })
        .on_press_maybe(if vault_name.trim().is_empty() {
            None
        } else {
            Some(Message::CreateVault)
        });

    let actions = row![Space::with_width(Length::Fill), cancel_btn, Space::with_width(12), create_btn,]
        .align_y(Vertical::Center);

    let content = column![
        header,
        Space::with_height(24),
        name_label,
        Space::with_height(8),
        name_input,
        Space::with_height(24),
        actions,
    ]
    .padding(24)
    .width(Length::Fixed(420.0));

    wrap_modal(theme, content)
}

/// Render the delete confirmation modal
pub fn delete_confirm_modal<'a>(theme: LilypadTheme, entry_title: &str) -> Element<'a, Message> {
    let palette = theme.palette();

    let icon = text(icons::TRIANGLE_EXCLAMATION)
        .size(48)
        .font(fonts::FONT_REGULAR)
        .color(palette.warning);

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
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::ghost_button_hovered(theme),
        _ => theme::ghost_button(theme),
    })
    .on_press(Message::CancelDelete);

    let delete_btn = button(
        container(text("Delete").size(14))
            .width(Length::Fill)
            .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 24])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::danger_button_hovered(theme),
        _ => theme::danger_button(theme),
    })
    .on_press(Message::ConfirmDelete);

    let actions = row![cancel_btn, Space::with_width(12), delete_btn,];

    let content = column![
        icon,
        Space::with_height(16),
        title,
        Space::with_height(12),
        message,
        Space::with_height(24),
        actions,
    ]
    .align_x(Horizontal::Center)
    .padding(32)
    .width(Length::Fixed(400.0));

    wrap_modal(theme, content)
}

/// Render the re-authentication modal
pub fn reauth_modal<'a>(theme: LilypadTheme, reauth_password: &str) -> Element<'a, Message> {
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
        .on_press(Message::CancelReauth);

    let confirm_btn = button(text("Confirm").size(14))
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::primary_button_hovered(theme),
            button::Status::Disabled => {
                let mut style = theme::primary_button(theme);
                style.background = Some(iced::Background::Color(palette.border));
                style
            }
            _ => theme::primary_button(theme),
        })
        .on_press_maybe(if reauth_password.is_empty() {
            None
        } else {
            Some(Message::ConfirmReauth)
        });

    let actions = row![Space::with_width(Length::Fill), cancel_btn, Space::with_width(12), confirm_btn,]
        .align_y(Vertical::Center);

    let content = column![
        text(icons::VAULT).size(48).font(fonts::FONT_REGULAR),
        Space::with_height(16),
        title,
        Space::with_height(8),
        message,
        Space::with_height(24),
        password_input,
        Space::with_height(24),
        actions,
    ]
    .align_x(Horizontal::Center)
    .padding(32)
    .width(Length::Fixed(400.0));

    wrap_modal(theme, content)
}

/// Render the change master password modal
pub fn change_password_modal<'a>(
    theme: LilypadTheme,
    new_password: &str,
) -> Element<'a, Message> {
    let palette = theme.palette();

    let icon = text(icons::KEY)
        .size(48)
        .font(fonts::FONT_REGULAR)
        .color(palette.primary);

    let title = text("Change Master Password")
        .size(20)
        .color(palette.text_primary);

    let message = text("Enter a new master password. All entries will be re-encrypted with the new key.")
        .size(14)
        .color(palette.text_secondary);

    let warning = row![
        text(icons::TRIANGLE_EXCLAMATION)
            .size(13)
            .font(fonts::FONT_REGULAR)
            .color(palette.warning),
        Space::with_width(8),
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
        .on_press(Message::CancelChangeMasterPassword);

    let confirm_btn = button(text("Change Password").size(14))
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::danger_button_hovered(theme),
            button::Status::Disabled => {
                let mut style = theme::danger_button(theme);
                style.background = Some(iced::Background::Color(palette.border));
                style
            }
            _ => theme::danger_button(theme),
        })
        .on_press_maybe(if new_password.trim().len() < 4 {
            None
        } else {
            Some(Message::ConfirmChangeMasterPassword)
        });

    let actions = row![Space::with_width(Length::Fill), cancel_btn, Space::with_width(12), confirm_btn,]
        .align_y(Vertical::Center);

    let content = column![
        icon,
        Space::with_height(16),
        title,
        Space::with_height(8),
        message,
        Space::with_height(16),
        warning,
        Space::with_height(16),
        password_input,
        Space::with_height(24),
        actions,
    ]
    .align_x(Horizontal::Center)
    .padding(32)
    .width(Length::Fixed(440.0));

    wrap_modal(theme, content)
}

/// Render the entry history overlay
pub fn entry_history_modal<'a>(
    theme: LilypadTheme,
    history_entries: &[(u64, String)],
) -> Element<'a, Message> {
    let palette = theme.palette();

    let icon = text(icons::CLOCK)
        .size(32)
        .font(fonts::FONT_REGULAR)
        .color(palette.primary);

    let title = text("Entry History")
        .size(20)
        .color(palette.text_primary);

    let close_btn = button(
        text(icons::CLOSE).size(16).font(fonts::FONT_REGULAR).color(palette.text_muted),
    )
    .padding([8, 12])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
    })
    .on_press(Message::CloseEntryHistory);

    let header = row![icon, Space::with_width(12), title, Space::with_width(Length::Fill), close_btn,]
        .align_y(Vertical::Center);

    let history_content: Element<'a, Message> = if history_entries.is_empty() {
        container(
            column![
                text(icons::CLOCK)
                    .size(32)
                    .font(fonts::FONT_REGULAR)
                    .color(palette.text_muted),
                Space::with_height(12),
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
                                Space::with_width(Length::Fill),
                                text(relative_time)
                                    .size(12)
                                    .color(palette.text_muted),
                            ],
                            Space::with_height(6),
                            text(pwd).size(13).color(palette.text_muted),
                        ]
                        .width(Length::Fill),
                    ]
                    .padding(12),
                )
                .width(Length::Fill)
                .style(move |_| theme::elevated_container(theme))
                .into()
            })
            .collect();

        scrollable(column(items).spacing(8))
            .height(Length::Fixed(300.0))
            .style(move |_theme, _status| theme::scrollable_style(theme))
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
        Space::with_height(16),
        hint,
        Space::with_height(12),
        history_content,
        Space::with_height(16),
        button(
            container(text("Close").size(14))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::secondary_button_hovered(theme),
            _ => theme::secondary_button(theme),
        })
        .on_press(Message::CloseEntryHistory),
    ]
    .padding(24)
    .width(Length::Fixed(480.0));

    wrap_modal(theme, content)
}

/// Render the audit log modal
pub fn audit_log_modal<'a>(
    theme: LilypadTheme,
    audit_events: &[(u64, String, Option<String>)],
) -> Element<'a, Message> {
    let palette = theme.palette();

    let icon = text(icons::SHIELD)
        .size(32)
        .font(fonts::FONT_REGULAR)
        .color(palette.primary);

    let title = text("Audit Log")
        .size(20)
        .color(palette.text_primary);

    let close_btn = button(
        text(icons::CLOSE).size(16).font(fonts::FONT_REGULAR).color(palette.text_muted),
    )
    .padding([8, 12])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::icon_button_hovered(theme),
        _ => theme::icon_button(theme),
    })
    .on_press(Message::CloseAuditLog);

    let header = row![icon, Space::with_width(12), title, Space::with_width(Length::Fill), close_btn,]
        .align_y(Vertical::Center);

    let log_content: Element<'a, Message> = if audit_events.is_empty() {
        container(
            column![
                text(icons::SHIELD)
                    .size(32)
                    .font(fonts::FONT_REGULAR)
                    .color(palette.text_muted),
                Space::with_height(12),
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
                        Space::with_width(Length::Fill),
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
                    .style(move |_| theme::elevated_container(theme))
                    .into()
            })
            .collect();

        scrollable(column(items).spacing(6))
            .height(Length::Fixed(400.0))
            .style(move |_theme, _status| theme::scrollable_style(theme))
            .into()
    };

    let count_text = text(format!(
        "{} event{}",
        audit_events.len(),
        if audit_events.len() == 1 { "" } else { "s" }
    ))
    .size(12)
    .color(palette.text_muted);

    let content = column![
        header,
        Space::with_height(12),
        count_text,
        Space::with_height(12),
        log_content,
        Space::with_height(16),
        button(
            container(text("Close").size(14))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding([12, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::secondary_button_hovered(theme),
            _ => theme::secondary_button(theme),
        })
        .on_press(Message::CloseAuditLog),
    ]
    .padding(24)
    .width(Length::Fixed(520.0));

    wrap_modal(theme, content)
}

/// Format audit action strings for display
fn format_audit_action(action: &str) -> String {
    match action {
        "entry_added" => "Entry added".to_string(),
        "entry_updated" => "Entry updated".to_string(),
        "entry_removed" => "Entry removed".to_string(),
        "entry_renamed" => "Entry renamed".to_string(),
        "entry_metadata_updated" => "Entry metadata updated".to_string(),
        "entry_folder_updated" => "Entry folder updated".to_string(),
        "entry_tag_added" => "Tag added".to_string(),
        "entry_tag_removed" => "Tag removed".to_string(),
        "vault_accessed" => "Vault accessed".to_string(),
        "vault_description_updated" => "Vault description updated".to_string(),
        "bulk_folder_updated" => "Bulk folder update".to_string(),
        "bulk_tag_added" => "Bulk tag added".to_string(),
        "bulk_tag_removed" => "Bulk tag removed".to_string(),
        other => other.replace('_', " "),
    }
}

/// Helper: wrap content in a centered modal overlay
fn wrap_modal<'a>(
    theme: LilypadTheme,
    content: iced::widget::Column<'a, Message>,
) -> Element<'a, Message> {
    let modal_content = container(content).style(move |_| theme::modal_container(theme));

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
