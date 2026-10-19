//! Verifying, staging and replacing for self-update (spec §4.1).

use rogg_core::update::apply::{
    Fault, ReplaceStyle, SWEEP_IN_USE, Transfer, apply, apply_with_fault, copy_tree, move_into, stage, sweep, sweep_at, verify,
};
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
    let r = apply_with_fault(&staged, &target, ReplaceStyle::Unix, Transfer::Move, Fault::SwapIn);
    assert!(matches!(r, Err(UpdateError::Replace { rolled_back: true, .. })), "{r:?}");
    assert_eq!(read(&target), b"precious");

    fs::remove_file(&target).unwrap();
    write(&tmp.path().join("bin/rogg.rogg-old"), b"precious");
    write(&staged, b"new");
    apply(&staged, &target, ReplaceStyle::Unix, Transfer::Move).unwrap();
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
    assert!(apply(&link, &target, ReplaceStyle::Unix, Transfer::Move).is_err());
    assert_eq!(fs::metadata(&victim).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(apply(&link, &target, ReplaceStyle::Unix, Transfer::Copy).is_err());
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
    apply(&staged, &target, ReplaceStyle::Unix, Transfer::Move).unwrap();
    assert_eq!(read(&target.join("Contents/MacOS/rogg")), b"new");

    let hollow = tmp.path().join("apps/rogg.app");
    write(&hollow.join("Contents/other"), b"x");
    let staged = tmp.path().join("stage2/rogg.app");
    write(&staged.join("Contents/MacOS/rogg"), b"new");
    let r = apply(&staged, &hollow, ReplaceStyle::Unix, Transfer::Move);
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
    apply(&staged, &target, ReplaceStyle::Unix, Transfer::Move).unwrap();
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
    apply(&staged, &target, ReplaceStyle::Unix, Transfer::Move).unwrap();
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
fn inode(p: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    fs::symlink_metadata(p).unwrap().ino()
}

#[cfg(unix)]
fn mode(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o7777
}

/// The elevated helper copies, so the installed files are created by the admin process and take the destination's
/// owner and inherited permissions; a rename would keep the user's ownership. The owner itself needs root to test.
#[test]
fn copy_mode_installs_a_fresh_binary_and_leaves_the_staged_one() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("bin/rogg");
    let staged = tmp.path().join("stage/rogg");
    write(&target, b"old");
    write(&staged, b"new");
    #[cfg(unix)]
    let staged_inode = inode(&staged);
    apply(&staged, &target, ReplaceStyle::Unix, Transfer::Copy).unwrap();
    assert_eq!(read(&target), b"new");
    assert!(leftovers(&tmp.path().join("bin"), "rogg").is_empty());
    // Left for the update job's work-dir cleanup.
    assert_eq!(read(&staged), b"new");
    #[cfg(unix)]
    {
        assert_ne!(inode(&target), staged_inode, "a new file, not the staged one renamed");
        assert_eq!(mode(&target), 0o755);
    }
}

#[test]
fn copy_mode_installs_a_fresh_app_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("apps/rogg.app");
    write(&target.join("Contents/MacOS/rogg"), b"old");
    write(&target.join("Contents/only-old.txt"), b"x");
    let staged = tmp.path().join("stage/rogg.app");
    write(&staged.join("Contents/MacOS/rogg"), b"new");
    write(&staged.join("Contents/Info.plist"), b"plist");
    write(&staged.join("Contents/MacOS/helper"), b"helper");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // A set-id bit must not survive into a copy made by an admin process.
        fs::set_permissions(staged.join("Contents/MacOS/helper"), fs::Permissions::from_mode(0o4755)).unwrap();
        std::os::unix::fs::symlink("../Info.plist", staged.join("Contents/MacOS/link")).unwrap();
    }
    #[cfg(unix)]
    let staged_inodes = [inode(&staged), inode(&staged.join("Contents/MacOS/rogg"))];
    apply(&staged, &target, ReplaceStyle::Unix, Transfer::Copy).unwrap();
    assert_eq!(read(&target.join("Contents/MacOS/rogg")), b"new");
    assert_eq!(read(&target.join("Contents/Info.plist")), b"plist");
    assert_eq!(read(&target.join("Contents/MacOS/helper")), b"helper");
    assert!(!target.join("Contents/only-old.txt").exists());
    assert!(leftovers(&tmp.path().join("apps"), "rogg.app").is_empty());
    assert_eq!(read(&staged.join("Contents/MacOS/rogg")), b"new");
    #[cfg(unix)]
    {
        assert_ne!(inode(&target), staged_inodes[0]);
        assert_ne!(inode(&target.join("Contents/MacOS/rogg")), staged_inodes[1]);
        assert_eq!(mode(&target.join("Contents/MacOS/rogg")), 0o755);
        assert_eq!(mode(&target.join("Contents/MacOS/helper")), 0o755);
        // The link is recreated, not followed.
        assert_eq!(fs::read_link(target.join("Contents/MacOS/link")).unwrap(), Path::new("../Info.plist"));
    }
}

#[test]
fn apply_refuses_a_target_that_is_not_rogg() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("bin/bash");
    let staged = tmp.path().join("stage/rogg");
    write(&target, b"old");
    write(&staged, b"new");
    let r = apply(&staged, &target, ReplaceStyle::Unix, Transfer::Move);
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
    let r = apply_with_fault(&staged, &target, ReplaceStyle::Unix, Transfer::Move, Fault::SwapIn);
    match r {
        Err(UpdateError::Replace { rolled_back: true, old, .. }) => assert_eq!(old, target),
        other => panic!("{other:?}"),
    }
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
    apply(&staged, &target, ReplaceStyle::Windows, Transfer::Move).unwrap();
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
