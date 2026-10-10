//! rogg.exe's resource script (build/winres.rs, used by build.rs): the version text must read exactly like
//! `rogg --version`, because install.ps1 parses both the same way.

use crate::about;
use crate::winres::{product_version, resource_script};

#[test]
fn the_exe_product_version_is_the_version_line() {
    let cases: [(Option<&'static str>, Option<&'static str>, Option<&'static str>); 4] =
        [(Some("3b4060e"), Some("6"), Some("main")), (Some("3b4060e"), Some("6"), None), (Some("abc1234"), None, None), (None, None, None)];
    for (commit, number, channel) in cases {
        let line = about::version_line(&about::describe(commit, None, None, number, channel));
        assert_eq!(product_version(env!("CARGO_PKG_VERSION"), commit, number, channel), line);
    }
}

#[test]
fn the_resource_script_carries_the_icon_and_versions() {
    let rc = resource_script(r"C:\src\rogg\assets\icon\rogg.ico", "0.1.0", Some("3b4060e"), Some("6"), Some("main"));
    assert!(rc.contains(r#"1 ICON "C:\\src\\rogg\\assets\\icon\\rogg.ico""#), "{rc}");
    assert!(rc.contains("FILEVERSION 0,1,0,6") && rc.contains("PRODUCTVERSION 0,1,0,6"), "{rc}");
    assert!(rc.contains(r#"VALUE "FileVersion", "0.1.0.6""#), "{rc}");
    assert!(rc.contains(r#"VALUE "ProductVersion", "rogg 0.1.0 (build 6, main, 3b4060e)""#), "{rc}");
    assert!(rc.contains(r#"VALUE "ProductName", "rogg""#) && rc.contains(r#"VALUE "OriginalFilename", "rogg.exe""#), "{rc}");
    let local = resource_script("/x/rogg.ico", "0.1.0", Some("abc1234"), None, None);
    assert!(local.contains("FILEVERSION 0,1,0,0") && local.contains("(local, abc1234)"), "{local}");
}
