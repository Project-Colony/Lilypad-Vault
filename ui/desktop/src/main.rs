use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use eframe::{egui, App};
use egui::{
    Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Margin, OutputCommand,
    RichText,
};
use lilypad_core::{
    decrypt, default_config, derive_key, encrypt, AppConfig, CryptoAlgorithm, Entry,
    KeyDerivationParams, KeyMaterial, KeyMetadata, Vault,
};
use lilypad_storage::LocalStore;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Lilypad Desktop",
        native_options,
        Box::new(|cc| {
            configure_fonts(&cc.egui_ctx);
            Ok(Box::<LilypadApp>::default())
        }),
    )
}

fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "JetBrainsMono-Regular".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-Regular.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-Medium".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-Medium.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-SemiBold".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-SemiBold.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-Bold".to_string(),
        FontData::from_static(include_bytes!("../../Assets/Fonts/JetBrainsMono-Bold.ttf")).into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-ExtraBold".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-ExtraBold.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-Light".to_string(),
        FontData::from_static(include_bytes!("../../Assets/Fonts/JetBrainsMono-Light.ttf")).into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-ExtraLight".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-ExtraLight.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-Thin".to_string(),
        FontData::from_static(include_bytes!("../../Assets/Fonts/JetBrainsMono-Thin.ttf")).into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-Italic".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-Italic.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-MediumItalic".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-MediumItalic.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-SemiBoldItalic".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-SemiBoldItalic.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-BoldItalic".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-BoldItalic.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-ExtraBoldItalic".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-ExtraBoldItalic.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-LightItalic".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-LightItalic.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-ExtraLightItalic".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-ExtraLightItalic.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono-ThinItalic".to_string(),
        FontData::from_static(include_bytes!(
            "../../Assets/Fonts/JetBrainsMono-ThinItalic.ttf"
        ))
        .into(),
    );

    let mix = vec![
        "JetBrainsMono-Regular",
        "JetBrainsMono-Medium",
        "JetBrainsMono-SemiBold",
        "JetBrainsMono-Bold",
        "JetBrainsMono-ExtraBold",
        "JetBrainsMono-Light",
        "JetBrainsMono-ExtraLight",
        "JetBrainsMono-Thin",
        "JetBrainsMono-Italic",
        "JetBrainsMono-MediumItalic",
        "JetBrainsMono-SemiBoldItalic",
        "JetBrainsMono-BoldItalic",
        "JetBrainsMono-ExtraBoldItalic",
        "JetBrainsMono-LightItalic",
        "JetBrainsMono-ExtraLightItalic",
        "JetBrainsMono-ThinItalic",
    ];

    fonts.families.insert(
        FontFamily::Proportional,
        mix.iter().map(|name| (*name).to_string()).collect(),
    );
    fonts.families.insert(
        FontFamily::Monospace,
        mix.iter().map(|name| (*name).to_string()).collect(),
    );

    ctx.set_fonts(fonts);
}

const DEFAULT_VAULT_NAME: &str = "primary";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum KeyFile {
    Raw { key_hex: String },
    Kdf { params: KeyDerivationParams },
}

#[derive(Debug, Serialize, Deserialize)]
struct DesktopEntryPayload {
    username: String,
    password: String,
    url: String,
    notes: String,
}

struct LilypadApp {
    show_welcome: bool,
    vault_unlocked: bool,
    search_query: String,
    selected_category: usize,
    status_message: Option<String>,
    welcome_ack_path: Option<std::path::PathBuf>,
    master_password: String,
    generated_password: String,
    config: AppConfig,
    store: LocalStore,
    active_vault: String,
    vault: Option<Vault>,
    vault_key: Option<KeyMaterial>,
    generator_length: usize,
    generator_lowercase: bool,
    generator_uppercase: bool,
    generator_digits: bool,
    generator_symbols: bool,
    show_add_entry: bool,
    show_settings: bool,
    vault_entries: Vec<VaultEntry>,
    entry_title: String,
    entry_username: String,
    entry_password: String,
    entry_url: String,
    entry_notes: String,
    settings_theme_index: usize,
    settings_auto_lock_minutes: u32,
    settings_clipboard_timeout_seconds: u32,
    settings_send_security_alerts: bool,
    account_display_name: String,
    account_email: String,
    account_timezone: String,
    account_two_factor_enabled: bool,
    account_marketing_opt_in: bool,
    security_auto_lock_minutes: u32,
    security_require_master_on_copy: bool,
    security_recovery_email: String,
    security_trusted_devices: Vec<String>,
}

