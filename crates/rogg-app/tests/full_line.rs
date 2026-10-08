mod common;
use gpui_kit::TestAppContext;
use rogg_app::args::Args;
use rogg_ui::logview::LogViewEvent;

#[gpui_kit::test]
fn show_full_line_opens_the_whole_line_and_copy_copies_it(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let long = format!("start {} end", "x".repeat(70 * 1024));
    let log = common::write_log(dir.path(), "a.log", &["short", &long]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let full = tab.read_with(cx, |t, _| t.full.clone());
    full.update(cx, |_, cx| cx.emit(LogViewEvent::ShowFullLine { number: 2 }));
    cx.run_until_parked();
    assert!(common::exists(cx, w, "full-line-text"));
    let (number, text) = ws.read_with(cx, |ws, _| ws.full_line_dialog()).expect("the dialog is open");
    assert_eq!((number, text.len()), (2, long.len()), "the whole line, past the 64 KB display cut");
    common::click(cx, w, "full-line-copy");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).map(|t| t.len()), Some(long.len()));
}
