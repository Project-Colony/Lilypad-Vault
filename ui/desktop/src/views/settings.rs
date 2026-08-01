//! Settings Views
//!
//! Account and security settings sections.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, checkbox, column, container, row, scrollable, slider, text, Space};
use iced::{Element, Length};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::theme::{self, LilypadTheme, UiVariation};
use crate::views::common::labeled_input;

/// Render the account settings section
#[allow(clippy::too_many_arguments)]
pub fn account_view(
    theme: LilypadTheme,
    v: UiVariation,
    github_authenticated: bool,
    github_username: Option<&str>,
    sync_in_progress: bool,
    device_flow_code: Option<&str>,
    device_flow_uri: Option<&str>,
    _two_factor_enabled: bool,
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
        v,
        github_authenticated,
        github_username,
        sync_in_progress,
        device_flow_code,
        device_flow_uri,
    );

    // Security preferences
    let security_title = text("Security").size(16).color(palette.text_primary);

    let two_factor_hint = text("Two-factor authentication — coming soon")
        .size(13)
        .color(palette.text_muted);

    let security_section =
        container(column![security_title, Space::new().height(16), two_factor_hint,].padding(24))
            .width(Length::Fill)
            .style(move |_| theme::card_container(theme, v));

    // Preferences
    let prefs_title = text("Preferences").size(16).color(palette.text_primary);

    let marketing_check = checkbox(marketing_opt_in)
        .label("Receive product updates and tips")
        .on_toggle(Message::ToggleMarketingOptIn)
        .text_size(14)
        .spacing(10);

    let prefs_section =
        container(column![prefs_title, Space::new().height(16), marketing_check,].padding(24))
            .width(Length::Fill)
            .style(move |_| theme::card_container(theme, v));

    let content = column![
        title,
        Space::new().height(4),
        subtitle,
        Space::new().height(24),
        github_section,
        Space::new().height(16),
        security_section,
        Space::new().height(16),
        prefs_section,
    ]
    .width(Length::Fixed(560.0));

    container(
        scrollable(content)
            .height(Length::Fill)
            .style(move |_theme, _status| theme::scrollable_style(theme, v)),
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
    v: UiVariation,
    authenticated: bool,
    username: Option<&str>,
    in_progress: bool,
    device_code: Option<&str>,
    device_uri: Option<&str>,
) -> Element<'static, Message> {
    let palette = theme.palette();

    let section_title = text("GitHub Account").size(16).color(palette.text_primary);

    let content: Element<'static, Message> = if authenticated {
        let user_display = username.unwrap_or("Unknown").to_string();
        column![
            row![
                fonts::centered_icon_colored(icons::CHECK, 16.0, palette.success),
                Space::new().width(8),
                text("Connected to GitHub")
                    .size(14)
                    .color(palette.text_primary),
            ]
            .align_y(Vertical::Center),
            Space::new().height(8),
            row![
                text("Username:").size(13).color(palette.text_secondary),
                Space::new().width(8),
                text(user_display).size(13).color(palette.text_primary),
            ],
            Space::new().height(16),
            button(
                container(text("Logout from GitHub").size(14))
                    .width(Length::Fill)
                    .align_x(Horizontal::Center),
            )
            .width(Length::Fill)
            .padding([10, 16])
            .style(move |_theme, status| match status {
                button::Status::Hovered => {
                    let mut style = theme::secondary_button(theme, v);
                    style.text_color = palette.danger;
                    style
                }
                button::Status::Pressed => {
                    let mut style = theme::secondary_button(theme, v);
                    style.text_color = palette.danger;
                    style
                }
                _ => theme::secondary_button(theme, v),
            })
            .on_press(Message::GitHubLogout),
        ]
        .into()
    } else if in_progress {
        let mut items: Vec<Element<'static, Message>> = vec![row![
            fonts::centered_icon(icons::CLOCK, 16.0),
            Space::new().width(8),
            text("Authenticating with GitHub...")
                .size(14)
                .color(palette.text_primary),
        ]
        .align_y(Vertical::Center)
        .into()];

        if let (Some(code), Some(_uri)) = (device_code, device_uri) {
            let code_owned = code.to_string();
            let code_for_copy = code.to_string();
            items.push(Space::new().height(16).into());
            items.push(
                container(
                    column![
                        text("Enter this code at GitHub:")
                            .size(13)
                            .color(palette.text_secondary),
                        Space::new().height(8),
                        button(
                            row![
                                text(code_owned).size(24).color(palette.primary),
                                Space::new().width(12),
                                fonts::centered_icon_colored(icons::COPY, 14.0, palette.text_muted),
                            ]
                            .align_y(Vertical::Center),
                        )
                        .padding([8, 16])
                        .style(move |_theme, status| match status {
                            button::Status::Hovered => {
                                let mut style = theme::ghost_button_hovered(theme, v);
                                style.background = Some(iced::Background::Color(palette.hover));
                                style
                            }
                            button::Status::Pressed => {
                                let mut style = theme::ghost_button_pressed(theme, v);
                                style.background = Some(iced::Background::Color(palette.hover));
                                style
                            }
                            _ => theme::ghost_button(theme, v),
                        })
                        .on_press(Message::CopyToClipboard(code_for_copy)),
                        Space::new().height(4),
                        text("Click code to copy")
                            .size(11)
                            .color(palette.text_muted),
                        Space::new().height(8),
                        button(
                            text("https://github.com/login/device")
                                .size(12)
                                .color(palette.primary)
                        )
                        .padding([4, 8])
                        .style(theme::ghost_style(theme, v))
                        .on_press(Message::OpenExternalLink(
                            "https://github.com/login/device".to_string()
                        )),
                    ]
                    .align_x(Horizontal::Center),
                )
                .width(Length::Fill)
                .padding(16)
                .style(move |_| theme::elevated_container(theme, v))
                .into(),
            );
        }

        column(items).into()
    } else {
        column![
            row![
                fonts::centered_icon_colored(icons::CIRCLE, 16.0, palette.text_muted),
                Space::new().width(8),
                text("Not connected to GitHub")
                    .size(14)
                    .color(palette.text_secondary),
            ]
            .align_y(Vertical::Center),
            Space::new().height(8),
            text("Connect to GitHub to sync your vault across devices securely. All data is encrypted before upload.")
                .size(12)
                .color(palette.text_muted),
            Space::new().height(16),
            button(
                container(
                    row![
                        fonts::centered_icon(icons::GITHUB, 14.0),
                        Space::new().width(8),
                        text("Login with GitHub").size(14),
                    ]
                    .align_y(Vertical::Center),
                )
                .width(Length::Fill)
                .align_x(Horizontal::Center),
            )
            .width(Length::Fill)
            .padding([12, 16])
            .style(theme::primary_style(theme, v))
            .on_press(Message::GitHubLogin),
        ]
        .into()
    };

    container(column![section_title, Space::new().height(16), content,].padding(24))
        .width(Length::Fill)
        .style(move |_| theme::card_container(theme, v))
        .into()
}

