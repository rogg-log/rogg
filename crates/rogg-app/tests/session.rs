mod common;
use gpui_kit::{AnyWindowHandle, AppContext, TestAppContext};
use rogg_app::args::Args;
use rogg_app::document::{EngineConfig, LoadState};
use rogg_app::settings::SettingsStore;
use rogg_core::filter::FilterSpec;
use rogg_core::session::{DerivedSpec, Session, SessionSource, TabState};
use rogg_core::source::SourceRef;
use rogg_genlog::bundle::{BundleKind, write_archive};

#[gpui_kit::test]
fn a_session_round_trips_through_save_and_start(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let a = common::write_log(dir.path(), "a.log", &["error one", "ok", "error two", "x"]);
    let b = common::write_log(dir.path(), "b.log", &["b"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![a.clone(), b] });
    let tab_a = ws.read_with(cx, |ws, _| ws.tabs()[0].clone());
    common::wait_until(cx, |cx| tab_a.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab_a.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    doc.update(cx, |d, cx| d.toggle_pin(3, cx));
    tab_a.update(cx, |t, cx| {
        t.toggle_wrap(rogg_app::document::Pane::Filtered, cx);
        t.set_profile("Services".into(), cx);
        t.split = 0.3;
    });
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.activate(0, window, cx))).unwrap();
    tab_a.update(cx, |t, cx| t.select(rogg_app::document::Pane::Full, 3, false, cx));
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    let (saved, _) = Session::load(&dir.path().join("session.json")).unwrap();
    assert_eq!(saved.tabs.len(), 2);
    assert_eq!(saved.active, 0);
    let t = &saved.tabs[0];
    // Opened paths are canonical (the temp dir is under /private/var on macOS).
    assert_eq!(t.source, SessionSource::File { path: a.canonicalize().unwrap() });
    assert_eq!((t.filter.expression(), t.pins.clone(), t.selection, t.wrap, t.profile.as_deref()), ("error".into(), vec![3], Some(3), [false, true], Some("Services")));
    assert!((t.split - 0.3).abs() < 1e-6);
    let (_w2, ws2) = common::open_workspace(cx, Args::default());
    let restored = ws2.read_with(cx, |ws, _| ws.tabs()[0].clone());
    common::wait_until(cx, |cx| restored.read_with(cx, |t, cx| t.is_ready(cx) && t.doc.read(cx).filter.stats.is_some()));
    restored.read_with(cx, |t, cx| {
        let d = t.doc.read(cx);
        assert_eq!(d.filter.spec.expression(), "error");
        assert!(d.filter.pins.contains(&3));
        assert_eq!(t.profile.as_ref(), "Services");
        assert!(t.filtered.read(cx).wrap() && !t.full.read(cx).wrap());
        assert_eq!(t.selection(), Some(3));
        assert!((t.split - 0.3).abs() < 1e-6);
    });
    assert_eq!(ws2.read_with(cx, |ws, _| (ws.tabs().len(), ws.active_index())), (2, 0));
}

#[gpui_kit::test]
fn restore_keeps_good_tabs_and_fails_missing_ones(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let good = common::write_log(dir.path(), "good.log", &["fine"]);
    let gone = dir.path().join("gone.log");
    let member = SourceRef::member(SourceRef::File(dir.path().join("missing.zip")), "api.log");
    let derived = DerivedSpec { parent: SessionSource::File { path: gone.clone() }, filter: FilterSpec::from_expression("x"), pins: vec![], lines: None, snapshot: None };
    let session = Session {
        tabs: vec![
            TabState::new(SessionSource::File { path: good }),
            TabState::new(SessionSource::File { path: gone }),
            TabState::new(SessionSource::Member { source: member }),
            TabState::new(SessionSource::Derived(Box::new(derived))),
        ],
        ..Session::default()
    };
    session.save(&dir.path().join("session.json")).unwrap();
    let (_w, ws) = common::open_workspace(cx, Args::default());
    let tabs = ws.read_with(cx, |ws, _| ws.tabs().to_vec());
    assert_eq!(tabs.len(), 4);
    common::wait_until(cx, |cx| tabs.iter().all(|t| t.read_with(cx, |t, cx| !matches!(t.doc.read(cx).load, LoadState::Loading { .. }))));
    let states: Vec<bool> = tabs.iter().map(|t| t.read_with(cx, |t, cx| matches!(t.doc.read(cx).load, LoadState::Ready(_)))).collect();
    assert_eq!(states, [true, false, false, false], "missing sources open in the error state; the good tab restores");
}

