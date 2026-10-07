//! The main window: transparent title bar with native traffic lights placed in the leading block.

use crate::args::Args;
use crate::workspace::Workspace;
use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size};

/// Default 1440×900 like the reference captures; traffic lights at (13, 14) to sit in the 53 px block of the
/// 42 px title bar (tuned during the M2 visual review).
/// With a stored frame (session restore), the window opens there, maximised if it was; a frame with non-finite values
/// is ignored, and one that would be out of sight on the current displays is centred (`session::fit_to_displays`).
pub fn window_options(state: Option<&rogg_core::session::WindowState>, cx: &App) -> WindowOptions {
    let displays = display_bounds(cx);
    let bounds = state
        .and_then(crate::session::restorable_bounds)
        .map(|bounds| match bounds {
            WindowBounds::Windowed(b) => WindowBounds::Windowed(crate::session::fit_to_displays(b, &displays)),
            WindowBounds::Maximized(b) => WindowBounds::Maximized(crate::session::fit_to_displays(b, &displays)),
            WindowBounds::Fullscreen(b) => WindowBounds::Fullscreen(crate::session::fit_to_displays(b, &displays)),
        })
        .unwrap_or_else(|| WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx)));
    WindowOptions {
        window_bounds: Some(bounds),
        titlebar: Some(TitlebarOptions { title: Some("rogg".into()), appears_transparent: true, traffic_light_position: Some(point(px(13.), px(14.))) }),
        app_owns_titlebar_drag: true,
        window_min_size: Some(size(px(crate::session::MIN_W), px(crate::session::MIN_H))),
        ..Default::default()
    }
}

/// The displays a restored frame is checked against, in the frame's coordinates. gpui (macOS) stores a window's frame
/// relative to the display it is on and opens a window relative to the primary display, and reports every display's
/// bounds at origin (0, 0): only the primary display's usable area is in the frame's coordinates. The other displays'
/// rectangles would overlap it at the origin and let a frame sized for a large external display pass as visible.
fn display_bounds(cx: &App) -> Vec<Bounds<gpui_kit::Pixels>> {
    cx.primary_display().or_else(|| cx.displays().into_iter().next()).map(|d| d.visible_bounds()).into_iter().collect()
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
