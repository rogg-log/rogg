//! The real binary answers `--version` on stdout and exits without opening a window (install.sh relies on this).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn rogg_version_prints_one_line_and_exits() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rogg"))
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .expect("rogg starts");
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().ok();
            panic!("rogg --version did not exit (did it open a window?)");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.starts_with(&format!("rogg {} (", env!("CARGO_PKG_VERSION"))), "{text:?}");
    assert_eq!(text.lines().count(), 1, "{text:?}");
}
