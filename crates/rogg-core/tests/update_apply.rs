//! Verifying, staging and replacing for self-update (spec §4.1).

use rogg_core::update::apply::{
    ArchiveKind, FolderOwner, MAC_ADMIN_GID, MAX_ARCHIVE, Fault, ReplaceStyle, SWEEP_IN_USE, apply, apply_archive,
    apply_archive_capped, apply_with_fault, copy_tree, move_into, stage, stage_from, sweep, sweep_at, trusted_folder,
    verify, verify_reader,
};
use rogg_core::platform::Os;
use rogg_core::update::{Install, UpdateError};
use rogg_genlog::bundle::{BundleKind, write_archive};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

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
fn apply_restores_a_stranded_old_copy_instead_of_deleting_it() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("bin/rogg");
    let staged = tmp.path().join("stage/rogg");
    // A failed apply left the only copy at `.rogg-old` and no target.
    write(&tmp.path().join("bin/rogg.rogg-old"), b"precious");
    write(&staged, b"new");
    // The fault makes the swap fail again after the restore; the old bytes must survive either way.
    let r = apply_with_fault(&staged, &target, ReplaceStyle::Unix, Fault::SwapIn);
    assert!(matches!(r, Err(UpdateError::Replace { rolled_back: true, .. })), "{r:?}");
    assert_eq!(read(&target), b"precious");

    fs::remove_file(&target).unwrap();
    write(&tmp.path().join("bin/rogg.rogg-old"), b"precious");
    write(&staged, b"new");
    apply(&staged, &target, ReplaceStyle::Unix).unwrap();
    assert_eq!(read(&target), b"new");
}

#[test]
fn sweep_restores_a_stranded_old_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg");
    write(&tmp.path().join("rogg.rogg-old"), b"precious");
    write(&tmp.path().join("rogg.rogg-new"), b"n");
    sweep(Some(&target), &tmp.path().join("cache"));
    assert_eq!(read(&target), b"precious");
    assert!(leftovers(tmp.path(), "rogg").is_empty());
}

#[cfg(unix)]
#[test]
fn staging_rejects_a_symlinked_item_and_never_chmods_through_it() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let victim = tmp.path().join("victim");
    fs::write(&victim, b"v").unwrap();
    fs::set_permissions(&victim, fs::Permissions::from_mode(0o600)).unwrap();

    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_size(0);
    header.set_mode(0o777);
    header.set_link_name("victim").unwrap();
    header.set_cksum();
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
    builder.append_data(&mut header, "rogg", std::io::empty()).unwrap();
    let archive = tmp.path().join("link.tar.gz");
    fs::write(&archive, builder.into_inner().unwrap().finish().unwrap()).unwrap();

    let dir = tmp.path().join("stage");
    let r = stage(&archive, &dir, &binary(Path::new("/x/rogg")));
    assert!(matches!(r, Err(UpdateError::MissingItem(_))), "{r:?}");

    // apply on a hand-made link must fail too and leave the link target's mode alone.
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    let link = tmp.path().join("stage2-rogg");
    std::os::unix::fs::symlink(&victim, &link).unwrap();
    assert!(apply(&link, &target, ReplaceStyle::Unix).is_err());
    assert_eq!(fs::metadata(&victim).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
    assert_eq!(read(&target), b"old");
}

#[test]
fn any_app_bundle_with_the_rogg_binary_is_a_target() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("apps/rogg 2.app");
    write(&target.join("Contents/MacOS/rogg"), b"old");
    let staged = tmp.path().join("stage/rogg.app");
    write(&staged.join("Contents/MacOS/rogg"), b"new");
    apply(&staged, &target, ReplaceStyle::Unix).unwrap();
    assert_eq!(read(&target.join("Contents/MacOS/rogg")), b"new");

    let hollow = tmp.path().join("apps/rogg.app");
    write(&hollow.join("Contents/other"), b"x");
    let staged = tmp.path().join("stage2/rogg.app");
    write(&staged.join("Contents/MacOS/rogg"), b"new");
    let r = apply(&staged, &hollow, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::NotRogg(_))), "{r:?}");
    assert_eq!(read(&hollow.join("Contents/other")), b"x");
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

