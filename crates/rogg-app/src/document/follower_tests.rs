//! Tests that need private state: dropping the follower by hand to simulate it ending (a watcher failure) mid-decompression,
//! and reloading while a scan runs.

use super::*;
use crate::paths::AppPaths;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_genlog::bundle::write_gz;

fn wait_until(cx: &mut TestAppContext, mut done: impl FnMut(&mut TestAppContext) -> bool) {
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

fn lines(doc: &Entity<Document>, cx: &mut TestAppContext) -> u64 {
    doc.read_with(cx, |d, _| d.text().map_or(0, |t| t.line_count()))
}

#[gpui_kit::test]
fn a_gz_whose_follower_ended_is_reloaded_complete_when_decompression_finishes(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| crate::init_with(cx, AppPaths::under(dir.path())));
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    const LINES: u64 = 3_000_000;
    let body: String = (0..LINES).map(|i| format!("line {i}\n")).collect();
    let gz = dir.path().join("big.log.gz");
    write_gz(&gz, body.as_bytes()).unwrap();
    let doc = cx.new(|cx| Document::new(Source::File(gz), DocumentInit::default(), cx));
    // The follower ends while the decompression is still running (the poll has not seen it finish yet).
    wait_until(cx, |cx| {
        doc.update(cx, |d, _| {
            let running = d.inflating.is_some() && d.follower.is_some();
            if running {
                d.stop_follower();
            }
            running
        })
    });
    wait_until(cx, |cx| lines(&doc, cx) == LINES && doc.read_with(cx, |d, _| d.inflating.is_none()));
    // The text was reloaded from the complete cache entry: not a `Reloaded` (no pins cleared), just a second load.
    assert_eq!(doc.read_with(cx, |d, _| d.load_revision()), 2);
}

#[gpui_kit::test]
fn a_reload_drops_the_running_scan_and_its_progress(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    cx.update(|cx| crate::init_with(cx, AppPaths::under(dir.path())));
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let body: String = (0..3_000_000u64).map(|i| format!("line {i}\n")).collect();
    let path = dir.path().join("big.log");
    std::fs::write(&path, body).unwrap();
    let init = DocumentInit { spec: FilterSpec::from_expression("line"), ..Default::default() };
    let doc = cx.new(|cx| Document::new(Source::File(path), init, cx));
    wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_some()));
    // The reload opens in the background, so the document is "Opening" when `reload` returns.
    doc.update(cx, |d, cx| {
        d.reload(cx);
        assert!(matches!(d.load, LoadState::Loading { .. }));
        assert!(d.filter.job.is_none(), "the superseded scan is retired");
        assert_eq!(d.filter.progress, None, "its progress must not show while the tab is opening");
    });
    wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none() && d.filter.matches.len() == 3_000_000));
    assert_eq!(doc.read_with(cx, |d, _| d.load_revision()), 2);
}
