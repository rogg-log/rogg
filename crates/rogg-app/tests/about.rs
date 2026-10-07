mod common;
use gpui_kit::{AppContext, TestAppContext};
use rogg_app::actions::ShowAbout;
use rogg_app::args::Args;

#[gpui_kit::test]
fn about_opens_from_its_action_and_reveals_the_data_folders(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![] });
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(ShowAbout), cx)).unwrap();
    cx.run_until_parked();
    assert!(common::exists(cx, w, "about-header"));
    assert!(common::exists(cx, w, "about-build-0") && common::exists(cx, w, "about-build-2"));
    assert!(common::exists(cx, w, "about-credit-13"));
    common::click(cx, w, "about-reveal-0");
    assert_eq!(ws.read_with(cx, |ws, _| ws.last_revealed().map(|p| p.to_path_buf())), Some(dir.path().to_path_buf()), "Application Support (the settings folder)");
    common::click(cx, w, "about-reveal-1");
    assert_eq!(ws.read_with(cx, |ws, _| ws.last_revealed().map(|p| p.to_path_buf())), Some(dir.path().join("cache")));
    assert!(dir.path().join("cache").is_dir(), "a missing cache folder is created so Finder can show it");
}
