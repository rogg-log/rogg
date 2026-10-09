//! The main window: transparent title bar with native traffic lights placed in the leading block (Mac style), or a
//! title bar rogg draws with its own window controls (Linux and Windows styles).

use crate::args::Args;
use crate::workspace::Workspace;
use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, point, px, size};
use rogg_ui::ActiveRoggTheme;
use rogg_ui::theme::ChromeStyle;

/// Default 1440×900 like the reference captures. Mac style: traffic lights at (13, 14) to sit in the 53 px block of the
/// 42 px title bar (tuned during the M2 visual review). Linux style: client-side decorations, so rogg draws the
/// controls (gpui-kit's root adds the frame and its resize edges). Windows style: the system frame stays, with rogg's title bar drawn over its caption.
/// With a stored frame (session restore), the window opens there, maximised if it was; a frame with non-finite values
/// is ignored, and one that would be out of sight on the current displays is centred (`session::fit_to_displays`).
pub fn window_options(state: Option<&rogg_core::session::WindowState>, chrome: ChromeStyle, cx: &App) -> WindowOptions {
    let displays = display_bounds(cx);
    let bounds = state
        .and_then(crate::session::restorable_bounds)
        .map(|bounds| match bounds {
            WindowBounds::Windowed(b) => WindowBounds::Windowed(crate::session::fit_to_displays(b, &displays)),
            WindowBounds::Maximized(b) => WindowBounds::Maximized(crate::session::fit_to_displays(b, &displays)),
            WindowBounds::Fullscreen(b) => WindowBounds::Fullscreen(crate::session::fit_to_displays(b, &displays)),
        })
        .unwrap_or_else(|| WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx)));
    let traffic_light_position = chrome.is_mac().then(|| point(px(13.), px(14.)));
    WindowOptions {
        window_bounds: Some(bounds),
        titlebar: Some(TitlebarOptions { title: Some("rogg".into()), appears_transparent: true, traffic_light_position }),
        window_decorations: (chrome == ChromeStyle::Linux).then_some(WindowDecorations::Client),
        app_owns_titlebar_drag: true,
        window_min_size: Some(size(px(crate::session::MIN_W), px(crate::session::MIN_H))),
        // The Wayland app_id and X11 WM_CLASS, which desktops use to group windows and find the launcher entry;
        // macOS and Windows ignore it.
        app_id: Some("rogg".into()),
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
    // The theme, and with it the chrome style, is installed by `init_with` before any window opens.
    gpui_kit::open_window(window_options(state.as_ref(), cx.rogg().chrome, cx), cx, move |window, cx| cx.new(|cx| Workspace::new(args, window, cx)))?;
    Ok(())
}
