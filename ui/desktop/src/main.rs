use anyhow::{anyhow, Context, Result};
use arboard::Clipboard;
use directories::ProjectDirs;
use eframe::{egui, App};
use egui::{
    Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Margin, OutputCommand,
    RichText,
};
use lilypad_common::{
    keyfile::{load_key, save_key, KeyFile},
    time::format_timestamp_relative,
};
use lilypad_core::{
    decrypt, default_config, derive_key, encrypt, AppConfig, CryptoAlgorithm, Entry,
    KeyDerivationParams, KeyMaterial, KeyMetadata, Vault,
};
use lilypad_storage::LocalStore;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

/// A String wrapper that zeroizes its contents on drop for security.
struct SecureString(String);

impl SecureString {
    fn new() -> Self {
        Self(String::new())
    }

    fn as_str(&self) -> &str {
        &self.0
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn clear(&mut self) {
        self.0.zeroize();
        self.0 = String::new();
    }
}

impl std::ops::Deref for SecureString {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for SecureString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

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
const MAX_LOGIN_ATTEMPTS: u32 = 5;
const LOCKOUT_DURATION_SECS: u64 = 300; // 5 minutes

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DesktopEntryPayload {
    username: String,
    password: String,
    url: String,
    notes: String,
}

/// Current settings format version for migration support.
const SETTINGS_VERSION: u32 = 1;

/// Persisted application settings with versioning for forward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettings {
    #[serde(default)]
    version: u32,
    theme_index: usize,
    auto_lock_minutes: u32,
    clipboard_timeout_seconds: u32,
    send_security_alerts: bool,
    require_master_on_copy: bool,
    #[serde(default)]
    exclude_ambiguous_chars: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            theme_index: 0,
            auto_lock_minutes: 10,
            clipboard_timeout_seconds: 30,
            send_security_alerts: true,
            require_master_on_copy: false,
            exclude_ambiguous_chars: false,
        }
    }
}

/// Persisted lockout state for brute-force protection.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct LockoutState {
    failed_attempts: u32,
    /// Unix timestamp when lockout expires (0 if not locked out)
    lockout_until_timestamp: u64,
}

