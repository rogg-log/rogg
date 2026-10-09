//! Headless app windows for integration tests. gpui's TestAppContext has a no-op text system: assert on
//! state and on fixed sizes, not on text widths.
#![allow(dead_code)]
use gpui_kit::prelude::*;
use gpui_kit::{AnyWindowHandle, Bounds, Entity, Pixels, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use rogg_app::args::Args;
use rogg_app::paths::AppPaths;
use rogg_app::workspace::Workspace;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Initialises the app with state files in a fresh temp dir (kept alive by the returned guard).
pub fn setup(cx: &mut TestAppContext) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(dir.path());
    cx.update(|cx| rogg_app::init_with(cx, paths));
    dir
}

pub fn open_workspace(cx: &mut TestAppContext, args: Args) -> (AnyWindowHandle, Entity<Workspace>) {
    let handle = cx.update(|cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(1440.), px(900.)) })),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, move |window, cx| {
            cx.new(|cx| {
                let mut ws = Workspace::new(args, window, cx);
                // gpui's test platform panics on reveal_path; `last_revealed` still records the folder.
                ws.reveal_in_finder = false;
                ws
            })
        })
        .expect("open window")
    });
    cx.run_until_parked();
    handle
}

pub fn render(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
    })
    .unwrap();
}

pub fn exists(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) -> bool {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .unwrap()
}

pub fn bounds(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) -> Bounds<Pixels> {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.find(id).bounds()
    })
    .unwrap()
}

pub fn click(cx: &mut TestAppContext, window: AnyWindowHandle, id: &'static str) {
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click(id, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// Presses `keys`, written with macOS names. Off macOS the primary modifier is Ctrl, so `cmd-` becomes `ctrl-`.
pub fn press(cx: &mut TestAppContext, window: AnyWindowHandle, keys: &str) {
    #[cfg(not(target_os = "macos"))]
    let keys = &keys.replace("cmd-", "ctrl-");
    cx.update_window(window, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.press(keys, cx);
    })
    .unwrap();
    cx.run_until_parked();
}

/// Advances the test clock (driving the 16 ms poll task) while real background threads work, until `done`.
pub fn wait_until(cx: &mut TestAppContext, mut done: impl FnMut(&mut TestAppContext) -> bool) {
    for _ in 0..3000 {
        if done(cx) {
            return;
        }
        cx.executor().advance_clock(Duration::from_millis(20));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("condition not reached within the time limit");
}

/// Writes `lines` (joined with `\n`) to `dir/name` and returns the path.
pub fn write_log(dir: &Path, name: &str, lines: &[&str]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, lines.join("\n")).unwrap();
    path
}