#[gpui_kit::test]
fn derived_tabs_restore_from_their_definition(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["x1", "y", "x3"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    tab.read_with(cx, |t, _| t.doc.clone()).update(cx, |d, cx| d.set_expression("x", cx)).unwrap();
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.doc.read(cx).filter.stats.is_some()));
    common::press(cx, w, "shift-cmd-t");
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, cx| ws.tabs().len() == 2 && ws.tabs()[1].read(cx).is_ready(cx)));
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    let (_w2, ws2) = common::open_workspace(cx, Args::default());
    let derived = ws2.read_with(cx, |ws, _| ws.tabs()[1].clone());
    common::wait_until(cx, |cx| derived.read_with(cx, |t, cx| t.is_ready(cx)));
    assert_eq!(derived.read_with(cx, |t, cx| t.title(cx)).as_ref(), "a.log ⟩ filtered");
    let numbers: Vec<u64> = derived.read_with(cx, |t, cx| {
        let rows = t.full.read(cx).rows().clone();
        (0..rows.row_count()).filter_map(|i| rows.number_at(i)).collect()
    });
    assert_eq!(numbers, [1, 2], "a derived tab numbers its own lines");
}

#[gpui_kit::test]
fn streams_are_not_saved_and_restore_can_be_turned_off(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_stream("feed", || Ok(std::io::Cursor::new(b"a\n".to_vec())), window, cx))).unwrap();
    assert!(ws.read_with(cx, |ws, cx| ws.capture_session(true, cx)).tabs.is_empty());
    Session { tabs: vec![TabState::new(SessionSource::File { path: dir.path().join("x.log") })], ..Session::default() }.save(&dir.path().join("session.json")).unwrap();
    cx.update(|cx| {
        SettingsStore::update(cx, |s| s.behavior.restore_session = false);
    });
    let (_w2, ws2) = common::open_workspace(cx, Args::default());
    assert!(ws2.read_with(cx, |ws, _| ws.tabs().is_empty()));
}

#[gpui_kit::test]
fn a_corrupt_session_toasts_and_rogg_files_open(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    std::fs::write(dir.path().join("session.json"), "{ nope").unwrap();
    let (w, ws) = common::open_workspace(cx, Args::default());
    ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("session.json.bak")), "{:?}", ws.toasts));
    let log = common::write_log(dir.path(), "r.log", &["r"]);
    let rogg = dir.path().join("shared.rogg");
    Session { tabs: vec![TabState::new(SessionSource::File { path: log })], ..Session::default() }.save(&rogg).unwrap();
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![rogg.clone()], window, cx))).unwrap();
    assert_eq!(ws.read_with(cx, |ws, cx| ws.active_tab().map(|t| t.read(cx).title(cx).to_string())), Some("r.log".into()));
    let broken = dir.path().join("broken.rogg");
    std::fs::write(&broken, "{").unwrap();
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_paths(vec![broken.clone()], window, cx))).unwrap();
    ws.read_with(cx, |ws, _| assert!(ws.toasts.iter().any(|(_, t)| t.contains("Could not open broken.rogg")), "{:?}", ws.toasts));
    assert!(broken.exists());
}

#[test]
fn window_frames_convert_both_ways() {
    use gpui_kit::{Bounds, WindowBounds, point, px, size};
    use rogg_app::session::{bounds_for, window_state};
    let b = Bounds { origin: point(px(10.), px(20.)), size: size(px(1200.), px(800.)) };
    let s = window_state(WindowBounds::Maximized(b), false, px(240.));
    assert!(s.maximized && !s.sidebar_visible && s.sidebar_width == Some(240.));
    assert_eq!(bounds_for(&s), WindowBounds::Maximized(b));
    let tiny = rogg_core::session::WindowState { x: 0., y: 0., width: 100., height: 50., maximized: false, sidebar_visible: true, sidebar_width: None };
    let WindowBounds::Windowed(t) = bounds_for(&tiny) else { panic!() };
    assert_eq!((t.size.width, t.size.height), (px(760.), px(520.)), "never smaller than the window's minimum");
}

// Carry-over checks beyond the brief's tests.

#[test]
fn frames_with_non_finite_values_are_not_restored() {
    use rogg_app::session::restorable_bounds;
    let good = rogg_core::session::WindowState { x: 0., y: 0., width: 900., height: 700., maximized: false, sidebar_visible: true, sidebar_width: None };
    assert!(restorable_bounds(&good).is_some());
    for bad in [
        rogg_core::session::WindowState { x: f32::NAN, ..good.clone() },
        rogg_core::session::WindowState { width: f32::INFINITY, ..good.clone() },
        rogg_core::session::WindowState { height: f32::NEG_INFINITY, ..good.clone() },
    ] {
        assert!(restorable_bounds(&bad).is_none(), "{bad:?}");
    }
}

