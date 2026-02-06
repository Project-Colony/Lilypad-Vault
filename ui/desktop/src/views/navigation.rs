//! Navigation Bar View
//!
//! The bottom navigation bar with category tabs.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length};

use crate::fonts::{self, icons};
use crate::message::Message;
use crate::state::Category;
use crate::theme::{self, LilypadTheme};

/// Navigation item data
struct NavItem {
    category: Category,
    icon: &'static str,
    label: &'static str,
}

const NAV_ITEMS: [NavItem; 6] = [
    NavItem {
        category: Category::Credentials,
        icon: icons::KEY,
        label: "Credentials",
    },
    NavItem {
        category: Category::Health,
        icon: icons::HEART_PULSE,
        label: "Health",
    },
    NavItem {
        category: Category::Generator,
        icon: icons::DICE,
        label: "Generator",
    },
    NavItem {
        category: Category::Sync,
        icon: icons::SYNC,
        label: "Sync",
    },
    NavItem {
        category: Category::Account,
        icon: icons::USER,
        label: "Account",
    },
    NavItem {
        category: Category::Security,
        icon: icons::SHIELD,
        label: "Security",
    },
];

/// Render the navigation bar
pub fn view(theme: LilypadTheme, selected_category: usize) -> Element<'static, Message> {
    let palette = theme.palette();

    let nav_buttons: Vec<Element<'static, Message>> = NAV_ITEMS
        .iter()
        .map(|item| {
            let is_active = item.category.to_index() == selected_category;

            let btn_content = column![
                text(item.icon)
                    .size(18)
                    .font(fonts::FONT_REGULAR)
                    .color(if is_active {
                        palette.primary
                    } else {
                        palette.text_muted
                    }),
                Space::with_height(4),
                text(item.label)
                    .size(11)
                    .font(fonts::FONT_MEDIUM)
                    .color(if is_active {
                        palette.primary
                    } else {
                        palette.text_muted
                    }),
            ]
            .width(Length::Fill)
            .align_x(Horizontal::Center);

            let style_fn = move |_theme: &iced::Theme, status: button::Status| {
                if is_active {
                    theme::nav_button_active(theme)
                } else {
                    match status {
                        button::Status::Hovered => {
                            let mut style = theme::nav_button(theme);
                            style.text_color = palette.text_secondary;
                            style
                        }
                        _ => theme::nav_button(theme),
                    }
                }
            };

            button(btn_content)
                .width(Length::FillPortion(1))
                .padding([12, 8])
                .style(style_fn)
                .on_press(Message::SelectCategory(item.category.to_index()))
                .into()
        })
        .collect();

    let nav_row = row(nav_buttons)
        .spacing(4)
        .padding([8, 16])
        .align_y(Vertical::Center);

    container(nav_row)
        .width(Length::Fill)
        .style(move |_| theme::nav_container(theme))
        .into()
}
