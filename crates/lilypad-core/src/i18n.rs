//! The interface strings of the desktop app and the TUI, in English and
//! French.
//!
//! Lilypad's own strings live in `config/i18n/en.json` and `fr.json` at the
//! root of the repository, embedded at build time. The labels every Colony
//! program shares (the Preferences skeleton, themes, accents) come from
//! `colony_ui::i18n`, and [`t`] falls through to them, so one call reaches
//! both. The CLI stays English: it is a scripting interface.
//!
//! The active locale is colony-ui's, so setting it once with [`set_locale`]
//! switches Lilypad's strings and the shared labels together.

use colony_ui::i18n as shared;
pub use colony_ui::i18n::{locale, set_locale, Locale};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;

const EN: &str = include_str!("../../../config/i18n/en.json");
const FR: &str = include_str!("../../../config/i18n/fr.json");

fn table(locale: Locale) -> &'static HashMap<String, String> {
    static EN_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    static FR_TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    let (cell, source) = match locale {
        Locale::En => (&EN_TABLE, EN),
        Locale::Fr => (&FR_TABLE, FR),
    };
    // Both files are checked by the tests below, so a panic here means the
    // crate was built from a broken checkout.
    cell.get_or_init(|| serde_json::from_str(source).expect("embedded locale file is valid JSON"))
}

/// The string for `key` in the active locale.
///
/// Lilypad's table first (English when the active locale lacks the key), then
/// the shared Colony labels, which end with the key itself. A missing key is a
/// bug: debug builds stop on it.
pub fn t(key: &str) -> &'static str {
    let found = table(locale())
        .get(key)
        .or_else(|| table(Locale::En).get(key));
    if let Some(text) = found {
        return text;
    }
    debug_assert!(
        shared::all(Locale::En).any(|(k, _)| k == key),
        "missing i18n key {key:?}"
    );
    shared::t(key)
}

/// [`t`], with every `{name}` placeholder replaced by its value.
pub fn tf(key: &str, args: &[(&str, &dyn Display)]) -> String {
    args.iter().fold(t(key).to_string(), |text, (name, value)| {
        text.replace(&format!("{{{name}}}"), &value.to_string())
    })
}

/// The system's language tag (`fr-FR`, `en_US`, ...), or `en` when the system
/// does not say.
pub fn system_language() -> String {
    sys_locale::get_locale().unwrap_or_else(|| "en".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys(locale: Locale) -> BTreeSet<&'static str> {
        table(locale).keys().map(String::as_str).collect()
    }

    #[test]
    fn fr_and_en_have_identical_key_sets() {
        let (en, fr) = (keys(Locale::En), keys(Locale::Fr));
        assert_eq!(
            en.difference(&fr).collect::<Vec<_>>(),
            Vec::<&&str>::new(),
            "missing in fr.json"
        );
        assert_eq!(
            fr.difference(&en).collect::<Vec<_>>(),
            Vec::<&&str>::new(),
            "missing in en.json"
        );
    }

    #[test]
    fn no_string_is_empty() {
        for locale in [Locale::En, Locale::Fr] {
            for (key, value) in table(locale) {
                assert!(!value.trim().is_empty(), "{key} is empty in {locale:?}");
            }
        }
    }

    #[test]
    fn lilypad_keys_never_shadow_the_shared_labels() {
        let ours = keys(Locale::En);
        for (key, _) in shared::all(Locale::En) {
            assert!(!ours.contains(key), "{key} is a shared colony-ui label");
        }
    }

    #[test]
    fn placeholders_match_between_locales() {
        fn placeholders(s: &str) -> BTreeSet<&str> {
            s.split('{')
                .skip(1)
                .filter_map(|part| part.split_once('}').map(|(name, _)| name))
                .collect()
        }
        for (key, en) in table(Locale::En) {
            let fr = &table(Locale::Fr)[key];
            assert_eq!(placeholders(en), placeholders(fr), "{key}");
        }
    }

    #[test]
    fn tf_fills_the_placeholders() {
        assert_eq!(
            tf("preferences_language_desc", &[]),
            t("preferences_language_desc")
        );
        let text = tf("status_copied", &[("what", &"x"), ("secs", &5)]);
        assert!(!text.contains('{'), "{text}");
    }

    #[test]
    fn a_shared_label_is_reachable() {
        assert!(!t("settings_title").is_empty());
    }
}
