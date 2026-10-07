//! Native menu bar (spec §5.9): rogg, File, Edit, View and Window.

use crate::actions::*;
use gpui_kit::{Menu, MenuItem, OsAction, SharedString};
use rogg_core::recent::RecentEntry;
use rogg_ui::logview::actions::CopySelection;

pub fn app_menus(recent: &[RecentEntry]) -> Vec<Menu> {
    let recent_items: Vec<MenuItem> = recent
        .iter()
        .enumerate()
        .map(|(index, e)| MenuItem::action(SharedString::from(e.label()), OpenRecent { index }))
        .collect();
    vec![
        Menu::new("rogg").items([
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_follow_the_spec_and_list_recent_entries() {
        let menus = app_menus(&[RecentEntry::file("/var/log/a.log"), RecentEntry::folder("/logs")]);
        let names: Vec<&str> = menus.iter().map(|m| m.name.as_ref()).collect();
        assert_eq!(names, ["rogg", "File", "Edit", "View", "Window"]);
        let app = &menus[0];
        assert_eq!(app.items.len(), 7, "Settings…, separator, Hide rogg, Hide Others, Show All, separator, Quit rogg");
        let file = &menus[1];
        let recent = file.items.iter().find_map(|i| match i {
            MenuItem::Submenu(m) if m.name.as_ref() == "Open Recent" => Some(m),
            _ => None,
        });
        assert_eq!(recent.expect("Open Recent submenu").items.len(), 2);
    }
}
