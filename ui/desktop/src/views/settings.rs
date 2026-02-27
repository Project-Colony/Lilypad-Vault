//! Settings Views
//!
//! Account and security settings sections.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, checkbox, column, container, row, scrollable, slider, text, text_input, Space};
use iced::{Element, Length};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::theme::{self, LilypadTheme};

/// Render the account settings section
pub fn account_view(
    theme: LilypadTheme,
    github_authenticated: bool,
    github_username: Option<&str>,
    sync_in_progress: bool,
    device_flow_code: Option<&str>,
    device_flow_uri: Option<&str>,
    two_factor_enabled: bool,
    marketing_opt_in: bool,
) -> Element<'static, Message> {
    let palette = theme.palette();

    let title = text("Account Settings")
        .size(24)
        .color(palette.text_primary);

    let subtitle = text("Manage your account connection and preferences")
        .size(14)
        .color(palette.text_secondary);

    // GitHub Account section
    let github_section = github_auth_section(
        theme,
        github_authenticated,
        github_username,
        sync_in_progress,
        device_flow_code,
        device_flow_uri,
    );

    // Security preferences
    let security_title = text("Security")
        .size(16)
        .color(palette.text_primary);

    let two_factor_check = checkbox("Two-Factor Authentication", two_factor_enabled)
        .on_toggle(Message::ToggleTwoFactor)
        .text_size(14)
        .spacing(10);

    let two_factor_hint = text("Add an extra layer of security to your account")
        .size(12)
        .color(palette.text_muted);

    let security_section = container(
        column![
            security_title,
            Space::with_height(16),
            two_factor_check,
            Space::with_height(4),
            two_factor_hint,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    // Preferences
    let prefs_title = text("Preferences")
        .size(16)
        .color(palette.text_primary);

    let marketing_check = checkbox("Receive product updates and tips", marketing_opt_in)
        .on_toggle(Message::ToggleMarketingOptIn)
        .text_size(14)
        .spacing(10);

    let prefs_section = container(
        column![prefs_title, Space::with_height(16), marketing_check,].padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    let content = column![
        title,
        Space::with_height(4),
        subtitle,
        Space::with_height(24),
        github_section,
        Space::with_height(16),
        security_section,
        Space::with_height(16),
        prefs_section,
    ]
    .width(Length::Fixed(560.0));

    container(
        scrollable(content)
            .height(Length::Fill)
            .style(move |_theme, _status| theme::scrollable_style(theme)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(24)
    .align_x(Horizontal::Center)
    .into()
}

/// GitHub authentication card
fn github_auth_section(
    theme: LilypadTheme,
    authenticated: bool,
    username: Option<&str>,
    in_progress: bool,
    device_code: Option<&str>,
    device_uri: Option<&str>,
) -> Element<'static, Message> {
    let palette = theme.palette();

    let section_title = text("GitHub Account")
        .size(16)
        .color(palette.text_primary);

    let content: Element<'static, Message> = if authenticated {
        let user_display = username.unwrap_or("Unknown").to_string();
        column![
            row![
                fonts::centered_icon_colored(icons::CHECK, 16.0, palette.success),
                Space::with_width(8),
                text("Connected to GitHub")
                    .size(14)
                    .color(palette.text_primary),
            ]
            .align_y(Vertical::Center),
            Space::with_height(8),
            row![
                text("Username:").size(13).color(palette.text_secondary),
                Space::with_width(8),
                text(user_display).size(13).color(palette.text_primary),
            ],
            Space::with_height(16),
            button(
                container(
                    text("Logout from GitHub").size(14)
                )
                .width(Length::Fill)
                .align_x(Horizontal::Center),
            )
            .width(Length::Fill)
            .padding([10, 16])
            .style(move |_theme, status| match status {
                button::Status::Hovered => {
                    let mut style = theme::secondary_button(theme);
                    style.text_color = palette.danger;
                    style
                }
                _ => theme::secondary_button(theme),
            })
            .on_press(Message::GitHubLogout),
        ]
        .into()
    } else if in_progress {
        let mut items: Vec<Element<'static, Message>> = vec![
            row![
                fonts::centered_icon(icons::CLOCK, 16.0),
                Space::with_width(8),
                text("Authenticating with GitHub...")
                    .size(14)
                    .color(palette.text_primary),
            ]
            .align_y(Vertical::Center)
            .into(),
        ];

        if let (Some(code), Some(uri)) = (device_code, device_uri) {
            let code_owned = code.to_string();
            let code_for_copy = code.to_string();
            let _uri_owned = uri.to_string();
            items.push(Space::with_height(16).into());
            items.push(
                container(
                    column![
                        text("Enter this code at GitHub:")
                            .size(13)
                            .color(palette.text_secondary),
                        Space::with_height(8),
                        button(
                            row![
                                text(code_owned)
                                    .size(24)
                                    .color(palette.primary),
                                Space::with_width(12),
                                fonts::centered_icon_colored(icons::COPY, 14.0, palette.text_muted),
                            ]
                            .align_y(Vertical::Center),
                        )
                        .padding([8, 16])
                        .style(move |_theme, status| match status {
                            button::Status::Hovered => {
                                let mut style = theme::ghost_button_hovered(theme);
                                style.background = Some(iced::Background::Color(palette.hover));
                                style
                            }
                            _ => theme::ghost_button(theme),
                        })
                        .on_press(Message::CopyToClipboard(code_for_copy)),
                        Space::with_height(4),
                        text("Click code to copy")
                            .size(11)
                            .color(palette.text_muted),
                        Space::with_height(8),
                        button(
                            text("https://github.com/login/device").size(12).color(palette.primary)
                        )
                        .padding([4, 8])
                        .style(move |_theme, status| match status {
                            button::Status::Hovered => theme::ghost_button_hovered(theme),
                            _ => theme::ghost_button(theme),
                        })
                        .on_press(Message::OpenExternalLink(
                            "https://github.com/login/device".to_string()
                        )),
                    ]
                    .align_x(Horizontal::Center),
                )
                .width(Length::Fill)
                .padding(16)
                .style(move |_| theme::elevated_container(theme))
                .into(),
            );
        }

        column(items).into()
    } else {
        column![
            row![
                fonts::centered_icon_colored(icons::CIRCLE, 16.0, palette.text_muted),
                Space::with_width(8),
                text("Not connected to GitHub")
                    .size(14)
                    .color(palette.text_secondary),
            ]
            .align_y(Vertical::Center),
            Space::with_height(8),
            text("Connect to GitHub to sync your vault across devices securely. All data is encrypted before upload.")
                .size(12)
                .color(palette.text_muted),
            Space::with_height(16),
            button(
                container(
                    row![
                        fonts::centered_icon(icons::GITHUB, 14.0),
                        Space::with_width(8),
                        text("Login with GitHub").size(14),
                    ]
                    .align_y(Vertical::Center),
                )
                .width(Length::Fill)
                .align_x(Horizontal::Center),
            )
            .width(Length::Fill)
            .padding([12, 16])
            .style(move |_theme, status| match status {
                button::Status::Hovered => theme::primary_button_hovered(theme),
                _ => theme::primary_button(theme),
            })
            .on_press(Message::GitHubLogin),
        ]
        .into()
    };

    container(
        column![section_title, Space::with_height(16), content,].padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme))
    .into()
}

