use rogg_core::CancelToken;
use rogg_core::archive::{ArchiveKind, Content, extract, gunzip, kind_from_name, list, sniff};
use rogg_core::error::CoreError;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, gz_bytes, write_archive, write_gz, write_stored_zip};
use std::path::{Path, PathBuf};

const MEMBERS: [(&str, &[u8]); 3] = [("logs/api.log", b"api one\napi two\n"), ("logs/worker.log", b"worker\n"), ("README", b"read me\n")];

fn bundle(dir: &Path, name: &str, kind: BundleKind) -> PathBuf {
    let path = dir.join(name);
    write_archive(&path, kind, &MEMBERS).unwrap();
    path
}

const KINDS: [(BundleKind, ArchiveKind, &str); 3] =
    [(BundleKind::Zip, ArchiveKind::Zip, "b.zip"), (BundleKind::Tar, ArchiveKind::Tar, "b.tar"), (BundleKind::TarGz, ArchiveKind::TarGz, "b.tgz")];

#[test]
fn sniffs_archives_compressed_text_and_binary_files() {
    let dir = tempfile::tempdir().unwrap();
    for (bundle_kind, kind, name) in KINDS {
        assert_eq!(sniff(&bundle(dir.path(), name, bundle_kind)).unwrap(), Content::Archive(kind), "{name}");
    }
    let gz = dir.path().join("a.log.gz");
    write_gz(&gz, b"plain text\n").unwrap();
    assert_eq!(sniff(&gz).unwrap(), Content::Gzip);
    let cases: [(&str, &[u8], Content); 4] = [
        ("t.log", b"hello\n", Content::Text),
        ("u16.log", &[0xFF, 0xFE, b'h', 0, b'i', 0], Content::Text),
        ("bin.dat", &[0x7F, b'E', b'L', b'F', 0, 1], Content::Binary),
        ("empty.log", b"", Content::Text),
    ];
    for (name, bytes, expected) in cases {
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        assert_eq!(sniff(&p).unwrap(), expected, "{name}");
    }
    assert!(sniff(&dir.path().join("missing")).is_err());
}

#[test]
fn lists_regular_files_of_every_kind() {
    let dir = tempfile::tempdir().unwrap();
    for (bundle_kind, kind, name) in KINDS {
        let path = bundle(dir.path(), name, bundle_kind);
        let mut last = (0, 0);
        let entries = list(&path, kind, &CancelToken::new(), |d, t| last = (d, t)).unwrap();
        let got: Vec<(&str, u64)> = entries.iter().map(|e| (e.path.as_str(), e.size)).collect();
        assert_eq!(got, [("logs/api.log", 16), ("logs/worker.log", 7), ("README", 8)], "{name}");
        assert!(last.0 > 0 && last.0 == last.1, "{name}: progress reaches its total ({last:?})");
    }
}

#[test]
fn extracts_members_and_reports_missing_ones() {
    let dir = tempfile::tempdir().unwrap();
    for (bundle_kind, kind, name) in KINDS {
        let path = bundle(dir.path(), name, bundle_kind);
        let mut out = Vec::new();
        let n = extract(&path, kind, "logs/worker.log", &mut out, &CancelToken::new(), |_, _| {}).unwrap();
        assert_eq!((n, out.as_slice()), (7, &b"worker\n"[..]), "{name}");
        let err = extract(&path, kind, "nope.log", &mut Vec::new(), &CancelToken::new(), |_, _| {}).unwrap_err();
        assert!(matches!(&err, CoreError::Archive { message, .. } if message.contains("nope.log")), "{name}: {err}");
    }
}

#[test]
fn a_damaged_member_fails_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("d.zip");
    write_stored_zip(&path, &MEMBERS).unwrap();
    damage_zip_member(&path, 0).unwrap();
    assert_eq!(list(&path, ArchiveKind::Zip, &CancelToken::new(), |_, _| {}).unwrap().len(), 3, "the directory is intact");
    let err = extract(&path, ArchiveKind::Zip, "logs/api.log", &mut Vec::new(), &CancelToken::new(), |_, _| {}).unwrap_err();
    assert!(matches!(&err, CoreError::Archive { message, .. } if message.contains("logs/api.log")), "{err}");
    let mut out = Vec::new();
    extract(&path, ArchiveKind::Zip, "README", &mut out, &CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(out, b"read me\n");
}

#[test]
fn truncated_and_garbage_archives_are_errors_not_panics() {
    let dir = tempfile::tempdir().unwrap();
    let tgz = bundle(dir.path(), "t.tgz", BundleKind::TarGz);
    let bytes = std::fs::read(&tgz).unwrap();
    std::fs::write(&tgz, &bytes[..bytes.len() / 2]).unwrap();
    assert!(matches!(list(&tgz, ArchiveKind::TarGz, &CancelToken::new(), |_, _| {}), Err(CoreError::Archive { .. })));
    let junk = dir.path().join("junk.zip");
    std::fs::write(&junk, b"PK\x03\x04 not really a zip").unwrap();
    assert!(matches!(list(&junk, ArchiveKind::Zip, &CancelToken::new(), |_, _| {}), Err(CoreError::Archive { .. })));
    let cancelled = CancelToken::new();
    cancelled.cancel();
    let zip = bundle(dir.path(), "c.zip", BundleKind::Zip);
    assert!(matches!(list(&zip, ArchiveKind::Zip, &cancelled, |_, _| {}), Err(CoreError::Cancelled)));
}

#[test]
fn gunzips_multi_member_files_with_progress_and_cancel() {
    let dir = tempfile::tempdir().unwrap();
    let gz = dir.path().join("multi.log.gz");
    let mut bytes = gz_bytes(b"first\n");
    bytes.extend(gz_bytes(b"second\n"));
    std::fs::write(&gz, &bytes).unwrap();
    let mut out = Vec::new();
    let mut last = (0, 0);
    let n = gunzip(&gz, &mut out, &CancelToken::new(), |d, t| last = (d, t)).unwrap();
    assert_eq!((n, out.as_slice()), (13, &b"first\nsecond\n"[..]));
    assert_eq!(last, (bytes.len() as u64, bytes.len() as u64));
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(gunzip(&gz, &mut Vec::new(), &cancel, |_, _| {}), Err(CoreError::Cancelled)));
    std::fs::write(&gz, &bytes[..bytes.len() - 5]).unwrap();
    assert!(matches!(gunzip(&gz, &mut Vec::new(), &CancelToken::new(), |_, _| {}), Err(CoreError::Archive { .. })), "a truncated stream");
}

#[test]
fn archive_kinds_from_member_names() {
    assert_eq!(kind_from_name("inner.ZIP"), Some(ArchiveKind::Zip));
    assert_eq!(kind_from_name("x.tar"), Some(ArchiveKind::Tar));
    assert_eq!(kind_from_name("x.tar.gz"), Some(ArchiveKind::TarGz));
    assert_eq!(kind_from_name("x.tgz"), Some(ArchiveKind::TarGz));
    assert_eq!(kind_from_name("app.log.1.gz"), None, "a compressed log is not an archive");
    assert_eq!(kind_from_name("app.log"), None);
}
