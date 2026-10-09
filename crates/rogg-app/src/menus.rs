//! Native menu bar (spec §5.9): rogg, File, Edit, View and Window; and the same menus as the ☰ app menu of the Linux and
//! Windows title bar (spec §4.3).

use crate::actions::*;
use gpui_kit::component::menu::PopupMenu;
use gpui_kit::{Action, Context, FocusHandle, Menu, MenuItem, OsAction, SharedString, Window};
use rogg_core::recent::RecentEntry;
use rogg_ui::logview::actions::CopySelection;
use std::rc::Rc;

/// The rogg menu's items that only make sense on macOS; the ☰ menu leaves them out.
pub const MAC_ONLY_ITEMS: [&str; 3] = ["Hide rogg", "Hide Others", "Show All"];
/// The rogg menu's last item, which the ☰ menu moves to its end.
const QUIT: &str = "Quit rogg";

pub fn app_menus(recent: &[RecentEntry]) -> Vec<Menu> {
    let recent_items: Vec<MenuItem> = recent
        .iter()
        .enumerate()
        .map(|(index, e)| MenuItem::action(SharedString::from(e.label()), OpenRecent { index }))
        .collect();
    vec![
        Menu::new("rogg").items([
            MenuItem::action("About rogg", ShowAbout),
            MenuItem::separator(),
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Hide rogg", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit rogg", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Open…", OpenFile),
            MenuItem::action("Open Folder…", OpenFolder),
            MenuItem::submenu(Menu::new("Open Recent").items(recent_items)),
            MenuItem::action("Export…", Export),
            MenuItem::separator(),
            MenuItem::action("Close Tab", CloseTab),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Copy", CopySelection, OsAction::Copy),
            MenuItem::action("Find", Find),
            MenuItem::action("Go to Line…", GoToLine),
        ]),
        Menu::new("View").items([
            MenuItem::action("Sidebar", ToggleSidebar),
            MenuItem::action("Wrap", ToggleWrap),
            MenuItem::action("Open Filtered as Tab", OpenAsTab),
            MenuItem::submenu(Menu::new("Theme").items([
                MenuItem::action("System", ThemeSystem),
                MenuItem::action("Light", ThemeLight),
                MenuItem::action("Dark", ThemeDark),
            ])),
            MenuItem::submenu(Menu::new("Density").items([
                MenuItem::action("Compact", DensityCompact),
                MenuItem::action("Comfortable", DensityComfortable),
            ])),
        ]),
        Menu::new("Window").items([MenuItem::action("Minimize", Minimize), MenuItem::action("Zoom", Zoom)]),
    ]
}

/// An action's or a submenu's name; None for separators and system menus.
pub fn item_label(item: &MenuItem) -> Option<String> {
    match item {
        MenuItem::Action { name, .. } => Some(name.to_string()),
        MenuItem::Submenu(menu) => Some(menu.name.to_string()),
        MenuItem::Separator | MenuItem::SystemMenu(_) => None,
    }
}

/// One entry of the ☰ menu, owned so a submenu's builder (which gpui-kit keeps as a `'static` closure) can rebuild it.
enum PopupItem {
    Action(SharedString, Box<dyn Action>),
    Separator,
    Submenu(SharedString, Rc<Vec<PopupItem>>),
}

impl PopupItem {
    /// A submenu entry; None for an empty menu (Open Recent with no recent files), which would open an empty popup.
    fn from_menu(menu: &Menu) -> Option<PopupItem> {
        let items = popup_items_of(&menu.items, |_| true);
        (!items.is_empty()).then(|| PopupItem::Submenu(menu.name.clone(), Rc::new(items)))
    }
}

/// `items` as ☰ entries, keeping those `keep` accepts, without leading, trailing or doubled separators.
fn popup_items_of(items: &[MenuItem], keep: impl Fn(&str) -> bool) -> Vec<PopupItem> {
    let mut out: Vec<PopupItem> = Vec::new();
    for item in items {
        match item {
            MenuItem::Action { name, action, .. } if keep(name) => out.push(PopupItem::Action(name.clone(), action.boxed_clone())),
            MenuItem::Submenu(menu) if keep(&menu.name) => out.extend(PopupItem::from_menu(menu)),
            MenuItem::Separator if out.last().is_some_and(|last| !matches!(last, PopupItem::Separator)) => out.push(PopupItem::Separator),
            _ => {}
        }
    }
    if matches!(out.last(), Some(PopupItem::Separator)) {
        out.pop();
    }
    out
}

/// The ☰ menu (spec §4.3): the first (rogg) menu's items without the macOS-only ones and Quit, then every other menu
/// as a submenu, then Quit after a separator. Empty submenus are left out.
fn popup_items(menus: &[Menu]) -> Vec<PopupItem> {
    let Some((app, rest)) = menus.split_first() else { return Vec::new() };
    let mut items = popup_items_of(&app.items, |name| name != QUIT && !MAC_ONLY_ITEMS.contains(&name));
    items.extend(rest.iter().filter_map(PopupItem::from_menu));
    let quit = app.items.iter().find_map(|item| match item {
        MenuItem::Action { name, action, .. } if name.as_ref() == QUIT => Some(PopupItem::Action(name.clone(), action.boxed_clone())),
        _ => None,
    });
    if let Some(quit) = quit {
        items.push(PopupItem::Separator);
        items.push(quit);
    }
    items
}

