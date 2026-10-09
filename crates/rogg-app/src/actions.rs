//! App actions and the single keymap table (spec §5.9). Bindings for log-view navigation live here too, so
//! every shortcut is defined in one place and can be remapped later.

use gpui_kit::{App, KeyBinding, SharedString};
use rogg_core::platform::Os;
use rogg_ui::keys::shortcut_label;
use rogg_ui::logview::actions as log;
use rogg_ui::theme::ChromeStyle;

gpui_kit::actions!(
    rogg,
    [
        Quit,
        Hide,
        HideOthers,
        ShowAll,
        OpenFile,
        OpenFolder,
        CloseTab,
        NextTab,
        PrevTab,
        Find,
        EditFilters,
        ToggleRegex,
        ToggleCase,
        TogglePin,
        GoToLine,
        RevealInFullLog,
        FocusNext,
        FocusPrev,
        ToggleFollow,
        ToggleLive,
        ToggleSidebar,
        ToggleWrap,
        ShowShortcuts,
        OpenAsTab,
        Export,
        OpenSettings,
        ShowAbout,
        Dismiss,
        ThemeSystem,
        ThemeLight,
        ThemeDark,
        DensityCompact,
        DensityComfortable,
        Minimize,
        Zoom
    ]
);

/// ⌘1…⌘9: activate the tab at `index`.
#[derive(Clone, PartialEq, Debug, gpui_kit::Action)]
#[action(namespace = rogg, no_json)]
pub struct ActivateTab {
    pub index: usize,
}

/// File → Open Recent ▸ entry `index`.
#[derive(Clone, PartialEq, Debug, gpui_kit::Action)]
#[action(namespace = rogg, no_json)]
pub struct OpenRecent {
    pub index: usize,
}

pub const WORKSPACE_CONTEXT: &str = "Workspace";
/// Key context of the focused filter line (when not editing).
pub const FILTER_CONTEXT: &str = "FilterBand";

/// One keymap entry. `label` and `display` feed the shortcuts dialog; entries with an empty label are bound but
/// not listed (⌘2…⌘9 share the ⌘1 row).
pub struct Shortcut {
    pub label: &'static str,
    /// The Mac-style label; `shortcut_rows` reformats it for the chrome style.
    pub display: &'static str,
    pub keys: &'static str,
    pub binding: fn() -> KeyBinding,
}

macro_rules! shortcut {
    ($label:literal, $display:literal, $keys:literal, $action:expr, $context:expr) => {
        Shortcut { label: $label, display: $display, keys: $keys, binding: || KeyBinding::new($keys, $action, $context) }
    };
}

/// The keymap of the OS this build runs on.
pub fn shortcuts() -> Vec<Shortcut> {
    shortcuts_for(Os::current())
}