#[cfg(unix)]
fn mode(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
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
    match r {
        Err(UpdateError::Replace { rolled_back: true, old, .. }) => assert_eq!(old, target),
        other => panic!("{other:?}"),
    }
    assert_eq!(read(&target), b"old");
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
}

fn unique_olds(dir: &Path, stem: &str) -> Vec<String> {
    leftovers(dir, stem).into_iter().filter(|n| n.starts_with(&format!("{stem}.rogg-old-"))).collect()
}

/// A stale `.rogg-old` that can't be removed (root-owned, or on Windows an old exe still running) mustn't block every
/// later update: the old version goes aside under a name of its own instead.
#[test]
fn a_stuck_old_copy_does_not_block_the_update() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    let target = bin.join("rogg");
    let staged = tmp.path().join("stage/rogg");
    write(&target, b"cur");
    write(&bin.join("rogg.rogg-old"), b"stuck");
    write(&staged, b"new");
    apply_with_fault(&staged, &target, ReplaceStyle::Unix, Fault::StuckOld).unwrap();
    assert_eq!(read(&target), b"new");
    assert_eq!(read(&bin.join("rogg.rogg-old")), b"stuck");
    assert!(unique_olds(&bin, "rogg").is_empty(), "the Unix style deletes the old version");

    // The Windows style keeps the old exe as `.old`; when that is still running too, the unique name stays.
    let target = tmp.path().join("win/rogg.exe");
    let staged = tmp.path().join("stage/rogg.exe");
    write(&target, b"cur");
    write(&tmp.path().join("win/rogg.exe.rogg-old"), b"stuck");
    write(&staged, b"new");
    apply_with_fault(&staged, &target, ReplaceStyle::Windows, Fault::StuckOld).unwrap();
    assert_eq!(read(&target), b"new");
    assert_eq!(read(&tmp.path().join("win/rogg.exe.old")), b"cur");
}

/// The same without the fault hook: a `.rogg-old` folder holding a file in a read-only folder can't be removed.
#[cfg(unix)]
#[test]
fn a_really_undeletable_old_copy_does_not_block_the_update() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let apps = tmp.path().join("apps");
    let target = apps.join("rogg.app");
    write(&target.join("Contents/MacOS/rogg"), b"cur");
    let locked = apps.join("rogg.app.rogg-old/Contents/locked");
    write(&locked.join("f"), b"stuck");
    write(&locked.join("g"), b"stuck");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::remove_file(locked.join("g")).is_ok() {
        // Running as root: nothing is undeletable here.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let staged = tmp.path().join("stage/rogg.app");
    write(&staged.join("Contents/MacOS/rogg"), b"new");
    let applied = apply(&staged, &target, ReplaceStyle::Unix);
    let still_stuck = locked.join("f").exists();
    let unique = unique_olds(&apps, "rogg.app");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    applied.unwrap();
    assert_eq!(read(&target.join("Contents/MacOS/rogg")), b"new");
    assert!(still_stuck);
    assert!(unique.is_empty(), "{unique:?}");

    // Once it can be removed, sweep does.
    sweep_at(Some(&target), &tmp.path().join("cache"), SystemTime::now());
    assert!(leftovers(&apps, "rogg.app").is_empty());
}

#[test]
fn a_failed_swap_beside_a_stuck_old_copy_rolls_back_from_the_unique_name() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    let target = bin.join("rogg");
    write(&target, b"cur");
    write(&bin.join("rogg.rogg-old"), b"stuck");
    let staged = tmp.path().join("stage/rogg");
    write(&staged, b"new");
    let r = apply_with_fault(&staged, &target, ReplaceStyle::Unix, Fault::StuckOldAndSwapIn);
    match r {
        Err(UpdateError::Replace { rolled_back: true, old, .. }) => assert_eq!(old, target),
        other => panic!("{other:?}"),
    }
    assert_eq!(read(&target), b"cur");
    assert_eq!(read(&bin.join("rogg.rogg-old")), b"stuck");
    assert!(unique_olds(&bin, "rogg").is_empty());
    assert!(!bin.join("rogg.rogg-new").exists());
}

