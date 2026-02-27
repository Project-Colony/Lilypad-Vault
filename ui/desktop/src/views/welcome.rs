//! Welcome & Onboarding Flow
//!
//! Multi-step onboarding shown on first launch. Introduces Lilypad,
//! its features, the Colony ecosystem, walks the user through a short
//! interactive tutorial, offers optional GitHub sync, and guides
//! vault creation.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Element, Length};

use lilypad_common::validation::{validate_password_strength, PasswordStrength};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::state::{OnboardingStep, TutorialPhase};
use crate::theme::{self, LilypadPalette, LilypadTheme};

/// Parameters passed from the main app to render the onboarding flow.
pub struct OnboardingParams<'a> {
    pub theme: LilypadTheme,
    pub step: OnboardingStep,
    pub tutorial_phase: TutorialPhase,
    pub tutorial_generated_pw: &'a str,
    pub tutorial_pw_copied: bool,
    // GitHub state
    pub github_authenticated: bool,
    pub github_username: Option<&'a str>,
    pub sync_in_progress: bool,
    pub device_flow_code: Option<&'a str>,
    pub device_flow_uri: Option<&'a str>,
    // Vault creation state
    pub master_password: &'a str,
    pub confirm_password: &'a str,
    pub error_message: Option<&'a str>,
}

/// Render the onboarding flow.
pub fn view(params: OnboardingParams<'_>) -> Element<'static, Message> {
    let OnboardingParams {
        theme,
        step,
        tutorial_phase,
        tutorial_generated_pw,
        tutorial_pw_copied,
        github_authenticated,
        github_username,
        sync_in_progress,
        device_flow_code,
        device_flow_uri,
        master_password,
        confirm_password,
        error_message,
    } = params;
    let palette = theme.palette();

    let page_content: Element<'static, Message> = match step {
        OnboardingStep::Welcome => welcome_page(theme, &palette),
        OnboardingStep::Features => features_page(theme, &palette),
        OnboardingStep::Colony => colony_page(theme, &palette),
        OnboardingStep::Tutorial => tutorial_page(
            theme,
            &palette,
            tutorial_phase,
            tutorial_generated_pw,
            tutorial_pw_copied,
        ),
        OnboardingStep::GitHubConnect => github_connect_page(
            theme,
            &palette,
            github_authenticated,
            github_username,
            sync_in_progress,
            device_flow_code,
            device_flow_uri,
        ),
        OnboardingStep::CreateVault => create_vault_page(
            theme,
            &palette,
            master_password,
            confirm_password,
            error_message,
        ),
    };

    let is_create_vault = step == OnboardingStep::CreateVault;

    let dots = progress_dots(theme, &palette, step);

    let layout = if is_create_vault {
        // No navigation row on CreateVault — the "Create Vault" button handles it
        column![
            Space::with_height(Length::FillPortion(2)),
            page_content,
            Space::with_height(32),
            dots,
            Space::with_height(Length::FillPortion(3)),
        ]
        .align_x(Horizontal::Center)
        .width(Length::Fixed(520.0))
    } else {
        let nav = navigation_row(theme, &palette, step);
        column![
            Space::with_height(Length::FillPortion(2)),
            page_content,
            Space::with_height(32),
            dots,
            Space::with_height(16),
            nav,
            Space::with_height(Length::FillPortion(3)),
        ]
        .align_x(Horizontal::Center)
        .width(Length::Fixed(520.0))
    };

    container(
        container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| theme::app_container(theme))
    .into()
}

// ============================================================================
// Page 1 — Welcome
// ============================================================================

fn welcome_page(_theme: LilypadTheme, palette: &LilypadPalette) -> Element<'static, Message> {
    let logo = fonts::centered_icon_colored(icons::FLOWER, 64.0, palette.primary);

    let title = text("Welcome to Lilypad")
        .size(32)
        .font(fonts::FONT_BOLD)
        .color(palette.text_primary);

    let tagline = text("Your personal, secure vault for passwords and credentials.")
        .size(16)
        .font(fonts::FONT_LIGHT)
        .color(palette.text_secondary);

    let badge = text("A Colony Project")
        .size(13)
        .font(fonts::FONT_MEDIUM)
        .color(palette.primary);

    column![
        logo,
        Space::with_height(20),
        title,
        Space::with_height(8),
        tagline,
        Space::with_height(16),
        badge,
    ]
    .align_x(Horizontal::Center)
    .into()
}

