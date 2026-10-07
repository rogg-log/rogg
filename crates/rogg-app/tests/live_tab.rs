mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, ScrollDelta, TestAppContext, point, px};
use rogg_ui::logview::ScrollTo;
use rogg_app::args::Args;
use rogg_app::document::Source;
use rogg_app::tab::{RELOADED_TOAST, Tab, TabInit};
use rogg_app::workspace::Workspace;
use std::io::Write;
use std::path::{Path, PathBuf};

fn append(path: &Path, s: &str) {
    std::fs::OpenOptions::new().append(true).open(path).unwrap().write_all(s.as_bytes()).unwrap();
}

fn lines(tab: &Entity<Tab>, cx: &mut TestAppContext) -> u64 {
    tab.read_with(cx, |t, cx| t.doc.read(cx).text().map_or(0, |x| x.line_count()))
}

fn open_stream(cx: &mut TestAppContext, dir: &Path, first: usize) -> (AnyWindowHandle, Entity<Workspace>, Entity<Tab>, PathBuf) {
    let path = dir.join("stream.log");
    std::fs::write(&path, (0..first).map(|i| format!("early {i}\n")).collect::<String>()).unwrap();
    let (w, ws) = common::open_workspace(cx, Args::default());
    let source = Source::Stream { name: "stdin".into(), path: path.clone() };
    let tab = cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_source(source, TabInit::default(), window, cx))).unwrap();
    cx.run_until_parked();
    (w, ws, tab, path)
}

#[gpui_kit::test]
fn streams_follow_new_lines_until_paused(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, _ws, tab, path) = open_stream(cx, dir.path(), 1);
    assert!(common::exists(cx, w, "tool-live"));
    assert!(tab.read_with(cx, |t, _| t.live.following));
    append(&path, &(0..200).map(|i| format!("line {i}\n")).collect::<String>());
    common::wait_until(cx, |cx| lines(&tab, cx) == 201);
    common::render(cx, w);
    let full = tab.read_with(cx, |t, _| t.full.clone());
    assert_eq!(full.read_with(cx, |s, _| s.last_rows().last().unwrap().row), 200, "the newest line is in view");
    common::press(cx, w, "tab");
    common::press(cx, w, "space");
    assert!(!tab.read_with(cx, |t, _| t.live.following));
    append(&path, "a\nb\nc\nd\ne\n");
    common::wait_until(cx, |cx| lines(&tab, cx) == 206);
    assert_eq!(tab.read_with(cx, |t, _| (t.live.following, t.live.new_lines)), (false, 5));
    common::render(cx, w);
    assert!(common::exists(cx, w, "tool-live"), "the paused control stays in the header");
    common::render(cx, w);
    assert!(full.read_with(cx, |s, _| s.last_rows().last().unwrap().row) < 205, "paused: the view stays");
    common::click(cx, w, "tool-live");
    assert_eq!(tab.read_with(cx, |t, _| (t.live.following, t.live.new_lines)), (true, 0));
    common::render(cx, w);
    assert_eq!(full.read_with(cx, |s, _| s.last_rows().last().unwrap().row), 205, "resuming jumps to the end");
}

#[gpui_kit::test]
fn scrolling_the_full_log_up_pauses_following(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let (w, _ws, tab, _path) = open_stream(cx, dir.path(), 300);
    common::render(cx, w);
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("log-top", ScrollDelta::Pixels(point(px(0.), px(200.))), cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!tab.read_with(cx, |t, _| t.live.following));
}

#[gpui_kit::test]
fn files_follow_when_their_end_was_in_view(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let short = common::write_log(dir.path(), "short.log", &["one", "two", ""]);
    let long_lines: Vec<String> = (0..1000).map(|i| format!("l{i}")).collect();
    let mut long_refs: Vec<&str> = long_lines.iter().map(String::as_str).collect();
    long_refs.push("");
    let long = common::write_log(dir.path(), "long.log", &long_refs);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![short.clone(), long.clone()] });
    common::render(cx, w);
    let tabs = ws.read_with(cx, |ws, _| ws.tabs().to_vec());
    assert!(!common::exists(cx, w, "tool-live"), "a file is not live until it grows");
    append(&short, "three\n");
    append(&long, "more\nand more\n");
    common::wait_until(cx, |cx| lines(&tabs[0], cx) == 3 && lines(&tabs[1], cx) == 1002);
    assert!(tabs[0].read_with(cx, |t, _| t.live.following), "everything fitted: the end was in view");
    assert_eq!(tabs[1].read_with(cx, |t, _| (t.live.following, t.live.new_lines)), (false, 2), "scrolled to the top: paused, counting");
}

#[gpui_kit::test]
fn a_truncated_file_reloads_with_a_toast(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "t.log", &["a", "b", ""]);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path.clone()] });
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(0).unwrap();
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, m)| m.as_ref() == RELOADED_TOAST)));
}

#[gpui_kit::test]
fn a_file_scrolled_to_its_end_follows_when_it_grows(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let text: Vec<String> = (0..1000).map(|i| format!("l{i}")).collect();
    let mut refs: Vec<&str> = text.iter().map(String::as_str).collect();
    refs.push("");
    let path = common::write_log(dir.path(), "long.log", &refs);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path.clone()] });
    common::render(cx, w);
    let tab = ws.read_with(cx, |ws, _| ws.tabs()[0].clone());
    let full = tab.read_with(cx, |t, _| t.full.clone());
    full.update(cx, |s, cx| s.scroll_to_row(999, ScrollTo::Visible, cx));
    common::render(cx, w);
    assert!(full.read_with(cx, |s, _| s.at_bottom()), "the end is in view before the file grows");
    append(&path, "more\nand more\n");
    common::wait_until(cx, |cx| lines(&tab, cx) == 1002);
    assert_eq!(tab.read_with(cx, |t, _| (t.live.following, t.live.new_lines)), (true, 0), "the end was in view: follow");
    common::render(cx, w);
    assert_eq!(full.read_with(cx, |s, _| s.last_rows().last().unwrap().row), 1001, "the newest line is in view");
}

#[gpui_kit::test]
fn a_reloaded_file_is_not_live_until_it_grows_again(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "t.log", &["a", "b", ""]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path.clone()] });
    common::render(cx, w);
    let tab = ws.read_with(cx, |ws, _| ws.tabs()[0].clone());
    append(&path, "c\n");
    common::wait_until(cx, |cx| lines(&tab, cx) == 3);
    assert!(tab.read_with(cx, |t, _| t.live.following));
    std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_len(0).unwrap();
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, m)| m.as_ref() == RELOADED_TOAST)));
    assert_eq!(tab.read_with(cx, |t, _| t.live), Default::default());
    assert!(!tab.read_with(cx, |t, cx| t.full.read(cx).tail()), "tail mode is off");
    common::render(cx, w);
    assert!(!common::exists(cx, w, "tool-live"), "not live until it grows again");
}
