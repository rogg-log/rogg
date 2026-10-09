mod common;
use gpui_kit::TestAppContext;
use rogg_app::args::Args;

#[gpui_kit::test]
fn opens_to_the_welcome_view(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, _) = common::open_workspace(cx, Args::default());
    assert!(common::exists(cx, w, "welcome"));
    assert!(common::exists(cx, w, "titlebar"));
    assert!(common::exists(cx, w, "open-file"));
}

#[gpui_kit::test]
fn shortcuts_dialog_opens_from_the_title_bar_and_escape_closes_it(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, _) = common::open_workspace(cx, Args::default());
    common::click(cx, w, "shortcuts");
    assert!(common::exists(cx, w, "dialog"));
    common::press(cx, w, "escape");
    assert!(!common::exists(cx, w, "dialog"));
}

#[gpui_kit::test]
fn window_options_follow_the_chrome_style(cx: &mut TestAppContext) {
    use gpui_kit::WindowDecorations;
    use rogg_ui::theme::ChromeStyle;
    let _dir = common::setup(cx);
    cx.update(|cx| {
        let mac = rogg_app::window::window_options(None, ChromeStyle::Mac, cx);
        assert!(mac.titlebar.as_ref().is_some_and(|t| t.appears_transparent && t.traffic_light_position.is_some()));
        let linux = rogg_app::window::window_options(None, ChromeStyle::Linux, cx);
        assert_eq!(linux.window_decorations, Some(WindowDecorations::Client));
        assert!(linux.titlebar.as_ref().is_some_and(|t| t.traffic_light_position.is_none()));
        let windows = rogg_app::window::window_options(None, ChromeStyle::Windows, cx);
        assert!(windows.titlebar.as_ref().is_some_and(|t| t.appears_transparent && t.traffic_light_position.is_none()));
        for options in [mac, linux, windows] {
            assert!(options.app_owns_titlebar_drag);
            assert_eq!(options.app_id.as_deref(), Some("rogg"), "the Wayland app_id / X11 WM_CLASS");
        }
    });
}

#[gpui_kit::test]
fn the_app_menu_button_shows_only_outside_the_mac_style(cx: &mut TestAppContext) {
    use rogg_ui::ActiveRoggTheme;
    use rogg_ui::theme::ChromeStyle;
    let _dir = common::setup(cx);
    cx.update(|cx| cx.rogg().clone().with_chrome(ChromeStyle::Mac).install(cx));
    let (mac, _) = common::open_workspace(cx, Args::default());
    assert!(!common::exists(cx, mac, "app-menu"));
    cx.update(|cx| cx.rogg().clone().with_chrome(ChromeStyle::Linux).install(cx));
    let (linux, ws) = common::open_workspace(cx, Args::default());
    assert!(common::exists(cx, linux, "app-menu"));
    assert!(!common::exists(cx, linux, "window-close"), "the test platform's windows are server-decorated");
    ws.update(cx, |ws, cx| ws.set_traffic_lights(true, cx));
    assert!(common::exists(cx, linux, "window-close"), "a headless mock-up draws the controls");
}

#[gpui_kit::test]
fn the_drawn_close_control_saves_the_session(cx: &mut TestAppContext) {
    use rogg_ui::ActiveRoggTheme;
    use rogg_ui::theme::ChromeStyle;
    let dir = common::setup(cx);
    cx.update(|cx| cx.rogg().clone().with_chrome(ChromeStyle::Linux).install(cx));
    let (w, ws) = common::open_workspace(cx, Args::default());
    ws.update(cx, |ws, cx| ws.set_traffic_lights(true, cx));
    let session = dir.path().join("session.json");
    assert!(!session.exists());
    common::click(cx, w, "window-close");
    assert!(session.exists(), "closing through rogg's Linux control skips on_window_should_close, so it saves first");
}