impl LockoutState {
    fn is_locked_out(&self) -> bool {
        if self.lockout_until_timestamp == 0 {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now < self.lockout_until_timestamp
    }

    fn remaining_secs(&self) -> u64 {
        if self.lockout_until_timestamp == 0 {
            return 0;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.lockout_until_timestamp.saturating_sub(now)
    }

    fn record_failure(&mut self) {
        self.failed_attempts += 1;
        if self.failed_attempts >= MAX_LOGIN_ATTEMPTS {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.lockout_until_timestamp = now + LOCKOUT_DURATION_SECS;
        }
    }

    fn reset(&mut self) {
        self.failed_attempts = 0;
        self.lockout_until_timestamp = 0;
    }
}

struct LilypadApp {
    show_welcome: bool,
    vault_unlocked: bool,
    search_query: String,
    selected_category: usize,
    status_message: Option<String>,
    status_message_time: Option<Instant>,
    welcome_ack_path: Option<std::path::PathBuf>,
    settings_path: Option<std::path::PathBuf>,
    lockout_path: Option<std::path::PathBuf>,
    master_password: String, // Zeroized on drop
    generated_password: String, // Zeroized on drop
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
    entry_password: String, // Zeroized on drop
    entry_url: String,
    entry_notes: String,

    // Persisted settings
    settings: AppSettings,

    // Brute-force protection (persisted)
    lockout_state: LockoutState,

    // Auto-lock
    last_activity: Instant,

    // Clipboard management
    clipboard_clear_time: Option<Instant>,
    clipboard_value: Option<String>, // Zeroized on drop

    // Re-authentication modal for copy
    show_reauth_modal: bool,
    reauth_password: String, // Zeroized on drop
    pending_copy_password: Option<String>, // Zeroized on drop

    // Confirmation dialogs
    show_delete_confirm: bool,
    pending_delete_index: Option<usize>,

    // Account settings (not persisted - demo)
    account_display_name: String,
    account_email: String,
    account_timezone: String,
    account_two_factor_enabled: bool,
    account_marketing_opt_in: bool,
    security_recovery_email: String,
    security_trusted_devices: Vec<String>,
}

/// Implement Drop to zeroize sensitive fields when the app is closed.
impl Drop for LilypadApp {
    fn drop(&mut self) {
        // Zeroize all sensitive fields
        self.master_password.zeroize();
        self.generated_password.zeroize();
        self.entry_password.zeroize();
        self.reauth_password.zeroize();
        if let Some(ref mut value) = self.clipboard_value {
            value.zeroize();
        }
        if let Some(ref mut value) = self.pending_copy_password {
            value.zeroize();
        }
        // Zeroize passwords in vault entries
        for entry in &mut self.vault_entries {
            entry.password.zeroize();
        }
    }
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

impl Drop for VaultEntry {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

impl Default for LilypadApp {
    fn default() -> Self {
        Self::new()
    }
}

impl App for LilypadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Background tasks
        self.check_auto_lock();
        self.check_clipboard_clear();
        self.check_status_clear();

        // Request repaint for timers
        ctx.request_repaint_after(Duration::from_secs(1));

        if self.show_welcome {
            self.render_welcome_modal(ctx);
            return;
        }

        if !self.vault_unlocked {
            self.render_unlock_screen(ctx);
            return;
        }

        // Record activity on any input
        if ctx.input(|i| i.pointer.any_click() || i.keys_down.len() > 0) {
            self.record_activity();
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
        let now = Instant::now();

        let mut app = Self {
            show_welcome: true,
            vault_unlocked: false,
            search_query: String::new(),
            selected_category: 0,
            status_message: None,
            status_message_time: None,
            welcome_ack_path: None,
            settings_path: None,
            lockout_path: None,
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

            settings: AppSettings::default(),

            lockout_state: LockoutState::default(),

            last_activity: now,

            clipboard_clear_time: None,
            clipboard_value: None,

            show_reauth_modal: false,
            reauth_password: String::new(),
            pending_copy_password: None,

            show_delete_confirm: false,
            pending_delete_index: None,

            account_display_name: "Avery Quinn".to_string(),
            account_email: "avery@lilypad.app".to_string(),
            account_timezone: "Europe/Paris".to_string(),
            account_two_factor_enabled: true,
            account_marketing_opt_in: false,
            security_recovery_email: "recovery@lilypad.app".to_string(),
            security_trusted_devices: vec![
                "MacBook Pro • Paris".to_string(),
                "iPhone 15 • Bordeaux".to_string(),
            ],
        };

        if let Some(project_dirs) = ProjectDirs::from("", "", "Lilypad") {
            let config_dir = project_dirs.config_dir();
            let welcome_ack_path = config_dir.join("welcome_ack");
            let settings_path = config_dir.join("settings.json");
            let lockout_path = config_dir.join("lockout.json");

            app.welcome_ack_path = Some(welcome_ack_path.clone());
            app.settings_path = Some(settings_path.clone());
            app.lockout_path = Some(lockout_path.clone());

            // Load welcome acknowledgement
            if let Ok(contents) = fs::read_to_string(&welcome_ack_path) {
                if contents.trim() == "acknowledged=true" {
                    app.show_welcome = false;
                }
            }

            // Load persisted settings
            if let Ok(contents) = fs::read_to_string(&settings_path) {
                if let Ok(mut settings) = serde_json::from_str::<AppSettings>(&contents) {
                    // Migrate settings if needed
                    if settings.version < SETTINGS_VERSION {
                        settings.version = SETTINGS_VERSION;
                    }
                    app.settings = settings;
                }
            }

            // Load persisted lockout state
            if let Ok(contents) = fs::read_to_string(&lockout_path) {
                if let Ok(lockout) = serde_json::from_str::<LockoutState>(&contents) {
                    app.lockout_state = lockout;
                }
            }
        }

        app
    }

    /// Records user activity for auto-lock feature.
    fn record_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Checks if the vault should be auto-locked due to inactivity.
    fn check_auto_lock(&mut self) {
        if !self.vault_unlocked {
            return;
        }
        let timeout_secs = (self.settings.auto_lock_minutes as u64) * 60;
        if timeout_secs == 0 {
            return; // Auto-lock disabled
        }
        if self.last_activity.elapsed() > Duration::from_secs(timeout_secs) {
            self.lock_vault();
            self.status_message = Some("Vault auto-locked due to inactivity".to_string());
            self.status_message_time = Some(Instant::now());
        }
    }

    /// Locks the vault and clears sensitive data.
    fn lock_vault(&mut self) {
        self.vault_unlocked = false;
        self.vault = None;
        self.vault_key = None;
        self.vault_entries.clear();
        self.master_password.clear();
    }

    /// Checks if clipboard should be cleared.
    fn check_clipboard_clear(&mut self) {
        if let (Some(clear_time), Some(value)) = (&self.clipboard_clear_time, &self.clipboard_value)
        {
            if Instant::now() >= *clear_time {
                if let Ok(mut clipboard) = Clipboard::new() {
                    if clipboard.get_text().ok().as_deref() == Some(value) {
                        let _ = clipboard.set_text(String::new());
                    }
                }
                self.clipboard_clear_time = None;
                self.clipboard_value = None;
            }
        }
    }

    /// Copies a value to clipboard with automatic clearing.
    fn copy_with_timeout(&mut self, value: &str, ctx: &egui::Context) {
        ctx.send_cmd(OutputCommand::CopyText(value.to_string()));
        if self.settings.clipboard_timeout_seconds > 0 {
            self.clipboard_value = Some(value.to_string());
            self.clipboard_clear_time = Some(
                Instant::now() + Duration::from_secs(self.settings.clipboard_timeout_seconds as u64),
            );
        }
    }

    /// Sets a status message that will auto-clear after a few seconds.
    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some(message.into());
        self.status_message_time = Some(Instant::now());
    }

    /// Checks if status message should be cleared.
    fn check_status_clear(&mut self) {
        if let Some(time) = self.status_message_time {
            if time.elapsed() > Duration::from_secs(5) {
                self.status_message = None;
                self.status_message_time = None;
            }
        }
    }

    /// Saves settings to disk.
    fn save_settings(&self) {
        if let Some(path) = &self.settings_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&self.settings) {
                let _ = fs::write(path, json);
            }
        }
    }

