//! Getting data out: file downloads, clipboard copies and printing.

use dioxus::prelude::*;

/// Encodes text as a JavaScript string literal.
fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Saves `content` as a file (browser download).
pub fn download(filename: &str, mime: &str, content: &str) {
    let script = format!(
        "const blob = new Blob([{}], {{ type: {} }});
         const a = document.createElement('a');
         a.href = URL.createObjectURL(blob);
         a.download = {};
         document.body.appendChild(a);
         a.click();
         setTimeout(() => {{ URL.revokeObjectURL(a.href); a.remove(); }}, 1000);",
        js_string(content),
        js_string(mime),
        js_string(filename),
    );
    document::eval(&script);
}

/// Copies text to the clipboard; resolves to whether it worked.
pub async fn copy_to_clipboard(text: &str) -> bool {
    let script = format!(
        "try {{ await navigator.clipboard.writeText({}); return true; }} catch (e) {{ return false; }}",
        js_string(text)
    );
    document::eval(&script)
        .join::<bool>()
        .await
        .unwrap_or(false)
}

/// Prints the current page in the light Latte theme (dark text on white),
/// then switches back. Browsers offer "Save as PDF" in the print dialog.
pub fn print_report() {
    document::eval(
        "const dark = [...document.querySelectorAll('.mocha')];
         dark.forEach(e => e.classList.replace('mocha', 'latte'));
         const restore = () => {
             dark.forEach(e => e.classList.replace('latte', 'mocha'));
             window.removeEventListener('afterprint', restore);
         };
         window.addEventListener('afterprint', restore);
         setTimeout(() => window.print(), 50);",
    );
}

/// Download + Copy buttons for a generated CSV.
#[component]
pub fn ExportButtons(filename: String, csv: String) -> Element {
    let mut copied = use_signal(|| None::<bool>);
    let (name, body) = (filename.clone(), csv.clone());
    rsx! {
        span { class: "inline-flex gap-1 print:hidden",
            crate::page::GhostButton { label: "⇩ CSV", onclick: move |_| download(&name, "text/csv", &body) }
            crate::page::GhostButton {
                label: match copied() {
                    Some(true) => "Copied",
                    Some(false) => "Copy failed",
                    None => "Copy",
                },
                onclick: move |_| {
                    let text = csv.clone();
                    spawn(async move { copied.set(Some(copy_to_clipboard(&text).await)) });
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_for_javascript() {
        assert_eq!(js_string("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert_eq!(js_string("\u{1}"), "\"\\u0001\"");
    }
}