/// The keymap for `os`. `secondary-` is cmd on macOS and ctrl elsewhere.
pub fn shortcuts_for(os: Os) -> Vec<Shortcut> {
    const WS: Option<&str> = Some(WORKSPACE_CONTEXT);
    const LOG: Option<&str> = Some(log::KEY_CONTEXT);
    const FILTER: Option<&str> = Some(FILTER_CONTEXT);
    let mut list = vec![
        shortcut!("Open file", "⌘ O", "secondary-o", OpenFile, None),
        shortcut!("Open folder", "⇧ ⌘ O", "shift-secondary-o", OpenFolder, None),
        shortcut!("Close tab", "⌘ W", "secondary-w", CloseTab, WS),
        shortcut!("Switch to tab 1…9", "⌘ 1…9", "secondary-1", ActivateTab { index: 0 }, WS),
        shortcut!("", "", "secondary-2", ActivateTab { index: 1 }, WS),
        shortcut!("", "", "secondary-3", ActivateTab { index: 2 }, WS),
        shortcut!("", "", "secondary-4", ActivateTab { index: 3 }, WS),
        shortcut!("", "", "secondary-5", ActivateTab { index: 4 }, WS),
        shortcut!("", "", "secondary-6", ActivateTab { index: 5 }, WS),
        shortcut!("", "", "secondary-7", ActivateTab { index: 6 }, WS),
        shortcut!("", "", "secondary-8", ActivateTab { index: 7 }, WS),
        shortcut!("", "", "secondary-9", ActivateTab { index: 8 }, WS),
        shortcut!("Next / previous tab", "⌃ Tab / ⌃ ⇧ Tab", "ctrl-tab", NextTab, WS),
        shortcut!("", "", "ctrl-shift-tab", PrevTab, WS),
        shortcut!("Find in focused panel", "⌘ F", "secondary-f", Find, WS),
        shortcut!("Edit filters", "⌘ L", "secondary-l", EditFilters, WS),
        shortcut!("", "", "enter", EditFilters, FILTER),
        shortcut!("Toggle regex / case", "⌥ ⌘ R / ⌥ ⌘ C", "alt-secondary-r", ToggleRegex, WS),
        shortcut!("", "", "alt-secondary-c", ToggleCase, WS),
        shortcut!("Pin / unpin selected line", "⌘ B", "secondary-b", TogglePin, WS),
        shortcut!("Go to line", "⌘ G", "secondary-g", GoToLine, WS),
        shortcut!("Move through results", "↑ ↓", "down", log::SelectNext, LOG),
        shortcut!("", "", "up", log::SelectPrev, LOG),
        shortcut!("Extend selection", "⇧ ↑ ↓", "shift-down", log::ExtendNext, LOG),
        shortcut!("", "", "shift-up", log::ExtendPrev, LOG),
        shortcut!("Page", "PgUp / PgDn", "pagedown", log::PageNext, LOG),
        shortcut!("", "", "pageup", log::PagePrev, LOG),
        shortcut!("Top / bottom", "⌘ ↑ / ⌘ ↓", "secondary-up", log::SelectFirst, LOG),
        shortcut!("", "", "secondary-down", log::SelectLast, LOG),
        shortcut!("Copy lines", "⌘ C", "secondary-c", log::CopySelection, LOG),
        shortcut!("Reveal in full log", "Return", "enter", RevealInFullLog, LOG),
        shortcut!("Pause / resume live follow", "Space", "space", ToggleLive, LOG),
        shortcut!("Cycle focus", "Tab / ⇧ Tab", "tab", FocusNext, WS),
        shortcut!("", "", "shift-tab", FocusPrev, WS),
        shortcut!("Toggle follow selection", "⇧ ⌘ F", "shift-secondary-f", ToggleFollow, WS),
        shortcut!("Open filtered as tab", "⇧ ⌘ T", "shift-secondary-t", OpenAsTab, WS),
        shortcut!("Export", "⌘ E", "secondary-e", Export, WS),
        shortcut!("Toggle sidebar", "⌘ \\", "secondary-\\", ToggleSidebar, WS),
        shortcut!("Settings", "⌘ ,", "secondary-,", OpenSettings, None),
        shortcut!("Close dialog / cancel", "Esc", "escape", Dismiss, WS),
    ];
    match os {
        // Window management and Quit are app-menu conventions on macOS.
        Os::Mac => list.extend([
            shortcut!("Minimize", "⌘ M", "secondary-m", Minimize, None),
            shortcut!("Hide rogg", "⌘ H", "secondary-h", Hide, None),
            shortcut!("Hide others", "⌥ ⌘ H", "alt-secondary-h", HideOthers, None),
            shortcut!("Quit", "⌘ Q", "secondary-q", Quit, None),
        ]),
        Os::Linux => list.push(shortcut!("Quit", "⌘ Q", "ctrl-q", Quit, None)),
        // Windows quits with Alt+F4, which the OS handles.
        Os::Windows => {}
    }
    list
}

/// Installs every keymap entry.
pub fn bind(cx: &mut App) {
    cx.bind_keys(shortcuts().into_iter().map(|s| (s.binding)()));
}

