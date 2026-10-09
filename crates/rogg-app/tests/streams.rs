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

#[cfg(unix)]
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

#[gpui_kit::test]
fn a_failed_stream_toasts_once_and_keeps_its_tab(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    let tab = cx
        .update_window(w, |_, window, cx| {
            ws.update(cx, |ws, cx| ws.open_stream("broken", || Err::<std::fs::File, _>(std::io::Error::other("no writer")), window, cx))
        })
        .unwrap()
        .expect("a stream tab");
    let count = |cx: &mut TestAppContext| ws.read_with(cx, |ws, _| ws.toasts.iter().filter(|(_, m)| m.as_ref() == "Could not read broken: no writer").count());
    common::wait_until(cx, |cx| count(cx) == 1);
    // Several more timer ticks (still inside the toast's lifetime): no repeat.
    for _ in 0..100 {
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        cx.run_until_parked();
    }
    assert_eq!(count(cx), 1, "the failure is reported once");
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 1);
    assert_eq!(lines(&tab, cx), 0);
}

#[gpui_kit::test]
fn closing_a_stream_tab_stops_and_deletes_its_capture(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args::default());
    let (reader, mut writer) = std::io::pipe().unwrap();
    let tab = cx
        .update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_stream("stdin", move || Ok(reader), window, cx)))
        .unwrap()
        .expect("a stream tab");
    writer.write_all(b"one\n").unwrap();
    common::wait_until(cx, |cx| lines(&tab, cx) == 1);
    let paths = ws.read_with(cx, |ws, _| ws.stream_paths());
    assert_eq!(paths.len(), 1);
    assert!(paths[0].exists());
    drop(tab);
    common::press(cx, w, "cmd-w");
    assert!(ws.read_with(cx, |ws, _| ws.stream_paths().is_empty()), "the capture is dropped with its tab");
    assert!(!paths[0].exists(), "the capture file is deleted");
    // The producer keeps writing; the capture thread ends on that write and recreates nothing.
    writer.write_all(b"two\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(!paths[0].exists());
}
