mod common;
use gpui_kit::{AnyWindowHandle, AppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::document::LoadState;
use rogg_app::recent::RecentStore;
use rogg_app::workspace::Workspace;
use rogg_core::source::SourceRef;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, write_archive, write_stored_zip};

fn titles(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Vec<String> {
    ws.read_with(cx, |ws, cx| ws.tabs().iter().map(|t| t.read(cx).title(cx).to_string()).collect())
}

const ROW_IDS: [&str; 12] = [
    "tree-row-0", "tree-row-1", "tree-row-2", "tree-row-3", "tree-row-4", "tree-row-5",
    "tree-row-6", "tree-row-7", "tree-row-8", "tree-row-9", "tree-row-10", "tree-row-11",
];

fn rows(cx: &mut TestAppContext, w: AnyWindowHandle) -> usize {
    ROW_IDS.iter().take_while(|&&id| common::exists(cx, w, id)).count()
}

fn wait_rows(cx: &mut TestAppContext, w: AnyWindowHandle, n: usize) {
    common::wait_until(cx, |cx| rows(cx, w) == n);
}

#[gpui_kit::test]
fn folders_list_their_logs_and_open_the_first(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let logs = dir.path().join("logs");
    std::fs::create_dir_all(logs.join("sub")).unwrap();
    common::write_log(&logs, "one.log", &["1"]);
    common::write_log(&logs, "two.log", &["2"]);
    common::write_log(&logs, ".hidden.log", &["h"]);
    common::write_log(&logs.join("sub"), "three.log", &["3"]);
    std::fs::write(logs.join("blob.bin"), [0u8, 159, 146, 150]).unwrap();
    std::fs::write(logs.join("utf16.log"), [0xFF, 0xFE, b'h', 0, b'i', 0]).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![logs.clone()] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    assert_eq!(titles(&ws, cx), ["one.log"], "the first top-level log opens");
    // logs (expanded), sub (collapsed), one.log, two.log, utf16.log
    wait_rows(cx, w, 5);
    common::click(cx, w, "tree-row-1");
    wait_rows(cx, w, 6);
    common::click(cx, w, "tree-row-2");
    assert_eq!(titles(&ws, cx), ["one.log", "three.log"]);
    assert_eq!(ws.read_with(cx, |ws, _| ws.active_index()), 1);
    cx.update(|cx| {
        let entries = cx.global::<RecentStore>().entries();
        assert!(entries[0].path.ends_with("three.log"), "a clicked file is recorded");
        assert!(entries.iter().any(|e| e.is_folder && e.path.ends_with("logs")));
        assert!(!entries.iter().any(|e| e.path.ends_with("one.log")), "the automatically opened file is not");
    });
}

#[gpui_kit::test]
fn archives_list_lazily_and_open_nested_members(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let inner = dir.path().join("inner.tgz");
    write_archive(&inner, BundleKind::TarGz, &[("app/x.log", b"nested\n")]).unwrap();
    let inner_bytes = std::fs::read(&inner).unwrap();
    let zip = dir.path().join("outer.zip");
    write_archive(&zip, BundleKind::Zip, &[("logs/api.log", b"api\n"), ("inner.tgz", &inner_bytes), ("README", b"read me\n")]).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![zip.clone()] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    assert_eq!(titles(&ws, cx), ["README"]);
    // outer.zip, logs (collapsed), inner.tgz (not listed yet), README
    wait_rows(cx, w, 4);
    common::click(cx, w, "tree-row-2");
    // … inner.tgz lists in the background: + app
    wait_rows(cx, w, 5);
    common::click(cx, w, "tree-row-3");
    wait_rows(cx, w, 6);
    common::click(cx, w, "tree-row-4");
    assert_eq!(titles(&ws, cx), ["README", "x.log"]);
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    let crumbs: Vec<String> = tab.read_with(cx, |t, cx| t.doc.read(cx).source.crumbs().iter().map(|c| c.to_string()).collect());
    assert_eq!(crumbs, ["outer.zip", "inner.tgz", "app", "x.log"]);
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.doc.read(cx).text().is_some()));
    let footer = ws.read_with(cx, |ws, cx| ws.sidebar_footer(cx)).unwrap();
    assert_eq!(footer.title.as_ref(), "outer.zip");
    assert_eq!(footer.lines.iter().map(|l| l.to_string()).collect::<Vec<_>>(), ["3 members · 19 B expanded", "Members open individually"]);
    cx.update(|cx| {
        let entry = &cx.global::<RecentStore>().entries()[0];
        assert_eq!(entry.members, ["inner.tgz", "app/x.log"]);
        assert!(entry.path.ends_with("outer.zip"));
    });
}

