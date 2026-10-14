//! Preferences > Sync: the GitHub account, the sync status of the vault,
//! sync (merge), and push and pull.

use super::LilypadApp;
use crate::icons;
use crate::message::Message;
use crate::theme;
use crate::view::{hfill, hgap, vgap};
use colony_ui::theme::{contrast_on, Palette};
use colony_ui::{sz, ui_font, ui_font_bold, widgets, Typography};
use iced::widget::{button, column, container, row, text, text_input, Column};
use iced::{Alignment, Color, Element};
use lilypad_core::i18n::{t, tf};
use lilypad_core::SyncStatusView;

impl LilypadApp {
    pub(super) fn sync<'a>(&self, typo: &Typography) -> Column<'a, Message> {
        let mut col =
            widgets::category_heading(typo, t("preferences_cat_sync"), t("preferences_sync_desc"))
                .push(self.section(
                    typo,
                    "sync_account",
                    "preferences_section_github_account",
                    self.account_body(typo),
                ));
        // Status and transfers only mean something once signed in.
        if self.authed.is_some() {
            col = col
                .push(self.section(
                    typo,
                    "sync_status",
                    "preferences_section_sync_status",
                    self.status_body(typo),
                ))
                .push(self.section(
                    typo,
                    "sync_merge",
                    "preferences_section_sync_now",
                    self.merge_body(typo),
                ))
                .push(self.section(
                    typo,
                    "sync_transfer",
                    "preferences_section_push_pull",
                    self.transfer_body(typo),
                ));
        }
        col
    }

    fn account_body(&self, typo: &Typography) -> Element<'static, Message> {
        let d = self.density;
        match (&self.authed, &self.sync_login) {
            (_, Some(login)) => column![
                widgets::section_description(typo, t("preferences_github_code_desc")),
                container(
                    text(login.user_code.clone())
                        .size(sz(26))
                        .font(ui_font_bold())
                        .color(Palette::ACCENT()),
                )
                .padding([10, 18])
                .style(move |_| theme::elevated_container(d)),
                vgap(8.0),
                text(login.verification_uri.clone())
                    .font(ui_font())
                    .size(sz(12))
                    .color(Palette::TEXT_MUTED()),
                vgap(8.0),
                row![
                    text(t("preferences_github_waiting"))
                        .font(ui_font())
                        .size(sz(12))
                        .color(Palette::TEXT_SECONDARY()),
                    hgap(12.0),
                    button(text(t("form_cancel")).font(ui_font()).size(sz(12)))
                        .on_press(Message::SyncLoginCancel)
                        .padding([4, 10])
                        .style(theme::ghost_style(d)),
                ]
                .align_y(Alignment::Center),
            ]
            .into(),
            (Some(user), None) => row![
                icons::icon(icons::CIRCLE_CHECK, 13, Palette::SUCCESS()),
                hgap(8.0),
                text(tf("preferences_connected_as", &[("user", user)]))
                    .font(ui_font())
                    .size(sz(13))
                    .color(Palette::TEXT_PRIMARY()),
                hfill(),
                button(text(t("preferences_sign_out")).font(ui_font()).size(sz(12)))
                    .on_press_maybe((!self.sync_busy).then_some(Message::SyncLogout))
                    .padding([5, 10])
                    .style(theme::ghost_style(d)),
            ]
            .align_y(Alignment::Center)
            .into(),
            (None, None) => {
                let ink = contrast_on(Palette::ACCENT());
                column![
                    widgets::section_description(typo, t("preferences_github_signin_desc")),
                    button(
                        row![
                            icons::icon(icons::GITHUB, 13, ink),
                            hgap(8.0),
                            text(if self.sync_busy {
                                t("preferences_github_contacting")
                            } else {
                                t("preferences_github_sign_in")
                            })
                            .font(ui_font())
                            .size(sz(13)),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .on_press_maybe((!self.sync_busy).then_some(Message::SyncLoginStart))
                    .padding([8, 14])
                    .style(theme::primary_style(d)),
                ]
                .into()
            }
        }
    }

    fn status_body(&self, typo: &Typography) -> Element<'static, Message> {
        let d = self.density;
        let vault = self
            .active_vault_name()
            .unwrap_or_else(|| t("preferences_no_vault").to_string());
        let (status, hint, color) = status_text(self.sync_status.as_ref());
        column![
            row![
                icons::icon(icons::CIRCLE, 9, color),
                hgap(8.0),
                text(t(status))
                    .size(sz(13))
                    .font(ui_font_bold())
                    .color(Palette::TEXT_PRIMARY()),
                hgap(10.0),
                text(tf("preferences_sync_vault", &[("vault", &vault)]))
                    .font(ui_font())
                    .size(sz(12))
                    .color(Palette::TEXT_MUTED()),
            ]
            .align_y(Alignment::Center),
            vgap(4.0),
            widgets::section_description(typo, t(hint)),
            button(
                text(if self.sync_busy {
                    t("preferences_sync_checking")
                } else {
                    t("preferences_sync_check")
                })
                .font(ui_font())
                .size(sz(12)),
            )
            .on_press_maybe((!self.sync_busy).then_some(Message::SyncRefreshStatus))
            .padding([6, 12])
            .style(theme::secondary_style(d)),
        ]
        .into()
    }

