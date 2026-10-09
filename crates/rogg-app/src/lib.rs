//! The rogg application: windows, tabs, actions, and the wiring between rogg-core and rogg-ui (spec §3.2).

// The Action derive emits `gpui::` paths; gpui-kit documents aliasing itself as `gpui` for that.
extern crate gpui_kit as gpui;

pub mod about;
pub mod actions;
pub mod args;
pub mod cache;
pub mod demo;
pub mod derived;
pub mod dialogs;
pub mod document;
pub mod export;
pub mod format;
pub mod history;
pub mod menus;
pub mod paths;
pub mod profiles;
pub mod recent;
pub mod rows;
pub mod session;
pub mod settings;
pub mod sidebar_model;
pub mod sources;
pub mod tab;
pub mod tab_view;
pub mod window;
pub mod workspace;

use gpui_kit::{App, QuitMode};
use paths::AppPaths;

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
    // The only place the style is resolved (ROGG_CHROME or the build OS); everything later reads the installed theme.
    let chrome = rogg_ui::theme::ChromeStyle::resolved();
    // The bundled Inter and JetBrains Mono are the Linux and Windows styles' fonts. The Mac style keeps the system's
    // fonts only, so its "Other…" log-font list is unchanged.
    if !chrome.is_mac() {
        rogg_ui::fonts::register(cx);
    }
    let settings = settings::SettingsStore::load(paths.settings.clone());
    settings::theme_for(settings.settings(), cx.window_appearance(), chrome).install(cx);
    cx.set_global(settings);
    install_globals(cx, paths);
    let recent = cx.global::<recent::RecentStore>().entries().to_vec();
    cx.set_menus(menus::app_menus(&recent));
}

/// App globals and key bindings only, for hosts that initialise gpui-kit and the theme themselves (the gallery).
pub fn install_globals(cx: &mut App, paths: AppPaths) {
    if !cx.has_global::<settings::SettingsStore>() {
        cx.set_global(settings::SettingsStore::load(paths.settings.clone()));
    }
    let store = rogg_core::cache::Cache::new(paths.cache.clone(), settings::SettingsStore::get(cx).cache_limit_bytes());
    cache::start_maintenance(store.clone());
    cx.set_global(cache::CacheStore(store));
    cx.set_global(document::EngineConfig::default());
    cx.set_global(history::HistoryStore::load(paths.history.clone()));
    cx.set_global(profiles::ProfileStore::load(paths.profiles.clone()));
    cx.set_global(recent::RecentStore::load(paths.recent.clone()));
    cx.set_global(paths);
    actions::bind(cx);
    cx.on_action(|_: &actions::Quit, cx| cx.quit());
    cx.on_action(|_: &actions::Hide, cx| cx.hide());
    cx.on_action(|_: &actions::HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &actions::ShowAll, cx| cx.unhide_other_apps());
}
