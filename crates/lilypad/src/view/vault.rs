//! The open vault: the entry list beside the detail pane.

use super::{hgap, type_icon, type_label};
use crate::icons;
use crate::message::{Filter, Message, QuickKind};
use crate::state::LilypadApp;
use crate::theme;
use colony_ui::theme::Palette;
use colony_ui::{sz, ui_font, ui_font_bold};
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, text, text_input, Space,
};
use iced::{Alignment, Background, Element, Length};
use lilypad_core::i18n::t;
use lilypad_core::EntryView;

/// The entry list width at a text scale of 1.0.
const LIST_WIDTH: f32 = 330.0;

impl LilypadApp {
    pub fn view_vault(&self) -> Element<'_, Message> {
        row![
            container(self.item_list())
                .width(Length::Fixed(LIST_WIDTH * colony_ui::font_scale()))
                .height(Length::Fill),
            container(self.detail_pane())
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .height(Length::Fill)
        .into()
    }

    pub fn item_list(&self) -> Element<'_, Message> {
        let d = self.density;
        let visible = self.visible();
        let search = text_input(t("list_search_placeholder"), &self.search)
            .font(ui_font())
            .on_input(Message::SearchChanged)
            .size(sz(13))
            .padding(9)
            .style(theme::text_input_style(d));

        let accent = Palette::ACCENT();
        let toolbar = row![
            search,
            hgap(8.0),
            button(icons::icon(icons::PLUS, 14, colony_ui::contrast_on(accent)))
                .on_press(Message::ShowAddForm)
                .padding(9)
                .style(theme::primary_style(d)),
            button(icons::icon(icons::WAND, 14, accent))
                .on_press(Message::ShowGenerator)
                .padding(9)
                .style(theme::secondary_style(d)),
            button(icons::icon(icons::LOCK, 14, Palette::ERROR()))
                .on_press(Message::Lock)
                .padding(9)
                .style(theme::ghost_style(d)),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let list: Element<Message> = if visible.is_empty() {
            container(
                text(t("list_empty"))
                    .font(ui_font())
                    .size(sz(13))
                    .color(Palette::TEXT_MUTED()),
            )
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
        button(icons::icon(glyph, 12, Palette::TEXT_SECONDARY()))
            .on_press(msg)
            .padding(4)
            .style(theme::ghost_style(self.density))
            .into()
    }

    pub fn list_row(&self, e: &EntryView) -> Element<'static, Message> {
        let d = self.density;
        let label = e.label.clone();
        let selected = self.selected.as_deref() == Some(label.as_str());
        let hovered = self.hovered.as_deref() == Some(label.as_str());
        let (strength, has_totp) = self.derived.get(&label).copied().unwrap_or((0, false));

        let tile = container(icons::icon(type_icon(&e.entry_type), 15, Palette::ACCENT()))
            .center(Length::Fixed(sz(32)))
            .style(|_| theme::icon_badge_container());
        let subtitle = e
            .username
            .clone()
            .or_else(|| e.url.clone())
            .unwrap_or_else(|| type_label(&e.entry_type).to_string());

        let in_trash = self.filter == Filter::Trash;
        let mut trailing = row![].spacing(5).align_y(Alignment::Center);
        if in_trash {
            trailing = trailing.push(
                button(text(t("list_restore")).font(ui_font()).size(sz(11)))
                    .on_press(Message::RestoreEntry(label.clone()))
                    .padding([2, 8])
                    .style(theme::secondary_style(d)),
            );
        } else if hovered {
            if e.username.is_some() {
                trailing = trailing.push(self.quick_btn(
                    icons::USER,
                    Message::QuickCopy(label.clone(), QuickKind::Username),
                ));
            }
            trailing = trailing.push(self.quick_btn(
                icons::KEY,
                Message::QuickCopy(label.clone(), QuickKind::Password),
            ));
            if has_totp {
                trailing = trailing.push(self.quick_btn(
                    icons::CLOCK,
                    Message::QuickCopy(label.clone(), QuickKind::Totp),
                ));
            }
            if let Some(url) = e.url.clone() {
                trailing =
                    trailing.push(self.quick_btn(icons::EXTERNAL_LINK, Message::OpenUrl(url)));
            }
        }
        if e.is_favorite {
            trailing = trailing.push(icons::icon(icons::STAR, 13, Palette::WARNING()));
        }
        let dot_color = theme::strength_color(strength);
        trailing = trailing.push(
            container(Space::new())
                .width(Length::Fixed(8.0))
                .height(Length::Fixed(8.0))
                .style(move |_| theme::dot_indicator(dot_color)),
        );

        // Colour label: a slim bar on the row's leading edge.
        let color_bar: Element<'static, Message> = match e.color {
            Some(c) => {
                let fill = theme::entry_color(c);
                container(Space::new())
                    .width(Length::Fixed(3.0))
                    .height(Length::Fixed(sz(30)))
                    .style(move |_| theme::filled(fill, 2.0))
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
                    .size(sz(14))
                    .font(ui_font_bold())
                    .color(Palette::TEXT_PRIMARY()),
                text(subtitle)
                    .font(ui_font())
                    .size(sz(12))
                    .color(Palette::TEXT_MUTED()),
            ]
            .spacing(2)
            .width(Length::Fill),
            trailing,
        ]
        .align_y(Alignment::Center);

        let bg = move |_: &iced::Theme| container::Style {
            background: if selected {
                Some(Background::Color(Palette::BG_SELECTED()))
            } else if hovered {
                Some(Background::Color(Palette::BG_CARD_HOVER()))
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