#[test]
fn sweep_removes_unique_old_copies_once_no_update_is_in_progress() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg.exe");
    write(&target, b"cur");
    write(&tmp.path().join("rogg.exe.rogg-old-4242-0"), b"o");
    write(&tmp.path().join("rogg.exe.rogg-old-4242-1"), b"o");
    write(&tmp.path().join("other.exe.rogg-old-1-0"), b"not ours");
    let work = tmp.path().join("cache/update");
    write(&work.join("14/a.zip.part"), b"fresh");
    sweep(Some(&target), &work);
    assert_eq!(unique_olds(tmp.path(), "rogg.exe").len(), 2, "left alone while an update may be swapping");

    sweep_at(Some(&target), &work, SystemTime::now() + SWEEP_IN_USE + Duration::from_secs(1));
    assert!(unique_olds(tmp.path(), "rogg.exe").is_empty());
    assert_eq!(read(&target), b"cur");
    assert_eq!(read(&tmp.path().join("other.exe.rogg-old-1-0")), b"not ours");
}

#[test]
fn a_stranded_old_copy_under_a_unique_name_is_restored() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg");
    write(&tmp.path().join("rogg.rogg-old-77-0"), b"precious");
    sweep(Some(&target), &tmp.path().join("cache"));
    assert_eq!(read(&target), b"precious");
    assert!(leftovers(tmp.path(), "rogg").is_empty());

    // apply restores it first too, then replaces it.
    fs::rename(&target, tmp.path().join("rogg.rogg-old-77-0")).unwrap();
    let staged = tmp.path().join("stage/rogg");
    write(&staged, b"new");
    apply(&staged, &target, ReplaceStyle::Unix).unwrap();
    assert_eq!(read(&target), b"new");
    assert!(leftovers(tmp.path(), "rogg").is_empty());
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

/// Backdates every entry under `path` (and `path` itself) by `age`. Unix only: Windows opens a directory only with
/// `FILE_FLAG_BACKUP_SEMANTICS`, and setting its time needs write-attribute access.
#[cfg(unix)]
fn age_tree(path: &Path, age: Duration) {
    let when = SystemTime::now() - age;
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            age_tree(&entry.unwrap().path(), age);
        }
    }
    fs::File::options().read(true).open(path).unwrap().set_modified(when).unwrap();
}

#[test]
fn sweep_removes_leftovers() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg.exe");
    write(&target, b"cur");
    write(&tmp.path().join("rogg.exe.old"), b"o");
    write(&tmp.path().join("rogg.exe.rogg-new"), b"n");
    write(&tmp.path().join("rogg.exe.rogg-old"), b"o");
    let work = tmp.path().join("cache/update");
    write(&work.join("7/rogg.exe"), b"staged");
    // Long after the update: nothing is in use any more.
    sweep_at(Some(&target), &work, SystemTime::now() + SWEEP_IN_USE + Duration::from_secs(1));
    assert!(target.exists());
    for gone in ["rogg.exe.old", "rogg.exe.rogg-new", "rogg.exe.rogg-old", "cache/update"] {
        assert!(!tmp.path().join(gone).exists(), "{gone}");
    }
}

/// R12: another rogg process may be downloading or installing right now; its fresh build folder marks that.
#[test]
fn sweep_leaves_an_update_in_progress_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg");
    write(&tmp.path().join("rogg.rogg-new"), b"n");
    write(&tmp.path().join("rogg.rogg-old"), b"o");
    let work = tmp.path().join("cache/update");
    write(&work.join("14/rogg-linux-x86_64.tar.gz.part"), b"half");
    sweep(Some(&target), &work);
    assert_eq!(read(&work.join("14/rogg-linux-x86_64.tar.gz.part")), b"half");
    assert_eq!(read(&tmp.path().join("rogg.rogg-new")), b"n");
    assert_eq!(read(&tmp.path().join("rogg.rogg-old")), b"o", "no restore while another update may be swapping");
    assert!(!target.exists());
}

