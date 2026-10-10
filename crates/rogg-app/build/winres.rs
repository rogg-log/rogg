//! rogg.exe's Windows resource script: its icon and version information (install spec §9). build.rs writes it for
//! Windows targets; the library includes this file in its tests (src/winres_tests.rs).

/// The `rogg --version` text. It must match `about::version_line` (a build script can't call the crate, so it is
/// repeated here, and a test compares the two).
pub fn product_version(version: &str, commit: Option<&str>, number: Option<&str>, channel: Option<&str>) -> String {
    let mut parts = match number {
        Some(n) => vec![format!("build {n}")],
        None => vec!["local".to_string()],
    };
    if let (Some(_), Some(c)) = (number, channel) {
        parts.push(c.to_string());
    }
    parts.push(commit.unwrap_or("unknown").to_string());
    format!("rogg {version} ({})", parts.join(", "))
}

/// The `.rc` text: `1 ICON` from `icon`, and a `VERSIONINFO` block whose fourth number is the CI build (0 locally).
pub fn resource_script(icon: &str, version: &str, commit: Option<&str>, number: Option<&str>, channel: Option<&str>) -> String {
    let mut nums: Vec<u32> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    nums.resize(3, 0);
    let build: u32 = number.and_then(|n| n.parse().ok()).unwrap_or(0);
    let numeric = format!("{},{},{},{build}", nums[0], nums[1], nums[2]);
    let file_version = format!("{}.{}.{}.{build}", nums[0], nums[1], nums[2]);
    let product = product_version(version, commit, number, channel);
    let icon = icon.replace('\\', "\\\\");
    format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "rogg-log"
      VALUE "FileDescription", "rogg log viewer"
      VALUE "FileVersion", "{file_version}"
      VALUE "OriginalFilename", "rogg.exe"
      VALUE "ProductName", "rogg"
      VALUE "ProductVersion", "{product}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}
