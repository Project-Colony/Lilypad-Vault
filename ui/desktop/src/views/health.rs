//! Health Dashboard View
//!
//! Displays password health analysis and recommendations.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Element, Length};

use lilypad_common::{HealthGrade, HealthReport};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::theme::{self, LilypadTheme};

/// Render the health dashboard
pub fn view(
    theme: LilypadTheme,
    health_report: Option<&HealthReport>,
    breached_entries: &[String],
) -> Element<'static, Message> {
    let palette = theme.palette();

    let title = text("Password Health")
        .size(24)
        .color(palette.text_primary);

    let subtitle = text("Monitor the security of your credentials")
        .size(14)
        .color(palette.text_secondary);

    let content = if let Some(report) = health_report {
        // Overall grade card
        let grade_color = grade_to_color(&report.score.grade, &palette);
        let grade_letter = match report.score.grade {
            HealthGrade::A => "A",
            HealthGrade::B => "B",
            HealthGrade::C => "C",
            HealthGrade::D => "D",
            HealthGrade::F => "F",
        };

        let score_value = report.score.score;
        let grade_card = container(
            row![
                container(
                    text(grade_letter)
                        .size(48)
                        .color(grade_color),
                )
                .width(Length::Fixed(80.0))
                .height(Length::Fixed(80.0))
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(iced::Color {
                        a: 0.15,
                        ..grade_color
                    })),
                    border: iced::Border {
                        radius: 12.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                Space::with_width(24),
                column![
                    text("Overall Health Score").size(16).color(palette.text_primary),
                    Space::with_height(4),
                    text(format!("{} / 100", score_value)).size(28).color(grade_color),
                    Space::with_height(4),
                    text(grade_description(&report.score.grade))
                        .size(13)
                        .color(palette.text_secondary),
                ],
            ]
            .align_y(Vertical::Center)
            .padding(24),
        )
        .width(Length::Fill)
        .style(move |_| theme::card_container(theme));

        // Get stats
        let total_entries = report.stats.total_entries;
        let weak_passwords = report.stats.weak_passwords;
        let reused_passwords = report.stats.reused_passwords;
        let expired_passwords = report.stats.expired_passwords;

        // Statistics cards
        let stats_row = row![
            stat_card(
                theme,
                "Total Entries",
                total_entries,
                icons::VAULT,
                palette.primary,
            ),
            Space::with_width(16),
            stat_card(
                theme,
                "Weak Passwords",
                weak_passwords,
                icons::TRIANGLE_EXCLAMATION,
                if weak_passwords > 0 {
                    palette.warning
                } else {
                    palette.success
                },
            ),
            Space::with_width(16),
            stat_card(
                theme,
                "Reused Passwords",
                reused_passwords,
                icons::REFRESH,
                if reused_passwords > 0 {
                    palette.danger
                } else {
                    palette.success
                },
            ),
            Space::with_width(16),
            stat_card(
                theme,
                "Expired",
                expired_passwords,
                icons::CLOCK,
                if expired_passwords > 0 {
                    palette.warning
                } else {
                    palette.success
                },
            ),
        ];

        // Issues list
        let issues_title = text("Issues to Address")
            .size(16)
            .color(palette.text_primary);

        let issues_content: Element<'static, Message> = if weak_passwords == 0
            && reused_passwords == 0
            && expired_passwords == 0
        {
            container(
                column![
                    text(icons::CIRCLE_CHECK)
                        .size(32)
                        .font(fonts::FONT_REGULAR)
                        .color(palette.success),
                    Space::with_height(12),
                    text("All passwords are healthy!")
                        .size(14)
                        .color(palette.text_secondary),
                ]
                .align_x(Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(32)
            .align_x(Horizontal::Center)
            .into()
        } else {
            let mut issues: Vec<Element<'static, Message>> = Vec::new();

            if weak_passwords > 0 {
                issues.push(issue_item(
                    theme,
                    "Weak Passwords",
                    format!(
                        "{} password{} need{} to be strengthened",
                        weak_passwords,
                        if weak_passwords == 1 { "" } else { "s" },
                        if weak_passwords == 1 { "s" } else { "" }
                    ),
                    palette.warning,
                ));
            }

            if reused_passwords > 0 {
                issues.push(issue_item(
                    theme,
                    "Reused Passwords",
                    format!(
                        "{} password{} {} being reused across accounts",
                        reused_passwords,
                        if reused_passwords == 1 { "" } else { "s" },
                        if reused_passwords == 1 { "is" } else { "are" }
                    ),
                    palette.danger,
                ));
            }

            if expired_passwords > 0 {
                issues.push(issue_item(
                    theme,
                    "Expired Passwords",
                    format!(
                        "{} password{} {} older than recommended",
                        expired_passwords,
                        if expired_passwords == 1 { "" } else { "s" },
                        if expired_passwords == 1 { "is" } else { "are" }
                    ),
                    palette.warning,
                ));
            }

            scrollable(column(issues).spacing(12))
                .height(Length::Fill)
                .style(move |_theme, _status| theme::scrollable_style(theme))
                .into()
        };

        // Refresh button
        let refresh_btn = button(
            row![
                text(icons::REFRESH).size(14).font(fonts::FONT_REGULAR),
                Space::with_width(8),
                text("Refresh").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .padding([10, 16])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::secondary_button_hovered(theme),
            _ => theme::secondary_button(theme),
        })
        .on_press(Message::RefreshHealthReport);

        // Breach check button
        let breach_btn = button(
            row![
                text(icons::SHIELD).size(14).font(fonts::FONT_REGULAR),
                Space::with_width(8),
                text("Check Breaches").size(14),
            ]
            .align_y(Vertical::Center),
        )
        .padding([10, 16])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::primary_button_hovered(theme),
            _ => theme::primary_button(theme),
        })
        .on_press(Message::CheckBreaches);

        // Breached entries section
        let breach_section: Element<'static, Message> = if !breached_entries.is_empty() {
            let breach_title = text("Breached Passwords")
                .size(16)
                .color(palette.danger);

            let breach_hint = text(format!(
                "{} password{} found in known data breaches. Change {} immediately!",
                breached_entries.len(),
                if breached_entries.len() == 1 { "" } else { "s" },
                if breached_entries.len() == 1 { "it" } else { "them" }
            ))
            .size(13)
            .color(palette.text_secondary);

            let breach_items: Vec<Element<'static, Message>> = breached_entries
                .iter()
                .map(|entry_label| {
                    let label = entry_label.clone();
                    container(
                        row![
                            text(icons::TRIANGLE_EXCLAMATION)
                                .size(14)
                                .font(fonts::FONT_REGULAR)
                                .color(palette.danger),
                            Space::with_width(12),
                            text(label).size(14).color(palette.text_primary),
                            Space::with_width(Length::Fill),
                            text("COMPROMISED").size(11).color(palette.danger),
                        ]
                        .align_y(Vertical::Center)
                        .padding(12),
                    )
                    .width(Length::Fill)
                    .style(move |_| theme::elevated_container(theme))
                    .into()
                })
                .collect();

            container(
                column![
                    breach_title,
                    Space::with_height(8),
                    breach_hint,
                    Space::with_height(12),
                    column(breach_items).spacing(8),
                ]
                .padding(24),
            )
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(iced::Color {
                    a: 0.08,
                    ..palette.danger
                })),
                border: iced::Border {
                    color: palette.danger,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                ..Default::default()
            })
            .into()
        } else {
            Space::new(0, 0).into()
        };

        let mut content_col = column![
            title,
            Space::with_height(4),
            subtitle,
            Space::with_height(24),
            grade_card,
            Space::with_height(24),
            stats_row,
            Space::with_height(24),
            row![
                issues_title,
                Space::with_width(Length::Fill),
                breach_btn,
                Space::with_width(8),
                refresh_btn,
            ]
            .align_y(Vertical::Center),
            Space::with_height(16),
            issues_content,
        ];

        if !breached_entries.is_empty() {
            content_col = content_col
                .push(Space::with_height(24))
                .push(breach_section);
        }

        content_col
    } else {
        // No report yet
        let analyze_btn = button(
            row![
                text(icons::HEART_PULSE).size(16).font(fonts::FONT_REGULAR),
                Space::with_width(8),
                text("Analyze Passwords").size(15),
            ]
            .align_y(Vertical::Center),
        )
        .padding([14, 24])
        .style(move |_theme, status| match status {
            button::Status::Hovered => theme::primary_button_hovered(theme),
            _ => theme::primary_button(theme),
        })
        .on_press(Message::RefreshHealthReport);

        column![
            title,
            Space::with_height(4),
            subtitle,
            Space::with_height(48),
            container(
                column![
                    text(icons::HEART_PULSE).size(64).font(fonts::FONT_REGULAR),
                    Space::with_height(24),
                    text("Analyze Your Passwords")
                        .size(18)
                        .color(palette.text_primary),
                    Space::with_height(8),
                    text("Check for weak, reused, or expired passwords")
                        .size(14)
                        .color(palette.text_secondary),
                    Space::with_height(24),
                    analyze_btn,
                ]
                .align_x(Horizontal::Center),
            )
            .width(Length::Fill)
            .padding(48)
            .align_x(Horizontal::Center),
        ]
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(24)
        .into()
}