#[cfg(unix)]
#[test]
fn sweep_removes_a_stale_build_folder_and_keeps_a_fresh_one() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg");
    write(&target, b"cur");
    write(&tmp.path().join("rogg.rogg-old"), b"o");
    let work = tmp.path().join("cache/update");
    write(&work.join("13/staged/rogg"), b"old staging");
    age_tree(&work.join("13"), SWEEP_IN_USE + Duration::from_secs(60));
    write(&work.join("14/a.tar.gz.part"), b"fresh");
    // Only the staged binary is old: a recently written file anywhere in the folder keeps it.
    write(&work.join("15/staged/rogg"), b"x");
    age_tree(&work.join("15"), SWEEP_IN_USE + Duration::from_secs(60));
    write(&work.join("15/staged/new"), b"y");
    sweep(Some(&target), &work);
    assert!(!work.join("13").exists(), "stale");
    assert!(work.join("14/a.tar.gz.part").exists() && work.join("15/staged/rogg").exists());
    assert!(tmp.path().join("rogg.rogg-old").exists(), "the siblings wait while a folder is in use");

    age_tree(&work, SWEEP_IN_USE + Duration::from_secs(60));
    sweep(Some(&target), &work);
    assert!(!work.exists());
    assert!(!tmp.path().join("rogg.rogg-old").exists());
    assert_eq!(read(&target), b"cur");
}

#[cfg(unix)]
#[test]
fn can_write_reports_read_only_folders() {
    use rogg_core::update::apply::can_write;
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

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// `../evil` as a hand-written tar.gz entry (`tar::Builder` refuses `..`).
fn escaping_tar_gz() -> Vec<u8> {
    let mut header = tar::Header::new_gnu();
    header.as_gnu_mut().unwrap().name[..8].copy_from_slice(b"../evil\0");
    header.set_size(4);
    header.set_mode(0o644);
    header.set_cksum();
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
    builder.append(&header, &b"evil"[..]).unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

fn archive_bytes(kind: BundleKind, members: &[(&str, &[u8])]) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a");
    write_archive(&path, kind, members).unwrap();
    read(&path)
}

#[test]
fn verify_reader_checks_what_it_reads() {
    let good = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert!(verify_reader(&b"abc"[..], good).is_ok());
    assert!(verify_reader(&b"abc"[..], &good.to_uppercase()).is_ok());
    assert!(matches!(verify_reader(&b"abd"[..], good), Err(UpdateError::Checksum)));
    assert!(matches!(verify_reader(&b""[..], good), Err(UpdateError::Checksum)));
}

#[test]
fn the_archive_kind_comes_from_the_name() {
    assert_eq!(ArchiveKind::of(Path::new("/c/rogg-linux-x86_64.tar.gz")), Some(ArchiveKind::TarGz));
    assert_eq!(ArchiveKind::of(Path::new("/c/rogg-macos-arm64.TAR.GZ")), Some(ArchiveKind::TarGz));
    assert_eq!(ArchiveKind::of(Path::new("C:/c/rogg-windows-x86_64.zip")), Some(ArchiveKind::Zip));
    assert_eq!(ArchiveKind::of(Path::new("/c/rogg.gz")), None);
    assert_eq!(ArchiveKind::of(Path::new("/c/rogg.tar")), None);
    assert_eq!(ArchiveKind::of(Path::new("/c/rogg")), None);
}

#[test]
fn stage_from_unpacks_from_a_reader() {
    let tmp = tempfile::tempdir().unwrap();
    let tgz = archive_bytes(BundleKind::TarGz, &[("rogg", b"bin")]);
    let d = tmp.path().join("d1");
    let staged = stage_from(std::io::Cursor::new(tgz), ArchiveKind::TarGz, &d, &binary(Path::new("/x/rogg"))).unwrap();
    assert_eq!(staged, d.join("rogg"));
    assert_eq!(read(&staged), b"bin");

    let zip = archive_bytes(BundleKind::Zip, &[("rogg.exe", b"exe")]);
    let d = tmp.path().join("d2");
    let staged = stage_from(std::io::Cursor::new(zip), ArchiveKind::Zip, &d, &binary(Path::new("C:/x/rogg.exe"))).unwrap();
    assert_eq!(read(&staged), b"exe");

    let d = tmp.path().join("d3");
    let r = stage_from(std::io::Cursor::new(escaping_tar_gz()), ArchiveKind::TarGz, &d, &binary(Path::new("/x/rogg")));
    assert!(matches!(r, Err(UpdateError::Unpack(_))), "{r:?}");
    assert!(!tmp.path().join("evil").exists());
}

/// No set-id bits and no group or other write survive an unpack, from either archive kind.
#[cfg(unix)]
#[test]
fn staging_masks_modes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
    for (name, mode) in [("rogg.app/Contents/MacOS/rogg", 0o4777), ("rogg.app/Contents/MacOS/helper", 0o2775)] {
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        header.set_mode(mode);
        builder.append_data(&mut header, name, &b"x"[..]).unwrap();
    }
    let tgz = builder.into_inner().unwrap().finish().unwrap();
    let app = Install::App { bundle: "/Applications/rogg.app".into() };
    let staged = stage_from(std::io::Cursor::new(tgz), ArchiveKind::TarGz, &tmp.path().join("t"), &app).unwrap();
    assert_eq!(mode(&staged.join("Contents/MacOS/rogg")), 0o755);
    assert_eq!(mode(&staged.join("Contents/MacOS/helper")), 0o755);

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().unix_permissions(0o4777);
    zip.start_file("rogg", options).unwrap();
    std::io::Write::write_all(&mut zip, b"x").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let dir = tmp.path().join("z");
    let staged = stage_from(std::io::Cursor::new(bytes), ArchiveKind::Zip, &dir, &binary(Path::new("/x/rogg"))).unwrap();
    assert_eq!(mode(&staged), 0o755);
}