/// Render the security settings section
pub fn security_view(
    theme: LilypadTheme,
    v: UiVariation,
    recovery_email: &str,
    _trusted_devices: &[String],
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
    let autolock_title = text("Auto-Lock").size(16).color(palette.text_primary);

    let autolock_value = if auto_lock_minutes == 0 {
        "Disabled".to_string()
    } else {
        format!("{} minutes", auto_lock_minutes)
    };

    let autolock_label = row![
        text("Lock after inactivity")
            .size(14)
            .color(palette.text_secondary),
        Space::new().width(Length::Fill),
        text(autolock_value).size(14).color(palette.text_primary),
    ];

    let autolock_slider =
        slider(0..=60, auto_lock_minutes, Message::ChangeAutoLock).width(Length::Fill);

    let autolock_section = container(
        column![
            autolock_title,
            Space::new().height(16),
            autolock_label,
            Space::new().height(8),
            autolock_slider,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme, v));

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
        Space::new().width(Length::Fill),
        text(clipboard_value).size(14).color(palette.text_primary),
    ];

    let clipboard_slider =
        slider(0..=120, clipboard_timeout, Message::ChangeClipboardTimeout).width(Length::Fill);

    let require_master_check = checkbox(require_master_on_copy)
        .label("Require master password when copying passwords")
        .on_toggle(Message::ToggleRequireMasterOnCopy)
        .text_size(14)
        .spacing(10);

    let clipboard_section = container(
        column![
            clipboard_title,
            Space::new().height(16),
            clipboard_label,
            Space::new().height(8),
            clipboard_slider,
            Space::new().height(16),
            require_master_check,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme, v));

    // Recovery section
    let recovery_title = text("Account Recovery")
        .size(16)
        .color(palette.text_primary);

    let recovery_input = labeled_input(
        theme,
        v,
        "Recovery Email",
        "recovery@email.com",
        recovery_email_owned,
        Message::RecoveryEmailChanged,
    );

    let recovery_section =
        container(column![recovery_title, Space::new().height(16), recovery_input,].padding(24))
            .width(Length::Fill)
            .style(move |_| theme::card_container(theme, v));

    // Master password section
    let master_pw_title = text("Master Password").size(16).color(palette.text_primary);

    let change_pw_btn = button(
        container(
            row![
                fonts::centered_icon(icons::KEY, 14.0),
                Space::new().width(8),
                text("Change Master Password").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([12, 16])
    .style(theme::danger_style(theme, v))
    .on_press(Message::ChangeMasterPassword);

    let master_pw_hint =
        text("Re-encrypts all vault entries with a new key derived from your new password")
            .size(12)
            .color(palette.text_muted);

    let master_pw_section = container(
        column![
            master_pw_title,
            Space::new().height(16),
            change_pw_btn,
            Space::new().height(8),
            master_pw_hint,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme, v));

    // Audit log section
    let audit_title = text("Audit Log").size(16).color(palette.text_primary);

    let audit_btn = button(
        container(
            row![
                fonts::centered_icon(icons::SHIELD, 14.0),
                Space::new().width(8),
                text("View Audit Log").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([12, 16])
    .style(theme::secondary_style(theme, v))
    .on_press(Message::ShowAuditLog);

    let audit_hint = text("View all vault actions: entry additions, updates, deletions, and more")
        .size(12)
        .color(palette.text_muted);

    let audit_section = container(
        column![
            audit_title,
            Space::new().height(16),
            audit_btn,
            Space::new().height(8),
            audit_hint,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme, v));

    // Backup management section
    let backup_title = text("Backup Management")
        .size(16)
        .color(palette.text_primary);

    let backup_btn = button(
        container(
            row![
                fonts::centered_icon(icons::SAVE, 14.0),
                Space::new().width(8),
                text("Create Backup").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 16])
    .style(theme::secondary_style(theme, v))
    .on_press(Message::BackupVault);

    let restore_btn = button(
        container(
            row![
                fonts::centered_icon(icons::UPLOAD, 14.0),
                Space::new().width(8),
                text("Restore").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 16])
    .style(theme::secondary_style(theme, v))
    .on_press(Message::RestoreVault);

    let prune_btn = button(
        container(
            row![
                fonts::centered_icon(icons::TRASH, 14.0),
                Space::new().width(8),
                text("Prune Old").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 16])
    .style(theme::danger_style(theme, v))
    .on_press(Message::PruneBackups);

    let backup_actions = row![
        backup_btn,
        Space::new().width(8),
        restore_btn,
        Space::new().width(8),
        prune_btn,
    ];

    let backup_hint = text("Prune removes old backups, keeping the 5 most recent per vault")
        .size(12)
        .color(palette.text_muted);

    let backup_section = container(
        column![
            backup_title,
            Space::new().height(16),
            backup_actions,
            Space::new().height(8),
            backup_hint,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme, v));

    // Trusted devices — coming soon
    let devices_title = text("Trusted Devices").size(16).color(palette.text_primary);

    let devices_hint = text("Device management — coming soon")
        .size(13)
        .color(palette.text_muted);

    let devices_section =
        container(column![devices_title, Space::new().height(16), devices_hint].padding(24))
            .width(Length::Fill)
            .style(move |_| theme::card_container(theme, v));

    let content = column![
        title,
        Space::new().height(4),
        subtitle,
        Space::new().height(24),
        autolock_section,
        Space::new().height(16),
        clipboard_section,
        Space::new().height(16),
        master_pw_section,
        Space::new().height(16),
        audit_section,
        Space::new().height(16),
        backup_section,
        Space::new().height(16),
        recovery_section,
        Space::new().height(16),
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
    .style(move |_theme, _status| theme::scrollable_style(theme, v))
    .into()
}
