//! Effects level: everything, or Lite for slow and older machines (no
//! animations, transitions or rolling numbers, and live prices refresh
//! less often). Remembered on this device. Auto picks Lite when the device
//! asks for reduced motion or looks low-end.

use dioxus::{core::Runtime, prelude::*};

const STORAGE_KEY: &str = "akhsakov.effects";

/// How often streamed prices reach the screen, in milliseconds.
const PRICE_FLUSH_MS: u32 = 1000;
const PRICE_FLUSH_LITE_MS: u32 = 3000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effects {
    /// Lite on reduced-motion or low-end devices, full effects otherwise.
    Auto,
    Full,
    Lite,
}

impl Effects {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Full, Self::Lite];

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Full => "Full effects",
            Self::Lite => "Lite",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Full => "full",
            Self::Lite => "lite",
        }
    }
}

static EFFECTS: GlobalSignal<Effects> = Signal::global(|| Effects::Auto);
/// Set when the device prefers reduced motion or looks low-end.
static LOW_END: GlobalSignal<bool> = Signal::global(|| false);

fn resolve(effects: Effects, low_end: bool) -> bool {
    match effects {
        Effects::Auto => low_end,
        Effects::Full => false,
        Effects::Lite => true,
    }
}

/// Whether Lite is on. Reading it subscribes the calling component.
pub fn lite() -> bool {
    Runtime::try_current().is_some() && resolve(EFFECTS(), LOW_END())
}

/// [`lite`] without subscribing, for timers and scripts.
pub fn lite_now() -> bool {
    Runtime::try_current().is_some() && resolve(*EFFECTS.peek(), *LOW_END.peek())
}

/// The chosen level (Auto may still mean Lite; see [`lite`]).
pub fn current() -> Effects {
    EFFECTS()
}

/// Whether Auto resolves to Lite on this device.
pub fn low_end() -> bool {
    LOW_END()
}

/// Delay before streamed prices are shown: longer in Lite, so slow
/// machines redraw less often.
pub fn price_flush_ms() -> u32 {
    if lite_now() {
        PRICE_FLUSH_LITE_MS
    } else {
        PRICE_FLUSH_MS
    }
}

pub fn set_effects(effects: Effects) {
    *EFFECTS.write() = effects;
    document::eval(&format!(
        "try {{ localStorage.setItem('{STORAGE_KEY}', '{}'); }} catch (e) {{}}",
        effects.key()
    ));
    tag_page();
}

/// Tags `<html>` with `lite`, which the rules in `input.css` and the chart
/// scripts look for.
fn tag_page() {
    document::eval(&format!(
        "document.documentElement.classList.toggle('lite', {});",
        lite_now()
    ));
}

/// Loads the saved level and checks the device. Call once from the app root.
pub fn use_effects_init() {
    use_future(|| async {
        let result = document::eval(&format!(
            "let e = 'auto'; try {{ e = localStorage.getItem('{STORAGE_KEY}') || 'auto'; }} catch (_) {{}}
             const n = navigator;
             const low = !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches)
                 || (n.hardwareConcurrency > 0 && n.hardwareConcurrency <= 2)
                 || (n.deviceMemory > 0 && n.deviceMemory <= 2)
                 || !!(n.connection && n.connection.saveData);
             return [e, low];"
        ))
        .join::<(String, bool)>()
        .await;
        if let Ok((saved, low)) = result {
            *LOW_END.write() = low;
            if let Some(e) = Effects::ALL.into_iter().find(|e| e.key() == saved) {
                *EFFECTS.write() = e;
            }
        }
        tag_page();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_device() {
        assert!(resolve(Effects::Auto, true));
        assert!(!resolve(Effects::Auto, false));
        assert!(!resolve(Effects::Full, true));
        assert!(resolve(Effects::Lite, false));
        // Outside an app there's no state: full effects.
        assert!(!lite_now());
    }
}
