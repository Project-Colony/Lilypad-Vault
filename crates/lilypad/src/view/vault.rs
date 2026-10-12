//! The vault shell: sidebar filters and the entry list.

use super::{
    boxed_btn, entry_color, grade_display, grade_letter, hfill, hgap, strength_score, type_icon,
    type_label, vgap, BtnStyle,
};
use crate::fonts;
use crate::message::{Filter, Message, QuickKind};
use crate::state::LilypadApp;
use crate::theme::{self};
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, text, text_input, Space,
};
use iced::{Alignment, Background, Element, Length};
use lilypad_core::{EntryType, EntryView};

impl LilypadApp {
    pub fn view_vault(&self) -> Element<'_, Message> {
        row![
            container(self.sidebar())
                .width(Length::Fixed(232.0))
                .height(Length::Fill),
            container(self.item_list())
                .width(Length::Fixed(330.0))
                .height(Length::Fill),
            container(self.detail_pane())
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .height(Length::Fill)
        .into()
    }

    pub fn sidebar(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let all = self.entries.len();
        let favs = self.entries.iter().filter(|e| e.is_favorite).count();

        let mut col = column![
            row![
                self.icon(fonts::icons::FLOWER, 16.0, p.primary),
                hgap(8.0),
                text(
                    self.session
                        .as_ref()
                        .map(|s| s.name().to_string())
                        .unwrap_or_default()
                )
                .size(14)
                .font(fonts::FONT_SEMIBOLD),
                hfill(),
                self.icon(fonts::icons::CIRCLE, 9.0, p.success),
            ]
            .align_y(Alignment::Center)
            .padding([4, 8]),
            vgap(6.0),
            self.nav_row(
                fonts::icons::VAULT,
                "All items",
                Some(all),
                self.filter == Filter::All,
                Filter::All
            ),
            self.nav_row(
                fonts::icons::STAR,
                "Favorites",
                Some(favs),
                self.filter == Filter::Favorites,
                Filter::Favorites
            ),
        ]
        .spacing(2);

        // Types present
        let mut types: Vec<EntryType> = Vec::new();
        for e in &self.entries {
            if !types.contains(&e.entry_type) {
                types.push(e.entry_type.clone());
            }
        }
        if !types.is_empty() {
            col = col.push(self.section_header("Types"));
            for ty in types {
                let count = self.entries.iter().filter(|e| e.entry_type == ty).count();
                let active = self.filter == Filter::Type(ty.clone());
                col = col.push(self.nav_row(
                    type_icon(&ty),
                    type_label(&ty),
                    Some(count),
                    active,
                    Filter::Type(ty),
                ));
            }
        }

        // Folders
        if let Some(session) = self.session.as_ref() {
            let folders = session.vault().list_folder_tree();
            if !folders.is_empty() {
                col = col.push(self.section_header("Folders"));
                for f in folders {
                    let depth = f.matches('/').count();
                    let leaf = f.rsplit('/').next().unwrap_or(&f).to_string();
                    let active = self.filter == Filter::Folder(f.clone());
                    let label = format!("{}{leaf}", "  ".repeat(depth));
                    col = col.push(self.nav_row_owned(
                        fonts::icons::FOLDER,
                        label,
                        None,
                        active,
                        Filter::Folder(f),
                    ));
                }
            }
            let tags = session.vault().list_tags();
            if !tags.is_empty() {
                col = col.push(self.section_header("Tags"));
                for tag in tags {
                    let active = self.filter == Filter::Tag(tag.clone());
                    col = col.push(self.nav_row_owned(
                        fonts::icons::TAG,
                        tag.clone(),
                        None,
                        active,
                        Filter::Tag(tag),
                    ));
                }
            }
        }

        // Footer status
        let lock_line = self
            .session
            .as_ref()
            .and_then(|s| s.time_until_lock())
            .map(|d| format!("Locks in {}:{:02}", d.as_secs() / 60, d.as_secs() % 60))
            .unwrap_or_else(|| "Unlocked".to_string());
        let mut footer = column![].spacing(6).padding([8, 8]);
        if let Some(report) = &self.health {
            let letter = grade_letter(report.score.grade);
            let display = grade_display(report.score.score, report.score.grade);
            let gc = theme::health_grade_color(letter, &p);
            footer = footer.push(
                row![
                    container(
                        text(display)
                            .size(11)
                            .font(fonts::FONT_BOLD)
                            .color(p.background)
                    )
                    .padding([1, 7])
                    .style(move |_| container::Style {
                        background: Some(Background::Color(gc)),
                        border: iced::Border {
                            radius: 5.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                    hgap(8.0),
                    text("Vault health").size(11).color(p.text_muted),
                ]
                .align_y(Alignment::Center),
            );
        }
        if !self.trashed.is_empty() {
            footer = footer.push(self.nav_row_owned(
                fonts::icons::TRASH,
                "Trash".to_string(),
                Some(self.trashed.len()),
                self.filter == Filter::Trash,
                Filter::Trash,
            ));
        }
        footer = footer.push(
            row![
                self.icon(fonts::icons::CIRCLE, 8.0, p.success),
                hgap(8.0),
                text(lock_line).size(11).color(p.text_muted)
            ]
            .align_y(Alignment::Center),
        );
        if let Some(user) = &self.authed {
            footer = footer.push(
                row![
                    self.icon(fonts::icons::SYNC, 10.0, p.primary),
                    hgap(8.0),
                    text(format!("GitHub · {user}"))
                        .size(11)
                        .color(p.text_muted)
                ]
                .align_y(Alignment::Center),
            );
        }
        footer = footer.push(row![button(
            row![
                self.icon(fonts::icons::COG, 12.0, p.text_muted),
                hgap(6.0),
                text("Settings").size(12)
            ]
            .align_y(Alignment::Center)
        )
        .on_press(Message::OpenSettings)
        .style(theme::ghost_style(t, d)),]);

        container(
            column![
                scrollable(col.padding([8, 6])).height(Length::Fill),
                container(footer).width(Length::Fill),
            ]
            .height(Length::Fill),
        )
        .style(move |_| theme::sidebar_container(t, d))
        .height(Length::Fill)
        .into()
    }

    pub fn section_header(&self, label: &str) -> Element<'_, Message> {
        let p = self.theme.palette();
        text(label.to_uppercase())
            .size(10)
            .color(p.text_muted)
            .font(fonts::FONT_MEDIUM)
            .into()
    }

    pub fn nav_row(
        &self,
        glyph: &str,
        label: &str,
        count: Option<usize>,
        active: bool,
        filter: Filter,
    ) -> Element<'_, Message> {
        self.nav_row_owned(glyph, label.to_string(), count, active, filter)
    }

    pub fn nav_row_owned(
        &self,
        glyph: &str,
        label: String,
        count: Option<usize>,
        active: bool,
        filter: Filter,
    ) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let mut r = row![
            self.icon(
                glyph,
                14.0,
                if active { p.primary } else { p.text_secondary }
            ),
            hgap(9.0),
            text(label).size(13),
            hfill(),
        ]
        .align_y(Alignment::Center);
        if let Some(c) = count {
            r = r.push(
                container(text(format!("{c}")).size(11).color(p.text_muted))
                    .padding([0, 6])
                    .style(move |_| theme::badge_container(t, d)),
            );
        }
        let style: BtnStyle = if active {
            boxed_btn(move |_, _| theme::nav_button_active(t, d))
        } else {
            boxed_btn(move |_, s| match s {
                button::Status::Hovered => theme::nav_button_hovered(t, d),
                button::Status::Pressed => theme::nav_button_pressed(t, d),
                _ => theme::nav_button(t, d),
            })
        };
        button(r)
            .on_press(Message::FilterSelected(filter))
            .width(Length::Fill)
            .padding([7, 9])
            .style(style)
            .into()
    }