// ============================================================================
// Page 2 — Features
// ============================================================================

fn features_page(theme: LilypadTheme, palette: &LilypadPalette) -> Element<'static, Message> {
    let title = text("What Lilypad Does")
        .size(24)
        .font(fonts::FONT_BOLD)
        .color(palette.text_primary);

    let subtitle = text("Everything you need to keep your credentials safe.")
        .size(14)
        .font(fonts::FONT_LIGHT)
        .color(palette.text_secondary);

    let features = column![
        feature_item(
            theme,
            palette,
            icons::VAULT,
            "Encrypted Vault",
            "Your passwords are stored in a locally encrypted vault, \
             protected by your master password.",
        ),
        Space::with_height(16),
        feature_item(
            theme,
            palette,
            icons::DICE,
            "Password Generator",
            "Generate strong, unique passwords with customizable \
             length and character options.",
        ),
        Space::with_height(16),
        feature_item(
            theme,
            palette,
            icons::HEART_PULSE,
            "Health Dashboard",
            "Identify weak, reused, or expired passwords across \
             all your accounts at a glance.",
        ),
        Space::with_height(16),
        feature_item(
            theme,
            palette,
            icons::SYNC,
            "Secure Sync",
            "Optionally synchronize your vault across devices \
             using end-to-end encrypted storage.",
        ),
    ];

    column![
        title,
        Space::with_height(8),
        subtitle,
        Space::with_height(32),
        features,
    ]
    .align_x(Horizontal::Center)
    .into()
}

fn feature_item(
    theme: LilypadTheme,
    palette: &LilypadPalette,
    icon: &'static str,
    title_text: &'static str,
    description: &'static str,
) -> Element<'static, Message> {
    let icon_el = container(
        fonts::centered_icon_colored(icon, 20.0, palette.primary),
    )
    .width(Length::Fixed(40.0))
    .height(Length::Fixed(40.0))
    .align_x(Horizontal::Center)
    .align_y(Vertical::Center)
    .style(move |_| theme::elevated_container(theme));

    let text_col = column![
        text(title_text)
            .size(15)
            .font(fonts::FONT_SEMIBOLD)
            .color(palette.text_primary),
        Space::with_height(4),
        text(description)
            .size(13)
            .font(fonts::FONT_REGULAR)
            .color(palette.text_secondary),
    ];

    row![icon_el, Space::with_width(16), text_col]
        .align_y(Vertical::Top)
        .into()
}

// ============================================================================
// Page 3 — Colony
// ============================================================================

fn colony_page(theme: LilypadTheme, palette: &LilypadPalette) -> Element<'static, Message> {
    let globe = fonts::centered_icon_colored(icons::GLOBE, 48.0, palette.primary);

    let title = text("Part of Colony")
        .size(24)
        .font(fonts::FONT_BOLD)
        .color(palette.text_primary);

    let description = text(
        "Colony is an ecosystem of privacy-focused tools \
         designed to give you control over your digital life. \
         Lilypad is one of several companion utilities in \
         this growing family of open-source projects.",
    )
    .size(14)
    .font(fonts::FONT_REGULAR)
    .color(palette.text_secondary);

    let philosophy = text(
        "Every Colony tool is built with the same principles: \
         local-first data ownership, strong encryption, and \
         transparency through open source.",
    )
    .size(14)
    .font(fonts::FONT_REGULAR)
    .color(palette.text_secondary);

    let github_btn = button(
        row![
            fonts::centered_icon_colored(icons::GITHUB, 16.0, palette.text_primary),
            Space::with_width(8),
            text("Learn more on GitHub")
                .size(13)
                .color(palette.text_primary),
        ]
        .align_y(Vertical::Center),
    )
    .padding([10, 20])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::secondary_button_hovered(theme),
        _ => theme::secondary_button(theme),
    })
    .on_press(Message::OpenExternalLink(
        "https://github.com/MotherSphere/Colony".to_string(),
    ));

    column![
        globe,
        Space::with_height(20),
        title,
        Space::with_height(16),
        description,
        Space::with_height(12),
        philosophy,
        Space::with_height(24),
        github_btn,
    ]
    .align_x(Horizontal::Center)
    .into()
}

