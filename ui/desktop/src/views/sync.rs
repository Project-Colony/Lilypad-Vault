//! GitHub Sync View
//!
//! UI for GitHub OAuth login, vault sync (push/pull), and export/import.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length};

use crate::message::Message;
use crate::theme::{self, LilypadTheme};

/// Render the GitHub sync & data management section
pub fn view(
    theme: LilypadTheme,
    github_authenticated: bool,
    github_username: Option<&str>,
    sync_in_progress: bool,
    device_flow_code: Option<&str>,
    device_flow_uri: Option<&str>,
) -> Element<'static, Message> {
    let palette = theme.palette();

    let title = text("Sync & Data")
        .size(24)
        .color(palette.text_primary);

    let subtitle = text("Manage vault synchronization and data export/import")
        .size(14)
        .color(palette.text_secondary);

    // GitHub OAuth Section
    let github_section = github_auth_section(
        theme,
        github_authenticated,
        github_username,
        sync_in_progress,
        device_flow_code,
        device_flow_uri,
    );

    // Sync Actions (only if authenticated and vault unlocked)
    let sync_section = if github_authenticated {
        sync_actions_section(theme, sync_in_progress)
    } else {
        Space::new(0, 0).into()
    };

    // Export/Import Section
    let data_section = data_management_section(theme);

    let content = column![
        title,
        Space::with_height(4),
        subtitle,
        Space::with_height(24),
        github_section,
        Space::with_height(16),
        sync_section,
        Space::with_height(16),
        data_section,
    ]
    .width(Length::Fixed(560.0));

    container(content)
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
        // Logged in state
        let user_display = username.unwrap_or("Unknown").to_string();
        column![
            row![
                text("✓").size(16).color(palette.success),
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
        // Login in progress
        let mut items: Vec<Element<'static, Message>> = vec![
            row![
                text("⏳").size(16),
                Space::with_width(8),
                text("Authenticating with GitHub...")
                    .size(14)
                    .color(palette.text_primary),
            ]
            .align_y(Vertical::Center)
            .into(),
        ];

        // Show device flow code if available
        if let (Some(code), Some(uri)) = (device_code, device_uri) {
            let code_owned = code.to_string();
            let uri_owned = uri.to_string();
            items.push(Space::with_height(16).into());
            items.push(
                container(
                    column![
                        text("Enter this code at GitHub:")
                            .size(13)
                            .color(palette.text_secondary),
                        Space::with_height(8),
                        text(code_owned)
                            .size(24)
                            .color(palette.primary),
                        Space::with_height(8),
                        button(
                            text(uri_owned).size(12).color(palette.primary)
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
        // Not logged in
        column![
            row![
                text("○").size(16).color(palette.text_muted),
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
                        text("🔗").size(14),
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

/// Sync actions card (push/pull/status)
fn sync_actions_section(
    theme: LilypadTheme,
    in_progress: bool,
) -> Element<'static, Message> {
    let palette = theme.palette();

    let section_title = text("Vault Sync")
        .size(16)
        .color(palette.text_primary);

    let hint = text("Push your vault to GitHub or pull the latest version from remote")
        .size(12)
        .color(palette.text_muted);

    let push_btn = button(
        container(
            row![
                text("⬆").size(14),
                Space::with_width(8),
                text("Push to GitHub").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme),
        _ => theme::primary_button(theme),
    });

    let push_btn = if in_progress {
        push_btn
    } else {
        push_btn.on_press(Message::SyncPush)
    };

    let pull_btn = button(
        container(
            row![
                text("⬇").size(14),
                Space::with_width(8),
                text("Pull from GitHub").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::FillPortion(1))
    .padding([12, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::secondary_button(theme),
        _ => theme::secondary_button(theme),
    });

    let pull_btn = if in_progress {
        pull_btn
    } else {
        pull_btn.on_press(Message::SyncPull)
    };

    let status_btn = button(
        container(
            text("Check Status").size(13)
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([8, 12])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::ghost_button_hovered(theme),
        _ => theme::ghost_button(theme),
    });

    let status_btn = if in_progress {
        status_btn
    } else {
        status_btn.on_press(Message::SyncCheckStatus)
    };

    let progress_hint: Element<'static, Message> = if in_progress {
        text("Sync in progress...")
            .size(12)
            .color(palette.warning)
            .into()
    } else {
        Space::new(0, 0).into()
    };

    container(
        column![
            section_title,
            Space::with_height(8),
            hint,
            Space::with_height(16),
            row![push_btn, Space::with_width(12), pull_btn,]
                .spacing(0),
            Space::with_height(12),
            status_btn,
            Space::with_height(8),
            progress_hint,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme))
    .into()
}

/// Export/Import data management card
fn data_management_section(theme: LilypadTheme) -> Element<'static, Message> {
    let palette = theme.palette();

    let section_title = text("Data Management")
        .size(16)
        .color(palette.text_primary);

    let hint = text("Export your vault for backup or import credentials from a file")
        .size(12)
        .color(palette.text_muted);

    let export_encrypted_btn = button(
        container(
            row![
                text("📦").size(14),
                Space::with_width(8),
                text("Export Encrypted (.lily)").size(13),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([10, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::secondary_button(theme),
        _ => theme::secondary_button(theme),
    })
    .on_press(Message::ExportVault);

    let export_json_btn = button(
        container(
            row![
                text("📄").size(14),
                Space::with_width(8),
                text("Export JSON (plaintext)").size(13),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([10, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => {
            let mut style = theme::secondary_button(theme);
            style.text_color = palette.warning;
            style
        }
        _ => theme::secondary_button(theme),
    })
    .on_press(Message::ExportVaultJson);

    let import_btn = button(
        container(
            row![
                text("📥").size(14),
                Space::with_width(8),
                text("Import from File").size(13),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([10, 16])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::secondary_button(theme),
        _ => theme::secondary_button(theme),
    })
    .on_press(Message::ImportVault);

    let warning = text("⚠ JSON export contains passwords in plaintext. Handle with care.")
        .size(11)
        .color(palette.warning);

    container(
        column![
            section_title,
            Space::with_height(8),
            hint,
            Space::with_height(16),
            export_encrypted_btn,
            Space::with_height(8),
            export_json_btn,
            Space::with_height(4),
            warning,
            Space::with_height(12),
            container(
                container(Space::new(Length::Fill, Length::Fixed(1.0)))
                    .style(move |_| container::Style {
                        background: Some(iced::Background::Color(palette.border)),
                        ..Default::default()
                    }),
            )
            .padding([8, 0]),
            import_btn,
        ]
        .padding(24),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme))
    .into()
}