fn stage_dirs(dir: &Path, stem: &str) -> Vec<String> {
    leftovers(dir, stem).into_iter().filter(|n| n.starts_with(&format!("{stem}.rogg-stage-"))).collect()
}

/// The elevated helper's whole job: the archive is checked and unpacked by the helper itself, beside the target.
#[test]
fn apply_archive_installs_a_verified_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("cache/rogg-linux-x86_64.tar.gz");
    fs::create_dir_all(archive.parent().unwrap()).unwrap();
    write_archive(&archive, BundleKind::TarGz, &[("rogg", b"new"), ("README.md", b"hi")]).unwrap();
    let sha = sha256_hex(&read(&archive));
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    apply_archive(&archive, &sha, &target, ReplaceStyle::Unix).unwrap();
    assert_eq!(read(&target), b"new");
    #[cfg(unix)]
    assert_eq!(mode(&target), 0o755);
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
    // The download is only read.
    assert_eq!(sha256_hex(&read(&archive)), sha);
}

#[test]
fn apply_archive_installs_an_app_bundle_and_a_zip() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("rogg-macos-arm64.tar.gz");
    let members: &[(&str, &[u8])] = &[("rogg.app/Contents/MacOS/rogg", b"mac"), ("rogg.app/Contents/Info.plist", b"p")];
    write_archive(&archive, BundleKind::TarGz, members).unwrap();
    let target = tmp.path().join("apps/rogg 2.app");
    write(&target.join("Contents/MacOS/rogg"), b"old");
    write(&target.join("Contents/only-old.txt"), b"x");
    apply_archive(&archive, &sha256_hex(&read(&archive)), &target, ReplaceStyle::Unix).unwrap();
    assert_eq!(read(&target.join("Contents/MacOS/rogg")), b"mac");
    assert_eq!(read(&target.join("Contents/Info.plist")), b"p");
    assert!(!target.join("Contents/only-old.txt").exists());
    assert!(leftovers(&tmp.path().join("apps"), "rogg 2.app").is_empty());

    let archive = tmp.path().join("rogg-windows-x86_64.zip");
    write_archive(&archive, BundleKind::Zip, &[("rogg.exe", b"exe")]).unwrap();
    let target = tmp.path().join("win/rogg.exe");
    write(&target, b"old");
    apply_archive(&archive, &sha256_hex(&read(&archive)), &target, ReplaceStyle::Windows).unwrap();
    assert_eq!(read(&target), b"exe");
    assert_eq!(read(&tmp.path().join("win/rogg.exe.old")), b"old");
    assert!(stage_dirs(&tmp.path().join("win"), "rogg.exe").is_empty());
}

#[test]
fn apply_archive_refuses_a_wrong_checksum() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("rogg-linux-x86_64.tar.gz");
    write_archive(&archive, BundleKind::TarGz, &[("rogg", b"new")]).unwrap();
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    let r = apply_archive(&archive, &"0".repeat(64), &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::Checksum)), "{r:?}");
    assert_eq!(read(&target), b"old");
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
}