// ============================================================================
// Page 4 — Tutorial
// ============================================================================

fn tutorial_page(
    theme: LilypadTheme,
    palette: &LilypadPalette,
    phase: TutorialPhase,
    generated_pw: &str,
    pw_copied: bool,
) -> Element<'static, Message> {
    let generated_pw_owned = generated_pw.to_string();

    let title = text("A Quick Tour")
        .size(24)
        .font(fonts::FONT_BOLD)
        .color(palette.text_primary);

    let subtitle = text("Try out a few things before you begin.")
        .size(14)
        .font(fonts::FONT_LIGHT)
        .color(palette.text_secondary);

    let phase_content: Element<'static, Message> = match phase {
        TutorialPhase::GeneratePassword => tutorial_generate(theme, palette),
        TutorialPhase::ViewVault => tutorial_view_vault(theme, palette, &generated_pw_owned),
        TutorialPhase::CopyPassword => tutorial_copy(theme, palette, pw_copied),
    };

    let phase_indicator = tutorial_phase_dots(palette, phase);

    column![
        title,
        Space::with_height(8),
        subtitle,
        Space::with_height(28),
        phase_content,
        Space::with_height(20),
        phase_indicator,
    ]
    .align_x(Horizontal::Center)
    .into()
}

fn tutorial_generate(theme: LilypadTheme, palette: &LilypadPalette) -> Element<'static, Message> {
    let instruction = text("Step 1 of 3 — Generate a password")
        .size(14)
        .font(fonts::FONT_MEDIUM)
        .color(palette.text_primary);

    let explanation = text(
        "Lilypad can create strong passwords for you. \
         Press the button below to try it.",
    )
    .size(13)
    .font(fonts::FONT_REGULAR)
    .color(palette.text_secondary);

    let gen_btn = button(
        container(
            row![
                fonts::centered_icon(icons::DICE, 16.0),
                Space::with_width(8),
                text("Generate Password").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([14, 24])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme),
        _ => theme::primary_button(theme),
    })
    .on_press(Message::OnboardingTutorialGenerate);

    column![
        instruction,
        Space::with_height(8),
        explanation,
        Space::with_height(20),
        gen_btn,
    ]
    .into()
}

fn tutorial_view_vault(
    theme: LilypadTheme,
    palette: &LilypadPalette,
    generated_pw: &str,
) -> Element<'static, Message> {
    let generated_pw = generated_pw.to_string();

    let instruction = text("Step 2 of 3 — This is what an entry looks like")
        .size(14)
        .font(fonts::FONT_MEDIUM)
        .color(palette.text_primary);

    let explanation = text(
        "Each credential is stored as an entry in your vault. \
         Here is a preview with the password you just generated.",
    )
    .size(13)
    .font(fonts::FONT_REGULAR)
    .color(palette.text_secondary);

    let mock_card = container(
        column![
            row![
                fonts::centered_icon_colored(icons::GLOBE, 18.0, palette.primary),
                Space::with_width(12),
                column![
                    text("Example Account")
                        .size(15)
                        .font(fonts::FONT_SEMIBOLD)
                        .color(palette.text_primary),
                    text("user@example.com")
                        .size(12)
                        .font(fonts::FONT_REGULAR)
                        .color(palette.text_muted),
                ],
            ]
            .align_y(Vertical::Center),
            Space::with_height(12),
            row![
                fonts::centered_icon_colored(icons::KEY, 14.0, palette.text_muted),
                Space::with_width(8),
                text(generated_pw)
                    .size(13)
                    .font(fonts::FONT_REGULAR)
                    .color(palette.text_primary),
            ]
            .align_y(Vertical::Center),
        ]
        .padding(16),
    )
    .width(Length::Fill)
    .style(move |_| theme::card_container(theme));

    let next_btn = button(text("Got it").size(14))
        .padding([10, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::primary_button_hovered(theme),
            _ => theme::primary_button(theme),
        })
        .on_press(Message::OnboardingTutorialNext);

    column![
        instruction,
        Space::with_height(8),
        explanation,
        Space::with_height(20),
        mock_card,
        Space::with_height(16),
        next_btn,
    ]
    .into()
}

