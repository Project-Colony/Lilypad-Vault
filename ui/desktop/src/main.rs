use directories::ProjectDirs;
use eframe::{egui, App};
use egui::{Align2, Color32, RichText};
use std::fs;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Lilypad Desktop",
        native_options,
        Box::new(|_cc| Ok(Box::<LilypadApp>::default())),
    )
}

struct LilypadApp {
    show_welcome: bool,
    vault_unlocked: bool,
    search_query: String,
    selected_category: usize,
    status_message: Option<String>,
    welcome_ack_path: Option<std::path::PathBuf>,
    master_password: String,
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
        self.render_sidebar(ctx);
        self.render_main_panel(ctx);
        self.render_status_bar(ctx);
    }
}

impl LilypadApp {
    fn new() -> Self {
        let mut app = Self {
            show_welcome: true,
            vault_unlocked: false,
            search_query: String::new(),
            selected_category: 0,
            status_message: None,
            welcome_ack_path: None,
            master_password: String::new(),
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
            .frame(egui::Frame::none().fill(background))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    let card_frame = egui::Frame::none()
                        .fill(Color32::from_rgb(24, 36, 54))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(45, 78, 120)))
                        .rounding(egui::Rounding::from(12.0))
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
                                            ui.label(RichText::new(label).color(Color32::from_gray(220)));
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
                                    .rounding(8.0);

                                    if ui.add_enabled(all_met, button).clicked() {
                                        self.vault_unlocked = true;
                                        self.status_message = Some("Vault unlocked".to_string());
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
                    self.status_message = Some("Entry creation coming soon".to_string());
                }
                if ui.button("Settings").clicked() {
                    self.status_message = Some("Settings placeholder".to_string());
                }
            });
        });
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        let categories = ["All Items", "Logins", "Secure Notes", "Cards", "Identities"];

        egui::SidePanel::left("sidebar")
            .resizable(true)
            .min_width(180.0)
            .show(ctx, |ui| {
                ui.heading("Vaults");
                ui.separator();
                for (index, label) in categories.iter().enumerate() {
                    let selected = self.selected_category == index;
                    if ui.selectable_label(selected, *label).clicked() {
                        self.selected_category = index;
                    }
                }
            });
    }

    fn render_main_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Credentials");
            ui.separator();
            ui.label(
                "This area will list stored items. Use the Add Entry action to populate the vault.",
            );
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Example: Email Account").strong());
                    ui.separator();
                    ui.label("user@example.com");
                });
                ui.label("Last updated: pending");
            });
        });
    }

    fn render_status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            if let Some(message) = &self.status_message {
                ui.label(RichText::new(message).color(Color32::from_rgb(40, 120, 40)));
            } else {
                ui.label("Ready");
            }
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
