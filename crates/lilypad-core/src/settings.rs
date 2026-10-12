//! Persisted user preferences, shared by all frontends.
//!
//! Stored in `preferences.json` in the config root (see [`crate::paths`]).
//! Holds the Colony Appearance and Accessibility preferences every Colony
//! program shares ([`StandardPreferences`], flattened into the same object),
//! app-level policy (auto-lock and clipboard timeouts, the last active vault)
//! and a few layout values the desktop app interprets. Nothing here is a
//! secret, so it is stored in cleartext with the same 0600 discipline as
//! everything else.

use crate::error::{AppError, Result};
use crate::paths::PREFERENCES_FILE;
use colony_ui::preferences::StandardPreferences;
use serde::{Deserialize, Serialize};
use std::path::Path;

fn default_auto_lock() -> u64 {
    5
}
fn default_clipboard() -> u64 {
    20
}
fn default_sidebar_ratio() -> f32 {
    0.2
}
fn default_list_ratio() -> f32 {
    0.34
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Theme, accent, text sizes, motion and the other preferences every
    /// Colony program shares.
    #[serde(flatten)]
    pub standard: StandardPreferences,
    /// Idle minutes before a session auto-locks.
    #[serde(default = "default_auto_lock")]
    pub auto_lock_minutes: u64,
    /// Seconds a copied secret stays on the clipboard before it is cleared.
    #[serde(default = "default_clipboard")]
    pub clipboard_clear_secs: u64,
    /// The interface language as a tag (`en`, `fr`); `None` follows the
    /// system.
    #[serde(default)]
    pub language: Option<String>,
    /// The vault to preselect on the unlock screen.
    #[serde(default)]
    pub active_vault: Option<String>,
    /// Opaque density/variation index (interpreted by the frontend).
    #[serde(default)]
    pub density: u32,
    /// Sidebar width as a fraction of the window (0.0..1.0).
    #[serde(default = "default_sidebar_ratio")]
    pub sidebar_ratio: f32,
    /// Item-list width as a fraction of the window (0.0..1.0).
    #[serde(default = "default_list_ratio")]
    pub list_ratio: f32,
    /// Reopen the last-used vault on the unlock screen.
    #[serde(default = "default_true")]
    pub restore_last_vault: bool,
    /// Default sidebar filter applied on unlock (0 = All, 1 = Favorites).
    #[serde(default)]
    pub default_filter: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: default_auto_lock(),
            clipboard_clear_secs: default_clipboard(),
            language: None,
            active_vault: None,
            standard: StandardPreferences::default(),
            density: 0,
            sidebar_ratio: default_sidebar_ratio(),
            list_ratio: default_list_ratio(),
            restore_last_vault: true,
            default_filter: 0,
        }
    }
}

impl Settings {
    /// Loads the preferences in `config_dir`, falling back to defaults on a
    /// missing or unreadable file (preferences are convenience, never a reason
    /// to block the app).
    pub fn load(config_dir: &Path) -> Self {
        std::fs::read(config_dir.join(PREFERENCES_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
            .unwrap_or_default()
    }

    /// Persists the preferences in `config_dir`.
    pub fn save(&self, config_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(config_dir)
            .map_err(|e| AppError::Io(format!("failed to create config dir: {e}")))?;
        self.save_to(&config_dir.join(PREFERENCES_FILE))
    }

    /// Writes the preferences to `path` atomically (temp file + rename).
    pub(crate) fn save_to(&self, path: &Path) -> Result<()> {
        let dir = path
            .parent()
            .ok_or_else(|| AppError::Io(format!("{} has no parent", path.display())))?;
        let json = serde_json::to_vec_pretty(self)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)
            .map_err(|e| AppError::Io(format!("failed to create temp settings: {e}")))?;
        {
            use std::io::Write as _;
            tmp.write_all(&json)
                .map_err(|e| AppError::Io(format!("failed to write settings: {e}")))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o600));
        }
        tmp.persist(path)
            .map_err(|e| AppError::Io(format!("failed to persist settings: {e}")))?;
        Ok(())
    }

    /// Reads the `settings.json` of releases up to 0.2. Its fields carry over
    /// as they are, except the theme: an index into the desktop app's own 15
    /// palettes, mapped to the nearest Colony theme.
    pub(crate) fn from_legacy_json(bytes: &[u8]) -> serde_json::Result<Self> {
        #[derive(Deserialize)]
        struct LegacyTheme {
            #[serde(default)]
            theme: usize,
        }
        let mut settings: Settings = serde_json::from_slice(bytes)?;
        let LegacyTheme { theme } = serde_json::from_slice(bytes)?;
        let (family, variant) = legacy_theme(theme);
        settings.standard.selected_theme = family.to_string();
        settings.standard.selected_variant = variant.to_string();
        Ok(settings)
    }

    /// The interface locale: the chosen language, or the system's.
    pub fn locale(&self) -> crate::i18n::Locale {
        let tag = self
            .language
            .clone()
            .unwrap_or_else(crate::i18n::system_language);
        crate::i18n::Locale::from_tag(&tag)
    }

    /// Auto-lock duration, or `None` if disabled (0).
    pub fn auto_lock_duration(&self) -> Option<std::time::Duration> {
        if self.auto_lock_minutes == 0 {
            None
        } else {
            Some(std::time::Duration::from_secs(self.auto_lock_minutes * 60))
        }
    }
}

