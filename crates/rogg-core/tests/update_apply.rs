//! Verifying, staging and replacing for self-update (spec §4.1).

use rogg_core::update::apply::{
    Fault, ReplaceStyle, apply, apply_with_fault, can_write, copy_tree, move_into, stage, sweep, verify,
};
use rogg_core::update::{Install, UpdateError};
use rogg_genlog::bundle::{BundleKind, write_archive};
use std::fs;
use std::path::Path;

fn read(p: &Path) -> Vec<u8> {
    fs::read(p).unwrap()
}

fn write(p: &Path, bytes: &[u8]) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

fn binary(path: &Path) -> Install {
    Install::Binary { path: path.to_path_buf() }
}

#[test]
fn verify_accepts_the_right_digest_and_rejects_others() {
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("f");
    fs::write(&f, b"abc").unwrap();
    let good = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert!(verify(&f, good).is_ok());
    assert!(verify(&f, &good.to_uppercase()).is_ok());
    assert!(matches!(verify(&f, &"0".repeat(64)), Err(UpdateError::Checksum)));
}

#[test]
fn stages_each_archive_kind() {
    let tmp = tempfile::tempdir().unwrap();

    let a = tmp.path().join("a.tar.gz");
    write_archive(&a, BundleKind::TarGz, &[("rogg", b"bin"), ("README.md", b"hi")]).unwrap();
    let d = tmp.path().join("d1");
    let staged = stage(&a, &d, &binary(Path::new("/x/rogg"))).unwrap();
    assert_eq!(staged, d.join("rogg"));
    assert_eq!(read(&staged), b"bin");

    let z = tmp.path().join("w.zip");
    write_archive(&z, BundleKind::Zip, &[("rogg.exe", b"exe")]).unwrap();
    let d = tmp.path().join("d2");
    let staged = stage(&z, &d, &binary(Path::new("C:/x/rogg.exe"))).unwrap();
    assert_eq!(staged, d.join("rogg.exe"));
    assert_eq!(read(&staged), b"exe");

    let m = tmp.path().join("m.tar.gz");
    write_archive(
        &m,
        BundleKind::TarGz,
        &[("rogg.app/Contents/MacOS/rogg", b"mac"), ("rogg.app/Contents/Info.plist", b"plist")],
    )
    .unwrap();
    let d = tmp.path().join("d3");
    let install = Install::App { bundle: "/Applications/rogg.app".into() };
    let staged = stage(&m, &d, &install).unwrap();
    assert_eq!(staged, d.join("rogg.app"));
    assert_eq!(read(&staged.join("Contents/MacOS/rogg")), b"mac");
}

#[test]
fn staging_rejects_a_missing_item_and_escaping_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.tar.gz");
    write_archive(&a, BundleKind::TarGz, &[("README.md", b"hi")]).unwrap();
    let r = stage(&a, &tmp.path().join("d1"), &binary(Path::new("/x/rogg")));
    assert!(matches!(r, Err(UpdateError::MissingItem("rogg"))), "{r:?}");

    // `tar::Builder` refuses `..`, so the entry is written by hand.
    let mut header = tar::Header::new_gnu();
    header.as_gnu_mut().unwrap().name[..8].copy_from_slice(b"../evil\0");
    header.set_size(4);
    header.set_mode(0o644);
    header.set_cksum();
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
    builder.append(&header, &b"evil"[..]).unwrap();
    let bytes = builder.into_inner().unwrap().finish().unwrap();
    let evil = tmp.path().join("evil.tar.gz");
    fs::write(&evil, bytes).unwrap();

    let dir = tmp.path().join("stage");
    let r = stage(&evil, &dir, &binary(Path::new("/x/rogg")));
    assert!(matches!(r, Err(UpdateError::Unpack(_))), "{r:?}");
    assert!(!tmp.path().join("evil").exists());
    assert!(!dir.join("evil").exists());
}

fn leftovers(dir: &Path, stem: &str) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(stem) && n != stem)
        .collect()
}

#[test]
fn apply_replaces_a_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("bin/rogg");
    let staged = tmp.path().join("stage/rogg");
    write(&target, b"old");
    write(&staged, b"new");
    apply(&staged, &target, ReplaceStyle::Unix).unwrap();
    assert_eq!(read(&target), b"new");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o755);
    }
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
    assert!(!staged.exists());
}