#[gpui_kit::test]
fn a_tab_with_a_non_utf8_path_is_skipped_not_the_whole_session(cx: &mut TestAppContext) {
    use std::os::unix::ffi::OsStrExt;
    let dir = common::setup(cx);
    let good = common::write_log(dir.path(), "good.log", &["fine"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![good.clone()] });
    let odd = dir.path().join(std::ffi::OsStr::from_bytes(b"odd-\xff.log"));
    let member = SourceRef::member(SourceRef::File(dir.path().join(std::ffi::OsStr::from_bytes(b"odd-\xfe.zip"))), "api.log");
    let derived = DerivedSpec { parent: SessionSource::File { path: odd.clone() }, filter: FilterSpec::from_expression("x"), pins: vec![], lines: None, snapshot: None };
    cx.update_window(w, |_, window, cx| {
        ws.update(cx, |ws, cx| {
            let init = || rogg_app::tab::TabInit::default();
            ws.open_source(rogg_app::document::Source::File(odd.clone()), init(), window, cx);
            ws.open_source(rogg_app::document::Source::Member(member.clone()), init(), window, cx);
            let derived = rogg_app::derived::DerivedSource::restored(derived.clone());
            ws.open_source(rogg_app::document::Source::Derived(std::sync::Arc::new(derived)), init(), window, cx);
            ws.activate(0, window, cx);
        })
    })
    .unwrap();
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    let (saved, recovered) = Session::load(&dir.path().join("session.json")).unwrap();
    assert!(!recovered);
    assert_eq!(saved.tabs.iter().map(|t| t.source.clone()).collect::<Vec<_>>(), [SessionSource::File { path: good.canonicalize().unwrap() }]);
}

#[gpui_kit::test]
fn non_finite_values_never_reach_the_session_file(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["a"]);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.tabs()[0].clone());
    tab.update(cx, |t, _| t.split = f32::NAN);
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    let text = std::fs::read_to_string(dir.path().join("session.json")).unwrap();
    assert!(!text.contains("\"split\": null"), "{text}");
    let (saved, recovered) = Session::load(&dir.path().join("session.json")).unwrap();
    assert!(!recovered);
    assert!((saved.tabs[0].split - rogg_core::session::DEFAULT_SPLIT).abs() < 1e-6);
}

fn top_numbers(tab: &gpui_kit::Entity<rogg_app::tab::Tab>, cx: &mut TestAppContext) -> [Option<u64>; 2] {
    tab.read_with(cx, |t, cx| [t.full.read(cx).top_number(), t.filtered.read(cx).top_number()])
}

#[gpui_kit::test]
fn top_lines_restore_even_when_the_filter_runs_in_the_background(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    // Every scan runs on a background thread: the filtered rows are empty when the text first loads.
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let lines: Vec<String> = (0..400).map(|i| if i % 2 == 0 { format!("x {i}") } else { format!("y {i}") }).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let log = common::write_log(dir.path(), "big.log", &refs);
    let mut state = TabState::new(SessionSource::File { path: log });
    state.filter = FilterSpec::from_expression("x");
    state.top = [Some(150), Some(201)];
    Session { tabs: vec![state], ..Session::default() }.save(&dir.path().join("session.json")).unwrap();
    let (w, ws): (AnyWindowHandle, _) = common::open_workspace(cx, Args::default());
    let tab = ws.read_with(cx, |ws, _| ws.tabs()[0].clone());
    // Before the text loads, a save keeps the stored tops.
    assert_eq!(ws.read_with(cx, |ws, cx| ws.capture_session(true, cx)).tabs[0].top, [Some(150), Some(201)]);
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx) && t.doc.read(cx).filter.progress.is_none() && t.doc.read(cx).filter.stats.is_some()));
    common::render(cx, w);
    assert_eq!(top_numbers(&tab, cx), [Some(150), Some(201)]);
    assert_eq!(ws.read_with(cx, |ws, cx| ws.capture_session(true, cx)).tabs[0].top, [Some(150), Some(201)]);
}