fn tutorial_copy(
    theme: LilypadTheme,
    palette: &LilypadPalette,
    pw_copied: bool,
) -> Element<'static, Message> {
    let instruction = text("Step 3 of 3 — Copy to clipboard")
        .size(14)
        .font(fonts::FONT_MEDIUM)
        .color(palette.text_primary);

    let explanation = text(
        "When you need a password, you can copy it to your \
         clipboard with a single click. Try it now.",
    )
    .size(13)
    .font(fonts::FONT_REGULAR)
    .color(palette.text_secondary);

    let success_color = palette.success;

    let copy_btn: Element<'static, Message> = if pw_copied {
        button(
            row![
                fonts::centered_icon(icons::CHECK, 16.0),
                Space::with_width(8),
                text("Copied!").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .padding([10, 24])
        .style(move |_theme, _status| {
            let mut style = theme::primary_button(theme);
            style.background = Some(iced::Background::Color(success_color));
            style
        })
        .into()
    } else {
        button(
            row![
                fonts::centered_icon(icons::COPY, 16.0),
                Space::with_width(8),
                text("Copy Password").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .padding([10, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::primary_button_hovered(theme),
            _ => theme::primary_button(theme),
        })
        .on_press(Message::OnboardingTutorialCopy)
        .into()
    };

    let feedback: Element<'static, Message> = if pw_copied {
        text("The password has been copied to your clipboard. You are ready to go.")
            .size(13)
            .font(fonts::FONT_REGULAR)
            .color(palette.success)
            .into()
    } else {
        Space::with_height(0).into()
    };

    column![
        instruction,
        Space::with_height(8),
        explanation,
        Space::with_height(20),
        copy_btn,
        Space::with_height(12),
        feedback,
    ]
    .into()
}

fn tutorial_phase_dots(
    palette: &LilypadPalette,
    current: TutorialPhase,
) -> Element<'static, Message> {
    let current_idx = current as usize;
    let labels = ["Generate", "View", "Copy"];
    let primary = palette.primary;
    let success = palette.success;
    let muted = palette.text_muted;

    let items: Vec<Element<'static, Message>> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let color = if i == current_idx {
                primary
            } else if i < current_idx {
                success
            } else {
                muted
            };
            text(*label).size(11).color(color).into()
        })
        .collect();

    row(items).spacing(16).into()
}

// ============================================================================
// Page 5 — GitHub Connect
// ============================================================================

fn github_connect_page(
    theme: LilypadTheme,
    palette: &LilypadPalette,
    authenticated: bool,
    username: Option<&str>,
    in_progress: bool,
    device_flow_code: Option<&str>,
    device_flow_uri: Option<&str>,
) -> Element<'static, Message> {
    let title = text("Sync with GitHub")
        .size(24)
        .font(fonts::FONT_BOLD)
        .color(palette.text_primary);

    let subtitle = text(
        "Connect your GitHub account to automatically back up \
         and sync your vault across devices.",
    )
    .size(14)
    .font(fonts::FONT_LIGHT)
    .color(palette.text_secondary);

    let content: Element<'static, Message> = if authenticated {
        // Connected — show success
        github_connected(theme, palette, username)
    } else if in_progress {
        // Device flow in progress
        github_in_progress(theme, palette, device_flow_code, device_flow_uri)
    } else {
        // Not connected — show connect button
        github_not_connected(theme, palette)
    };

    column![
        title,
        Space::with_height(8),
        subtitle,
        Space::with_height(28),
        content,
    ]
    .align_x(Horizontal::Center)
    .into()
}

fn github_not_connected(
    theme: LilypadTheme,
    palette: &LilypadPalette,
) -> Element<'static, Message> {
    let icon = fonts::centered_icon_colored(icons::GITHUB, 48.0, palette.text_muted);

    let description = text(
        "All vault data is encrypted before upload. \
         Lilypad will create a private repository to store \
         your encrypted vault securely.",
    )
    .size(13)
    .font(fonts::FONT_REGULAR)
    .color(palette.text_secondary);

    let connect_btn = button(
        container(
            row![
                fonts::centered_icon(icons::GITHUB, 16.0),
                Space::with_width(8),
                text("Connect to GitHub").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([14, 24])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme),
        _ => theme::primary_button(theme),
    })
    .on_press(Message::GitHubLogin);

    let skip_hint = text("You can also set this up later in Settings.")
        .size(12)
        .font(fonts::FONT_REGULAR)
        .color(palette.text_muted);

    column![
        icon,
        Space::with_height(16),
        description,
        Space::with_height(24),
        connect_btn,
        Space::with_height(12),
        skip_hint,
    ]
    .align_x(Horizontal::Center)
    .into()
}