    /// Checks if login is currently locked out (uses persistent state).
    fn is_locked_out(&self) -> bool {
        self.lockout_state.is_locked_out()
    }

    /// Returns remaining lockout time in seconds.
    fn lockout_remaining_secs(&self) -> u64 {
        self.lockout_state.remaining_secs()
    }

    /// Records a failed login attempt and persists state.
    fn record_failed_login(&mut self) {
        self.lockout_state.record_failure();
        self.save_lockout();
    }

    /// Resets login attempts after successful login and persists state.
    fn reset_login_attempts(&mut self) {
        self.lockout_state.reset();
        self.save_lockout();
    }

    /// Saves lockout state to disk for persistence across app restarts.
    fn save_lockout(&self) {
        if let Some(path) = &self.lockout_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(&self.lockout_state) {
                let _ = fs::write(path, json);
            }
        }
    }

    /// Verifies the master password for re-authentication.
    fn verify_master_password(&self, password: &str) -> bool {
        // Compare with the stored master password used to unlock the vault
        password == self.master_password
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

                                            // Check if locked out
                                            let locked_out = self.is_locked_out();
                                            if locked_out {
                                                let remaining = self.lockout_remaining_secs();
                                                ui.colored_label(
                                                    Color32::from_rgb(240, 105, 105),
                                                    format!(
                                                        "Too many failed attempts. Try again in {} seconds.",
                                                        remaining
                                                    ),
                                                );
                                            }

                                            let can_unlock = all_met && !locked_out;
                                            if ui.add_enabled(can_unlock, button).clicked() {
                                                match self.unlock_vault() {
                                                    Ok(()) => {
                                                        self.vault_unlocked = true;
                                                        self.reset_login_attempts();
                                                        self.record_activity();
                                                        self.set_status("Vault unlocked");
                                                    }
                                                    Err(error) => {
                                                        self.record_failed_login();
                                                        if self.is_locked_out() {
                                                            self.set_status(format!(
                                                                "Account locked for {} seconds",
                                                                LOCKOUT_DURATION_SECS
                                                            ));
                                                        } else {
                                                            self.set_status(format!(
                                                                "Unable to unlock vault: {error}"
                                                            ));
                                                        }
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
                self.lock_vault();
                self.status_message = Some("Vault locked".to_string());
                self.status_message_time = Some(Instant::now());
            }
            if ui.button("Generate password").clicked() {
                self.selected_category = 1; // Switch to password generator tab
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

        // Collect matching entry indices to avoid borrow conflicts
        let matching_indices: Vec<usize> = self
            .vault_entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                if query.is_empty() {
                    true
                } else {
                    entry.title.to_lowercase().contains(&query)
                        || entry.username.to_lowercase().contains(&query)
                        || entry.url.to_lowercase().contains(&query)
                }
            })
            .map(|(i, _)| i)
            .collect();

        if matching_indices.is_empty() {
            ui.label("No entries match your search yet.");
        } else {
            // Track which entry's password was copied
            let mut copied_password: Option<String> = None;

            for &idx in &matching_indices {
                let entry = &self.vault_entries[idx];
                let title = entry.title.clone();
                let username = entry.username.clone();
                let url = entry.url.clone();
                let notes = entry.notes.clone();
                let last_updated = entry.last_updated.clone();
                let password = entry.password.clone();

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&title).strong());
                        ui.separator();
                        ui.label(&username);
                    });
                    if !url.is_empty() {
                        ui.label(format!("URL: {}", url));
                    }
                    if !notes.is_empty() {
                        ui.label(format!("Notes: {}", notes));
                    }
                    ui.horizontal(|ui| {
                        ui.label(format!("Updated: {}", last_updated));
                        if ui.button("Copy password").clicked() {
                            copied_password = Some(password.clone());
                        }
                    });
                });
                ui.add_space(8.0);
            }

            // Handle password copy outside of the borrow
            if let Some(password) = copied_password {
                if self.settings.require_master_on_copy {
                    // Require re-authentication before copying
                    self.pending_copy_password = Some(password);
                    self.show_reauth_modal = true;
                    self.reauth_password.clear();
                } else {
                    self.copy_with_timeout(&password, ctx);
                    let timeout = self.settings.clipboard_timeout_seconds;
                    if timeout > 0 {
                        self.set_status(format!("Password copied (auto-clears in {}s)", timeout));
                    } else {
                        self.set_status("Password copied to clipboard");
                    }
                }
                self.record_activity();
            }
        }

        // Render re-authentication modal if needed
        if self.show_reauth_modal {
            self.render_reauth_modal(ctx);
        }

        // Render delete confirmation dialog if needed
        if self.show_delete_confirm {
            self.render_delete_confirm_modal(ctx);
        }
    }

    fn render_reauth_modal(&mut self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("reauth_overlay"),
        ));
        painter.rect_filled(ctx.available_rect(), 0.0, Color32::from_black_alpha(150));

        egui::Window::new("Re-authentication Required")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Please enter your master password to copy this password.");
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.reauth_password)
                        .password(true)
                        .hint_text("Master password"),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_reauth_modal = false;
                        self.reauth_password.clear();
                        self.pending_copy_password = None;
                    }
                    if ui.button("Confirm").clicked() {
                        if self.verify_master_password(&self.reauth_password) {
                            if let Some(password) = self.pending_copy_password.take() {
                                self.copy_with_timeout(&password, ctx);
                                let timeout = self.settings.clipboard_timeout_seconds;
                                if timeout > 0 {
                                    self.set_status(format!(
                                        "Password copied (auto-clears in {}s)",
                                        timeout
                                    ));
                                } else {
                                    self.set_status("Password copied to clipboard");
                                }
                            }
                            self.show_reauth_modal = false;
                        } else {
                            self.set_status("Incorrect master password");
                        }
                        self.reauth_password.clear();
                    }
                });
            });
    }

    fn render_delete_confirm_modal(&mut self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("delete_confirm_overlay"),
        ));
        painter.rect_filled(ctx.available_rect(), 0.0, Color32::from_black_alpha(150));

        egui::Window::new("Confirm Deletion")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Are you sure you want to delete this entry? This cannot be undone.");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_delete_confirm = false;
                        self.pending_delete_index = None;
                    }
                    let delete_button = egui::Button::new(
                        RichText::new("Delete").color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(220, 53, 69));
                    if ui.add(delete_button).clicked() {
                        // TODO: Implement actual deletion when we add delete functionality
                        self.show_delete_confirm = false;
                        self.pending_delete_index = None;
                        self.set_status("Entry deleted");
                    }
                });
            });
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
                    .selected_text(match self.settings.theme_index {
                        1 => "Night Bloom",
                        2 => "Pond Light",
                        _ => "Classic Green",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.settings.theme_index, 0, "Classic Green");
                        ui.selectable_value(&mut self.settings.theme_index, 1, "Night Bloom");
                        ui.selectable_value(&mut self.settings.theme_index, 2, "Pond Light");
                    });
                ui.add_space(6.0);
                ui.label(RichText::new("Vault protection").strong());
                ui.add(
                    egui::Slider::new(&mut self.settings.auto_lock_minutes, 0..=60)
                        .text("Auto-lock (minutes, 0 = disabled)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.settings.clipboard_timeout_seconds, 0..=120)
                        .text("Clipboard clear (seconds, 0 = disabled)"),
                );
                ui.checkbox(
                    &mut self.settings.send_security_alerts,
                    "Send security notifications",
                );
                ui.checkbox(
                    &mut self.settings.require_master_on_copy,
                    "Require master password to copy",
                );
                ui.checkbox(
                    &mut self.settings.exclude_ambiguous_chars,
                    "Exclude ambiguous chars in generator",
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        self.show_settings = false;
                    }
                    if ui.button("Save settings").clicked() {
                        self.save_settings();
                        self.show_settings = false;
                        self.set_status("Settings saved");
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
                egui::Slider::new(&mut self.settings.auto_lock_minutes, 0..=30)
                    .text("Auto-lock (minutes, 0 = disabled)"),
            );
            ui.checkbox(
                &mut self.settings.require_master_on_copy,
                "Require master password on copy",
            );
            if ui.button("Save security settings").clicked() {
                self.save_settings();
                self.set_status("Security settings saved");
            }
        });
        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(RichText::new("Recovery").strong());
            ui.label("Recovery email");
            ui.add(egui::TextEdit::singleline(
                &mut self.security_recovery_email,
            ));
            if ui.button("Update recovery email").clicked() {
                self.set_status("Recovery email updated");
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
                self.set_status("Device revoked");
            }
            if ui.button("Add current device").clicked() {
                self.security_trusted_devices
                    .push("New device • Active now".to_string());
                self.set_status("Device added");
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
                last_updated: Self::format_entry_timestamp(entry.updated_at),
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

    fn format_entry_timestamp(timestamp: u64) -> String {
        format_timestamp_relative(timestamp)
    }

    fn load_or_create_key(&self, master_password: &str) -> Result<(KeyMaterial, KeyFile)> {
        let path = key_path(&self.config);
        if path.exists() {
            return load_key(&path, Some(master_password));
        }
        let params = KeyDerivationParams::generate();
        let key = derive_key(master_password, &params)?;
        let key_file = KeyFile::from_kdf(params);
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
            ui.checkbox(
                &mut self.settings.exclude_ambiguous_chars,
                "Exclude ambiguous characters (l, I, O, 0, 1)",
            );

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
                    self.copy_with_timeout(&password, ctx);
                    self.set_status("New password generated and copied");
                    self.record_activity();
                }
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Generated password").strong());
                if ui.button("Copy").clicked() {
                    let password = self.generated_password.clone();
                    self.copy_with_timeout(&password, ctx);
                    self.set_status("Password copied to clipboard");
                    self.record_activity();
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
            let chars = if self.settings.exclude_ambiguous_chars {
                "abcdefghijkmnopqrstuvwxyz" // excludes 'l'
            } else {
                "abcdefghijklmnopqrstuvwxyz"
            };
            charset.push_str(chars);
        }
        if self.generator_uppercase {
            let chars = if self.settings.exclude_ambiguous_chars {
                "ABCDEFGHJKLMNPQRSTUVWXYZ" // excludes 'I', 'O'
            } else {
                "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
            };
            charset.push_str(chars);
        }
        if self.generator_digits {
            let chars = if self.settings.exclude_ambiguous_chars {
                "23456789" // excludes '0', '1'
            } else {
                "0123456789"
            };
            charset.push_str(chars);
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

