//! The main window: transparent title bar with native traffic lights placed in the leading block.

use crate::args::Args;
use crate::workspace::Workspace;
use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size};

/// Default 1440×900 like the reference captures; traffic lights at (13, 14) to sit in the 53 px block of the
/// 42 px title bar (tuned during the M2 visual review).
pub fn window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx))),
        titlebar: Some(TitlebarOptions { title: Some("rogg".into()), appears_transparent: true, traffic_light_position: Some(point(px(13.), px(14.))) }),
        app_owns_titlebar_drag: true,
        window_min_size: Some(size(px(760.), px(520.))),
        ..Default::default()
    }
}

pub fn open_main_window(args: Args, cx: &mut App) -> anyhow::Result<()> {
    gpui_kit::open_window(window_options(cx), cx, move |window, cx| cx.new(|cx| Workspace::new(args, window, cx)))?;
    Ok(())
}
