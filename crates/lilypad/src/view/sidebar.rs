//! The sidebar: the Lilypad name (the way into Preferences), the sections of
//! the open vault or the list of vaults, and the footer.

use super::{grade_badge, hfill, hgap, type_icon, type_label, vgap};
use crate::icons;
use crate::message::{Filter, Message};
use crate::state::LilypadApp;
use crate::theme;
use colony_ui::theme::{contrast_on, Palette};
use colony_ui::{sz, widgets, Typography};
use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Alignment, Element, Length};
use lilypad_core::i18n::{t, tf};
use lilypad_core::EntryType;

/// The sidebar width at a text scale of 1.0; it grows with the scale so the
/// name and the section labels still fit at 1.68x.
const SIDEBAR_WIDTH: f32 = 232.0;

impl LilypadApp {
    pub fn sidebar(&self) -> Element<'_, Message> {
        let typo = Typography::current();
        let mut top = column![widgets::identity_button(
            &typo,
            "Lilypad",
            30,
            self.preferences_open,
            Message::TogglePreferences,
        )]
        .spacing(4);

        let sections = if self.session.is_some() {
            self.vault_sections(&typo)
        } else {
            self.vault_list(&typo)
        };
        top = top.push(vgap(12.0)).push(sections);

        let d = self.density;
        container(
            column![
                scrollable(top.padding([12, 10])).height(Length::Fill),
                container(self.sidebar_footer(&typo)).width(Length::Fill),
            ]
            .height(Length::Fill),
        )
        .style(move |_| theme::sidebar_container(d))
        .width(Length::Fixed(SIDEBAR_WIDTH * typo.scale))
        .height(Length::Fill)
        .into()
    }

    /// Locked: the vaults to unlock, one section each.
    fn vault_list(&self, typo: &Typography) -> Column<'_, Message> {
        let mut col = column![widgets::nav_label(typo, t("sidebar_vaults"))].spacing(2);
        for name in &self.vaults {
            let selected =
                !self.preferences_open && self.selected_vault.as_deref() == Some(name.as_str());
            col = col.push(widgets::nav_item(
                typo,
                name,
                selected,
                Message::VaultSelected(name.clone()),
            ));
        }
        col.push(vgap(6.0)).push(widgets::nav_item(
            typo,
            t("unlock_create_link"),
            !self.preferences_open && self.screen == crate::state::Screen::Create,
            Message::ShowCreate,
        ))
    }

    /// Unlocked: the vault's name, then its items by filter, type, folder and
    /// tag.
    fn vault_sections(&self, typo: &Typography) -> Column<'_, Message> {
        let name = self
            .session
            .as_ref()
            .map(|s| s.name().to_string())
            .unwrap_or_default();
        let all = self.entries.len();
        let favs = self.entries.iter().filter(|e| e.is_favorite).count();

        let mut col = column![
            row![
                icons::icon(icons::FLOWER, 13, Palette::ACCENT()),
                hgap(8.0),
                text(name)
                    .size(typo.sz(13))
                    .font(typo.medium)
                    .color(Palette::TEXT_SECONDARY()),
                hfill(),
                icons::icon(icons::CIRCLE, 8, Palette::SUCCESS()),
            ]
            .align_y(Alignment::Center)
            .padding([2, 8]),
            vgap(8.0),
            widgets::nav_label(typo, t("sidebar_items")),
            self.nav_row(icons::VAULT, t("filter_all"), Some(all), Filter::All),
            self.nav_row(
                icons::STAR,
                t("filter_favorites"),
                Some(favs),
                Filter::Favorites
            ),
        ]
        .spacing(2);

        let mut types: Vec<EntryType> = Vec::new();
        for e in &self.entries {
            if !types.contains(&e.entry_type) {
                types.push(e.entry_type.clone());
            }
        }
        if !types.is_empty() {
            col = col
                .push(vgap(8.0))
                .push(widgets::nav_label(typo, t("sidebar_types")));
            for ty in types {
                let count = self.entries.iter().filter(|e| e.entry_type == ty).count();
                col = col.push(self.nav_row(
                    type_icon(&ty),
                    type_label(&ty),
                    Some(count),
                    Filter::Type(ty),
                ));
            }
        }

        if let Some(session) = self.session.as_ref() {
            let folders = session.vault().list_folder_tree();
            if !folders.is_empty() {
                col = col
                    .push(vgap(8.0))
                    .push(widgets::nav_label(typo, t("sidebar_folders")));
                for f in folders {
                    let depth = f.matches('/').count();
                    let leaf = f.rsplit('/').next().unwrap_or(&f).to_string();
                    let label = format!("{}{leaf}", "  ".repeat(depth));
                    col = col.push(self.nav_row(icons::FOLDER, &label, None, Filter::Folder(f)));
                }
            }
            let tags = session.vault().list_tags();
            if !tags.is_empty() {
                col = col
                    .push(vgap(8.0))
                    .push(widgets::nav_label(typo, t("sidebar_tags")));
                for tag in tags {
                    col = col.push(self.nav_row(icons::TAG, &tag, None, Filter::Tag(tag.clone())));
                }
            }
        }
        col
    }

    fn sidebar_footer(&self, typo: &Typography) -> Column<'_, Message> {
        let mut footer = column![].spacing(6).padding([8, 10]);
        if self.session.is_some() {
            if let Some(report) = &self.health {
                footer = footer.push(
                    row![
                        grade_badge(report, 11),
                        hgap(8.0),
                        text(t("sidebar_vault_health"))
                            .size(sz(11))
                            .color(Palette::TEXT_MUTED()),
                    ]
                    .align_y(Alignment::Center),
                );
            }
            if !self.trashed.is_empty() {
                footer = footer.push(self.nav_row(
                    icons::TRASH,
                    t("filter_trash"),
                    Some(self.trashed.len()),
                    Filter::Trash,
                ));
            }
            let lock_line = self
                .session
                .as_ref()
                .and_then(|s| s.time_until_lock())
                .map(|d| {
                    tf(
                        "sidebar_locks_in",
                        &[(
                            "time",
                            &format!("{}:{:02}", d.as_secs() / 60, d.as_secs() % 60),
                        )],
                    )
                })
                .unwrap_or_else(|| t("sidebar_unlocked").to_string());
            footer = footer.push(
                row![
                    icons::icon(icons::CIRCLE, 8, Palette::SUCCESS()),
                    hgap(8.0),
                    text(lock_line).size(sz(11)).color(Palette::TEXT_MUTED())
                ]
                .align_y(Alignment::Center),
            );
        }
        if let Some(user) = &self.authed {
            footer = footer.push(
                row![
                    icons::icon(icons::SYNC, 10, Palette::ACCENT()),
                    hgap(8.0),
                    text(tf("sidebar_github_user", &[("user", user)]))
                        .size(sz(11))
                        .color(Palette::TEXT_MUTED())
                ]
                .align_y(Alignment::Center),
            );
        }
        footer.push(widgets::nav_hint(typo, t("sidebar_hint")))
    }

    /// A sidebar section: glyph, label and an optional count. Selected is
    /// the accent fill with legible ink on it (the Colony selection style).
    fn nav_row(
        &self,
        glyph: &str,
        label: &str,
        count: Option<usize>,
        filter: Filter,
    ) -> Element<'static, Message> {
        let active = !self.preferences_open && self.filter == filter;
        let ink = if active {
            contrast_on(Palette::ACCENT())
        } else {
            Palette::TEXT_SECONDARY()
        };
        let mut r = row![
            icons::icon(glyph, 13, ink),
            hgap(9.0),
            text(label.to_string()).size(sz(13)).color(ink),
            hfill(),
        ]
        .align_y(Alignment::Center);
        if let Some(c) = count {
            r = r.push(
                container(
                    text(c.to_string())
                        .size(sz(11))
                        .color(Palette::TEXT_MUTED()),
                )
                .padding([0, 6])
                .style(|_| theme::badge_container()),
            );
        }
        button(r)
            .on_press(Message::FilterSelected(filter))
            .width(Length::Fill)
            .padding([7, 9])
            .style(widgets::selection_style(active))
            .into()
    }
}