    pub fn item_list(&self) -> Element<'_, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let visible = self.visible();
        let search = text_input("Search", &self.search)
            .on_input(Message::SearchChanged)
            .padding(9)
            .style(theme::text_input_style_closure(t, d));

        let toolbar = row![
            search,
            hgap(8.0),
            button(self.icon(fonts::icons::PLUS, 14.0, p.text_primary))
                .on_press(Message::ShowAddForm)
                .padding(9)
                .style(theme::primary_style(t, d)),
            button(self.icon(fonts::icons::WAND, 14.0, p.text_secondary))
                .on_press(Message::ShowGenerator)
                .padding(9)
                .style(theme::secondary_style(t, d)),
            button(self.icon(fonts::icons::LOCK, 14.0, p.danger))
                .on_press(Message::Lock)
                .padding(9)
                .style(theme::ghost_style(t, d)),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let list: Element<Message> = if visible.is_empty() {
            container(text("No entries here.").size(13).color(p.text_muted))
                .padding(20)
                .into()
        } else {
            visible
                .iter()
                .fold(column![].spacing(4), |col, e| col.push(self.list_row(e)))
                .into()
        };

        column![
            container(toolbar).padding(10),
            scrollable(container(list).padding([0, 8])).height(Length::Fill),
        ]
        .height(Length::Fill)
        .into()
    }