struct VaultEntry {
    title: String,
    username: String,
    password: String,
    url: String,
    notes: String,
    last_updated: String,
    updated_at: u64,
}

impl Default for LilypadApp {
    fn default() -> Self {
        Self::new()
    }
}

impl App for LilypadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.show_welcome {
            self.render_welcome_modal(ctx);
            return;
        }

        if !self.vault_unlocked {
            self.render_unlock_screen(ctx);
            return;
        }

        self.render_header(ctx);
        self.render_main_panel(ctx);
        self.render_navigation_bar(ctx);
        self.render_status_bar(ctx);

        if self.show_settings {
            self.render_settings_modal(ctx);
        }
    }
}

impl LilypadApp {
    fn new() -> Self {
        let config = default_config();
        let store = LocalStore::new(&config).expect("store init");
        let mut app = Self {
            show_welcome: true,
            vault_unlocked: false,
            search_query: String::new(),
            selected_category: 0,
            status_message: None,
            welcome_ack_path: None,
            master_password: String::new(),
            generated_password: String::new(),
            config,
            store,
            active_vault: DEFAULT_VAULT_NAME.to_string(),
            vault: None,
            vault_key: None,
            generator_length: 16,
            generator_lowercase: true,
            generator_uppercase: true,
            generator_digits: true,
            generator_symbols: true,
            show_add_entry: false,
            show_settings: false,
            vault_entries: Vec::new(),
            entry_title: String::new(),
            entry_username: String::new(),
            entry_password: String::new(),
            entry_url: String::new(),
            entry_notes: String::new(),
            settings_theme_index: 0,
            settings_auto_lock_minutes: 10,
            settings_clipboard_timeout_seconds: 30,
            settings_send_security_alerts: true,
            account_display_name: "Avery Quinn".to_string(),
            account_email: "avery@lilypad.app".to_string(),
            account_timezone: "Europe/Paris".to_string(),
            account_two_factor_enabled: true,
            account_marketing_opt_in: false,
            security_auto_lock_minutes: 5,
            security_require_master_on_copy: true,
            security_recovery_email: "recovery@lilypad.app".to_string(),
            security_trusted_devices: vec![
                "MacBook Pro • Paris".to_string(),
                "iPhone 15 • Bordeaux".to_string(),
            ],
        };

        if let Some(project_dirs) = ProjectDirs::from("", "", "Lilypad") {
            let welcome_ack_path = project_dirs.config_dir().join("welcome_ack");
            app.welcome_ack_path = Some(welcome_ack_path.clone());

            if let Ok(contents) = fs::read_to_string(&welcome_ack_path) {
                if contents.trim() == "acknowledged=true" {
                    app.show_welcome = false;
                }
            }
        }

        app
    }