/// Get color for health grade
fn grade_to_color(grade: &HealthGrade, palette: &crate::theme::LilypadPalette) -> iced::Color {
    match grade {
        HealthGrade::A => iced::Color::from_rgb8(34, 197, 94),
        HealthGrade::B => iced::Color::from_rgb8(132, 204, 22),
        HealthGrade::C => iced::Color::from_rgb8(234, 179, 8),
        HealthGrade::D => iced::Color::from_rgb8(249, 115, 22),
        HealthGrade::F => palette.danger,
    }
}

/// Get description for health grade
fn grade_description(grade: &HealthGrade) -> &'static str {
    match grade {
        HealthGrade::A => "Excellent! Your passwords are very secure.",
        HealthGrade::B => "Good security, but there's room for improvement.",
        HealthGrade::C => "Moderate security. Consider updating some passwords.",
        HealthGrade::D => "Poor security. Several passwords need attention.",
        HealthGrade::F => "Critical! Many passwords are at risk.",
    }
}

/// Render a statistics card
fn stat_card(
    theme: LilypadTheme,
    label: &'static str,
    value: usize,
    icon: &'static str,
    accent_color: iced::Color,
) -> Element<'static, Message> {
    let palette = theme.palette();

    container(
        column![
            row![
                text(icon).size(20).font(fonts::FONT_REGULAR),
                Space::with_width(Length::Fill),
            ],
            Space::with_height(12),
            text(value.to_string()).size(28).color(accent_color),
            Space::with_height(4),
            text(label).size(12).color(palette.text_muted),
        ]
        .padding(16),
    )
    .width(Length::FillPortion(1))
    .style(move |_| theme::card_container(theme))
    .into()
}

/// Render an issue item
fn issue_item(
    theme: LilypadTheme,
    title: &'static str,
    description: String,
    accent_color: iced::Color,
) -> Element<'static, Message> {
    let palette = theme.palette();

    container(
        row![
            container(Space::new(Length::Fixed(4.0), Length::Fixed(40.0)))
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(accent_color)),
                    border: iced::Border {
                        radius: 2.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            Space::with_width(16),
            column![
                text(title).size(14).color(palette.text_primary),
                Space::with_height(4),
                text(description).size(13).color(palette.text_secondary),
            ],
        ]
        .align_y(Vertical::Center)
        .padding(16),
    )
    .width(Length::Fill)
    .style(move |_| theme::elevated_container(theme))
    .into()
}
