//! Preferences > Vault: what the open vault holds, its health, import and
//! export, and where the files are.

use super::{info_row, LilypadApp};
use crate::icons;
use crate::message::Message;
use crate::theme;
use crate::view::{grade_badge, hgap, severity_color, vgap};
use colony_ui::theme::{contrast_on, Palette};
use colony_ui::{sz, widgets, Typography};
use iced::widget::{button, column, row, text, text_input, Column};
use iced::{Alignment, Element};
use lilypad_core::i18n::{t, tf};

impl LilypadApp {
    pub(super) fn vault_category<'a>(&self, typo: &Typography) -> Column<'a, Message> {
        let mut col = widgets::category_heading(
            typo,
            t("preferences_cat_vault"),
            t("preferences_vault_desc"),
        );
        if let Some(session) = self.session.as_ref() {
            let vault = session.vault();
            let favs = self.entries.iter().filter(|e| e.is_favorite).count();
            let stats = column![
                widgets::section_description(
                    typo,
                    &tf("preferences_vault_name", &[("vault", &session.name())])
                ),
                info_row(
                    icons::VAULT,
                    t("preferences_entries"),
                    self.entries.len().to_string()
                ),
                info_row(
                    icons::FOLDER,
                    t("sidebar_folders"),
                    vault.list_folder_tree().len().to_string()
                ),
                info_row(
                    icons::TAG,
                    t("sidebar_tags"),
                    vault.list_tags().len().to_string()
                ),
                info_row(icons::STAR, t("filter_favorites"), favs.to_string()),
                info_row(
                    icons::TRASH,
                    t("preferences_in_trash"),
                    self.trashed.len().to_string()
                ),
            ];
            col = col.push(self.section(typo, "stats", "preferences_section_contents", stats));
            if let Some(report) = &self.health {
                col = col.push(self.section(
                    typo,
                    "health",
                    "preferences_section_health",
                    self.health_body(typo, report),
                ));
            }
            col = col.push(self.section(
                typo,
                "import_export",
                "preferences_section_import_export",
                self.import_export_body(typo),
            ));
        } else {
            let list = self.vaults.iter().fold(
                column![widgets::section_description(
                    typo,
                    t("preferences_vaults_locked")
                )],
                |col, v| col.push(info_row(icons::VAULT, v, String::new())),
            );
            col = col.push(self.section(typo, "vaults", "sidebar_vaults", list));
        }
        let location = column![
            widgets::section_description(typo, t("preferences_storage_desc")),
            info_row(
                icons::FOLDER,
                t("preferences_storage_vaults"),
                self.service.data_dir().display().to_string()
            ),
            info_row(
                icons::FOLDER,
                t("preferences_storage_preferences"),
                self.service.config_dir().display().to_string()
            ),
        ];
        col.push(self.section(typo, "location", "preferences_section_storage", location))
    }

    fn health_body(
        &self,
        typo: &Typography,
        report: &lilypad_core::HealthReport,
    ) -> Element<'static, Message> {
        let d = self.density;
        let mut body = column![row![
            grade_badge(report, 14),
            hgap(10.0),
            text(tf(
                "preferences_health_score",
                &[
                    ("score", &report.score.score),
                    ("issues", &report.issues.len())
                ]
            ))
            .size(sz(13))
            .color(Palette::TEXT_SECONDARY()),
        ]
        .align_y(Alignment::Center)]
        .spacing(6);
        for issue in report.issues.iter().take(4) {
            body = body.push(
                row![
                    icons::icon(icons::CIRCLE, 7, severity_color(issue.severity)),
                    hgap(8.0),
                    text(issue.localized_title().to_string())
                        .size(sz(12))
                        .color(Palette::TEXT_MUTED()),
                ]
                .align_y(Alignment::Center),
            );
        }
        let ink = contrast_on(Palette::ACCENT());
        body.push(vgap(8.0))
            .push(widgets::section_description(
                typo,
                t("preferences_breach_desc"),
            ))
            .push(
                button(
                    row![
                        icons::icon(icons::SHIELD, 12, ink),
                        hgap(8.0),
                        text(if self.breach_busy {
                            t("preferences_breach_busy")
                        } else {
                            t("preferences_breach_button")
                        })
                        .size(sz(13)),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press_maybe((!self.breach_busy).then_some(Message::BreachCheckRun))
                .padding([8, 14])
                .style(theme::primary_style(d)),
            )
            .into()
    }

    fn import_export_body(&self, typo: &Typography) -> Element<'static, Message> {
        let d = self.density;
        let ink = contrast_on(Palette::ACCENT());
        column![
            widgets::section_description(typo, t("preferences_import_desc")),
            row![
                text_input(t("preferences_import_placeholder"), &self.import_path)
                    .on_input(Message::ImportPathChanged)
                    .on_submit(Message::ImportRun)
                    .size(sz(13))
                    .padding(10)
                    .style(theme::text_input_style(d)),
                hgap(8.0),
                button(
                    row![
                        icons::icon(icons::DOWNLOAD, 12, ink),
                        hgap(8.0),
                        text(t("preferences_import_button")).size(sz(13)),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press(Message::ImportRun)
                .padding([8, 14])
                .style(theme::primary_style(d)),
            ]
            .align_y(Alignment::Center),
            vgap(16.0),
            widgets::section_description(typo, t("preferences_export_desc")),
            row![
                text_input(t("preferences_export_placeholder"), &self.export_path)
                    .on_input(Message::ExportPathChanged)
                    .on_submit(Message::ExportRun)
                    .size(sz(13))
                    .padding(10)
                    .style(theme::text_input_style(d)),
                hgap(8.0),
                button(
                    row![
                        icons::icon(icons::UPLOAD, 12, Palette::ACCENT()),
                        hgap(8.0),
                        text(t("preferences_export_button")).size(sz(13)),
                    ]
                    .align_y(Alignment::Center),
                )
                .on_press(Message::ExportRun)
                .padding([8, 14])
                .style(theme::secondary_style(d)),
            ]
            .align_y(Alignment::Center),
        ]
        .into()
    }
}