fn github_in_progress(
    theme: LilypadTheme,
    palette: &LilypadPalette,
    device_flow_code: Option<&str>,
    device_flow_uri: Option<&str>,
) -> Element<'static, Message> {
    let status_row = row![
        fonts::centered_icon_colored(icons::CLOCK, 16.0, palette.primary),
        Space::with_width(8),
        text("Authenticating with GitHub...")
            .size(14)
            .color(palette.text_primary),
    ]
    .align_y(Vertical::Center);

    let mut items: Vec<Element<'static, Message>> = vec![status_row.into()];

    if let (Some(code), Some(uri)) = (device_flow_code, device_flow_uri) {
        let code_owned = code.to_string();
        let code_for_copy = code.to_string();
        let uri_owned = uri.to_string();

        let code_card = container(
            column![
                text("Enter this code at GitHub:")
                    .size(13)
                    .color(palette.text_secondary),
                Space::with_height(8),
                button(
                    row![
                        text(code_owned).size(24).color(palette.primary),
                        Space::with_width(12),
                        fonts::centered_icon_colored(icons::COPY, 14.0, palette.text_muted),
                    ]
                    .align_y(Vertical::Center),
                )
                .padding([8, 16])
                .style(move |_theme, status| match status {
                    button::Status::Hovered => theme::ghost_button_hovered(theme),
                    _ => theme::ghost_button(theme),
                })
                .on_press(Message::CopyToClipboard(code_for_copy)),
                Space::with_height(4),
                text("Click code to copy")
                    .size(11)
                    .color(palette.text_muted),
                Space::with_height(8),
                button(text(uri_owned).size(12).color(palette.primary))
                    .padding([4, 8])
                    .style(move |_theme, status| match status {
                        button::Status::Hovered => theme::ghost_button_hovered(theme),
                        _ => theme::ghost_button(theme),
                    })
                    .on_press(Message::OpenExternalLink(
                        "https://github.com/login/device".to_string(),
                    )),
            ]
            .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .padding(16)
        .style(move |_| theme::elevated_container(theme));

        items.push(Space::with_height(16).into());
        items.push(code_card.into());
    }

    column(items).align_x(Horizontal::Center).into()
}

fn github_connected(
    theme: LilypadTheme,
    palette: &LilypadPalette,
    username: Option<&str>,
) -> Element<'static, Message> {
    let user_display = username.unwrap_or("Unknown").to_string();

    let success_icon = fonts::centered_icon_colored(icons::CIRCLE_CHECK, 48.0, palette.success);

    let connected_text = text("Connected to GitHub")
        .size(16)
        .font(fonts::FONT_SEMIBOLD)
        .color(palette.success);

    let username_row = row![
        text("Logged in as")
            .size(13)
            .color(palette.text_secondary),
        Space::with_width(6),
        text(user_display)
            .size(13)
            .font(fonts::FONT_SEMIBOLD)
            .color(palette.text_primary),
    ]
    .align_y(Vertical::Center);

    let repo_info = text("Your encrypted vault repository is ready.")
        .size(13)
        .font(fonts::FONT_REGULAR)
        .color(palette.text_secondary);

    let next_btn = button(
        container(text("Continue").size(14).font(fonts::FONT_SEMIBOLD))
            .width(Length::Fill)
            .align_x(Horizontal::Center),
    )
    .width(Length::Fixed(200.0))
    .padding([12, 24])
    .style(move |_theme, status| match status {
        button::Status::Hovered => theme::primary_button_hovered(theme),
        _ => theme::primary_button(theme),
    })
    .on_press(Message::OnboardingNext);

    column![
        success_icon,
        Space::with_height(12),
        connected_text,
        Space::with_height(8),
        username_row,
        Space::with_height(4),
        repo_info,
        Space::with_height(24),
        next_btn,
    ]
    .align_x(Horizontal::Center)
    .into()
}

// ============================================================================
// Page 6 — Create Vault
// ============================================================================

