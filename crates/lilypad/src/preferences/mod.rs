//! Preferences, opened and closed by the Lilypad name in the sidebar.
//!
//! General, Appearance and Accessibility come first, built from the Colony
//! preferences kit; then Lilypad's own Security, Vault and Sync; About last
//! (design/settings-page.md in Project-Colony-Resources). Every change applies
//! and is saved the moment it is made: there is no Save button.

mod security;
mod sync;
mod vault;

use crate::icons;
use crate::message::Message;
use crate::state::LilypadApp;
use crate::theme::{self, UiVariation};
use crate::view::{hfill, hgap, vgap};
use colony_ui::theme::Palette;
use colony_ui::widgets::{self, PreferencesView, ABOUT_CATEGORY, STANDARD_CATEGORIES};
use colony_ui::{sz, Typography};
use iced::widget::{column, container, row, text, Column, Space};
use iced::{Alignment, Element, Length};
use lilypad_core::i18n::{self, t, tf, Locale};

/// The category label keys, in order: the three Colony categories first,
/// Lilypad's own next, About last.
pub const CATEGORIES: [&str; 7] = [
    STANDARD_CATEGORIES[0],
    STANDARD_CATEGORIES[1],
    STANDARD_CATEGORIES[2],
    "preferences_cat_security",
    "preferences_cat_vault",
    "preferences_cat_sync",
    ABOUT_CATEGORY,
];

