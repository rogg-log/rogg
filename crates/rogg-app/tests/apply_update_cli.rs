//! The hidden `--apply-update <archive> <sha256> <target>` mode the elevated helper runs: no window, exit 0 / 1 / 2.
//! The tests can't run it as root, so the helper runs as the test user; the code path is the same.

use rogg_genlog::bundle::{BundleKind, write_archive};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(archive: &Path, sha256: &str, target: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rogg")).arg("--apply-update").arg(archive).arg(sha256).arg(target).output().unwrap()
}

fn sha256(path: &Path) -> String {
    Sha256::digest(fs::read(path).unwrap()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Names in `dir` starting with `<stem>.` (the helper's stage folder and the swap's leftovers).
fn leftovers(dir: &Path, stem: &str) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&format!("{stem}.")))
        .collect()
}

/// A temp folder with the user's download (`cache/<asset>`) and an installed copy (`bin/rogg`).
fn binary_fixture(new: &[u8]) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("cache")).unwrap();
    fs::create_dir_all(dir.path().join("bin")).unwrap();
    let archive = dir.path().join("cache/rogg-linux-x86_64.tar.gz");
    write_archive(&archive, BundleKind::TarGz, &[("rogg", new)]).unwrap();
    let target = dir.path().join("bin/rogg");
    fs::write(&target, b"old bytes").unwrap();
    (dir, archive, target)
}

#[test]
fn installs_a_verified_archive_and_exits_zero() {
    let (dir, archive, target) = binary_fixture(b"new bytes");
    let out = run(&archive, &sha256(&archive), &target);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fs::read(&target).unwrap(), b"new bytes");
    assert!(leftovers(&dir.path().join("bin"), "rogg").is_empty(), "no stage folder or swap leftovers");
    // The download is only read; the update job removes it.
    assert!(archive.exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&target).unwrap().permissions().mode() & 0o7777, 0o755);
    }
}

#[test]
fn installs_an_app_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("rogg-macos-arm64.tar.gz");
    write_archive(&archive, BundleKind::TarGz, &[("rogg.app/Contents/MacOS/rogg", b"mac"), ("rogg.app/Contents/Info.plist", b"p")])
        .unwrap();
    let target = dir.path().join("Applications/rogg.app");
    fs::create_dir_all(target.join("Contents/MacOS")).unwrap();
    fs::write(target.join("Contents/MacOS/rogg"), b"old").unwrap();
    let out = run(&archive, &sha256(&archive), &target);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fs::read(target.join("Contents/MacOS/rogg")).unwrap(), b"mac");
    assert_eq!(fs::read(target.join("Contents/Info.plist")).unwrap(), b"p");
    assert!(leftovers(&dir.path().join("Applications"), "rogg.app").is_empty());
}

#[test]
fn a_wrong_checksum_exits_one_and_changes_nothing() {
    let (dir, archive, target) = binary_fixture(b"new bytes");
    let out = run(&archive, &"0".repeat(64), &target);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("checksum"), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fs::read(&target).unwrap(), b"old bytes");
    assert!(leftovers(&dir.path().join("bin"), "rogg").is_empty());
}

/// The helper runs as root on the user's download: a link there could point at a file only root can read.
#[cfg(unix)]
#[test]
fn a_linked_archive_exits_one_and_changes_nothing() {
    let (dir, archive, target) = binary_fixture(b"new bytes");
    let link = dir.path().join("cache/link.tar.gz");
    std::os::unix::fs::symlink(&archive, &link).unwrap();
    let out = run(&link, &sha256(&archive), &target);
    assert_eq!(out.status.code(), Some(1));
    assert!(!out.stderr.is_empty());
    assert_eq!(fs::read(&target).unwrap(), b"old bytes");
    assert!(leftovers(&dir.path().join("bin"), "rogg").is_empty());
}

#[test]
fn a_missing_archive_exits_one_with_a_message() {
    let (dir, _, target) = binary_fixture(b"new bytes");
    let out = run(&dir.path().join("cache/missing.tar.gz"), &"0".repeat(64), &target);
    assert_eq!(out.status.code(), Some(1));
    assert!(!out.stderr.is_empty());
    assert_eq!(fs::read(&target).unwrap(), b"old bytes");
}

#[test]
fn refuses_a_target_that_is_not_rogg_with_exit_two() {
    let (dir, archive, _) = binary_fixture(b"new bytes");
    let target = dir.path().join("bin/notes.txt");
    fs::write(&target, b"keep me").unwrap();
    let out = run(&archive, &sha256(&archive), &target);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(fs::read(&target).unwrap(), b"keep me");
    assert!(leftovers(&dir.path().join("bin"), "notes.txt").is_empty());
}

#[test]
fn the_old_two_argument_form_is_refused() {
    let (_dir, archive, target) = binary_fixture(b"new bytes");
    let out = Command::new(env!("CARGO_BIN_EXE_rogg")).arg("--apply-update").arg(&archive).arg(&target).output().unwrap();
    assert_ne!(out.status.code(), Some(0));
    assert_eq!(fs::read(&target).unwrap(), b"old bytes");
}