/// Rows for the keyboard shortcuts dialog: listed entries only, labelled in the installed theme's `chrome` style.
pub fn shortcut_rows(chrome: ChromeStyle) -> Vec<(SharedString, SharedString)> {
    shortcuts()
        .into_iter()
        .filter(|s| !s.label.is_empty())
        .map(|s| (s.label.into(), shortcut_label(s.display, chrome).into()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rogg_core::platform::Os;
    use rogg_ui::theme::ChromeStyle;
    use std::collections::HashSet;

    fn labels(os: Os) -> Vec<&'static str> {
        shortcuts_for(os).into_iter().map(|s| s.label).collect()
    }

    fn mac_rows() -> Vec<(SharedString, SharedString)> {
        shortcut_rows(ChromeStyle::Mac)
    }

    #[test]
    fn mac_only_bindings_exist_only_on_mac() {
        for label in ["Hide rogg", "Hide others", "Minimize"] {
            assert!(labels(Os::Mac).contains(&label), "{label} on Mac");
            assert!(!labels(Os::Linux).contains(&label), "{label} on Linux");
            assert!(!labels(Os::Windows).contains(&label), "{label} on Windows");
        }
    }

    #[test]
    fn quit_is_ctrl_q_on_linux_and_absent_on_windows() {
        assert!(labels(Os::Mac).contains(&"Quit"));
        assert!(labels(Os::Linux).contains(&"Quit"));
        assert!(!labels(Os::Windows).contains(&"Quit"), "Alt+F4 is the OS's");
    }

    #[test]
    fn every_binding_parses_and_is_unique_per_context_on_every_os() {
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            let mut seen = HashSet::new();
            for s in shortcuts_for(os) {
                let binding = (s.binding)();
                let keys: Vec<String> = binding.keystrokes().iter().map(|k| format!("{k:?}")).collect();
                let context = binding.predicate().map(|p| format!("{p:?}"));
                assert!(seen.insert((keys.clone(), context.clone())), "{os:?}: duplicate {keys:?} in {context:?}");
            }
        }
    }

    #[test]
    fn the_keymap_uses_the_platform_modifier() {
        // No binding hard-codes cmd: secondary means cmd on macOS and ctrl elsewhere.
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            for s in shortcuts_for(os) {
                assert!(!s.keys.contains("cmd-"), "{os:?}: {} uses cmd-", s.keys);
            }
        }
    }

    /// gpui parses `secondary` by the build OS (`cfg!(target_os = "macos")`: cmd, otherwise ctrl), so only the
    /// meaning for the host OS is testable here; the other is covered by `the_keymap_uses_the_platform_modifier`
    /// (no hard-coded cmd) plus gpui's own parse.
    #[test]
    fn secondary_resolves_to_cmd_on_macos_and_ctrl_elsewhere() {
        let mac = cfg!(target_os = "macos");
        let mut checked = 0;
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            for s in shortcuts_for(os).into_iter().filter(|s| s.keys.contains("secondary-")) {
                let binding = (s.binding)();
                let mods = binding.keystrokes()[0].modifiers();
                assert_eq!(mods.platform, mac, "{}: cmd only on macOS", s.keys);
                assert_eq!(mods.control, !mac, "{}: ctrl everywhere else", s.keys);
                checked += 1;
            }
        }
        assert!(checked > 20, "only {checked} secondary bindings checked");
    }

    #[test]
    fn non_mac_rows_contain_no_mac_glyphs() {
        for chrome in [ChromeStyle::Linux, ChromeStyle::Windows] {
            for (label, keys) in shortcut_rows(chrome) {
                assert!(!keys.chars().any(|c| "⌘⌃⌥⇧".contains(c)), "{chrome:?}: {label}: {keys}");
            }
        }
    }

    #[test]
    fn rows_follow_the_given_chrome_style() {
        let linux = shortcut_rows(ChromeStyle::Linux);
        assert!(linux.iter().any(|(l, k)| l == "Open filtered as tab" && k == "Ctrl+Shift+T"));
        assert!(linux.iter().any(|(l, k)| l == "Next / previous tab" && k == "Ctrl+Tab / Ctrl+Shift+Tab"));
    }

    #[test]
    fn hide_is_bound_to_cmd_h_on_mac() {
        let rows = mac_rows();
        assert!(rows.iter().any(|(label, keys)| label == "Hide rogg" && keys == "⌘ H"));
    }

    #[test]
    fn minimize_is_bound_to_cmd_m_on_mac() {
        let rows = mac_rows();
        assert!(rows.iter().any(|(label, keys)| label == "Minimize" && keys == "⌘ M"));
    }

    #[test]
    fn export_settings_and_open_as_tab_are_listed() {
        let rows = mac_rows();
        for (label, keys) in [("Export", "⌘ E"), ("Settings", "⌘ ,"), ("Open filtered as tab", "⇧ ⌘ T")] {
            assert!(rows.iter().any(|(l, k)| l == label && k == keys), "missing row {label} {keys}");
        }
    }

    #[test]
    fn listed_rows_skip_the_unlabelled_entries() {
        let rows = mac_rows();
        assert!(rows.iter().any(|(label, keys)| label == "Edit filters" && keys == "⌘ L"));
        assert!(rows.iter().all(|(label, _)| !label.is_empty()));
    }
}