#[test]
fn apply_archive_rejects_escaping_entries_and_unknown_kinds() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    let evil = escaping_tar_gz();
    let archive = tmp.path().join("cache/rogg-linux-x86_64.tar.gz");
    write(&archive, &evil);
    let r = apply_archive(&archive, &sha256_hex(&evil), &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::Unpack(_))), "{r:?}");
    assert!(!tmp.path().join("evil").exists() && !tmp.path().join("bin/evil").exists());

    let odd = tmp.path().join("cache/rogg.bin");
    write(&odd, b"x");
    let r = apply_archive(&odd, &sha256_hex(b"x"), &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::Unpack(_))), "{r:?}");
    assert_eq!(read(&target), b"old");
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
}

#[test]
fn apply_archive_refuses_a_target_that_is_not_rogg() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("rogg-linux-x86_64.tar.gz");
    write_archive(&archive, BundleKind::TarGz, &[("rogg", b"new")]).unwrap();
    let target = tmp.path().join("bin/bash");
    write(&target, b"old");
    let r = apply_archive(&archive, &sha256_hex(&read(&archive)), &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::NotRogg(_))), "{r:?}");
    assert_eq!(read(&target), b"old");
    assert!(leftovers(&tmp.path().join("bin"), "bash").is_empty());
}

/// The helper's stage folder is its own: one already there (squatted, or another helper's) is never used or removed.
#[test]
fn apply_archive_never_reuses_an_existing_stage_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("rogg-linux-x86_64.tar.gz");
    write_archive(&archive, BundleKind::TarGz, &[("rogg", b"new")]).unwrap();
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    let squatted = tmp.path().join(format!("bin/rogg.rogg-stage-{}", std::process::id()));
    write(&squatted.join("staged/rogg"), b"planted");
    let r = apply_archive(&archive, &sha256_hex(&read(&archive)), &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::Io { .. })), "{r:?}");
    assert_eq!(read(&target), b"old");
    assert_eq!(read(&squatted.join("staged/rogg")), b"planted");
}

/// A link in place of the download is refused rather than followed (it could point at a file only root can read).
#[cfg(unix)]
#[test]
fn apply_archive_refuses_a_linked_archive() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real.tar.gz");
    write_archive(&real, BundleKind::TarGz, &[("rogg", b"new")]).unwrap();
    let link = tmp.path().join("rogg-linux-x86_64.tar.gz");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    let r = apply_archive(&link, &sha256_hex(&read(&real)), &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::Io { .. })), "{r:?}");
    assert_eq!(read(&target), b"old");
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
}

/// A FIFO (or a device) in place of the download fails at once instead of blocking the helper.
#[cfg(unix)]
#[test]
fn apply_archive_refuses_a_fifo_without_blocking() {
    use std::os::unix::ffi::OsStrExt;
    let tmp = tempfile::tempdir().unwrap();
    let fifo = tmp.path().join("rogg-linux-x86_64.tar.gz");
    let c = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    // SAFETY: `c` is a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o644) }, 0);
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    let r = apply_archive(&fifo, &"0".repeat(64), &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::Io { .. })), "{r:?}");
    assert_eq!(read(&target), b"old");
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
}

/// The helper's stage folder sits beside the target, outside the work dir: a fresh one marks an update in progress
/// by itself.
#[cfg(unix)]
#[test]
fn sweep_keeps_a_fresh_stage_folder_and_removes_a_stale_one() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg");
    write(&target, b"cur");
    write(&tmp.path().join("rogg.rogg-old"), b"o");
    let fresh = tmp.path().join("rogg.rogg-stage-4242");
    write(&fresh.join("staged/rogg"), b"x");
    write(&tmp.path().join("other.rogg-stage-1/f"), b"not ours");
    let work = tmp.path().join("cache/update");
    sweep(Some(&target), &work);
    assert!(fresh.join("staged/rogg").exists(), "a helper may be installing from it");
    assert!(tmp.path().join("rogg.rogg-old").exists(), "the siblings wait while a helper may be swapping");

    age_tree(&fresh, SWEEP_IN_USE + Duration::from_secs(60));
    sweep(Some(&target), &work);
    assert!(stage_dirs(tmp.path(), "rogg").is_empty());
    assert!(!tmp.path().join("rogg.rogg-old").exists());
    assert_eq!(read(&target), b"cur");
    assert!(tmp.path().join("other.rogg-stage-1/f").exists());
}

