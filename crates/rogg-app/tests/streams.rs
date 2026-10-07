mod common;
use gpui_kit::prelude::*;
use gpui_kit::{Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::recent::RecentStore;
use rogg_app::tab::Tab;
use std::io::Write;

fn lines(tab: &Entity<Tab>, cx: &mut TestAppContext) -> u64 {
    tab.read_with(cx, |t, cx| t.doc.read(cx).text().map_or(0, |x| x.line_count()))
}

#[gpui_kit::test]
fn piped_data_opens_as_a_live_tab(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    let (reader, mut writer) = std::io::pipe().unwrap();
    let tab = cx
        .update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_stream("stdin", move || Ok(reader), window, cx)))
        .unwrap()
        .expect("a stream tab");
    writer.write_all(b"one\ntwo\n").unwrap();
    common::wait_until(cx, |cx| lines(&tab, cx) == 2);
    assert_eq!(tab.read_with(cx, |t, cx| t.title(cx).to_string()), "stdin");
    assert!(tab.read_with(cx, |t, _| t.live.following), "a stream follows from the start");
    assert!(common::exists(cx, w, "tool-live"));
    writer.write_all(b"three\n").unwrap();
    drop(writer);
    common::wait_until(cx, |cx| lines(&tab, cx) == 3);
    cx.update(|cx| assert!(cx.global::<RecentStore>().entries().is_empty(), "streams are not recent files"));
}

#[gpui_kit::test]
fn fifos_are_read_as_streams(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let fifo = dir.path().join("events");
    let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![fifo.clone()] });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab()).expect("the FIFO opens a tab at once");
    assert_eq!(tab.read_with(cx, |t, cx| t.title(cx).to_string()), "events");
    let writer = std::thread::spawn(move || {
        let mut f = std::fs::OpenOptions::new().write(true).open(&fifo).unwrap();
        f.write_all(b"x\ny\n").unwrap();
    });
    common::wait_until(cx, |cx| lines(&tab, cx) == 2);
    writer.join().unwrap();
}
