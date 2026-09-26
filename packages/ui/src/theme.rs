//! Colour theme (Catppuccin flavours), remembered on this device.

use dioxus::{core::Runtime, prelude::*};

const STORAGE_KEY: &str = "akhsakov.theme";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    /// Latte or Mocha, following the device's light / dark setting.
    System,
    Latte,
    Frappe,
    Macchiato,
    Mocha,
}

impl Theme {
    pub const ALL: [Self; 5] = [Self::System, Self::Latte, Self::Frappe, Self::Macchiato, Self::Mocha];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Match device",
            Self::Latte => "Light · Latte",
            Self::Frappe => "Frappé",
            Self::Macchiato => "Macchiato",
            Self::Mocha => "Dark · Mocha",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Latte => "latte",
            Self::Frappe => "frappe",
            Self::Macchiato => "macchiato",
            Self::Mocha => "mocha",
        }
    }
}

static THEME: GlobalSignal<Theme> = Signal::global(|| Theme::Mocha);
static PREFERS_DARK: GlobalSignal<bool> = Signal::global(|| true);

/// The Catppuccin class for the page root, e.g. `mocha`.
pub fn theme_class() -> &'static str {
    if Runtime::try_current().is_none() {
        return "mocha";
    }
    match THEME() {
        Theme::System if PREFERS_DARK() => "mocha",
        Theme::System => "latte",
        t => t.key(),
    }
}

/// The active flavour's base colour as RGB, for painting outside the page
/// (the desktop webview's own background). Reactive, like [`theme_class`].
pub fn base_color() -> (u8, u8, u8) {
    match theme_class() {
        "latte" => (0xef, 0xf1, 0xf5),
        "frappe" => (0x30, 0x34, 0x46),
        "macchiato" => (0x24, 0x27, 0x3a),
        _ => (0x1e, 0x1e, 0x2e),
    }
}

pub fn current() -> Theme {
    THEME()
}

pub fn set_theme(theme: Theme) {
    *THEME.write() = theme;
    document::eval(&format!(
        "try {{ localStorage.setItem('{STORAGE_KEY}', '{}'); }} catch (e) {{}}",
        theme.key()
    ));
}

/// Loads the saved theme and the device's light / dark preference. Call
/// once from the app root.
pub fn use_theme_init() {
    use_future(|| async {
        let result = document::eval(&format!(
            "let t = 'system'; try {{ t = localStorage.getItem('{STORAGE_KEY}') || 'system'; }} catch (e) {{}}
             const dark = !window.matchMedia || window.matchMedia('(prefers-color-scheme: dark)').matches;
             return [t, dark];"
        ))
        .join::<(String, bool)>()
        .await;
        if let Ok((saved, dark)) = result {
            *PREFERS_DARK.write() = dark;
            if let Some(t) = Theme::ALL.into_iter().find(|t| t.key() == saved) {
                *THEME.write() = t;
            }
        }
    });
}
