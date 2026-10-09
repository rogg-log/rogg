//! Shortcut labels per chrome style. The keymap writes Mac labels (`⌘ O`, `⇧ ⌘ T`, `⌘ ↑ / ⌘ ↓`); other styles show
//! `Ctrl+O`, `Ctrl+Shift+T`, `Ctrl+↑ / Ctrl+↓` (modifiers in Ctrl, Alt, Shift order).

use crate::theme::ChromeStyle;

pub fn shortcut_label(mac_label: &str, chrome: ChromeStyle) -> String {
    if chrome.is_mac() {
        return mac_label.to_string();
    }
    mac_label.split(" / ").map(chord).collect::<Vec<_>>().join(" / ")
}

/// One chord: space-separated Mac modifier glyphs, then the key.
fn chord(mac: &str) -> String {
    let (mut ctrl, mut alt, mut shift) = (false, false, false);
    let mut key = Vec::new();
    for token in mac.split(' ') {
        match token {
            "⌘" | "⌃" => ctrl = true,
            "⌥" => alt = true,
            "⇧" => shift = true,
            other => key.push(other),
        }
    }
    let mut parts: Vec<&str> = Vec::new();
    if ctrl {
        parts.push("Ctrl");
    }
    if alt {
        parts.push("Alt");
    }
    if shift {
        parts.push("Shift");
    }
    let key = key.join(" ");
    if parts.is_empty() {
        key
    } else {
        format!("{}+{}", parts.join("+"), key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ChromeStyle::{Linux, Mac, Windows};

    #[test]
    fn mac_labels_are_kept() {
        for l in ["⌘ O", "⌘ ↑ / ⌘ ↓", "⌥ ⌘ C", "⇧ ⌘ T", "Esc", "⌘ 1…9"] {
            assert_eq!(shortcut_label(l, Mac), l);
        }
    }

    #[test]
    fn labels_convert_chords_ranges_and_modifier_order() {
        assert_eq!(shortcut_label("⌘ O", Linux), "Ctrl+O");
        assert_eq!(shortcut_label("⌘ ↑ / ⌘ ↓", Windows), "Ctrl+↑ / Ctrl+↓");
        assert_eq!(shortcut_label("⌥ ⌘ C", Linux), "Ctrl+Alt+C");
        assert_eq!(shortcut_label("⇧ ⌘ T", Linux), "Ctrl+Shift+T");
        assert_eq!(shortcut_label("⌘ 1…9", Windows), "Ctrl+1…9");
        assert_eq!(shortcut_label("⌘ ,", Linux), "Ctrl+,");
        assert_eq!(shortcut_label("⌘ \\", Linux), "Ctrl+\\");
        assert_eq!(shortcut_label("⌃ Tab", Linux), "Ctrl+Tab");
        assert_eq!(shortcut_label("Esc", Linux), "Esc");
        assert_eq!(shortcut_label("↑ / ↓", Linux), "↑ / ↓");
        assert_eq!(shortcut_label("⇧ ↑ / ⇧ ↓", Linux), "Shift+↑ / Shift+↓");
    }
}