    pub fn quick_btn(&self, glyph: &str, msg: Message) -> Element<'static, Message> {
        let (t, d) = (self.theme, self.density);
        let p = self.theme.palette();
        button(self.icon(glyph, 12.0, p.text_secondary))
            .on_press(msg)
            .padding(4)
            .style(theme::ghost_style(t, d))
            .into()
    }

    pub fn list_row(&self, e: &EntryView) -> Element<'static, Message> {
        let p = self.theme.palette();
        let (t, d) = (self.theme, self.density);
        let label = e.label.clone();
        let selected = self.selected.as_deref() == Some(label.as_str());
        let hovered = self.hovered.as_deref() == Some(label.as_str());
        let (strength, has_totp) = self.derived.get(&label).copied().unwrap_or((0, false));

        let tile = container(self.icon(type_icon(&e.entry_type), 15.0, p.primary))
            .center(Length::Fixed(32.0))
            .style(move |_| theme::icon_badge_container(t, d));
        let subtitle = e
            .username
            .clone()
            .or_else(|| e.url.clone())
            .unwrap_or_else(|| type_label(&e.entry_type).to_string());

        let in_trash = self.filter == Filter::Trash;
        let mut trailing = row![].spacing(5).align_y(Alignment::Center);
        if in_trash {
            trailing = trailing.push(
                button(text("Restore").size(11))
                    .on_press(Message::RestoreEntry(label.clone()))
                    .padding([2, 8])
                    .style(theme::secondary_style(t, d)),
            );
        } else if hovered {
            if e.username.is_some() {
                trailing = trailing.push(self.quick_btn(
                    fonts::icons::USER,
                    Message::QuickCopy(label.clone(), QuickKind::Username),
                ));
            }
            trailing = trailing.push(self.quick_btn(
                fonts::icons::KEY,
                Message::QuickCopy(label.clone(), QuickKind::Password),
            ));
            if has_totp {
                trailing = trailing.push(self.quick_btn(
                    fonts::icons::CLOCK,
                    Message::QuickCopy(label.clone(), QuickKind::Totp),
                ));
            }
            if let Some(url) = e.url.clone() {
                trailing = trailing
                    .push(self.quick_btn(fonts::icons::EXTERNAL_LINK, Message::OpenUrl(url)));
            }
        }
        if e.is_favorite {
            trailing = trailing.push(self.icon(fonts::icons::STAR, 13.0, p.warning));
        }
        let dot_color = if strength == 0 {
            p.text_muted
        } else {
            theme::strength_color(strength_score(strength), &p)
        };
        trailing = trailing.push(
            container(Space::new())
                .width(Length::Fixed(8.0))
                .height(Length::Fixed(8.0))
                .style(move |_| theme::dot_indicator(dot_color)),
        );

        // Color label: a slim tinted bar on the row's leading edge.
        let color_bar: Element<'static, Message> = match e.color {
            Some(c) => {
                let fill = entry_color(c);
                container(Space::new())
                    .width(Length::Fixed(3.0))
                    .height(Length::Fixed(30.0))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(fill)),
                        border: iced::Border {
                            radius: 2.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .into()
            }
            None => hgap(3.0).into(),
        };

        let content = row![
            color_bar,
            hgap(6.0),
            tile,
            hgap(10.0),
            column![
                text(label.clone())
                    .size(14)
                    .font(fonts::FONT_SEMIBOLD)
                    .color(p.text_primary),
                text(subtitle).size(12).color(p.text_muted),
            ]
            .spacing(2)
            .width(Length::Fill),
            trailing,
        ]
        .align_y(Alignment::Center);

        let sel_bg = p.surface_variant;
        let hov_bg = p.hover;
        let bg = move |_: &iced::Theme| container::Style {
            background: if selected {
                Some(Background::Color(sel_bg))
            } else if hovered {
                Some(Background::Color(hov_bg))
            } else {
                None
            },
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        mouse_area(container(content).width(Length::Fill).padding(9).style(bg))
            .on_press(Message::SelectEntry(label.clone()))
            .on_enter(Message::RowHovered(Some(label)))
            .on_exit(Message::RowHovered(None))
            .into()
    }
}