/// The labels the ☰ menu shows, in order, submenus followed by their items; `""` stands for a separator.
pub fn popup_entries(menus: &[Menu]) -> Vec<String> {
    fn walk(items: &[PopupItem], out: &mut Vec<String>) {
        for item in items {
            match item {
                PopupItem::Action(name, _) => out.push(name.to_string()),
                PopupItem::Separator => out.push(String::new()),
                PopupItem::Submenu(name, items) => {
                    out.push(name.to_string());
                    walk(items, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(&popup_items(menus), &mut out);
    out
}

/// Fills the ☰ menu from `menus` (see `popup_items`). Items dispatch their actions from `action_context` when given
/// (what had focus before the menu opened, as with the native menu bar), and gpui-kit shows each one's key binding from
/// the active keymap.
pub fn popup_menu(
    menus: &[Menu],
    action_context: Option<FocusHandle>,
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    fill(&popup_items(menus), action_context, menu, window, cx)
}

/// One menu level. gpui-kit builds each submenu as its own `PopupMenu`, which dispatches from its own focus unless it
/// has the action context too, so every level gets it.
fn fill(items: &[PopupItem], action_context: Option<FocusHandle>, menu: PopupMenu, window: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
    let menu = match action_context.clone() {
        Some(focus) => menu.action_context(focus),
        None => menu,
    };
    items.iter().fold(menu, |menu, item| match item {
        PopupItem::Action(name, action) => menu.menu(name.clone(), action.boxed_clone()),
        PopupItem::Separator => menu.separator(),
        PopupItem::Submenu(name, items) => {
            let (items, action_context) = (items.clone(), action_context.clone());
            menu.submenu(name.clone(), window, cx, move |menu, window, cx| fill(&items, action_context.clone(), menu, window, cx))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_follow_the_spec_and_list_recent_entries() {
        let menus = app_menus(&[RecentEntry::file("/var/log/a.log"), RecentEntry::folder("/logs")]);
        let names: Vec<&str> = menus.iter().map(|m| m.name.as_ref()).collect();
        assert_eq!(names, ["rogg", "File", "Edit", "View", "Window"]);
        let app = &menus[0];
        assert_eq!(app.items.len(), 9, "About rogg, separator, Settings…, separator, Hide rogg, Hide Others, Show All, separator, Quit rogg");
        assert!(matches!(&app.items[0], MenuItem::Action { name, .. } if name.as_ref() == "About rogg"));
        let file = &menus[1];
        let recent = file.items.iter().find_map(|i| match i {
            MenuItem::Submenu(m) if m.name.as_ref() == "Open Recent" => Some(m),
            _ => None,
        });
        assert_eq!(recent.expect("Open Recent submenu").items.len(), 2);
    }

    #[test]
    fn the_app_menu_mirrors_the_native_menus() {
        let menus = app_menus(&[RecentEntry::file("/var/log/a.log")]);
        let native: Vec<String> = menus.iter().flat_map(|m| m.items.iter()).filter_map(item_label).collect();
        let popup: Vec<String> = popup_entries(&menus).into_iter().filter(|l| !l.is_empty()).collect();
        for label in native.iter().filter(|l| !MAC_ONLY_ITEMS.contains(&l.as_str())) {
            assert!(popup.contains(label), "☰ menu lacks {label}");
        }
        for label in MAC_ONLY_ITEMS {
            assert!(!popup.iter().any(|l| l == label), "☰ menu has the macOS-only {label}");
        }
        // Spec §4.3 order: About, Settings…, the File…Window submenus, Quit last.
        assert_eq!(popup.first().map(String::as_str), Some("About rogg"));
        assert_eq!(popup.last().map(String::as_str), Some("Quit rogg"));
        let at = |l: &str| popup.iter().position(|p| p == l).unwrap_or_else(|| panic!("{l} missing"));
        assert!(at("Settings…") < at("File") && at("File") < at("Edit") && at("Edit") < at("View") && at("View") < at("Window"));
        assert!(at("Open Recent") > at("File"));
    }

    #[test]
    fn the_app_menu_has_no_stray_separators() {
        let entries = popup_entries(&app_menus(&[]));
        assert_ne!(entries.first().map(String::as_str), Some(""), "{entries:?}");
        assert_ne!(entries.last().map(String::as_str), Some(""), "{entries:?}");
        assert!(!entries.windows(2).any(|w| w[0].is_empty() && w[1].is_empty()), "doubled separator: {entries:?}");
        let quit = entries.iter().position(|l| l == "Quit rogg").unwrap();
        assert_eq!(entries[quit - 1], "", "a separator before Quit");
    }

    #[test]
    fn the_app_menu_leaves_out_an_empty_open_recent() {
        assert!(!popup_entries(&app_menus(&[])).iter().any(|l| l == "Open Recent"));
        let with_recent = popup_entries(&app_menus(&[RecentEntry::file("/var/log/a.log")]));
        let at = with_recent.iter().position(|l| l == "Open Recent").expect("Open Recent with an entry");
        assert_eq!(with_recent[at + 1], RecentEntry::file("/var/log/a.log").label());
    }
}