#[test]
fn sweep_removes_stale_stage_folders() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("rogg.exe");
    write(&target, b"cur");
    write(&tmp.path().join("rogg.exe.rogg-stage-7/staged/rogg.exe"), b"x");
    sweep_at(Some(&target), &tmp.path().join("cache"), SystemTime::now() + SWEEP_IN_USE + Duration::from_secs(1));
    assert!(stage_dirs(tmp.path(), "rogg.exe").is_empty());
    assert_eq!(read(&target), b"cur");
}

#[test]
fn the_archive_cap_is_512_mib() {
    assert_eq!(MAX_ARCHIVE, 512 * 1024 * 1024);
}

/// The helper holds the whole download in memory, so a bigger one is refused rather than read.
#[test]
fn apply_archive_refuses_an_archive_over_the_cap() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("rogg-linux-x86_64.tar.gz");
    write_archive(&archive, BundleKind::TarGz, &[("rogg", b"new")]).unwrap();
    let size = fs::metadata(&archive).unwrap().len();
    let sha = sha256_hex(&read(&archive));
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    let r = apply_archive_capped(&archive, &sha, &target, ReplaceStyle::Unix, size - 1);
    assert!(matches!(r, Err(UpdateError::TooLarge { limit }) if limit == size - 1), "{r:?}");
    assert!(r.unwrap_err().to_string().contains(&format!("{}", size - 1)));
    assert_eq!(read(&target), b"old");
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
    // Exactly at the cap is fine.
    apply_archive_capped(&archive, &sha, &target, ReplaceStyle::Unix, size).unwrap();
    assert_eq!(read(&target), b"new");
}

/// Nothing the helper opened reaches the disk before its checksum is known: with a wrong checksum the stage folder
/// isn't even created (the squatted one would otherwise fail the run first, with an I/O error).
#[test]
fn apply_archive_checks_the_checksum_before_writing_anything() {
    let tmp = tempfile::tempdir().unwrap();
    let archive = tmp.path().join("rogg-linux-x86_64.tar.gz");
    write_archive(&archive, BundleKind::TarGz, &[("rogg", b"new")]).unwrap();
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    let squatted = tmp.path().join(format!("bin/rogg.rogg-stage-{}", std::process::id()));
    write(&squatted.join("staged/rogg"), b"planted");
    let r = apply_archive(&archive, &"0".repeat(64), &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::Checksum)), "{r:?}");
    let mut names: Vec<_> = fs::read_dir(&squatted).unwrap().map(|e| e.unwrap().file_name()).collect();
    names.sort();
    assert_eq!(names, ["staged"], "no copy of the archive");
    fs::remove_dir_all(&squatted).unwrap();

    // A good run leaves nothing either, and never wrote the archive anywhere beside the target.
    apply_archive(&archive, &sha256_hex(&read(&archive)), &target, ReplaceStyle::Unix).unwrap();
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
}

/// Every regular file under `dir` whose bytes are `bytes`.
#[cfg(unix)]
fn files_holding(dir: &Path, bytes: &[u8]) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let meta = fs::symlink_metadata(&path).unwrap();
        if meta.is_dir() {
            found.extend(files_holding(&path, bytes));
        } else if meta.is_file() && fs::read(&path).unwrap() == bytes {
            found.push(path);
        }
    }
    found
}