impl LilypadApp {
    pub fn view_preferences(&self) -> Element<'_, Message> {
        let typo = Typography::current();
        let kit = PreferencesView::new(
            typo,
            &self.prefs,
            &self.expanded,
            Message::Preference,
            Message::ToggleSection,
        );
        let content: Element<Message> = match self.preferences_category {
            0 => self.general(&typo, kit.general()).into(),
            1 => self.appearance(&typo, &kit).into(),
            2 => kit.accessibility().into(),
            3 => self.security(&typo).into(),
            4 => self.vault_category(&typo).into(),
            5 => self.sync(&typo).into(),
            _ => self.about(&typo).into(),
        };
        widgets::preferences_page(
            &typo,
            widgets::category_list(
                &typo,
                CATEGORIES.iter().map(|key| t(key)),
                self.preferences_category,
                Message::PreferencesCategory,
            ),
            content,
            Message::TogglePreferences,
        )
    }

    /// One of Lilypad's collapsible sections, open while its key is in
    /// `expanded`.
    fn section<'a>(
        &self,
        typo: &Typography,
        key: &'static str,
        title_key: &str,
        content: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        widgets::collapsible_section(
            typo,
            t(title_key),
            self.expanded.contains(key),
            Message::ToggleSection(key),
            content.into(),
        )
    }

    fn general<'a>(&self, typo: &Typography, heading: Column<'a, Message>) -> Column<'a, Message> {
        let language = widgets::setting_row(
            typo,
            t("preferences_interface_language"),
            t("preferences_language_desc"),
            widgets::dropdown(
                typo,
                [
                    (Locale::En, t("language_english").to_string()),
                    (Locale::Fr, t("language_french").to_string()),
                ],
                &i18n::locale(),
                Message::SetLanguage,
            ),
        );
        let default_view = widgets::setting_row(
            typo,
            t("preferences_on_unlock_show"),
            t("preferences_default_view_desc"),
            widgets::dropdown(
                typo,
                [
                    (0u8, t("filter_all").to_string()),
                    (1u8, t("filter_favorites").to_string()),
                ],
                &self.default_filter.min(1),
                Message::SetDefaultFilter,
            ),
        );
        let startup = widgets::functional_toggle(
            typo,
            t("preferences_reopen_last_vault"),
            t("preferences_reopen_last_vault_desc"),
            self.restore_last_vault,
            Message::ToggleRestoreLastVault,
        );
        heading
            .push(self.section(typo, "language", "preferences_section_language", language))
            .push(self.section(
                typo,
                "default_view",
                "preferences_section_default_view",
                default_view,
            ))
            .push(self.section(typo, "startup", "preferences_section_startup", startup))
    }

    /// The kit's Appearance, with Lilypad's interface density between
    /// Typography and Effects.
    fn appearance<'a, F, S>(
        &self,
        typo: &Typography,
        kit: &PreferencesView<'_, F, S>,
    ) -> Column<'a, Message>
    where
        F: Fn(colony_ui::preferences::Change) -> Message + Clone + 'a,
        S: Fn(&'static str) -> Message,
    {
        widgets::category_heading(
            typo,
            t("settings_cat_appearance"),
            t("settings_appearance_desc"),
        )
        .push(kit.theme_section())
        .push(kit.colors_section())
        .push(kit.typography_section())
        .push(self.section(
            typo,
            "density",
            "preferences_section_density",
            self.density_body(typo),
        ))
        .push(kit.effects_section())
        .push(kit.preview_section())
    }

    fn density_body(&self, typo: &Typography) -> Element<'static, Message> {
        let (_, desc) = self.density.label_keys();
        let picker = widgets::dropdown(
            typo,
            UiVariation::ALL
                .iter()
                .map(|v| (*v, t(v.label_keys().0).to_string())),
            &self.density,
            Message::DensitySelected,
        );
        column![
            widgets::section_description(typo, t("preferences_density_desc")),
            widgets::setting_row(typo, t("preferences_section_density"), t(desc), picker),
            vgap(8.0),
            self.sample_entry(),
        ]
        .into()
    }

    /// A list row as the vault shows one, in the current density.
    fn sample_entry(&self) -> Element<'static, Message> {
        let d = self.density;
        container(
            row![
                container(icons::icon(icons::KEY, 15, Palette::ACCENT()))
                    .center(Length::Fixed(sz(32)))
                    .style(|_| theme::icon_badge_container()),
                hgap(10.0),
                column![
                    text(t("preferences_sample_entry"))
                        .size(sz(14))
                        .font(colony_ui::ui_font_bold())
                        .color(Palette::TEXT_PRIMARY()),
                    text("user@example.com")
                        .size(sz(12))
                        .color(Palette::TEXT_MUTED()),
                ]
                .spacing(2),
                hfill(),
                container(Space::new())
                    .width(Length::Fixed(8.0))
                    .height(Length::Fixed(8.0))
                    .style(|_| theme::dot_indicator(Palette::SUCCESS())),
            ]
            .align_y(Alignment::Center),
        )
        .padding(12)
        .width(Length::Fill)
        .style(move |_| theme::card_container(d))
        .into()
    }

    fn about<'a>(&self, typo: &Typography) -> Column<'a, Message> {
        let sync = self
            .authed
            .as_ref()
            .map(|u| tf("preferences_connected_as", &[("user", u)]))
            .unwrap_or_else(|| t("preferences_not_connected").to_string());
        let version = column![
            info_row(
                icons::LEAF,
                "Lilypad",
                format!("v{}", env!("CARGO_PKG_VERSION"))
            ),
            info_row(
                icons::GITHUB,
                t("preferences_repository"),
                "Project-Colony/Lilypad-Vault".to_string()
            ),
            info_row(icons::SYNC, t("preferences_github_sync"), sync),
        ];
        let tips = [
            "preferences_tip_reveal",
            "preferences_tip_quick_copy",
            "preferences_tip_favorite",
            "preferences_tip_trash",
        ]
        .into_iter()
        .fold(column![].spacing(6), |col, tip| {
            col.push(
                row![
                    icons::icon(icons::CIRCLE_CHECK, 11, Palette::ACCENT()),
                    hgap(8.0),
                    text(t(tip)).size(sz(12)).color(Palette::TEXT_MUTED()),
                ]
                .align_y(Alignment::Center),
            )
        });
        widgets::category_heading(typo, t(ABOUT_CATEGORY), t("preferences_about_desc"))
            .push(self.section(typo, "version", "preferences_section_version", version))
            .push(self.section(typo, "tips", "preferences_section_tips", tips))
    }
}

/// A label and its value on one line, with a glyph.
fn info_row(glyph: &str, label: &str, value: String) -> Element<'static, Message> {
    row![
        icons::icon(glyph, 13, Palette::ACCENT()),
        hgap(10.0),
        text(label.to_string())
            .size(sz(13))
            .color(Palette::TEXT_MUTED()),
        hfill(),
        text(value).size(sz(13)).color(Palette::TEXT_PRIMARY()),
    ]
    .align_y(Alignment::Center)
    .padding([6, 0])
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_keep_the_colony_order() {
        assert_eq!(&CATEGORIES[..3], &STANDARD_CATEGORIES);
        assert_eq!(
            CATEGORIES[3..6],
            [
                "preferences_cat_security",
                "preferences_cat_vault",
                "preferences_cat_sync",
            ]
        );
        assert_eq!(CATEGORIES.last(), Some(&ABOUT_CATEGORY));
    }
}
