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

    /// The UI's source files, with their text (the translations aside).
    fn ui_sources() -> Vec<(std::path::PathBuf, String)> {
        let mut files = vec![];
        let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("i18n.rs") && !path.ends_with("i18n/thai.rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    files.push((path, text));
                }
            }
        }
        files
    }

    /// Every literal passed to `tr` or `trf` in the UI's source has a Thai
    /// entry, so a new string can't ship in English only.
    #[test]
    fn every_translated_string_has_a_thai_entry() {
        let mut missing = vec![];
        for (path, text) in ui_sources() {
            for key in tr_literals(&text) {
                if !THAI.contains_key(key.as_str()) {
                    missing.push(format!("{}: {key:?}", path.display()));
                }
            }
        }
        assert!(missing.is_empty(), "add these to i18n/thai.rs:\n{}", missing.join("\n"));
    }

    /// Titles, subtitles, hints, labels, placeholders and warnings aren't
    /// built with `format!` around English words, which `tr` never sees.
    #[test]
    fn shown_text_is_not_formatted_in_english() {
        let mut found = vec![];
        for (path, text) in ui_sources() {
            for template in english_format_props(&text) {
                found.push(format!("{}: {template:?}", path.display()));
            }
        }
        assert!(found.is_empty(), "use trf(...) with a Thai entry for:\n{}", found.join("\n"));
    }

    /// The `format!` templates given to text props in `source` that have
    /// English words outside their `{placeholders}`.
    fn english_format_props(source: &str) -> Vec<String> {
        let mut found = vec![];
        for prop in ["title", "subtitle", "hint", "label", "placeholder", "warning"] {
            let call = format!("{prop}: format!(\"");
            for (at, _) in source.match_indices(&call) {
                if source[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let body = &source[at + call.len()..];
                let template = &body[..body.find('"').unwrap_or(body.len())];
                let mut words = String::new();
                let mut depth = 0;
                for c in template.chars() {
                    match c {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        c if depth == 0 => words.push(c),
                        _ => {}
                    }
                }
                if words.split(|c: char| !c.is_ascii_alphabetic()).any(|w| w.len() >= 2) {
                    found.push(template.to_string());
                }
            }
        }
        found
    }

    /// The string literals passed straight to `tr(` / `trf(` in `source`.
    fn tr_literals(source: &str) -> Vec<String> {
        let mut found = vec![];
        for call in ["tr(", "trf("] {
            for (at, _) in source.match_indices(call) {
                let before = source[..at].chars().next_back();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let rest = source[at + call.len()..].trim_start();
                let Some(body) = rest.strip_prefix('"') else { continue };
                let mut text = String::new();
                let mut chars = body.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => text.push('\n'),
                            Some(other) => text.push(other),
                            None => break,
                        },
                        c => text.push(c),
                    }
                }
                found.push(text);
            }
        }
        found
    }

    #[test]
    fn finds_english_format_props() {
        let code = r#"subtitle: format!("{} in total", x) label: format!("{} A", tr("Volatility")) aria_label: format!("Color {hex}") hint: format!("±{}", m) title: format!("{count} shown")"#;
        assert_eq!(english_format_props(code), ["shown", "{} in total"].map(|t| if t == "shown" { "{count} shown" } else { t }));
    }

    #[test]
    fn finds_translated_literals() {
        let code = r#"tr("Save") attr("x") trf("{} \"quoted\"", &[&a]) crate::i18n::tr( "Spaced")"#;
        assert_eq!(tr_literals(code), ["Save", "Spaced", "{} \"quoted\""]);
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