    fn render_welcome_modal(&mut self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("overlay"),
        ));
        painter.rect_filled(ctx.available_rect(), 0.0, Color32::from_black_alpha(40));

        egui::Window::new("Welcome to Lilypad (a Colony project)")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(
                    "Lilypad is a free companion tool in the Colony ecosystem. It's still in active development, so your feedback is essential to help improve it over time. You can follow the project and share feedback via the Colony repository on GitHub.",
                );
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Continue to Lilypad").strong()).clicked() {
                        self.persist_welcome_acknowledgement();
                        self.show_welcome = false;
                    }

                    if ui.button("Open Colony on GitHub").clicked() {
                        if let Err(error) = webbrowser::open("https://www.github.com/MotherSphere/Colony") {
                            self.status_message = Some(format!("Unable to open browser: {error}"));
                        }
                    }
                });
            });
    }

    fn render_unlock_screen(&mut self, ctx: &egui::Context) {
        let background = Color32::from_rgb(14, 22, 33);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(background))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    let card_frame = egui::Frame::new()
                        .fill(Color32::from_rgb(24, 36, 54))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(45, 78, 120)))
                        .corner_radius(CornerRadius::from(12.0))
                        .inner_margin(egui::Margin::same(24));

                    ui.allocate_ui_with_layout(
                        ui.available_size(),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                            ui.add_space(32.0);
                            ui.label(
                                RichText::new("Unlock Lilypad Vault")
                                    .size(28.0)
                                    .strong()
                                    .color(Color32::from_rgb(205, 225, 255)),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(
                                    "Secure your workspace with a strong master password before accessing your vault.",
                                )
                                .color(Color32::from_gray(200)),
                            );
                            ui.add_space(24.0);

                            let card_width = 460.0;
                            ui.allocate_ui_with_layout(
                                egui::vec2(card_width, 0.0),
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    card_frame.show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new("Master Password")
                                                    .size(16.0)
                                                    .color(Color32::from_rgb(185, 210, 240)),
                                            );
                                            ui.add_space(6.0);
                                            ui.add(
                                                egui::TextEdit::singleline(&mut self.master_password)
                                                    .password(true)
                                                    .hint_text("Enter your master password"),
                                            );
                                            ui.add_space(12.0);

                                            let requirements = self.password_requirements();
                                            let all_met = self.password_meets_requirements();

                                            ui.label(RichText::new("Password requirements").strong());
                                            ui.add_space(4.0);
                                            for (label, satisfied) in requirements {
                                                let color = if satisfied {
                                                    Color32::from_rgb(111, 207, 151)
                                                } else {
                                                    Color32::from_rgb(240, 105, 105)
                                                };
                                                ui.horizontal(|ui| {
                                                    ui.colored_label(color, if satisfied { "✔" } else { "○" });
                                                    ui.label(
                                                        RichText::new(label)
                                                            .color(Color32::from_gray(220)),
                                                    );
                                                });
                                            }

                                            ui.add_space(16.0);
                                            let button = egui::Button::new(
                                                RichText::new("Unlock Vault")
                                                    .strong()
                                                    .color(Color32::from_rgb(16, 22, 32)),
                                            )
                                            .fill(if all_met {
                                                Color32::from_rgb(111, 207, 151)
                                            } else {
                                                Color32::from_rgb(70, 94, 124)
                                            })
                                            .min_size(egui::vec2(240.0, 36.0))
                                            .corner_radius(8.0);

                                            if ui.add_enabled(all_met, button).clicked() {
                                                match self.unlock_vault() {
                                                    Ok(()) => {
                                                        self.vault_unlocked = true;
                                                        self.status_message =
                                                            Some("Vault unlocked".to_string());
                                                    }
                                                    Err(error) => {
                                                        self.status_message = Some(format!(
                                                            "Unable to unlock vault: {error}"
                                                        ));
                                                    }
                                                }
                                            }

                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new(
                                                    "Use a password manager-friendly secret to keep your vault secure.",
                                                )
                                                .color(Color32::from_gray(180))
                                                .italics(),
                                            );
                                        });
                                    });
                                },
                            );
                            ui.add_space(40.0);
                        },
                    );
                });
            });
    }

    fn password_requirements(&self) -> [(&'static str, bool); 4] {
        [
            (
                "At least 12 characters",
                self.master_password.chars().count() >= 12,
            ),
            (
                "Contains a lowercase letter",
                self.master_password.chars().any(|c| c.is_ascii_lowercase()),
            ),
            (
                "Contains an uppercase letter",
                self.master_password.chars().any(|c| c.is_ascii_uppercase()),
            ),
            (
                "Contains a special character",
                self.master_password
                    .chars()
                    .any(|c| !c.is_ascii_alphanumeric() && !c.is_whitespace()),
            ),
        ]
    }

    fn password_meets_requirements(&self) -> bool {
        self.password_requirements()
            .iter()
            .all(|(_, satisfied)| *satisfied)
    }

    fn render_header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Lilypad Vault");
                ui.separator();
                ui.label("Search");
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_query).hint_text("Search entries"),
                );
                ui.separator();
                if ui.button("Add Entry").clicked() {
                    self.show_add_entry = true;
                    self.selected_category = 0;
                }
                if ui.button("Settings").clicked() {
                    self.show_settings = true;
                }
            });
        });
    }

    fn render_navigation_bar(&mut self, ctx: &egui::Context) {
        let nav_items = [
            ("Vault", "🗄️"),
            ("Generator", "⚙️"),
            ("Alerts", "🔔"),
            ("Account", "👤"),
            ("Security", "🛡️"),
        ];

        let background = Color32::from_rgb(245, 247, 250);
        let accent = Color32::from_rgb(70, 118, 190);

        egui::TopBottomPanel::bottom("navigation_bar")
            .frame(
                egui::Frame::NONE
                    .fill(background)
                    .stroke(egui::Stroke::new(1.0, Color32::from_gray(210)))
                    .inner_margin(Margin::symmetric(12, 8)),
            )
            .show(ctx, |ui| {
                ui.set_height(96.0);
                ui.horizontal_centered(|ui| {
                    for (index, (label, icon)) in nav_items.iter().enumerate() {
                        let selected = self.selected_category == index;
                        let text_color = if selected {
                            Color32::from_rgb(16, 28, 46)
                        } else {
                            Color32::from_gray(60)
                        };

                        let desired_size = egui::vec2(108.0, 70.0);
                        let (rect, response) =
                            ui.allocate_exact_size(desired_size, egui::Sense::click());

                        if ui.is_rect_visible(rect) {
                            let fill = if selected {
                                accent
                            } else {
                                Color32::from_white_alpha(0)
                            };
                            let rounding = egui::CornerRadius::same(12);
                            ui.painter().rect(
                                rect,
                                rounding,
                                fill,
                                egui::Stroke::NONE,
                                egui::StrokeKind::Inside,
                            );

                            let icon_font = egui::FontId::proportional(22.0);
                            let label_font = egui::FontId::proportional(14.0);
                            let icon_galley = ui.fonts_mut(|fonts| {
                                fonts.layout_no_wrap(icon.to_string(), icon_font, text_color)
                            });
                            let label_galley = ui.fonts_mut(|fonts| {
                                fonts.layout_no_wrap(label.to_string(), label_font, text_color)
                            });
                            let spacing = 4.0;
                            let icon_height = icon_galley.size().y;
                            let label_height = label_galley.size().y;
                            let total_height = icon_height + spacing + label_height;
                            let start_y = rect.center().y - total_height / 2.0;

                            ui.painter().galley(
                                egui::pos2(rect.center().x - icon_galley.size().x / 2.0, start_y),
                                icon_galley.clone(),
                                text_color,
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    rect.center().x - label_galley.size().x / 2.0,
                                    start_y + icon_height + spacing,
                                ),
                                label_galley,
                                text_color,
                            );
                        }

                        if response.clicked() {
                            self.selected_category = index;
                        }
                    }
                });
            });
    }

    fn render_main_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.section_container(ui, |this, ui| match this.selected_category {
                1 => {
                    ui.heading("Password generator");
                    ui.separator();
                    ui.label(
                        "Create strong, randomized passwords directly from Lilypad before storing them in your vault.",
                    );
                    ui.add_space(8.0);
                    this.render_password_generator(ui, ctx);
                }
                2 => {
                    ui.heading("Alerts");
                    ui.separator();
                    ui.label(
                        "Stay ahead of security issues. Alerts will summarize important notices about your vault activity and account safety.",
                    );
                    ui.add_space(8.0);
                    ui.label("No alerts to show yet. Check back soon.");
                }
                3 => {
                    this.render_account_section(ui);
                }
                4 => {
                    this.render_security_section(ui);
                }
                _ => {
                    this.render_vault_section(ui, ctx);
                }
            });
        });
    }

    fn section_container<R>(
        &mut self,
        ui: &mut egui::Ui,
        add_content: impl FnOnce(&mut Self, &mut egui::Ui) -> R,
    ) -> R {
        let max_width: f32 = 680.0;
        ui.allocate_ui_with_layout(
            ui.available_size(),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                let width = max_width.min(ui.available_width());
                ui.set_min_width(width);
                ui.set_max_width(width);
                add_content(self, ui)
            },
        )
        .inner
    }

    fn render_vault_section(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Credentials");
        ui.separator();
        ui.label("Store and manage your secure vault items in one place.");
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Quick actions").strong());
            if ui.button("Add entry").clicked() {
                self.show_add_entry = true;
            }
            if ui.button("Lock vault").clicked() {
                self.vault_unlocked = false;
                self.vault = None;
                self.vault_key = None;
                self.vault_entries.clear();
                self.status_message = Some("Vault locked".to_string());
            }
        });
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            let button = egui::Button::new("Export .lily file").min_size(egui::vec2(220.0, 44.0));
            if ui.add(button).clicked() {
                self.export_vault_file();
            }
        });
        let export_path = vault_path_with_extension(&self.config, &self.active_vault, "lily");
        ui.label(format!("Vault file location: {}", export_path.display()));

        if self.show_add_entry {
            ui.add_space(12.0);
            self.render_add_entry_form(ui);
        }

        ui.add_space(16.0);
        ui.label(RichText::new("Saved entries").strong());
        ui.add_space(6.0);

        let query = self.search_query.trim().to_lowercase();
        let entries: Vec<&VaultEntry> = self
            .vault_entries
            .iter()
            .filter(|entry| {
                if query.is_empty() {
                    true
                } else {
                    entry.title.to_lowercase().contains(&query)
                        || entry.username.to_lowercase().contains(&query)
                        || entry.url.to_lowercase().contains(&query)
                }
            })
            .collect();

        if entries.is_empty() {
            ui.label("No entries match your search yet.");
        } else {
            for entry in entries {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&entry.title).strong());
                        ui.separator();
                        ui.label(&entry.username);
                    });
                    if !entry.url.is_empty() {
                        ui.label(format!("URL: {}", entry.url));
                    }
                    if !entry.notes.is_empty() {
                        ui.label(format!("Notes: {}", entry.notes));
                    }
                    ui.horizontal(|ui| {
                        ui.label(format!("Last updated: {}", entry.last_updated));
                        if ui.button("Copy password").clicked() {
                            ctx.send_cmd(OutputCommand::CopyText(entry.password.clone()));
                            self.status_message = Some("Password copied to clipboard".to_string());
                        }
                    });
                });
                ui.add_space(8.0);
            }
        }
    }

    fn render_add_entry_form(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.label(RichText::new("New vault entry").strong());
            ui.add_space(6.0);
            ui.label("Title");
            ui.add(egui::TextEdit::singleline(&mut self.entry_title).hint_text("e.g. Bank login"));
            ui.add_space(6.0);
            ui.label("Username");
            ui.add(
                egui::TextEdit::singleline(&mut self.entry_username).hint_text("username or email"),
            );
            ui.add_space(6.0);
            ui.label("Password");
            ui.add(
                egui::TextEdit::singleline(&mut self.entry_password)
                    .password(true)
                    .hint_text("store a strong password"),
            );
            ui.add_space(6.0);
            ui.label("Website");
            ui.add(egui::TextEdit::singleline(&mut self.entry_url).hint_text("https://"));
            ui.add_space(6.0);
            ui.label("Notes");
            ui.add(egui::TextEdit::multiline(&mut self.entry_notes).desired_rows(3));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Use generated password").clicked() {
                    self.entry_password = self.generated_password.clone();
                }
                if ui.button("Cancel").clicked() {
                    self.show_add_entry = false;
                }
                let save_enabled =
                    !self.entry_title.trim().is_empty() && !self.entry_password.trim().is_empty();
                if ui
                    .add_enabled(save_enabled, egui::Button::new("Save entry"))
                    .clicked()
                {
                    match self.save_entry_to_vault() {
                        Ok(()) => {
                            self.entry_title.clear();
                            self.entry_username.clear();
                            self.entry_password.clear();
                            self.entry_url.clear();
                            self.entry_notes.clear();
                            self.show_add_entry = false;
                            self.status_message = Some("Entry saved to vault".to_string());
                        }
                        Err(error) => {
                            self.status_message =
                                Some(format!("Unable to save entry to vault: {error}"));
                        }
                    }
                }
            });
        });
    }

    fn export_vault_file(&mut self) {
        let export_path = vault_path_with_extension(&self.config, &self.active_vault, "lily");
        if !export_path.exists() {
            self.status_message = Some("No .lily file found yet.".to_string());
            return;
        }
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Lilypad vault", &["lily"])
            .set_file_name(format!("{}.lily", self.active_vault.trim()));
        if let Some(parent) = export_path.parent() {
            dialog = dialog.set_directory(parent);
        }
        let Some(mut target_path) = dialog.save_file() else {
            self.status_message = Some("Export cancelled.".to_string());
            return;
        };
        if target_path.extension().and_then(|ext| ext.to_str()) != Some("lily") {
            target_path.set_extension("lily");
        }
        match fs::copy(&export_path, &target_path) {
            Ok(_) => {
                self.status_message =
                    Some(format!("Exported .lily file to {}", target_path.display()));
            }
            Err(error) => {
                self.status_message = Some(format!(
                    "Unable to export .lily file to {}: {error}",
                    target_path.display()
                ));
            }
        }
    }

    fn render_settings_modal(&mut self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("settings_overlay"),
        ));
        painter.rect_filled(ctx.available_rect(), 0.0, Color32::from_black_alpha(40));

        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Customize your Lilypad experience.");
                ui.add_space(8.0);
                ui.label(RichText::new("Appearance").strong());
                egui::ComboBox::from_label("Theme")
                    .selected_text(match self.settings_theme_index {
                        1 => "Night Bloom",
                        2 => "Pond Light",
                        _ => "Classic Green",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.settings_theme_index, 0, "Classic Green");
                        ui.selectable_value(&mut self.settings_theme_index, 1, "Night Bloom");
                        ui.selectable_value(&mut self.settings_theme_index, 2, "Pond Light");
                    });
                ui.add_space(6.0);
                ui.label(RichText::new("Vault protection").strong());
                ui.add(
                    egui::Slider::new(&mut self.settings_auto_lock_minutes, 1..=60)
                        .text("Auto-lock (minutes)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.settings_clipboard_timeout_seconds, 10..=120)
                        .text("Clipboard clear (seconds)"),
                );
                ui.checkbox(
                    &mut self.settings_send_security_alerts,
                    "Send security notifications",
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        self.show_settings = false;
                    }
                    if ui.button("Save settings").clicked() {
                        self.show_settings = false;
                        self.status_message = Some("Settings updated".to_string());
                    }
                });
            });
    }

    fn render_account_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Account");
        ui.separator();
        ui.label("Manage your profile, device approvals, and preferences in one place.");
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Profile").strong());
            ui.add_space(6.0);
            ui.label("Display name");
            ui.add(egui::TextEdit::singleline(&mut self.account_display_name));
            ui.label("Email address");
            ui.add(egui::TextEdit::singleline(&mut self.account_email));
            ui.label("Time zone");
            ui.add(egui::TextEdit::singleline(&mut self.account_timezone));
            ui.add_space(6.0);
            ui.checkbox(
                &mut self.account_two_factor_enabled,
                "Two-factor authentication",
            );
            ui.checkbox(
                &mut self.account_marketing_opt_in,
                "Product updates and tips",
            );
            ui.add_space(6.0);
            if ui.button("Save profile").clicked() {
                self.status_message = Some("Account profile saved".to_string());
            }
        });
    }

    fn render_security_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Security");
        ui.separator();
        ui.label("Centralize security options such as session locks and recovery methods.");
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Session security").strong());
            ui.add(
                egui::Slider::new(&mut self.security_auto_lock_minutes, 1..=30)
                    .text("Auto-lock (minutes)"),
            );
            ui.checkbox(
                &mut self.security_require_master_on_copy,
                "Require master password on copy",
            );
        });
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Recovery").strong());
            ui.label("Recovery email");
            ui.add(egui::TextEdit::singleline(
                &mut self.security_recovery_email,
            ));
            if ui.button("Update recovery email").clicked() {
                self.status_message = Some("Recovery email updated".to_string());
            }
        });
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Trusted devices").strong());
            let mut remove_index: Option<usize> = None;
            for (index, device) in self.security_trusted_devices.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(device);
                    if ui.button("Revoke").clicked() {
                        remove_index = Some(index);
                    }
                });
            }
            if let Some(index) = remove_index {
                self.security_trusted_devices.remove(index);
                self.status_message = Some("Device revoked".to_string());
            }
            if ui.button("Add current device").clicked() {
                self.security_trusted_devices
                    .push("New device • Active now".to_string());
                self.status_message = Some("Device added".to_string());
            }
        });
    }

    fn unlock_vault(&mut self) -> Result<()> {
        let password = self.master_password.trim();
        if password.is_empty() {
            return Err(anyhow!("master password is required"));
        }
        let (key, key_file) = self.load_or_create_key(password)?;
        let vault = self.load_or_create_vault(&key, &key_file)?;
        let entries = Self::entries_from_vault(&vault, &key)?;
        self.vault_entries = entries;
        self.vault = Some(vault);
        self.vault_key = Some(key);
        Ok(())
    }

    fn save_entry_to_vault(&mut self) -> Result<()> {
        let title = self.entry_title.trim();
        if title.is_empty() {
            return Err(anyhow!("title is required"));
        }
        let password = self.entry_password.trim();
        if password.is_empty() {
            return Err(anyhow!("password is required"));
        }
        let key = self
            .vault_key
            .as_ref()
            .ok_or_else(|| anyhow!("vault key is unavailable"))?;
        let vault = self
            .vault
            .as_mut()
            .ok_or_else(|| anyhow!("vault is not loaded"))?;
        let payload = DesktopEntryPayload {
            username: self.entry_username.trim().to_string(),
            password: password.to_string(),
            url: self.entry_url.trim().to_string(),
            notes: self.entry_notes.trim().to_string(),
        };
        let serialized = serde_json::to_vec(&payload)?;
        let ciphertext = encrypt(key, &serialized)?;
        if vault.find_entry(title).is_some() {
            vault.update_entry(title, ciphertext)?;
        } else {
            vault.add_entry(Entry::new(title, ciphertext))?;
        }
        self.store.save_vault(vault, key)?;
        self.vault_entries = Self::entries_from_vault(vault, key)?;
        Ok(())
    }

    fn entries_from_vault(vault: &Vault, key: &KeyMaterial) -> Result<Vec<VaultEntry>> {
        let mut entries = Vec::with_capacity(vault.entries.len());
        for entry in &vault.entries {
            let plaintext = decrypt(key, &entry.ciphertext)?;
            let payload = Self::parse_entry_payload(&plaintext);
            entries.push(VaultEntry {
                title: entry.label.clone(),
                username: payload.username,
                password: payload.password,
                url: payload.url,
                notes: payload.notes,
                last_updated: Self::format_timestamp(entry.updated_at),
                updated_at: entry.updated_at,
            });
        }
        entries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(entries)
    }

    fn parse_entry_payload(plaintext: &[u8]) -> DesktopEntryPayload {
        if let Ok(payload) = serde_json::from_slice::<DesktopEntryPayload>(plaintext) {
            return payload;
        }
        DesktopEntryPayload {
            username: String::new(),
            password: String::from_utf8_lossy(plaintext).to_string(),
            url: String::new(),
            notes: String::new(),
        }
    }

    fn format_timestamp(timestamp: u64) -> String {
        if timestamp == 0 {
            "Updated just now".to_string()
        } else {
            format!("Updated at {timestamp}")
        }
    }

    fn load_or_create_key(&self, master_password: &str) -> Result<(KeyMaterial, KeyFile)> {
        let path = key_path(&self.config);
        if path.exists() {
            return load_key(&path, master_password);
        }
        let params = KeyDerivationParams::generate();
        let key = derive_key(master_password, &params)?;
        let key_file = KeyFile::Kdf { params };
        save_key(&path, &key_file)?;
        Ok((key, key_file))
    }

    fn load_or_create_vault(&self, key: &KeyMaterial, key_file: &KeyFile) -> Result<Vault> {
        if vault_exists(&self.config, &self.active_vault) {
            return self
                .store
                .load_vault(&self.active_vault, key)
                .context("vault file unreadable");
        }

        let metadata = match key_file {
            KeyFile::Kdf { .. } => {
                KeyMetadata::new(key, CryptoAlgorithm::XChaCha20Poly1305).with_kdf("argon2id")
            }
            KeyFile::Raw { .. } => KeyMetadata::new(key, CryptoAlgorithm::XChaCha20Poly1305),
        };
        let vault = Vault::new(&self.active_vault, metadata);
        self.store.save_vault(&vault, key)?;
        Ok(vault)
    }

    fn render_password_generator(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Length").strong());
                ui.add(
                    egui::Slider::new(&mut self.generator_length, 8..=64)
                        .text("characters")
                        .step_by(1.0),
                );
            });

            ui.horizontal(|ui| {
                ui.checkbox(&mut self.generator_lowercase, "Lowercase (abc)");
                ui.checkbox(&mut self.generator_uppercase, "Uppercase (ABC)");
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.generator_digits, "Digits (0-9)");
                ui.checkbox(&mut self.generator_symbols, "Symbols (!#$)");
            });

            let generation_possible = self.generator_lowercase
                || self.generator_uppercase
                || self.generator_digits
                || self.generator_symbols;

            let (strength_label, strength_color) = self.generator_strength();

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Strength").strong());
                ui.colored_label(strength_color, strength_label);
            });

            ui.add_space(6.0);
            let generate_button = egui::Button::new(
                RichText::new("Generate password")
                    .strong()
                    .color(Color32::from_rgb(16, 22, 32)),
            )
            .fill(Color32::from_rgb(111, 207, 151))
            .min_size(egui::vec2(200.0, 32.0));

            if ui
                .add_enabled(generation_possible, generate_button)
                .clicked()
            {
                if let Some(password) = self.generate_password() {
                    self.generated_password = password.clone();
                    self.status_message = Some("New password generated".to_string());
                    ctx.send_cmd(OutputCommand::CopyText(password.clone()));
                }
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Generated password").strong());
                if ui.button("Copy").clicked() {
                    let password = self.generated_password.clone();
                    ctx.send_cmd(OutputCommand::CopyText(password));
                    self.status_message = Some("Password copied to clipboard".to_string());
                }
            });

            ui.add(
                egui::TextEdit::singleline(&mut self.generated_password)
                    .password(true)
                    .hint_text("Generate a password to display it here"),
            );
        });
    }

    fn generate_password(&self) -> Option<String> {
        let mut charset = String::new();
        if self.generator_lowercase {
            charset.push_str("abcdefghijklmnopqrstuvwxyz");
        }
        if self.generator_uppercase {
            charset.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        }
        if self.generator_digits {
            charset.push_str("0123456789");
        }
        if self.generator_symbols {
            charset.push_str("!#$%&()*+,-./:;<=>?@[]^_{|}~");
        }

        if charset.is_empty() {
            return None;
        }

        let mut rng = rand::thread_rng();
        let generated: String = (0..self.generator_length)
            .map(|_| {
                let idx = rng.gen_range(0..charset.len());
                charset.chars().nth(idx).unwrap_or('A')
            })
            .collect();

        Some(generated)
    }

    fn generator_strength(&self) -> (&'static str, Color32) {
        let mut score = 0;
        let length = self.generator_length as u32;

        if length >= 12 {
            score += 1;
        }
        if length >= 20 {
            score += 1;
        }
        if self.generator_lowercase && self.generator_uppercase {
            score += 1;
        }
        if self.generator_digits {
            score += 1;
        }
        if self.generator_symbols {
            score += 1;
        }

        match score {
            0 | 1 => ("Weak", Color32::from_rgb(240, 105, 105)),
            2 | 3 => ("Moderate", Color32::from_rgb(255, 193, 107)),
            4 => ("Strong", Color32::from_rgb(111, 207, 151)),
            _ => ("Very strong", Color32::from_rgb(76, 175, 80)),
        }
    }

    fn render_status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                if let Some(message) = &self.status_message {
                    ui.label(RichText::new(message).color(Color32::from_rgb(40, 120, 40)));
                } else {
                    ui.label("Ready");
                }
            });
        });
    }

    fn persist_welcome_acknowledgement(&mut self) {
        if let Some(path) = &self.welcome_ack_path {
            if let Some(parent) = path.parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    self.status_message = Some(format!("Unable to prepare config folder: {error}"));
                    return;
                }
            }

            if let Err(error) = fs::write(path, "acknowledged=true") {
                self.status_message = Some(format!("Unable to save welcome state: {error}"));
            }
        }
    }
}