fn create_vault_page(
    theme: LilypadTheme,
    palette: &LilypadPalette,
    master_password: &str,
    confirm_password: &str,
    error_message: Option<&str>,
) -> Element<'static, Message> {
    let master_password = master_password.to_string();
    let confirm_password = confirm_password.to_string();

    // Capture palette colors for use in closures
    let primary_color = palette.primary;
    let success_color = palette.success;
    let warning_color = palette.warning;
    let danger_color = palette.danger;
    let muted_color = palette.text_muted;

    let icon = fonts::centered_icon_colored(icons::SHIELD, 48.0, palette.primary);

    let title = text("Create Your Vault")
        .size(24)
        .font(fonts::FONT_BOLD)
        .color(palette.text_primary);

    let subtitle = text(
        "Choose a strong master password. This is the only \
         password you will need to remember.",
    )
    .size(14)
    .font(fonts::FONT_LIGHT)
    .color(palette.text_secondary);

    // Master password input
    let pw_label = text("Master Password")
        .size(13)
        .font(fonts::FONT_MEDIUM)
        .color(palette.text_secondary);

    let pw_input = text_input("Choose a strong password...", &master_password)
        .padding(14)
        .size(16)
        .font(fonts::FONT_REGULAR)
        .secure(true)
        .on_input(Message::MasterPasswordChanged)
        .style(move |_theme, status| match status {
            text_input::Status::Focused => theme::text_input_focused(theme),
            _ => theme::text_input_style(theme),
        });

    // Password strength indicator
    let strength_row: Element<'static, Message> = if !master_password.is_empty() {
        let strength = validate_password_strength(&master_password);
        let (label, color, bar_ratio) = match strength {
            PasswordStrength::VeryWeak => ("Very Weak", danger_color, 0.1),
            PasswordStrength::Weak => ("Weak", danger_color, 0.25),
            PasswordStrength::Fair => ("Fair", warning_color, 0.50),
            PasswordStrength::Strong => ("Strong", primary_color, 0.75),
            PasswordStrength::VeryStrong => ("Excellent", success_color, 1.0),
        };

        let bar_width = bar_ratio * 480.0;

        column![
            container(Space::new(bar_width, 3.0)).style(move |_| container::Style {
                background: Some(iced::Background::Color(color)),
                border: iced::Border {
                    radius: 1.5.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            Space::with_height(4),
            text(label).size(11).font(fonts::FONT_MEDIUM).color(color),
        ]
        .into()
    } else {
        Space::with_height(0).into()
    };

    // Confirm password input
    let confirm_label = text("Confirm Password")
        .size(13)
        .font(fonts::FONT_MEDIUM)
        .color(palette.text_secondary);

    let confirm_input = text_input("Re-enter your password...", &confirm_password)
        .padding(14)
        .size(16)
        .font(fonts::FONT_REGULAR)
        .secure(true)
        .on_input(Message::ConfirmPasswordChanged)
        .on_submit(Message::CreateVaultWithPassword)
        .style(move |_theme, status| match status {
            text_input::Status::Focused => theme::text_input_focused(theme),
            _ => theme::text_input_style(theme),
        });

    // Mismatch indicator
    let mismatch: Element<'static, Message> =
        if !confirm_password.is_empty() && master_password != confirm_password {
            text("Passwords do not match")
                .size(11)
                .color(danger_color)
                .into()
        } else {
            Space::with_height(0).into()
        };

    // Error message
    let danger_bg = theme::with_alpha(danger_color, 0.1);
    let error_el: Element<'static, Message> = if let Some(err) = error_message {
        let err = err.to_string();
        container(
            text(err)
                .size(12)
                .font(fonts::FONT_REGULAR)
                .color(danger_color),
        )
        .width(Length::Fill)
        .padding([8, 12])
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(danger_bg)),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    } else {
        Space::with_height(0).into()
    };

    // Create vault button
    let can_create =
        !master_password.is_empty() && !confirm_password.is_empty() && master_password == confirm_password;

    let create_btn = button(
        container(
            row![
                fonts::centered_icon(icons::VAULT, 16.0),
                Space::with_width(8),
                text("Create Vault").size(16).font(fonts::FONT_SEMIBOLD),
            ]
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Center),
    )
    .width(Length::Fill)
    .padding([14, 32])
    .style(move |_theme, status| match status {
        button::Status::Hovered if can_create => theme::primary_button_hovered(theme),
        _ if can_create => theme::primary_button(theme),
        _ => {
            let mut style = theme::primary_button(theme);
            style.background = Some(iced::Background::Color(muted_color));
            style
        }
    })
    .on_press_maybe(if can_create {
        Some(Message::CreateVaultWithPassword)
    } else {
        None
    });

    column![
        icon,
        Space::with_height(16),
        title,
        Space::with_height(8),
        subtitle,
        Space::with_height(24),
        pw_label,
        Space::with_height(4),
        pw_input,
        Space::with_height(4),
        strength_row,
        Space::with_height(16),
        confirm_label,
        Space::with_height(4),
        confirm_input,
        Space::with_height(4),
        mismatch,
        Space::with_height(8),
        error_el,
        Space::with_height(16),
        create_btn,
    ]
    .into()
}

