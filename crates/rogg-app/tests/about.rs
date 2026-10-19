mod common;
use gpui_kit::{AppContext, TestAppContext};
use rogg_app::actions::ShowAbout;
use rogg_app::args::Args;

#[gpui_kit::test]
fn about_opens_from_its_action_and_reveals_the_data_folders(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![], ..Default::default() });
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

/// Linux keeps settings and profiles in `~/.config/rogg` but session, history and recent files in `~/.local/share/rogg`:
/// each folder gets its own row and reveal, then the cache.
#[gpui_kit::test]
fn separate_config_and_data_folders_each_reveal(cx: &mut TestAppContext) {
    use rogg_app::paths::AppPaths;
    let dir = tempfile::tempdir().unwrap();
    let (config, data) = (dir.path().join("config"), dir.path().join("data"));
    let paths = AppPaths {
        history: data.join("history.json"),
        recent: data.join("recent.json"),
        cache: dir.path().join("cache"),
        settings: config.join("settings.toml"),
        profiles: config.join("profiles"),
        session: data.join("session.json"),
    };
    common::init(cx, paths);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![], ..Default::default() });
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(ShowAbout), cx)).unwrap();
    cx.run_until_parked();
    let revealed = |cx: &mut TestAppContext| ws.read_with(cx, |ws, _| ws.last_revealed().map(|p| p.to_path_buf()));
    common::click(cx, w, "about-reveal-0");
    assert_eq!(revealed(cx), Some(config), "settings and profiles");
    common::click(cx, w, "about-reveal-1");
    assert_eq!(revealed(cx), Some(data), "session, history and recent files");
    common::click(cx, w, "about-reveal-2");
    assert_eq!(revealed(cx), Some(dir.path().join("cache")));
}
