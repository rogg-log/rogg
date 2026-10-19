//! The hidden `--apply-update` mode the elevated helper runs: no window, exit 0 / 1 / 2.

use std::process::Command;

fn run(staged: &std::path::Path, target: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rogg")).arg("--apply-update").arg(staged).arg(target).output().unwrap()
}

#[test]
fn replaces_a_rogg_target_and_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let staged = dir.path().join("staged");
    let target = dir.path().join("rogg");
    std::fs::write(&staged, b"new bytes").unwrap();
    std::fs::write(&target, b"old bytes").unwrap();
    #[cfg(unix)]
    let staged_inode = std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(&staged).unwrap());
    let out = run(&staged, &target);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(std::fs::read(&target).unwrap(), b"new bytes");
    // The helper copies (the new files then belong to the admin process) and leaves the staged copy to the job.
    assert_eq!(std::fs::read(&staged).unwrap(), b"new bytes");
    #[cfg(unix)]
    assert_ne!(std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(&target).unwrap()), staged_inode);
}

#[test]
fn refuses_a_target_that_is_not_rogg_with_exit_two() {
    let dir = tempfile::tempdir().unwrap();
    let staged = dir.path().join("staged");
    let target = dir.path().join("notes.txt");
    std::fs::write(&staged, b"new bytes").unwrap();
    std::fs::write(&target, b"keep me").unwrap();
    let out = run(&staged, &target);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(std::fs::read(&target).unwrap(), b"keep me");
}

#[test]
fn a_failed_replace_exits_one_with_a_message() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(&dir.path().join("missing"), &dir.path().join("rogg"));
    assert_eq!(out.status.code(), Some(1));
    assert!(!out.stderr.is_empty());
}