    fn merge_body(&self, typo: &Typography) -> Element<'static, Message> {
        let d = self.density;
        let ink = contrast_on(Palette::ACCENT());
        column![
            widgets::section_description(typo, t("preferences_sync_now_desc")),
            // No on_submit on purpose: Sync and Pull share this field and both
            // can replace data, so an accidental Enter must fire neither. The
            // buttons are the only triggers.
            text_input(t("preferences_sync_password"), &self.sync_pull_password)
                .font(ui_font())
                .on_input(Message::SyncPullPasswordChanged)
                .secure(true)
                .size(sz(13))
                .padding(10)
                .style(theme::text_input_style(d)),
            vgap(8.0),
            button(
                row![
                    icons::icon(icons::SYNC, 12, ink),
                    hgap(8.0),
                    text(if self.sync_busy {
                        t("preferences_sync_working")
                    } else {
                        t("preferences_sync_now")
                    })
                    .font(ui_font())
                    .size(sz(13)),
                ]
                .align_y(Alignment::Center),
            )
            .on_press_maybe((!self.sync_busy).then_some(Message::SyncMerge))
            .padding([8, 14])
            .style(theme::primary_style(d)),
        ]
        .into()
    }

    fn transfer_body(&self, typo: &Typography) -> Element<'static, Message> {
        let d = self.density;
        let busy = |key| {
            if self.sync_busy {
                t("preferences_sync_working")
            } else {
                t(key)
            }
        };
        column![
            widgets::section_description(typo, t("preferences_push_desc")),
            button(
                row![
                    icons::icon(icons::UPLOAD, 12, Palette::ACCENT()),
                    hgap(8.0),
                    text(busy("preferences_push")).font(ui_font()).size(sz(13)),
                ]
                .align_y(Alignment::Center),
            )
            .on_press_maybe((!self.sync_busy).then_some(Message::SyncPush))
            .padding([8, 14])
            .style(theme::secondary_style(d)),
            vgap(16.0),
            widgets::section_description(typo, t("preferences_pull_desc")),
            button(
                row![
                    icons::icon(icons::DOWNLOAD, 12, Palette::ACCENT()),
                    hgap(8.0),
                    text(busy("preferences_pull")).font(ui_font()).size(sz(13)),
                ]
                .align_y(Alignment::Center),
            )
            .on_press_maybe((!self.sync_busy).then_some(Message::SyncPull))
            .padding([8, 14])
            .style(theme::secondary_style(d)),
        ]
        .into()
    }
}

/// The sync status as (title key, hint key, dot colour).
fn status_text(status: Option<&SyncStatusView>) -> (&'static str, &'static str, Color) {
    match status {
        Some(SyncStatusView::InSync) => ("sync_in_sync", "sync_in_sync_hint", Palette::SUCCESS()),
        Some(SyncStatusView::LocalAhead) => (
            "sync_local_ahead",
            "sync_local_ahead_hint",
            Palette::WARNING(),
        ),
        Some(SyncStatusView::RemoteAhead) => (
            "sync_remote_ahead",
            "sync_remote_ahead_hint",
            Palette::WARNING(),
        ),
        Some(SyncStatusView::Conflict) => ("sync_conflict", "sync_conflict_hint", Palette::ERROR()),
        Some(SyncStatusView::NoRemote) => (
            "sync_no_remote",
            "sync_no_remote_hint",
            Palette::TEXT_MUTED(),
        ),
        Some(SyncStatusView::NoBaseline) => (
            "sync_no_baseline",
            "sync_no_baseline_hint",
            Palette::TEXT_MUTED(),
        ),
        None => ("sync_unknown", "sync_unknown_hint", Palette::TEXT_MUTED()),
    }
}
