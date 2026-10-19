//! rogg component gallery: every rogg-ui component in every state, plus a rebuild of the reference mockup.
pub mod capture;
pub mod fixtures;
pub mod gallery;
pub mod pages;
pub mod replica;

/// The app's globals for the gallery's real-app page, with an updater that never checks: the gallery shows the app,
/// it isn't a release build of it, and a badge must not appear in its captures.
pub fn install_app_globals(cx: &mut gpui_kit::App, paths: rogg_app::paths::AppPaths) {
    use rogg_app::updater::{Updater, UpdaterConfig};
    cx.set_global(Updater::new(UpdaterConfig::disabled(paths.cache.join("update"))));
    rogg_app::install_globals(cx, paths);
}