#[gpui_kit::test]
fn a_damaged_member_fails_only_its_own_tab(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("d.zip");
    write_stored_zip(&zip, &[("api.log", b"api one\napi two\n"), ("README", b"read me\n")]).unwrap();
    damage_zip_member(&zip, 0).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![zip.clone()] });
    // d.zip, api.log (opened first, by name), README
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    wait_rows(cx, w, 3);
    common::click(cx, w, "tree-row-2");
    let tabs = ws.read_with(cx, |ws, _| ws.tabs().to_vec());
    assert_eq!(tabs.len(), 2);
    let state = |i: usize, cx: &mut TestAppContext| tabs[i].read_with(cx, |t, cx| t.doc.read(cx).load.clone());
    common::wait_until(cx, |cx| matches!(state(0, cx), LoadState::Failed { .. }));
    common::wait_until(cx, |cx| matches!(state(1, cx), LoadState::Ready(_)));
    let LoadState::Failed { message } = state(0, cx) else { unreachable!() };
    assert!(message.contains("api.log"), "{message}");
}

#[gpui_kit::test]
fn unreadable_archives_are_marked_and_toasted(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let junk = dir.path().join("junk.zip");
    std::fs::write(&junk, b"PK\x03\x04 not really a zip").unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![junk] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, m)| m.starts_with("Could not read junk.zip"))));
    assert!(common::exists(cx, w, "tree-note-0"), "the archive row says it is unreadable");
    assert!(ws.read_with(cx, |ws, _| ws.tabs().is_empty()));
}

#[gpui_kit::test]
fn huge_folders_list_the_first_files_with_a_toast(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let big = dir.path().join("big");
    std::fs::create_dir(&big).unwrap();
    for i in 0..=rogg_core::folder::MAX_SCAN_FILES {
        std::fs::write(big.join(format!("f{i:05}.log")), "").unwrap();
    }
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![big] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, m)| m.contains("showing the first 10,000 files"))));
}

#[gpui_kit::test]
fn recent_archive_members_reopen_their_member(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("r.zip");
    write_archive(&zip, BundleKind::Zip, &[("a.log", b"a\n"), ("b.log", b"b\n")]).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![zip.clone()] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    wait_rows(cx, w, 3);
    common::click(cx, w, "tree-row-2");
    assert_eq!(titles(&ws, cx), ["a.log", "b.log"]);
    common::press(cx, w, "cmd-w");
    common::press(cx, w, "cmd-w");
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.open_recent(0, window, cx))).unwrap();
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    assert_eq!(titles(&ws, cx), ["b.log"], "the member opens, not the archive's first file");
    let source = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.read(cx).source.source_ref());
    assert_eq!(source, Some(SourceRef::member(SourceRef::File(zip.canonicalize().unwrap()), "b.log")));
}

#[gpui_kit::test]
fn a_failed_listing_is_retried_when_the_archive_is_expanded_again(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("fixed.zip");
    std::fs::write(&zip, b"PK\x03\x04 not really a zip").unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![zip.clone()] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.toasts.iter().any(|(_, m)| m.starts_with("Could not read fixed.zip"))));
    assert_eq!(rows(cx, w), 1);
    write_archive(&zip, BundleKind::Zip, &[("a.log", b"a\n")]).unwrap();
    common::click(cx, w, "tree-row-0");
    common::click(cx, w, "tree-row-0");
    wait_rows(cx, w, 2);
    assert!(!common::exists(cx, w, "tree-note-0"), "the unreadable note is gone once it lists");
}
