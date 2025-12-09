use directories::ProjectDirs;
use eframe::{egui, App};
use egui::{Align2, Color32, CornerRadius, OutputCommand, RichText};
use rand::Rng;
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
    generated_password: String,
    generator_length: usize,
    generator_lowercase: bool,
    generator_uppercase: bool,
    generator_digits: bool,
    generator_symbols: bool,
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
            generated_password: String::new(),
            generator_length: 16,
            generator_lowercase: true,
            generator_uppercase: true,
            generator_digits: true,
            generator_symbols: true,
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
                                    .corner_radius(8.0);

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

            ui.add_space(16.0);
            ui.separator();
            ui.heading("Password generator");
            ui.label(
                "Create strong, randomized passwords directly from Lilypad before storing them in your vault.",
            );
            ui.add_space(8.0);
            self.render_password_generator(ui, ctx);
        });
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
