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
fn skipping_to_a_tar_member_reports_archive_progress_first() {
    let dir = tempfile::tempdir().unwrap();
    // Incompressible members: the archive is read in many steps before the last member is reached.
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut noise = |len: usize| -> Vec<u8> {
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 24) as u8
            })
            .collect()
    };
    let members = [("a.log", noise(300_000)), ("b.log", noise(300_000)), ("c.log", noise(300_000)), ("last.log", noise(100_000))];
    let refs: Vec<(&str, &[u8])> = members.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    for (bundle_kind, kind, name) in [(BundleKind::Tar, ArchiveKind::Tar, "n.tar"), (BundleKind::TarGz, ArchiveKind::TarGz, "n.tgz")] {
        let path = dir.path().join(name);
        write_archive(&path, bundle_kind, &refs).unwrap();
        let archive_size = std::fs::metadata(&path).unwrap().len();
        let mut calls = Vec::new();
        let mut out = Vec::new();
        extract(&path, kind, "last.log", &mut out, &CancelToken::new(), |done, total| calls.push((done, total))).unwrap();
        assert_eq!(out, members[3].1, "{name}");
        // Progress against the archive size comes first; the member's own bytes (total = its size) follow.
        let skipping: Vec<u64> = calls.iter().take_while(|&&(_, total)| total == archive_size).map(|&(done, _)| done).collect();
        assert!(skipping.len() > 1, "{name}: archive progress while skipping ({calls:?})");
        assert!(skipping.windows(2).all(|w| w[0] <= w[1]) && skipping[0] < archive_size, "{name}: {skipping:?}");
        assert!(calls[skipping.len()..].iter().all(|&(_, total)| total == 100_000), "{name}: then the member's bytes");
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

#[test]
fn tar_listing_reports_progress_as_it_reads() {
    // Deterministic, poorly compressible members (xorshift bytes) so the archive is much larger than any read buffer.
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut noise = |len: usize| -> Vec<u8> {
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 24) as u8
            })
            .collect()
    };
    let bodies: Vec<Vec<u8>> = (0..4).map(|_| noise(1 << 20)).collect();
    let names = ["m0.bin", "m1.bin", "m2.bin", "m3.bin"];
    let members: Vec<(&str, &[u8])> = names.iter().copied().zip(bodies.iter().map(Vec::as_slice)).collect();
    let dir = tempfile::tempdir().unwrap();
    for (bundle_kind, kind, name) in [(BundleKind::TarGz, ArchiveKind::TarGz, "big.tgz"), (BundleKind::Tar, ArchiveKind::Tar, "big.tar")] {
        let path = dir.path().join(name);
        write_archive(&path, bundle_kind, &members).unwrap();
        let mut calls: Vec<(u64, u64)> = Vec::new();
        let entries = list(&path, kind, &CancelToken::new(), |d, t| calls.push((d, t))).unwrap();
        assert_eq!(entries.len(), 4, "{name}");
        assert!(calls.iter().any(|&(d, t)| d < t), "{name}: some call reports partial progress ({calls:?})");
        assert!(calls.windows(2).all(|w| w[0].0 <= w[1].0), "{name}: progress never decreases ({calls:?})");
        assert_eq!(calls.last().map(|&(d, t)| d == t), Some(true), "{name}: ends complete");
    }
}

#[test]
fn trailing_bytes_after_the_last_gzip_member_are_ignored() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let gz = dir.path().join("t.log.gz");
    let member = |text: &str| {
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(text.as_bytes()).unwrap();
        enc.finish().unwrap()
    };
    let mut bytes = member("one\n");
    bytes.extend(member("two\n"));
    bytes.extend(b"\0\0\0garbage after the stream");
    std::fs::write(&gz, &bytes).unwrap();
    let mut out = Vec::new();
    rogg_core::archive::gunzip(&gz, &mut out, &rogg_core::CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(out, b"one\ntwo\n", "both members, then the trailing bytes are ignored like gzip -d does");
    let mut corrupt = member("x\n");
    let mid = corrupt.len() / 2;
    corrupt[mid] ^= 0xff;
    std::fs::write(&gz, &corrupt).unwrap();
    assert!(rogg_core::archive::gunzip(&gz, &mut Vec::new(), &rogg_core::CancelToken::new(), |_, _| {}).is_err(), "a damaged member still fails");
}

fn gz_member(text: &[u8], level: flate2::Compression) -> Vec<u8> {
    use std::io::Write;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), level);
    enc.write_all(text).unwrap();
    enc.finish().unwrap()
}

fn gunzip_bytes(bytes: &[u8]) -> Result<Vec<u8>, rogg_core::CoreError> {
    let dir = tempfile::tempdir().unwrap();
    let gz = dir.path().join("t.log.gz");
    std::fs::write(&gz, bytes).unwrap();
    let mut out = Vec::new();
    rogg_core::archive::gunzip(&gz, &mut out, &rogg_core::CancelToken::new(), |_, _| {})?;
    Ok(out)
}

#[test]
fn a_member_boundary_one_byte_before_the_buffer_edge_is_read() {
    // gunzip buffers 1 MiB: stored members sized so the next member's first byte is the last one in the first buffer.
    const COPY_CHUNK: usize = 1 << 20;
    let none = flate2::Compression::none();
    let big_text = vec![b'a'; 1_000_000];
    let big = gz_member(&big_text, none);
    // The small member's payload length that makes the two members exactly COPY_CHUNK - 1 bytes (its overhead is a few
    // dozen bytes of header, block headers and trailer).
    let guess = COPY_CHUNK - 1 - big.len();
    let small_len = (guess.saturating_sub(100)..guess).find(|&n| big.len() + gz_member(&vec![b'b'; n], none).len() == COPY_CHUNK - 1).expect("a small member that fits exactly");
    let small_text = vec![b'b'; small_len];
    let mut bytes = big;
    bytes.extend(gz_member(&small_text, none));
    assert_eq!(bytes.len(), COPY_CHUNK - 1);
    bytes.extend(gz_member(b"third\n", flate2::Compression::fast()));
    let mut expected = big_text;
    expected.extend(&small_text);
    expected.extend(b"third\n");
    let got = gunzip_bytes(&bytes).unwrap();
    assert!(got == expected, "the member after the buffer edge is not dropped: got {} bytes, expected {}", got.len(), expected.len());
}

#[test]
fn a_lone_magic_byte_after_the_last_member_is_ignored() {
    let mut bytes = gz_member(b"one\n", flate2::Compression::fast());
    bytes.push(0x1f);
    assert_eq!(gunzip_bytes(&bytes).unwrap(), b"one\n");
}

#[test]
fn an_empty_gz_file_is_an_error() {
    assert!(gunzip_bytes(b"").is_err(), "gzip -d fails an empty file too");
}
