//! Interface language: English (built in) or Thai. `tr("English text")`
//! returns the translation for the chosen language, falling back to the
//! English text. Reading the language subscribes the calling component, so
//! switching re-renders the page. The choice is remembered on this device.

use dioxus::{core::Runtime, prelude::*};
use std::{collections::HashMap, sync::LazyLock};

mod thai;

const STORAGE_KEY: &str = "akhsakov.lang";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Th,
}

impl Lang {
    pub const ALL: [Self; 2] = [Self::En, Self::Th];

    pub fn label(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Th => "ไทย",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Th => "th",
        }
    }
}

static LANG: GlobalSignal<Lang> = Signal::global(|| Lang::En);

static THAI: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| thai::PAIRS.iter().copied().collect());

/// `text` in the current language.
pub fn tr(text: &'static str) -> &'static str {
    if Runtime::try_current().is_none() {
        return text;
    }
    match LANG() {
        Lang::En => text,
        Lang::Th => THAI.get(text).copied().unwrap_or(text),
    }
}

/// Like [`tr`] for text that isn't a literal (labels kept in tables and
/// lists): the translation if there is one, otherwise `text` unchanged.
pub fn tr_str(text: &str) -> &str {
    if Runtime::try_current().is_none() {
        return text;
    }
    match LANG() {
        Lang::En => text,
        Lang::Th => THAI.get(text).copied().unwrap_or(text),
    }
}

/// [`tr`] for a sentence with values in it: each `{}` in the (English)
/// template is replaced, in order, by the next of `args`. The translation
/// keeps the `{}`s, so word order can differ between languages.
pub fn trf(template: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut parts = tr(template).split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for part in parts {
        if let Some(arg) = args.next() {
            out.push_str(&arg.to_string());
        }
        out.push_str(part);
    }
    out
}

pub fn current() -> Lang {
    LANG()
}

pub fn set_language(lang: Lang) {
    *LANG.write() = lang;
    document::eval(&format!(
        "try {{ localStorage.setItem('{STORAGE_KEY}', '{}'); document.documentElement.lang = '{}'; }} catch (e) {{}}",
        lang.key(),
        lang.key()
    ));
}

/// Loads the saved language. Call once from the app root.
pub fn use_language_init() {
    use_future(|| async {
        let saved = document::eval(&format!(
            "try {{ return localStorage.getItem('{STORAGE_KEY}') || ''; }} catch (e) {{ return ''; }}"
        ))
        .join::<String>()
        .await
        .unwrap_or_default();
        if let Some(lang) = Lang::ALL.into_iter().find(|l| l.key() == saved) {
            set_language(lang);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_has_no_duplicate_keys() {
        let mut keys: Vec<&str> = thai::PAIRS.iter().map(|(en, _)| *en).collect();
        let n = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), n);
        assert_eq!(THAI.get("Settings"), Some(&"ตั้งค่า"));
        // Outside an app there's no language state: English.
        assert_eq!(tr("Settings"), "Settings");
        assert_eq!(trf("{} would be worth {}", &[&"VOO", &"$5"]), "VOO would be worth $5");
        assert_eq!(tr_str("Largest"), "Largest");
    }

    #[test]
    fn templates_keep_their_placeholders() {
        for (en, th) in thai::PAIRS {
            assert_eq!(en.matches("{}").count(), th.matches("{}").count(), "{en}");
            for name in ["{n}", "{name}", "{count}"] {
                assert_eq!(en.contains(name), th.contains(name), "{en}");
            }
        }
    }
}
