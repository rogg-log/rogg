//! App actions and the single keymap table (spec §5.9). Bindings for log-view navigation live here too, so
//! every shortcut is defined in one place and can be remapped later.

use gpui_kit::{App, KeyBinding, SharedString};
use rogg_ui::logview::actions as log;

gpui_kit::actions!(
    rogg,
    [
        Quit,
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
        ToggleSidebar,
        ToggleWrap,
        ShowShortcuts,
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
    pub display: &'static str,
    pub binding: fn() -> KeyBinding,
}

macro_rules! shortcut {
    ($label:literal, $display:literal, $keys:literal, $action:expr, $context:expr) => {
        Shortcut { label: $label, display: $display, binding: || KeyBinding::new($keys, $action, $context) }
    };
}

/// The keymap. Shortcuts for features that arrive in later milestones (open as tab, export, settings, live
/// follow pause) are added with those features.
pub fn shortcuts() -> Vec<Shortcut> {
    const WS: Option<&str> = Some(WORKSPACE_CONTEXT);
    const LOG: Option<&str> = Some(log::KEY_CONTEXT);
    const FILTER: Option<&str> = Some(FILTER_CONTEXT);
    vec![
        shortcut!("Open file", "⌘ O", "cmd-o", OpenFile, None),
        shortcut!("Open folder", "⇧ ⌘ O", "shift-cmd-o", OpenFolder, None),
        shortcut!("Close tab", "⌘ W", "cmd-w", CloseTab, WS),
        shortcut!("Switch to tab 1…9", "⌘ 1…9", "cmd-1", ActivateTab { index: 0 }, WS),
        shortcut!("", "", "cmd-2", ActivateTab { index: 1 }, WS),
        shortcut!("", "", "cmd-3", ActivateTab { index: 2 }, WS),
        shortcut!("", "", "cmd-4", ActivateTab { index: 3 }, WS),
        shortcut!("", "", "cmd-5", ActivateTab { index: 4 }, WS),
        shortcut!("", "", "cmd-6", ActivateTab { index: 5 }, WS),
        shortcut!("", "", "cmd-7", ActivateTab { index: 6 }, WS),
        shortcut!("", "", "cmd-8", ActivateTab { index: 7 }, WS),
        shortcut!("", "", "cmd-9", ActivateTab { index: 8 }, WS),
        shortcut!("Next / previous tab", "⌃ Tab / ⌃ ⇧ Tab", "ctrl-tab", NextTab, WS),
        shortcut!("", "", "ctrl-shift-tab", PrevTab, WS),
        shortcut!("Find in focused panel", "⌘ F", "cmd-f", Find, WS),
        shortcut!("Edit filters", "⌘ L", "cmd-l", EditFilters, WS),
        shortcut!("", "", "enter", EditFilters, FILTER),
        shortcut!("Toggle regex / case", "⌥ ⌘ R / ⌥ ⌘ C", "alt-cmd-r", ToggleRegex, WS),
        shortcut!("", "", "alt-cmd-c", ToggleCase, WS),
        shortcut!("Pin / unpin selected line", "⌘ B", "cmd-b", TogglePin, WS),
        shortcut!("Go to line", "⌘ G", "cmd-g", GoToLine, WS),
        shortcut!("Move through results", "↑ ↓", "down", log::SelectNext, LOG),
        shortcut!("", "", "up", log::SelectPrev, LOG),
        shortcut!("Extend selection", "⇧ ↑ ↓", "shift-down", log::ExtendNext, LOG),
        shortcut!("", "", "shift-up", log::ExtendPrev, LOG),
        shortcut!("Page", "PgUp / PgDn", "pagedown", log::PageNext, LOG),
        shortcut!("", "", "pageup", log::PagePrev, LOG),
        shortcut!("Top / bottom", "⌘ ↑ / ⌘ ↓", "cmd-up", log::SelectFirst, LOG),
        shortcut!("", "", "cmd-down", log::SelectLast, LOG),
        shortcut!("Copy lines", "⌘ C", "cmd-c", log::CopySelection, LOG),
        shortcut!("Reveal in full log", "Return", "enter", RevealInFullLog, LOG),
        shortcut!("Cycle focus", "Tab / ⇧ Tab", "tab", FocusNext, WS),
        shortcut!("", "", "shift-tab", FocusPrev, WS),
        shortcut!("Toggle follow selection", "⇧ ⌘ F", "shift-cmd-f", ToggleFollow, WS),
        shortcut!("Toggle sidebar", "⌘ \\", "cmd-\\", ToggleSidebar, WS),
        shortcut!("Close dialog / cancel", "Esc", "escape", Dismiss, WS),
        shortcut!("Minimize", "⌘ M", "cmd-m", Minimize, None),
        shortcut!("Quit", "⌘ Q", "cmd-q", Quit, None),
    ]
}

/// Installs every keymap entry.
pub fn bind(cx: &mut App) {
    cx.bind_keys(shortcuts().into_iter().map(|s| (s.binding)()));
}

/// Rows for the keyboard shortcuts dialog: listed entries only.
pub fn shortcut_rows() -> Vec<(SharedString, SharedString)> {
    shortcuts().into_iter().filter(|s| !s.label.is_empty()).map(|s| (s.label.into(), s.display.into())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_binding_parses_and_is_unique_per_context() {
        let mut seen = HashSet::new();
        for s in shortcuts() {
            let binding = (s.binding)();
            let keys: Vec<String> = binding.keystrokes().iter().map(|k| format!("{k:?}")).collect();
            let context = binding.predicate().map(|p| format!("{p:?}"));
            assert!(seen.insert((keys.clone(), context.clone())), "duplicate binding {keys:?} in {context:?}");
        }
    }

    #[test]
    fn minimize_is_bound_to_cmd_m() {
        let rows = shortcut_rows();
        assert!(rows.iter().any(|(label, keys)| label == "Minimize" && keys == "⌘ M"));
    }

    #[test]
    fn listed_rows_skip_the_unlabelled_entries() {
        let rows = shortcut_rows();
        assert!(rows.iter().any(|(label, keys)| label == "Edit filters" && keys == "⌘ L"));
        assert!(rows.iter().all(|(label, _)| !label.is_empty()));
    }
}