// ============================================================================
// Progress Dots
// ============================================================================

fn progress_dots(
    _theme: LilypadTheme,
    palette: &LilypadPalette,
    current: OnboardingStep,
) -> Element<'static, Message> {
    let current_index = current.index();
    let primary = palette.primary;
    let muted = palette.text_muted;

    let dots: Vec<Element<'static, Message>> = (0..OnboardingStep::COUNT)
        .map(|i| {
            let is_current = i == current_index;
            let color = if is_current { primary } else { muted };
            let size: f32 = if is_current { 10.0 } else { 8.0 };

            button(
                container(Space::new(size, size)).style(move |_| container::Style {
                    background: Some(iced::Background::Color(color)),
                    border: iced::Border {
                        radius: (size / 2.0).into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            )
            .padding(4)
            .style(|_theme, _status| button::Style {
                background: None,
                ..Default::default()
            })
            .on_press(Message::OnboardingGoTo(i))
            .into()
        })
        .collect();

    row(dots).spacing(8).align_y(Vertical::Center).into()
}

// ============================================================================
// Navigation Row
// ============================================================================

fn navigation_row(
    theme: LilypadTheme,
    palette: &LilypadPalette,
    step: OnboardingStep,
) -> Element<'static, Message> {
    let is_first = step.index() == 0;
    // GitHub Connect page: "Next" advances, no special last-page behavior
    // CreateVault page is excluded from this function (handled in view())
    let hide_next = step == OnboardingStep::GitHubConnect;

    let skip_btn: Element<'static, Message> = if step == OnboardingStep::GitHubConnect {
        // On GitHub page, skip goes directly to CreateVault
        button(
            text("Skip, use local vault")
                .size(13)
                .font(fonts::FONT_REGULAR)
                .color(palette.text_muted),
        )
        .padding([8, 16])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::ghost_button_hovered(theme),
            _ => theme::ghost_button(theme),
        })
        .on_press(Message::OnboardingNext)
        .into()
    } else {
        button(
            text("Skip")
                .size(13)
                .font(fonts::FONT_REGULAR)
                .color(palette.text_muted),
        )
        .padding([8, 16])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::ghost_button_hovered(theme),
            _ => theme::ghost_button(theme),
        })
        .on_press(Message::OnboardingSkip)
        .into()
    };

    let back_btn: Element<'static, Message> = if !is_first {
        button(text("Back").size(13).font(fonts::FONT_REGULAR))
            .padding([8, 16])
            .style(move |_theme, status| match status {
                button::Status::Hovered => theme::ghost_button_hovered(theme),
                _ => theme::ghost_button(theme),
            })
            .on_press(Message::OnboardingPrev)
            .into()
    } else {
        Space::with_width(0).into()
    };

    let next_btn: Element<'static, Message> = if !hide_next {
        button(text("Next").size(14).font(fonts::FONT_SEMIBOLD))
            .padding([10, 24])
            .style(move |_theme, status| match status {
                button::Status::Hovered => theme::primary_button_hovered(theme),
                _ => theme::primary_button(theme),
            })
            .on_press(Message::OnboardingNext)
            .into()
    } else {
        Space::with_width(0).into()
    };

    row![
        skip_btn,
        Space::with_width(Length::Fill),
        back_btn,
        Space::with_width(8),
        next_btn,
    ]
    .align_y(Vertical::Center)
    .width(Length::Fill)
    .into()
}
