//! The main window: transparent title bar with native traffic lights placed in the leading block.

use crate::args::Args;
use crate::workspace::Workspace;
use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size};

/// Default 1440×900 like the reference captures; traffic lights at (13, 14) to sit in the 53 px block of the
/// 42 px title bar (tuned during the M2 visual review).
/// With a stored frame (session restore), the window opens there, maximised if it was; a frame with non-finite values
/// is ignored.
pub fn window_options(state: Option<&rogg_core::session::WindowState>, cx: &App) -> WindowOptions {
    let bounds = state
        .and_then(crate::session::restorable_bounds)
        .unwrap_or_else(|| WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx)));
    WindowOptions {
        window_bounds: Some(bounds),
        titlebar: Some(TitlebarOptions { title: Some("rogg".into()), appears_transparent: true, traffic_light_position: Some(point(px(13.), px(14.))) }),
        app_owns_titlebar_drag: true,
        window_min_size: Some(size(px(crate::session::MIN_W), px(crate::session::MIN_H))),
        ..Default::default()
    }
}

pub fn open_main_window(args: Args, cx: &mut App) -> anyhow::Result<()> {
    // Only the frame is read here, strictly: a damaged file is left for the workspace, which sets it aside with a toast.
    let restore = !args.demo && crate::settings::SettingsStore::get(cx).behavior.restore_session;
    let state = restore
        .then(|| cx.try_global::<crate::paths::AppPaths>().map(|p| p.session.clone()))
        .flatten()
        .and_then(|path| rogg_core::session::Session::read_rogg(&path).ok())
        .and_then(|s| s.window);
    gpui_kit::open_window(window_options(state.as_ref(), cx), cx, move |window, cx| cx.new(|cx| Workspace::new(args, window, cx)))?;
    Ok(())
}
