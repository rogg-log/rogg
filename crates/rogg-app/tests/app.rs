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
