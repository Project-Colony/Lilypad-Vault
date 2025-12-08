use eframe::{egui, App};
use egui::{Align2, Color32, RichText};

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
    search_query: String,
    selected_category: usize,
    status_message: Option<String>,
}

impl Default for LilypadApp {
    fn default() -> Self {
        Self {
            show_welcome: true,
            search_query: String::new(),
            selected_category: 0,
            status_message: None,
        }
    }
}

impl App for LilypadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.show_welcome {
            self.render_welcome_modal(ctx);
            return;
        }

        self.render_header(ctx);
        self.render_sidebar(ctx);
        self.render_main_panel(ctx);
        self.render_status_bar(ctx);
    }
}

impl LilypadApp {
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
}