/// Render the security settings section
pub fn security_view(
    theme: LilypadTheme,
    recovery_email: &str,
    trusted_devices: &[String],
    auto_lock_minutes: u32,
    clipboard_timeout: u32,
    require_master_on_copy: bool,
) -> Element<'static, Message> {
    let palette = theme.palette();
    let recovery_email_owned = recovery_email.to_string();

    let title = text("Security Settings")
        .size(24)
        .color(palette.text_primary);

    let subtitle = text("Configure security options for your vault")
        .size(14)
        .color(palette.text_secondary);

    // Auto-lock section
    let autolock_title = text("Auto-Lock")
        .size(16)
        .color(palette.text_primary);

    let autolock_value = if auto_lock_minutes == 0 {
        "Disabled".to_string()
    } else {
        format!("{} minutes", auto_lock_minutes)
    };

    let autolock_label = row![
        text("Lock after inactivity")
            .size(14)
            .color(palette.text_secondary),
        Space::with_width(Length::Fill),
        text(autolock_value).size(14).color(palette.text_primary),
    ];

    let autolock_slider = slider(0..=60, auto_lock_minutes, Message::ChangeAutoLock).width(Length::Fill);

    let autolock_section = container(
        column![
            autolock_title,
            Space::with_height(16),
            autolock_label,
            Space::with_height(8),
            autolock_slider,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    // Clipboard section
    let clipboard_title = text("Clipboard Security")
        .size(16)
        .color(palette.text_primary);

    let clipboard_value = if clipboard_timeout == 0 {
        "Never".to_string()
    } else {
        format!("{} seconds", clipboard_timeout)
    };

    let clipboard_label = row![
        text("Clear clipboard after")
            .size(14)
            .color(palette.text_secondary),
        Space::with_width(Length::Fill),
        text(clipboard_value).size(14).color(palette.text_primary),
    ];

    let clipboard_slider =
        slider(0..=120, clipboard_timeout, Message::ChangeClipboardTimeout).width(Length::Fill);

    let require_master_check = checkbox(
        "Require master password when copying passwords",
        require_master_on_copy,
    )
    .on_toggle(Message::ToggleRequireMasterOnCopy)
    .text_size(14)
    .spacing(10);

    let clipboard_section = container(
        column![
            clipboard_title,
            Space::with_height(16),
            clipboard_label,
            Space::with_height(8),
            clipboard_slider,
            Space::with_height(16),
            require_master_check,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    // Recovery section
    let recovery_title = text("Account Recovery")
        .size(16)
        .color(palette.text_primary);

    let recovery_input = labeled_input(
        theme,
        "Recovery Email",
        "recovery@email.com",
        recovery_email_owned,
        Message::RecoveryEmailChanged,
    );

    let recovery_section = container(
        column![recovery_title, Space::with_height(16), recovery_input,].padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    // Master password section
    let master_pw_title = text("Master Password")
        .size(16)
        .color(palette.text_primary);

    let change_pw_btn = button(
        container(
            row![
                fonts::centered_icon(icons::KEY, 14.0),
                Space::with_width(8),
                text("Change Master Password").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([12, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::danger_button_hovered(theme),
        _ => theme::danger_button(theme),
    })
    .on_press(Message::ChangeMasterPassword);

    let master_pw_hint = text("Re-encrypts all vault entries with a new key derived from your new password")
        .size(12)
        .color(palette.text_muted);

    let master_pw_section = container(
        column![
            master_pw_title,
            Space::with_height(16),
            change_pw_btn,
            Space::with_height(8),
            master_pw_hint,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    // Audit log section
    let audit_title = text("Audit Log")
        .size(16)
        .color(palette.text_primary);

    let audit_btn = button(
        container(
            row![
                fonts::centered_icon(icons::SHIELD, 14.0),
                Space::with_width(8),
                text("View Audit Log").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([12, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::secondary_button_hovered(theme),
        _ => theme::secondary_button(theme),
    })
    .on_press(Message::ShowAuditLog);

    let audit_hint = text("View all vault actions: entry additions, updates, deletions, and more")
        .size(12)
        .color(palette.text_muted);

    let audit_section = container(
        column![
            audit_title,
            Space::with_height(16),
            audit_btn,
            Space::with_height(8),
            audit_hint,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    // Backup management section
    let backup_title = text("Backup Management")
        .size(16)
        .color(palette.text_primary);

    let backup_btn = button(
        container(
            row![
                fonts::centered_icon(icons::SAVE, 14.0),
                Space::with_width(8),
                text("Create Backup").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::secondary_button_hovered(theme),
        _ => theme::secondary_button(theme),
    })
    .on_press(Message::BackupVault);

    let restore_btn = button(
        container(
            row![
                fonts::centered_icon(icons::UPLOAD, 14.0),
                Space::with_width(8),
                text("Restore").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::secondary_button_hovered(theme),
        _ => theme::secondary_button(theme),
    })
    .on_press(Message::RestoreVault);

    let prune_btn = button(
        container(
            row![
                fonts::centered_icon(icons::TRASH, 14.0),
                Space::with_width(8),
                text("Prune Old").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::danger_button_hovered(theme),
        _ => theme::danger_button(theme),
    })
    .on_press(Message::PruneBackups);

    let backup_actions = row![
        backup_btn,
        Space::with_width(8),
        restore_btn,
        Space::with_width(8),
        prune_btn,
    ];

    let backup_hint = text("Prune removes old backups, keeping the 5 most recent per vault")
        .size(12)
        .color(palette.text_muted);

    let backup_section = container(
        column![
            backup_title,
            Space::with_height(16),
            backup_actions,
            Space::with_height(8),
            backup_hint,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    // Trusted devices section
    let devices_title = text("Trusted Devices")
        .size(16)
        .color(palette.text_primary);

    let devices_list: Vec<Element<'static, Message>> = trusted_devices
        .iter()
        .enumerate()
        .map(|(index, device)| {
            let device_name = device.clone();
            row![
                fonts::centered_icon(icons::DESKTOP, 16.0),
                Space::with_width(12),
                text(device_name).size(14).color(palette.text_primary),
                Space::with_width(Length::Fill),
                button(text("Remove").size(12).color(palette.danger))
                    .padding([6, 12])
                    .style(move |_theme, status| match status {
                        button::Status::Hovered => {
                            let mut style = theme::ghost_button_hovered(theme);
                            style.text_color = palette.danger;
                            style
                        }
                        _ => theme::ghost_button(theme),
                    })
                    .on_press(Message::RemoveTrustedDevice(index)),
            ]
            .align_y(Vertical::Center)
            .padding([8, 0])
            .into()
        })
        .collect();

    let devices_content: Element<'static, Message> = if devices_list.is_empty() {
        text("No trusted devices")
            .size(14)
            .color(palette.text_muted)
            .into()
    } else {
        column(devices_list).spacing(8).into()
    };

    let devices_section = container(
        column![devices_title, Space::with_height(16), devices_content,].padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    let content = column![
        title,
        Space::with_height(4),
        subtitle,
        Space::with_height(24),
        autolock_section,
        Space::with_height(16),
        clipboard_section,
        Space::with_height(16),
        master_pw_section,
        Space::with_height(16),
        audit_section,
        Space::with_height(16),
        backup_section,
        Space::with_height(16),
        recovery_section,
        Space::with_height(16),
        devices_section,
    ]
    .width(Length::Fixed(560.0));

    scrollable(
        container(content)
            .width(Length::Fill)
            .padding(24)
            .align_x(Horizontal::Center),
    )
    .height(Length::Fill)
    .style(move |_theme, _status| theme::scrollable_style(theme))
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