fn key_path(config: &AppConfig) -> PathBuf {
    PathBuf::from(&config.data_dir).join("key.json")
}

fn vault_exists(config: &AppConfig, name: &str) -> bool {
    if name.trim().is_empty() {
        return false;
    }
    let lily_path = vault_path_with_extension(config, name, "lily");
    if lily_path.exists() {
        return true;
    }
    let legacy_path = vault_path_with_extension(config, name, "json");
    legacy_path.exists()
}

fn vault_path_with_extension(config: &AppConfig, name: &str, extension: &str) -> PathBuf {
    PathBuf::from(&config.data_dir)
        .join("vaults")
        .join(format!("{name}.{extension}"))
}

fn save_key(path: &Path, key_file: &KeyFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(key_file)?;
    fs::write(path, payload)?;
    Ok(())
}

fn load_key(path: &Path, master_password: &str) -> Result<(KeyMaterial, KeyFile)> {
    let payload = fs::read(path).with_context(|| {
        format!(
            "key file not found: {} (run the CLI init first)",
            path.display()
        )
    })?;
    let key_file: KeyFile = serde_json::from_slice(&payload)?;
    let key = match &key_file {
        KeyFile::Raw { key_hex } => {
            let bytes = decode_hex(key_hex)?;
            KeyMaterial::from_bytes(&bytes).context("invalid key material")?
        }
        KeyFile::Kdf { params } => {
            if master_password.trim().is_empty() {
                return Err(anyhow!("master password is required"));
            }
            derive_key(master_password, params).context("invalid kdf params")?
        }
    };
    Ok((key, key_file))
}

fn decode_hex(hex: &str) -> Result<Vec<u8>> {
    let value = hex.trim();
    if value.is_empty() {
        return Err(anyhow!("key cannot be empty"));
    }
    if value.len() % 2 != 0 {
        return Err(anyhow!("invalid hex string"));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks(2) {
        let chunk_str = std::str::from_utf8(chunk)?;
        let byte = u8::from_str_radix(chunk_str, 16).map_err(|_| anyhow!("invalid hex string"))?;
        bytes.push(byte);
    }
    Ok(bytes)
}
