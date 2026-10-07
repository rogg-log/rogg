use rogg_core::CancelToken;
use rogg_core::cache::{Cache, DEFAULT_LIMIT_BYTES};
use rogg_core::encoding::Encoding;
use rogg_core::error::CoreError;
use rogg_core::source::{InflatePoll, OpenStage, OpenedSource, SourceRef, list_archive, open_source};
use rogg_core::text::LogText;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, gz_bytes, write_archive, write_gz, write_stored_zip};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn lines(text: &LogText) -> Vec<String> {
    (0..text.line_count()).map(|i| text.line_text(i).into_owned()).collect()
}

fn cache(dir: &Path) -> Arc<Cache> {
    Cache::new(dir.join("cache"), DEFAULT_LIMIT_BYTES)
}

/// The complete text of an opened source: waits for a progressive `.gz` to finish, then opens its entry.
fn finish(opened: OpenedSource, cache: &Cache) -> LogText {
    let Some(mut inflating) = opened.inflating else { return opened.text };
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match inflating.poll() {
            InflatePoll::Done(result) => {
                result.unwrap();
                return LogText::open(inflating.dst(), cache.dir(), &CancelToken::new(), |_| {}).unwrap();
            }
            InflatePoll::Running => {
                assert!(Instant::now() < deadline, "decompression did not finish");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

#[test]
fn plain_files_open_in_place_and_are_watched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.log");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    let opened = open_source(&SourceRef::File(path.clone()), &cache(dir.path()), &CancelToken::new(), |_| {}).unwrap();
    assert_eq!(lines(&opened.text), ["one", "two"]);
    assert_eq!(opened.watch, Some(path.clone()));
    assert_eq!(opened.text.backing_path(), Some(path.as_path()));
    assert!(opened.leases.is_empty() && opened.inflating.is_none());
}

#[test]
fn members_are_extracted_once_and_leased() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("b.zip");
    write_archive(&zip, BundleKind::Zip, &[("logs/api.log", b"api one\napi two\n")]).unwrap();
    let cache = cache(dir.path());
    let member = SourceRef::member(SourceRef::File(zip), "logs/api.log");
    let mut stages = Vec::new();
    let first = open_source(&member, &cache, &CancelToken::new(), |u| stages.push((u.stage, u.name))).unwrap();
    assert!(stages.contains(&(OpenStage::Extracting, "api.log".to_string())), "{stages:?}");
    assert_eq!(lines(&first.text), ["api one", "api two"]);
    assert_eq!(first.watch, None, "cache files are not watched");
    assert_eq!(first.leases.len(), 1);
    assert!(cache.is_leased(first.leases[0].path()));
    assert_eq!(cache.evict_to(0).removed, 0, "an open member is never evicted");
    let mut again = Vec::new();
    let second = open_source(&member, &cache, &CancelToken::new(), |u| again.push(u.stage)).unwrap();
    assert!(!again.contains(&OpenStage::Extracting), "the cached member is reused");
    drop((first, second));
    assert_eq!(cache.evict_to(0).removed, 1, "closed members are evictable");
}

#[test]
fn nested_archives_and_compressed_members_open() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("inner.tgz");
    write_archive(&inner, BundleKind::TarGz, &[("app/x.log", b"nested line\n")]).unwrap();
    let inner_bytes = std::fs::read(&inner).unwrap();
    let gz = gz_bytes(b"gz line\n");
    let zip = dir.path().join("outer.zip");
    write_archive(&zip, BundleKind::Zip, &[("inner.tgz", &inner_bytes), ("old.log.1.gz", &gz)]).unwrap();
    let cache = cache(dir.path());
    let outer = SourceRef::File(zip);
    let nested = SourceRef::member(SourceRef::member(outer.clone(), "inner.tgz"), "app/x.log");
    let opened = open_source(&nested, &cache, &CancelToken::new(), |_| {}).unwrap();
    assert_eq!(lines(&opened.text), ["nested line"]);
    let compressed = open_source(&SourceRef::member(outer.clone(), "old.log.1.gz"), &cache, &CancelToken::new(), |_| {}).unwrap();
    assert_eq!(lines(&finish(compressed, &cache)), ["gz line"]);
    let entries = list_archive(&SourceRef::member(outer, "inner.tgz"), &cache, &CancelToken::new()).unwrap();
    assert_eq!(entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), ["app/x.log"]);
}

