//! A tab's right-click menu: close variants relative to the right-clicked tab, Copy Path and Show in Finder.

use super::{Workspace, act};
use crate::actions::CloseTab;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{App, ClipboardItem, Context, Entity, Window};
use rogg_ui::dialogs::reveal_label;
use rogg_ui::theme::{ActiveRoggTheme, ChromeStyle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabMenuAction {
    Close,
    CloseOthers,
    CloseLeft,
    CloseRight,
    CloseAll,
    CopyPath,
    Reveal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabMenuItem {
    pub label: &'static str,
    pub action: TabMenuAction,
    pub enabled: bool,
}

/// The menu for tab `index` of `count`; `None` is a separator. `has_path`: the tab shows a file or an archive member
/// (not stdin, a filtered tab or in-memory text).
pub fn tab_menu_items(index: usize, count: usize, has_path: bool, chrome: ChromeStyle) -> Vec<Option<TabMenuItem>> {
    let item = |label, action, enabled| Some(TabMenuItem { label, action, enabled });
    vec![
        item("Close Tab", TabMenuAction::Close, true),
        item("Close Other Tabs", TabMenuAction::CloseOthers, count > 1),
        item("Close Tabs to the Left", TabMenuAction::CloseLeft, index > 0),
        item("Close Tabs to the Right", TabMenuAction::CloseRight, index + 1 < count),
        item("Close All Tabs", TabMenuAction::CloseAll, true),
        None,
        item("Copy Path", TabMenuAction::CopyPath, has_path),
        item(reveal_label(chrome), TabMenuAction::Reveal, has_path),
    ]
}

/// Fills the title bar's menu for tab `index`.
pub(super) fn build(this: &Entity<Workspace>, index: usize, menu: PopupMenu, cx: &App) -> PopupMenu {
    let ws = this.read(cx);
    let has_path = ws.tabs.get(index).is_some_and(|t| t.read(cx).doc.read(cx).source.source_ref().is_some());
    tab_menu_items(index, ws.tabs.len(), has_path, cx.rogg().chrome).into_iter().fold(menu, |menu, item| match item {
        None => menu.separator(),
        Some(item) => {
            let run = act(this, move |ws, window, cx| ws.run_tab_action(index, item.action, window, cx));
            let entry = PopupMenuItem::new(item.label).disabled(!item.enabled).on_click(move |_, window, cx| run(window, cx));
            // The action only shows ⌘W; the click handler runs instead of it, on the right-clicked tab.
            menu.item(if item.action == TabMenuAction::Close { entry.action(Box::new(CloseTab)) } else { entry })
        }
    })
}

impl Workspace {
    /// Runs a tab-menu entry on tab `index`, which need not be the active tab.
    pub fn run_tab_action(&mut self, index: usize, action: TabMenuAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            TabMenuAction::Close => self.close_tab(index, window, cx),
            TabMenuAction::CloseOthers => self.close_tabs_where(index, |i| i != index, window, cx),
            TabMenuAction::CloseLeft => self.close_tabs_where(index, |i| i < index, window, cx),
            TabMenuAction::CloseRight => self.close_tabs_where(index, |i| i > index, window, cx),
            TabMenuAction::CloseAll => self.close_tabs_where(index, |_| true, window, cx),
            TabMenuAction::CopyPath | TabMenuAction::Reveal => {
                let Some(source) = self.tabs.get(index).and_then(|t| t.read(cx).doc.read(cx).source.source_ref()) else { return };
                if action == TabMenuAction::CopyPath {
                    cx.write_to_clipboard(ClipboardItem::new_string(source.full_path().display().to_string()));
                } else {
                    // A member has no file of its own: show the archive it came from.
                    self.reveal(source.root_path().to_path_buf(), cx);
                }
            }
        }
    }
}