/// The Colony theme (family, variant) closest to each palette the desktop app
/// shipped up to 0.2, in the order its settings stored them. An unknown index
/// gets the first one, as those releases did.
fn legacy_theme(index: usize) -> (&'static str, &'static str) {
    const THEMES: [(&str, &str); 15] = [
        ("everforest", "dark"),  // Classic Green: dark, green accent
        ("rosepine", "main"),    // Night Bloom: near black, violet accent
        ("everforest", "light"), // Pond Light: light, green accent
        ("gruvbox", "dark"),     // Solar Flare: dark brown, amber accent
        ("nord", "dark"),        // Arctic: dark blue, cyan accent
        ("catppuccin", "mocha"),
        ("catppuccin", "latte"),
        ("gruvbox", "dark"),
        ("gruvbox", "light"),
        ("everblush", "dark"),
        ("kanagawa", "dark"),
        ("nord", "dark"),
        ("dracula", "dark"),
        ("solarized", "dark"),
        ("tokyonight", "night"),
    ];
    THEMES.get(index).copied().unwrap_or(THEMES[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_legacy_theme_maps_to_a_colony_theme() {
        for index in 0..16 {
            let (family, variant) = legacy_theme(index);
            let known = colony_ui::THEME_FAMILIES
                .iter()
                .find(|f| f.key == family)
                .is_some_and(|f| f.variants.iter().any(|v| v.key == variant));
            assert!(known, "index {index}: {family}/{variant}");
        }
        assert_eq!(legacy_theme(99), legacy_theme(0));
    }

    #[test]
    fn legacy_settings_keep_their_values() {
        let legacy = br#"{"auto_lock_minutes": 0, "clipboard_clear_secs": 45,
            "active_vault": "work", "theme": 12, "density": 3,
            "restore_last_vault": false, "default_filter": 1}"#;
        let s = Settings::from_legacy_json(legacy).unwrap();
        assert_eq!(s.auto_lock_minutes, 0);
        assert_eq!(s.clipboard_clear_secs, 45);
        assert_eq!(s.active_vault.as_deref(), Some("work"));
        assert_eq!(s.density, 3);
        assert!(!s.restore_last_vault);
        assert_eq!(s.default_filter, 1);
        assert_eq!(
            (
                s.standard.selected_theme.as_str(),
                s.standard.selected_variant.as_str()
            ),
            ("dracula", "dark")
        );
    }

    #[test]
    fn preferences_round_trip_as_one_flat_object() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut s = Settings::default();
        s.standard.selected_theme = "nord".into();
        s.standard.high_contrast = true;
        s.auto_lock_minutes = 7;
        s.save(dir.path()).unwrap();

        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join(PREFERENCES_FILE)).unwrap())
                .unwrap();
        assert_eq!(raw["selected_theme"], "nord");
        assert_eq!(raw["auto_lock_minutes"], 7);

        let back = Settings::load(dir.path());
        assert_eq!(back.standard, s.standard);
        assert_eq!(back.auto_lock_minutes, 7);
    }

    #[test]
    fn a_chosen_language_wins_over_the_system() {
        let mut s = Settings {
            language: Some("fr".to_string()),
            ..Settings::default()
        };
        assert_eq!(s.locale(), crate::i18n::Locale::Fr);
        s.language = Some("en-GB".to_string());
        assert_eq!(s.locale(), crate::i18n::Locale::En);
    }

    #[test]
    fn an_unreadable_file_gives_the_defaults() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join(PREFERENCES_FILE), b"not json").unwrap();
        let s = Settings::load(dir.path());
        assert_eq!(s.standard, StandardPreferences::default());
        assert_eq!(s.auto_lock_minutes, 5);
    }
}
