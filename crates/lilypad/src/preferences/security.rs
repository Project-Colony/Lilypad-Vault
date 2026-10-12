//! Preferences > Security: auto-lock, the clipboard, the master password.

use super::LilypadApp;
use crate::message::Message;
use crate::theme;
use crate::view::vgap;
use colony_ui::{sz, ui_font, widgets, Typography};
use iced::widget::{button, column, text, text_input, Column};
use iced::Element;
use lilypad_core::i18n::{t, tf};

/// Auto-lock choices in minutes; 0 never locks.
const AUTO_LOCK_MINUTES: [u64; 6] = [0, 1, 5, 15, 30, 60];

/// Clipboard clearing delays in seconds.
const CLIPBOARD_SECONDS: [u64; 5] = [10, 20, 30, 45, 60];

impl LilypadApp {
    pub(super) fn security<'a>(&self, typo: &Typography) -> Column<'a, Message> {
        let auto_lock = widgets::setting_row(
            typo,
            t("preferences_lock_after"),
            t("preferences_auto_lock_desc"),
            widgets::dropdown(
                typo,
                AUTO_LOCK_MINUTES.map(|m| (m, minutes_label(m))),
                &self.auto_lock_minutes,
                Message::SetAutoLock,
            ),
        );
        let clipboard = widgets::setting_row(
            typo,
            t("preferences_clear_clipboard_after"),
            t("preferences_clipboard_desc"),
            widgets::dropdown(
                typo,
                CLIPBOARD_SECONDS.map(|s| (s, tf("preferences_seconds", &[("count", &s)]))),
                &self.clipboard_secs,
                Message::SetClipboardSecs,
            ),
        );
        widgets::category_heading(
            typo,
            t("preferences_cat_security"),
            t("preferences_security_desc"),
        )
        .push(self.section(typo, "autolock", "preferences_section_auto_lock", auto_lock))
        .push(self.section(
            typo,
            "clipboard",
            "preferences_section_clipboard",
            clipboard,
        ))
        .push(self.section(
            typo,
            "master",
            "preferences_section_master_password",
            self.change_master_body(typo),
        ))
    }

    fn change_master_body(&self, typo: &Typography) -> Element<'static, Message> {
        if self.session.is_none() {
            return widgets::section_description(typo, t("preferences_master_locked"));
        }
        let d = self.density;
        column![
            widgets::section_description(typo, t("preferences_master_desc")),
            text_input(t("preferences_new_master"), &self.cm_new)
                .font(ui_font())
                .on_input(Message::CmNewChanged)
                .secure(true)
                .size(sz(13))
                .padding(10)
                .style(theme::text_input_style(d)),
            vgap(6.0),
            text_input(t("preferences_confirm_master"), &self.cm_confirm)
                .font(ui_font())
                .on_input(Message::CmConfirmChanged)
                .on_submit(Message::ChangeMasterSubmit)
                .secure(true)
                .size(sz(13))
                .padding(10)
                .style(theme::text_input_style(d)),
            vgap(8.0),
            button(
                text(t("preferences_change_master"))
                    .font(ui_font())
                    .size(sz(13))
            )
            .on_press(Message::ChangeMasterSubmit)
            .padding([8, 14])
            .style(theme::primary_style(d)),
        ]
        .into()
    }
}

fn minutes_label(minutes: u64) -> String {
    match minutes {
        0 => t("preferences_never").to_string(),
        60 => t("preferences_one_hour").to_string(),
        m => tf("preferences_minutes", &[("count", &m)]),
    }
}