/// `O_NOFOLLOW` doesn't stop a hard link: the user can link a file only root reads under the archive's name. Its
/// bytes fail the checksum and are never written anywhere a user could read them.
#[cfg(unix)]
#[test]
fn a_hard_linked_secret_is_refused_and_never_copied() {
    let tmp = tempfile::tempdir().unwrap();
    let secret = tmp.path().join("secret/shadow");
    write(&secret, b"root-only secret");
    let release = tmp.path().join("release.tar.gz");
    write_archive(&release, BundleKind::TarGz, &[("rogg", b"new")]).unwrap();
    let archive = tmp.path().join("cache/rogg-linux-x86_64.tar.gz");
    fs::create_dir_all(archive.parent().unwrap()).unwrap();
    fs::hard_link(&secret, &archive).unwrap();
    let target = tmp.path().join("bin/rogg");
    write(&target, b"old");
    let r = apply_archive(&archive, &sha256_hex(&read(&release)), &target, ReplaceStyle::Unix);
    assert!(matches!(r, Err(UpdateError::Checksum)), "{r:?}");
    assert_eq!(read(&target), b"old");
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
    let mut holding = files_holding(tmp.path(), b"root-only secret");
    holding.sort();
    assert_eq!(holding, [archive, secret]);
}

const ROOT: u32 = 0;
const USER: u32 = 501;

fn folder(uid: u32, gid: u32, mode: u32) -> FolderOwner {
    FolderOwner { uid, gid, mode: 0o040000 | mode }
}

/// R17: the elevated helper only installs where every folder up to `/` is root's and only root can change it, or a
/// user could race its chmod of the new binary onto any file.
#[test]
fn only_root_controlled_folders_are_trusted() {
    for os in [Os::Mac, Os::Linux] {
        assert!(trusted_folder(folder(ROOT, 0, 0o755), ROOT, os), "root 0755");
        assert!(trusted_folder(folder(ROOT, 0, 0o555), ROOT, os));
        assert!(!trusted_folder(folder(USER, 20, 0o755), ROOT, os), "a user's folder");
        assert!(!trusted_folder(folder(ROOT, 0, 0o777), ROOT, os), "world-writable");
        assert!(!trusted_folder(folder(ROOT, 0, 0o1777), ROOT, os), "sticky doesn't help");
        assert!(!trusted_folder(folder(ROOT, 0, 0o757), ROOT, os), "other-writable");
        assert!(!trusted_folder(folder(ROOT, 0, 0o775), ROOT, os), "group-writable by wheel/root");
        assert!(!trusted_folder(folder(ROOT, MAC_ADMIN_GID, 0o777), ROOT, os), "admin group, but world-writable");
        let link = FolderOwner { uid: ROOT, gid: 0, mode: 0o120755 };
        assert!(!trusted_folder(link, ROOT, os), "a link, not a folder");
    }
    assert_eq!(MAC_ADMIN_GID, 80);
    // `/Applications` is root:admin 0775.
    assert!(trusted_folder(folder(ROOT, MAC_ADMIN_GID, 0o775), ROOT, Os::Mac));
    assert!(!trusted_folder(folder(ROOT, MAC_ADMIN_GID, 0o775), ROOT, Os::Linux), "gid 80 is nobody special on Linux");
    // Run without admin rights (the tests), the helper can only do what its user can anyway: its own folders count.
    assert!(trusted_folder(folder(USER, 20, 0o755), USER, Os::Mac));
    assert!(!trusted_folder(folder(USER, 20, 0o775), USER, Os::Mac));
    assert!(!trusted_folder(folder(USER + 1, 20, 0o755), USER, Os::Linux));
}

#[cfg(unix)]
#[test]
fn trusted_location_checks_every_folder_up_to_the_root() {
    use rogg_core::update::apply::trusted_location;
    use std::os::unix::fs::PermissionsExt;
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).canonicalize().unwrap();
    let tmp = tempfile::tempdir_in(&base).unwrap();
    let bin = tmp.path().join("a/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::set_permissions(tmp.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(trusted_location(&bin.join("rogg")), Ok(()));

    let a = tmp.path().join("a");
    fs::set_permissions(&a, fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(trusted_location(&bin.join("rogg")), Err(a.clone()), "an ancestor, not just the parent");
    fs::set_permissions(&a, fs::Permissions::from_mode(0o755)).unwrap();

    // A link on the way could be re-pointed; the updater passes canonical paths.
    std::os::unix::fs::symlink(&bin, tmp.path().join("link")).unwrap();
    assert_eq!(trusted_location(&tmp.path().join("link/rogg")), Err(tmp.path().join("link")));
    assert!(trusted_location(Path::new("rogg")).is_err(), "relative");
}
