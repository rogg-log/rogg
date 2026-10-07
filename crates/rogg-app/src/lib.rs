//! The rogg application: windows, tabs, actions, and the wiring between rogg-core and rogg-ui (spec §3.2).

// The Action derive emits `gpui::` paths; gpui-kit documents aliasing itself as `gpui` for that.
extern crate gpui_kit as gpui;

pub mod actions;
pub mod args;
pub mod cache;
pub mod decorate;
pub mod demo;
pub mod document;
pub mod format;
pub mod history;
pub mod menus;
pub mod paths;
pub mod recent;
pub mod rows;
pub mod sidebar_model;
pub mod sources;
pub mod tab;
pub mod tab_view;
pub mod window;
pub mod workspace;

use gpui_kit::{App, QuitMode};
use paths::AppPaths;
use rogg_ui::theme::{Density, RoggTheme};

/// Runs the app until its last window closes.
pub fn run(args: args::Args) {
    gpui_kit::application().with_assets(rogg_ui::assets::Assets).with_quit_mode(QuitMode::LastWindowClosed).run(move |cx: &mut App| {
        init_with(cx, AppPaths::system());
        if let Err(e) = window::open_main_window(args, cx) {
            eprintln!("rogg: could not open the main window: {e:#}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}

/// Full initialisation: gpui-kit, the default theme, app globals and key bindings, menus.
pub fn init_with(cx: &mut App, paths: AppPaths) {
    gpui_kit::init(cx);
    let appearance = workspace::keys::appearance_for(workspace::keys::ThemeChoice::System, cx.window_appearance());
    RoggTheme::new(appearance, Density::Compact).install(cx);
    install_globals(cx, paths);
    let recent = cx.global::<recent::RecentStore>().entries().to_vec();
    cx.set_menus(menus::app_menus(&recent));
}

/// App globals and key bindings only, for hosts that initialise gpui-kit and the theme themselves (the gallery).
pub fn install_globals(cx: &mut App, paths: AppPaths) {
    let store = rogg_core::cache::Cache::new(paths.cache.clone(), rogg_core::cache::DEFAULT_LIMIT_BYTES);
    cache::start_maintenance(store.clone());
    cx.set_global(cache::CacheStore(store));
    cx.set_global(document::EngineConfig::default());
    cx.set_global(history::HistoryStore::load(paths.history.clone()));
    cx.set_global(recent::RecentStore::load(paths.recent.clone()));
    cx.set_global(paths);
    actions::bind(cx);
    cx.on_action(|_: &actions::Quit, cx| cx.quit());
    cx.on_action(|_: &actions::Hide, cx| cx.hide());
    cx.on_action(|_: &actions::HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &actions::ShowAll, cx| cx.unhide_other_apps());
}
