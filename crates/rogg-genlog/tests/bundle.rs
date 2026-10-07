use flate2::read::MultiGzDecoder;
use rogg_genlog::GenOptions;
use rogg_genlog::bundle::{BundleKind, damage_zip_member, generate_bundle, gz_bytes, write_archive, write_gz, write_stored_zip};
use std::io::Read;

const MEMBERS: [(&str, &[u8]); 2] = [("logs/api.log", b"api one\napi two\n"), ("README", b"read me\n")];

fn tar_members(reader: impl Read) -> Vec<(String, Vec<u8>)> {
    let mut archive = tar::Archive::new(reader);
    archive
        .entries()
        .unwrap()
        .map(|e| {
            let mut e = e.unwrap();
            let mut bytes = Vec::new();
            e.read_to_end(&mut bytes).unwrap();
            (e.path().unwrap().to_string_lossy().into_owned(), bytes)
        })
        .collect()
}

fn zip_members(path: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut e = zip.by_index(i).unwrap();
            let mut bytes = Vec::new();
            e.read_to_end(&mut bytes).unwrap();
            (e.name().to_string(), bytes)
        })
        .collect()
}

fn expected() -> Vec<(String, Vec<u8>)> {
    MEMBERS.iter().map(|(n, b)| (n.to_string(), b.to_vec())).collect()
}

#[test]
fn archives_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("b.zip");
    write_archive(&zip, BundleKind::Zip, &MEMBERS).unwrap();
    assert_eq!(zip_members(&zip), expected());
    let tar = dir.path().join("b.tar");
    write_archive(&tar, BundleKind::Tar, &MEMBERS).unwrap();
    assert_eq!(tar_members(std::fs::File::open(&tar).unwrap()), expected());
    let tgz = dir.path().join("b.tgz");
    write_archive(&tgz, BundleKind::TarGz, &MEMBERS).unwrap();
    assert_eq!(tar_members(MultiGzDecoder::new(std::fs::File::open(&tgz).unwrap())), expected());
    let gz = dir.path().join("a.log.gz");
    write_gz(&gz, b"x\ny\n").unwrap();
    let mut out = String::new();
    MultiGzDecoder::new(std::fs::File::open(&gz).unwrap()).read_to_string(&mut out).unwrap();
    assert_eq!(out, "x\ny\n");
    assert_eq!(std::fs::read(&gz).unwrap(), gz_bytes(b"x\ny\n"));
    assert_eq!(BundleKind::parse("tar.gz"), Some(BundleKind::TarGz));
    assert_eq!(BundleKind::parse("rar"), None);
}

#[test]
fn a_damaged_stored_member_fails_its_checksum_only() {
    let dir = tempfile::tempdir().unwrap();
    let zip = dir.path().join("d.zip");
    write_stored_zip(&zip, &MEMBERS).unwrap();
    damage_zip_member(&zip, 0).unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip).unwrap()).unwrap();
    let mut sink = Vec::new();
    assert!(archive.by_index(0).unwrap().read_to_end(&mut sink).is_err(), "checksum mismatch");
    sink.clear();
    archive.by_index(1).unwrap().read_to_end(&mut sink).unwrap();
    assert_eq!(sink, b"read me\n");
}

#[test]
fn bundles_hold_one_log_per_file() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("bundle.zip");
    let opts = GenOptions { size_bytes: 20_000, ..Default::default() };
    let stats = generate_bundle(&out, BundleKind::Zip, 3, &opts).unwrap();
    let members = zip_members(&out);
    assert_eq!(members.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["service-00.log", "service-01.log", "service-02.log"]);
    assert_eq!(members.iter().map(|(_, b)| b.len() as u64).sum::<u64>(), stats.bytes);
    assert_ne!(members[0].1, members[1].1, "each file has its own seed");
    let leftovers: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).filter(|n| n != "bundle.zip").collect();
    assert!(leftovers.is_empty(), "temp files removed: {leftovers:?}");
}
