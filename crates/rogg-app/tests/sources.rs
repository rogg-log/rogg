mod common;
use gpui_kit::{AppContext, Entity, SharedString, TestAppContext};
use rogg_app::cache::CacheStore;
use rogg_app::document::{Document, DocumentInit, EngineConfig, LoadState, Source};
use rogg_core::source::SourceRef;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, write_archive, write_gz, write_stored_zip};
use std::path::Path;

fn open(cx: &mut TestAppContext, source: Source) -> Entity<Document> {
    cx.new(|cx| Document::new(source, DocumentInit::default(), cx))
}

fn lines(doc: &Entity<Document>, cx: &mut TestAppContext) -> Option<Vec<String>> {
    doc.read_with(cx, |d, _| d.text().map(|t| (0..t.line_count()).map(|i| t.line_text(i).into_owned()).collect()))
}

fn failure(doc: &Entity<Document>, cx: &mut TestAppContext) -> Option<String> {
    doc.read_with(cx, |d, _| match &d.load {
        LoadState::Failed { message } => Some(message.to_string()),
        _ => None,
    })
}

fn files_with_ext(dir: &Path, ext: &str) -> usize {
    std::fs::read_dir(dir).map(|r| r.flatten().filter(|e| e.path().extension().is_some_and(|x| x == ext)).count()).unwrap_or(0)
}

fn member(zip: &Path, path: &str) -> Source {
    Source::Member(SourceRef::member(SourceRef::File(zip.to_path_buf()), path))
}

#[gpui_kit::test]
fn archive_members_open_in_the_background(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("bundle.zip");
    write_archive(&zip, BundleKind::Zip, &[("logs/api.log", b"one\ntwo error\nthree\n")]).unwrap();
    let source = member(&zip, "logs/api.log");
    assert_eq!(source.title(), SharedString::from("api.log"));
    assert_eq!(source.crumbs(), ["bundle.zip", "logs", "api.log"].map(SharedString::from));
    assert_eq!(source.tooltip(), SharedString::from("api.log · bundle.zip"));
    assert_eq!(source.location(), SharedString::from("bundle.zip › logs › api.log"));
    assert!(source.path().is_none());
    let doc = open(cx, source);
    assert!(matches!(doc.read_with(cx, |d, _| d.load.clone()), LoadState::Loading { .. }), "members always open off the UI thread");
    common::wait_until(cx, |cx| lines(&doc, cx).is_some());
    assert_eq!(lines(&doc, cx).unwrap(), ["one", "two error", "three"]);
    assert_eq!(doc.read_with(cx, |d, _| (d.load_revision(), d.text_revision())), (1, 1));
}

#[gpui_kit::test]
fn open_members_are_leased_until_their_tab_closes(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("bundle.zip");
    write_archive(&zip, BundleKind::Zip, &[("a.log", b"a\n")]).unwrap();
    let doc = open(cx, member(&zip, "a.log"));
    common::wait_until(cx, |cx| lines(&doc, cx).is_some());
    let cache = cx.update(|cx| CacheStore::get(cx));
    assert_eq!(files_with_ext(cache.dir(), "member"), 1);
    assert_eq!(cache.evict_to(0).removed, 0, "the open member is leased");
    drop(doc);
    // Dropped entities are released when gpui next flushes effects.
    cx.update(|_| {});
    cx.run_until_parked();
    assert_eq!(cache.evict_to(0).removed, 1, "closing the tab releases it");
    assert_eq!(files_with_ext(cache.dir(), "member"), 0);
}

#[gpui_kit::test]
fn gz_files_decompress_into_a_growing_text(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let body: String = (0..50_000).map(|i| format!("line {i}\n")).collect();
    let gz = dir.path().join("app.log.1.gz");
    write_gz(&gz, body.as_bytes()).unwrap();
    let doc = open(cx, Source::File(gz));
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some_and(|t| t.line_count() == 50_000) && d.job_progress().is_none()));
    let all = lines(&doc, cx).unwrap();
    assert_eq!((all[0].as_str(), all[49_999].as_str()), ("line 0", "line 49999"));
    assert_eq!(doc.read_with(cx, |d, _| d.load_revision()), 1, "growth while decompressing is not a reload");
}

#[gpui_kit::test]
fn a_damaged_member_shows_the_error_state_and_others_still_open(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("d.zip");
    write_stored_zip(&zip, &[("logs/api.log", b"api one\napi two\n"), ("README", b"read me\n")]).unwrap();
    damage_zip_member(&zip, 0).unwrap();
    let bad = open(cx, member(&zip, "logs/api.log"));
    common::wait_until(cx, |cx| failure(&bad, cx).is_some());
    assert!(failure(&bad, cx).unwrap().contains("logs/api.log"));
    let good = open(cx, member(&zip, "README"));
    common::wait_until(cx, |cx| lines(&good, cx).is_some());
    assert_eq!(lines(&good, cx).unwrap(), ["read me"]);
}

#[gpui_kit::test]
fn archives_opened_as_files_say_so(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let zip = dir.path().join("b.zip");
    write_archive(&zip, BundleKind::Zip, &[("a.log", b"a\n")]).unwrap();
    let doc = open(cx, Source::File(zip));
    common::wait_until(cx, |cx| failure(&doc, cx).is_some());
    assert!(failure(&doc, cx).unwrap().contains("is an archive"));
}

#[gpui_kit::test]
fn small_utf16_files_lease_their_conversion(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = dir.path().join("w.log");
    std::fs::write(&path, [0xFF, 0xFE, b'h', 0, b'i', 0, b'\n', 0]).unwrap();
    let doc = open(cx, Source::File(path));
    assert_eq!(lines(&doc, cx).unwrap(), ["hi"], "small files still open synchronously");
    let cache = cx.update(|cx| CacheStore::get(cx));
    assert_eq!(files_with_ext(cache.dir(), "utf8"), 1);
    assert_eq!(cache.evict_to(0).removed, 0);
    drop(doc);
    // Dropped entities are released when gpui next flushes effects.
    cx.update(|_| {});
    cx.run_until_parked();
    assert_eq!(cache.evict_to(0).removed, 1);
}

#[gpui_kit::test]
fn streams_open_in_the_background_when_large(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let path = dir.path().join("stream.log");
    std::fs::write(&path, b"one\ntwo\nthree\n").unwrap();
    let doc = open(cx, Source::Stream { name: "stdin".into(), path });
    assert!(matches!(doc.read_with(cx, |d, _| d.load.clone()), LoadState::Loading { .. }), "big streams open off the UI thread");
    common::wait_until(cx, |cx| lines(&doc, cx).is_some());
    assert_eq!(lines(&doc, cx).unwrap(), ["one", "two", "three"]);
}