#[test]
fn large_gz_files_are_viewable_while_they_decompress() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.log.gz");
    let line = format!("{}\n", "x".repeat(99));
    write_gz(&path, line.repeat(1_000_000).as_bytes()).unwrap();
    let cache = cache(dir.path());
    let mut stages = Vec::new();
    let opened = open_source(&SourceRef::File(path.clone()), &cache, &CancelToken::new(), |u| stages.push(u.stage)).unwrap();
    assert!(stages.contains(&OpenStage::Decompressing));
    assert!(opened.inflating.is_some(), "100 MB take longer to decompress than the first bytes");
    let prefix = opened.text.line_count();
    assert!(prefix < 1_000_000, "the first text is a prefix ({prefix} lines)");
    let full = finish(opened, &cache);
    assert_eq!(full.line_count(), 1_000_000);
    let mut again = Vec::new();
    let reopened = open_source(&SourceRef::File(path), &cache, &CancelToken::new(), |u| again.push(u.stage)).unwrap();
    assert!(reopened.inflating.is_none() && !again.contains(&OpenStage::Decompressing), "the decompressed entry is reused");
    assert_eq!(reopened.text.line_count(), 1_000_000);
}

#[test]
fn utf16_gz_files_are_complete_before_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let mut utf16 = vec![0xFF, 0xFE];
    for u in "héllo\r\nwörld\n".encode_utf16() {
        utf16.extend(u.to_le_bytes());
    }
    let path = dir.path().join("w.log.gz");
    write_gz(&path, &utf16).unwrap();
    let cache = cache(dir.path());
    let opened = open_source(&SourceRef::File(path), &cache, &CancelToken::new(), |_| {}).unwrap();
    assert!(opened.inflating.is_none());
    assert_eq!(opened.text.encoding(), Encoding::Utf16Le);
    assert_eq!(lines(&opened.text), ["héllo", "wörld"]);
    assert_eq!(opened.leases.len(), 2, "the decompressed file and its UTF-8 conversion");
}

#[test]
fn errors_name_what_failed() {
    let dir = tempfile::tempdir().unwrap();
    let cache = cache(dir.path());
    let zip = dir.path().join("d.zip");
    write_stored_zip(&zip, &[("logs/api.log", b"api one\napi two\n"), ("README", b"read me\n")]).unwrap();
    damage_zip_member(&zip, 0).unwrap();
    let damaged = open_source(&SourceRef::member(SourceRef::File(zip.clone()), "logs/api.log"), &cache, &CancelToken::new(), |_| {});
    assert!(matches!(&damaged, Err(CoreError::Archive { message, .. }) if message.contains("logs/api.log")));
    assert!(open_source(&SourceRef::member(SourceRef::File(zip.clone()), "README"), &cache, &CancelToken::new(), |_| {}).is_ok());
    let as_file = open_source(&SourceRef::File(zip.clone()), &cache, &CancelToken::new(), |_| {});
    assert!(matches!(&as_file, Err(CoreError::Archive { message, .. }) if message.contains("is an archive")));
    let plain = dir.path().join("p.log");
    std::fs::write(&plain, "x\n").unwrap();
    let not_archive = open_source(&SourceRef::member(SourceRef::File(plain), "x"), &cache, &CancelToken::new(), |_| {});
    assert!(matches!(&not_archive, Err(CoreError::Archive { message, .. }) if message.contains("is not an archive")));
    let cancel = CancelToken::new();
    cancel.cancel();
    let zip2 = dir.path().join("c.zip");
    write_archive(&zip2, BundleKind::Zip, &[("a.log", b"a\n")]).unwrap();
    assert!(matches!(open_source(&SourceRef::member(SourceRef::File(zip2), "a.log"), &cache, &cancel, |_| {}), Err(CoreError::Cancelled)));
    assert!(matches!(open_source(&SourceRef::File(dir.path().join("missing.log")), &cache, &CancelToken::new(), |_| {}), Err(CoreError::Io { .. })));
}
