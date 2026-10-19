use rogg_core::platform::Os;
use rogg_core::update::{Install, Manifest, UpdateError, asset_name, is_newer};
use std::path::PathBuf;

const MANIFEST: &str = r#"{
  "version":"0.1.0","build":14,"commit":"a1b2c3d4e5f60718293a4b5c6d7e8f9012345678",
  "date":"2026-10-18T10:00:00Z","channel":"main","future_field":{"x":1},
  "assets":[
    {"name":"rogg-linux-arm64.tar.gz","os":"linux","arch":"arm64","sha256":"ab","size":12345678,"kind":"binary"},
    {"name":"rogg-windows-x86_64.zip","os":"windows","arch":"x86_64","sha256":"cd","size":99}
  ]}"#;

#[test]
fn parses_a_release_manifest() {
    let m = Manifest::parse(MANIFEST).unwrap();
    assert_eq!(m.build, 14);
    assert_eq!(m.short_commit(), "a1b2c3d");
    assert_eq!(m.asset("rogg-linux-arm64.tar.gz").unwrap().size, 12345678);
    assert!(m.asset("nope").is_none());
    assert_eq!(m.release_url(), "https://github.com/rogg-log/rogg/releases/tag/v0.1.0-build.14");
}

#[test]
fn a_manifest_without_build_is_rejected() {
    assert!(matches!(Manifest::parse(r#"{"version":"0.1.0"}"#), Err(UpdateError::BadManifest(_))));
}

#[test]
fn is_newer_compares_builds() {
    assert!(is_newer(14, 11));
    assert!(!is_newer(11, 11));
    assert!(!is_newer(10, 11));
}

fn touch(p: &std::path::Path) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, b"x").unwrap();
}

#[test]
fn detects_an_app_bundle_and_a_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let exe = root.join("Apps/rogg.app/Contents/MacOS/rogg");
    touch(&exe);
    assert_eq!(Install::detect(&exe, Os::Mac), Install::App { bundle: root.join("Apps/rogg.app") });
    let bin = root.join("bin/rogg");
    touch(&bin);
    assert_eq!(Install::detect(&bin, Os::Mac), Install::Binary { path: bin.clone() });
    assert_eq!(Install::detect(&exe, Os::Linux), Install::Binary { path: exe.clone() });
}

#[cfg(unix)]
#[test]
fn a_symlinked_binary_resolves_to_the_real_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let real = root.join("opt/rogg");
    touch(&real);
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::os::unix::fs::symlink(&real, root.join("bin/rogg")).unwrap();
    assert_eq!(Install::detect(&root.join("bin/rogg"), Os::Linux), Install::Binary { path: real });
}

#[test]
fn a_translocated_app_is_unsupported() {
    let exe = PathBuf::from("/private/var/folders/x/AppTranslocation/ABC/d/rogg.app/Contents/MacOS/rogg");
    assert_eq!(Install::detect(&exe, Os::Mac), Install::Unsupported);
    assert_eq!(Install::Unsupported.target(), None);
}

#[test]
fn asset_names_per_platform() {
    let app = Install::App { bundle: PathBuf::from("/Applications/rogg.app") };
    let bin = Install::Binary { path: PathBuf::from("/x/rogg") };
    assert_eq!(asset_name(&app, Os::Mac, "aarch64"), Some("rogg-macos-universal-app.tar.gz"));
    assert_eq!(asset_name(&bin, Os::Mac, "x86_64"), Some("rogg-macos-universal.tar.gz"));
    assert_eq!(asset_name(&bin, Os::Linux, "x86_64"), Some("rogg-linux-x86_64.tar.gz"));
    assert_eq!(asset_name(&bin, Os::Linux, "aarch64"), Some("rogg-linux-arm64.tar.gz"));
    assert_eq!(asset_name(&bin, Os::Windows, "x86_64"), Some("rogg-windows-x86_64.zip"));
    assert_eq!(asset_name(&bin, Os::Linux, "riscv64"), None);
    assert_eq!(asset_name(&bin, Os::Windows, "aarch64"), None);
    assert_eq!(asset_name(&Install::Unsupported, Os::Linux, "x86_64"), None);
    assert_eq!(app.target(), Some(std::path::Path::new("/Applications/rogg.app")));
    assert_eq!(bin.target(), Some(std::path::Path::new("/x/rogg")));
}
