//! Runs the real binary, because headless Metal capture must own the main thread.
use std::process::Command;

fn png_size(path: &std::path::Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[1..4], b"PNG");
    let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (w, h)
}

fn capture(args: &[&str]) {
    let status = Command::new(env!("CARGO_BIN_EXE_rogg-gallery")).args(args).status().expect("spawn gallery");
    assert!(status.success(), "capture failed: {args:?}");
}

#[test]
fn captures_controls_page_at_2x_with_geometry() {
    let dir = std::env::temp_dir().join(format!("rogg-capture-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("controls.png");
    let json = dir.join("controls.json");
    capture(&["--capture", "controls", "--size", "800x600", "--out", png.to_str().unwrap(), "--geometry", json.to_str().unwrap()]);
    assert_eq!(png_size(&png), (1600, 1200));
    let geometry: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    assert_eq!(geometry["elements"]["flag-regex-on"]["h"], 27.0);
}

#[test]
fn captures_replica_with_rows_and_dialog() {
    let dir = std::env::temp_dir().join(format!("rogg-capture-replica-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("replica.png");
    let json = dir.join("replica.json");
    capture(&["--capture", "replica", "--theme", "light", "--density", "comfortable", "--dialog", "shortcuts", "--out", png.to_str().unwrap(), "--geometry", json.to_str().unwrap()]);
    assert_eq!(png_size(&png), (2880, 1800));
    let g: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    for key in ["titlebar", "tab-shell-0", "token-3", "row-top-0", "row-bottom-1", "gap-bottom-0", "statusbar", "dialog", "tree-row-10"] {
        assert!(g["elements"].get(key).is_some(), "missing geometry key {key}");
    }
    assert_eq!(g["elements"]["row-top-0"]["h"], 29.0, "comfortable row height");
}

#[test]
fn captures_the_real_app_page_with_rows_and_history() {
    let dir = std::env::temp_dir().join(format!("rogg-capture-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("app.png");
    let json = dir.join("app.json");
    capture(&["--capture", "app", "--dialog", "history", "--out", png.to_str().unwrap(), "--geometry", json.to_str().unwrap()]);
    assert_eq!(png_size(&png), (2880, 1800));
    let g: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    for key in ["titlebar", "tab-shell-2", "token-3", "row-top-0", "row-bottom-1", "gap-bottom-0", "statusbar", "dialog", "tree-row-10", "history-pick-1"] {
        assert!(g["elements"].get(key).is_some(), "missing geometry key {key}");
    }
}

#[test]
fn app_page_rejects_dialogs_it_does_not_have() {
    let out = Command::new(env!("CARGO_BIN_EXE_rogg-gallery"))
        .args(["--capture", "app", "--dialog", "profile", "--out", "/tmp/never.png"])
        .output()
        .expect("spawn gallery");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not available on the app page"));
}

#[test]
fn rejects_bad_arguments() {
    let out = Command::new(env!("CARGO_BIN_EXE_rogg-gallery")).args(["--capture", "nope", "--out", "/tmp/x.png"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown page"));
}