#[test]
fn apply_replaces_an_app_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("apps/rogg.app");
    write(&target.join("Contents/MacOS/rogg"), b"old");
    write(&target.join("Contents/only-old.txt"), b"x");
    let staged = tmp.path().join("stage/rogg.app");
    write(&staged.join("Contents/MacOS/rogg"), b"new");
    write(&staged.join("Contents/Info.plist"), b"plist");
    apply(&staged, &target, ReplaceStyle::Unix).unwrap();
    assert_eq!(read(&target.join("Contents/MacOS/rogg")), b"new");
    assert_eq!(read(&target.join("Contents/Info.plist")), b"plist");
    assert!(!target.join("Contents/only-old.txt").exists());
    assert!(leftovers(&tmp.path().join("apps"), "rogg.app").is_empty());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(target.join("Contents/MacOS/rogg")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}

#[test]
fn apply_refuses_a_target_that_is_not_rogg() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("bin/bash");
    let staged = tmp.path().join("stage/rogg");
    write(&target, b"old");
    write(&staged, b"new");
    let r = apply(&staged, &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::NotRogg(_))), "{r:?}");
    assert_eq!(read(&target), b"old");
    assert!(staged.exists());
}

#[test]
fn apply_rolls_back_when_the_swap_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("bin/rogg");
    let staged = tmp.path().join("stage/rogg");
    write(&target, b"old");
    write(&staged, b"new");
    let r = apply_with_fault(&staged, &target, ReplaceStyle::Unix, Fault::SwapIn);
    assert!(matches!(r, Err(UpdateError::Replace { rolled_back: true, .. })), "{r:?}");
    assert_eq!(read(&target), b"old");
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
}

#[test]
fn windows_style_keeps_the_old_exe_aside() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg.exe");
    let staged = tmp.path().join("stage/rogg.exe");
    write(&target, b"old");
    write(&staged, b"new");
    apply(&staged, &target, ReplaceStyle::Windows).unwrap();
    assert_eq!(read(&target), b"new");
    assert_eq!(read(&tmp.path().join("rogg.exe.old")), b"old");
    assert!(!tmp.path().join("rogg.exe.rogg-old").exists());
    assert!(!tmp.path().join("rogg.exe.rogg-new").exists());
}

#[test]
fn move_into_falls_back_to_copying() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let src = a.path().join("tree");
    write(&src.join("one.txt"), b"1");
    write(&src.join("sub/deep/two.txt"), b"22");

    let copy = a.path().join("copy");
    copy_tree(&src, &copy).unwrap();
    assert_eq!(read(&copy.join("one.txt")), b"1");
    assert_eq!(read(&copy.join("sub/deep/two.txt")), b"22");

    let dst = b.path().join("moved");
    move_into(&src, &dst).unwrap();
    assert_eq!(read(&dst.join("sub/deep/two.txt")), b"22");
    assert!(!src.exists());

    let file = a.path().join("f");
    fs::write(&file, b"x").unwrap();
    move_into(&file, &b.path().join("f2")).unwrap();
    assert_eq!(read(&b.path().join("f2")), b"x");
    assert!(!file.exists());
}

#[test]
fn sweep_removes_leftovers() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg.exe");
    write(&target, b"cur");
    write(&tmp.path().join("rogg.exe.old"), b"o");
    write(&tmp.path().join("rogg.exe.rogg-new"), b"n");
    write(&tmp.path().join("rogg.exe.rogg-old"), b"o");
    let update = tmp.path().join("cache/update/7");
    write(&update.join("rogg.exe"), b"staged");
    sweep(Some(&target), &update);
    assert!(target.exists());
    for gone in ["rogg.exe.old", "rogg.exe.rogg-new", "rogg.exe.rogg-old", "cache/update/7"] {
        assert!(!tmp.path().join(gone).exists(), "{gone}");
    }
}

#[cfg(unix)]
#[test]
fn can_write_reports_read_only_folders() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    assert!(can_write(tmp.path()));
    let ro = tmp.path().join("ro");
    fs::create_dir(&ro).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
    let writable = can_write(&ro);
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!writable);
}