#[gpui_kit::test]
fn restored_members_put_their_archive_in_the_sidebar(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("r.zip");
    write_archive(&zip, BundleKind::Zip, &[("a.log", b"a\n"), ("b.log", b"b\n")]).unwrap();
    let member = SourceRef::member(SourceRef::File(zip.canonicalize().unwrap()), "b.log");
    Session { tabs: vec![TabState::new(SessionSource::Member { source: member })], ..Session::default() }.save(&dir.path().join("session.json")).unwrap();
    let (w, ws) = common::open_workspace(cx, Args::default());
    // r.zip, a.log, b.log
    common::wait_until(cx, |cx| common::exists(cx, w, "tree-row-2"));
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, cx| ws.tabs()[0].read(cx).is_ready(cx)));
    let titles: Vec<String> = ws.read_with(cx, |ws, cx| ws.tabs().iter().map(|t| t.read(cx).title(cx).to_string()).collect());
    assert_eq!(titles, ["b.log"], "the archive is listed, not opened at its first member");
}

#[gpui_kit::test]
fn a_failing_session_save_is_reported_once(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["a"]);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let set_mode = |mode: u32| std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap();
    set_mode(0o555);
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    set_mode(0o755);
    let failures = ws.read_with(cx, |ws, _| ws.toasts.iter().filter(|(_, t)| t.contains("Could not save the session")).count());
    assert_eq!(failures, 1, "a failure is toasted once, not on every autosave");
    assert!(ws.read_with(cx, |ws, _| ws.session_error.is_some()));
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    assert!(ws.read_with(cx, |ws, _| ws.session_error.is_none()), "a successful save clears the error");
    assert!(dir.path().join("session.json").exists());
}

#[gpui_kit::test]
fn the_demo_neither_restores_nor_saves_the_session(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "mine.log", &["mine"]);
    let path = dir.path().join("session.json");
    Session { tabs: vec![TabState::new(SessionSource::File { path: log })], ..Session::default() }.save(&path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let (_w, ws) = common::open_workspace(cx, Args { demo: true, paths: vec![] });
    let titles: Vec<String> = ws.read_with(cx, |ws, cx| ws.tabs().iter().map(|t| t.read(cx).title(cx).to_string()).collect());
    assert!(!titles.is_empty() && !titles.iter().any(|t| t == "mine.log"), "{titles:?}");
    ws.update(cx, |ws, cx| ws.save_session_now(cx));
    assert_eq!(std::fs::read(&path).unwrap(), before, "the demo never overwrites the user's session");
}

fn restore_big_filtered(cx: &mut TestAppContext) -> (tempfile::TempDir, AnyWindowHandle, gpui_kit::Entity<rogg_app::tab::Tab>) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let lines: Vec<String> = (0..400).map(|i| if i % 2 == 0 { format!("x {i}") } else { format!("y {i}") }).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let log = common::write_log(dir.path(), "big.log", &refs);
    let mut state = TabState::new(SessionSource::File { path: log });
    state.filter = FilterSpec::from_expression("x");
    state.top = [None, Some(201)];
    Session { tabs: vec![state], ..Session::default() }.save(&dir.path().join("session.json")).unwrap();
    let (w, ws) = common::open_workspace(cx, Args::default());
    let tab = ws.read_with(cx, |ws, _| ws.tabs()[0].clone());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    (dir, w, tab)
}

fn settle(cx: &mut TestAppContext, w: AnyWindowHandle, tab: &gpui_kit::Entity<rogg_app::tab::Tab>) {
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.doc.read(cx).filter_settled()));
    common::render(cx, w);
}

#[gpui_kit::test]
fn scrolling_the_filtered_log_during_the_scan_drops_its_restored_top(cx: &mut TestAppContext) {
    use gpui_kit::{ScrollDelta, point, px};
    let (_dir, w, tab) = restore_big_filtered(cx);
    assert!(tab.read_with(cx, |t, cx| !t.doc.read(cx).filter_settled()), "the scan is still running");
    common::render(cx, w);
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("log-bottom", ScrollDelta::Pixels(point(px(0.), px(-10.))), cx)
    })
    .unwrap();
    assert!(tab.read_with(cx, |t, cx| t.filtered.read(cx).user_scrolls() > 0));
    settle(cx, w, &tab);
    assert_ne!(top_numbers(&tab, cx)[1], Some(201), "the restored top does not yank a pane the user scrolled");
}

#[gpui_kit::test]
fn selecting_in_the_filtered_log_during_the_scan_drops_its_restored_top(cx: &mut TestAppContext) {
    let (_dir, w, tab) = restore_big_filtered(cx);
    assert!(tab.read_with(cx, |t, cx| !t.doc.read(cx).filter_settled()), "the scan is still running");
    let filtered = tab.read_with(cx, |t, _| t.filtered.clone());
    filtered.update(cx, |_, cx| cx.emit(rogg_ui::logview::LogViewEvent::Selected { row: 0, number: 1 }));
    settle(cx, w, &tab);
    assert_ne!(top_numbers(&tab, cx)[1], Some(201));
}
