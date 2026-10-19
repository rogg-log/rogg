//! Running the hidden `rogg --apply-update <staged> <target>` with admin rights: osascript on macOS, pkexec on
//! Linux, the UAC prompt (`ShellExecuteExW` "runas") on Windows.
//!
//! The helper's paths reach it as separate arguments everywhere. The one string that is parsed (the AppleScript)
//! escapes each path into a string literal and shell-quotes it with `quoted form of`.

use super::Failure;
use rogg_core::platform::Os;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Replaces `target` with `staged` by running this executable's `--apply-update` with admin rights. Blocks until
/// the prompt is answered and the helper has exited.
pub fn apply_elevated(staged: &Path, target: &Path) -> Result<(), Failure> {
    let exe = std::env::current_exe().map_err(|_| Failure::RolledBack)?;
    imp::run(&exe, staged, target)
}

/// The osascript program that runs the helper with administrator privileges.
pub fn applescript(exe: &Path, staged: &Path, target: &Path) -> String {
    format!(
        "do shell script quoted form of {} & \" --apply-update \" & quoted form of {} & \" \" & quoted form of {} with administrator privileges",
        applescript_string(exe),
        applescript_string(staged),
        applescript_string(target),
    )
}

/// `path` as an AppleScript string literal: `\` and `"` are backslash-escaped, so no path can end the literal.
fn applescript_string(path: &Path) -> String {
    let mut literal = String::from('"');
    for c in path.to_string_lossy().chars() {
        if matches!(c, '\\' | '"') {
            literal.push('\\');
        }
        literal.push(c);
    }
    literal.push('"');
    literal
}

/// pkexec's arguments: the helper command, one argument per path.
pub fn pkexec_argv(exe: &Path, staged: &Path, target: &Path) -> Vec<OsString> {
    vec![exe.into(), "--apply-update".into(), staged.into(), target.into()]
}

/// How the elevated run ended, from the launcher's exit status. Only osascript's own cancel code (-128, in its
/// stderr) is read from stderr; on Windows the helper is a GUI program whose stderr is lost.
pub fn map_status(os: Os, status: Option<i32>, stderr: &str) -> Result<(), Failure> {
    match (os, status) {
        (_, Some(0)) => Ok(()),
        // "execution error: User canceled. (-128)". A failing helper ends the message with its own code instead.
        (Os::Mac, Some(1)) if stderr.trim_end().ends_with("(-128)") => Err(Failure::Declined),
        // pkexec: 126 the dialog was dismissed, 127 not authorized (or no authentication agent).
        (Os::Linux, Some(126 | 127)) => Err(Failure::Declined),
        // The helper failed (1, after rolling back) or refused the target (2), or never ran: the current version
        // is kept.
        _ => Err(Failure::RolledBack),
    }
}

/// The first executable file `name` in the `PATH`-style list `path`. Empty and relative entries are skipped: the
/// result runs with admin rights, so it never comes from the current folder.
pub fn find_on_path(name: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

/// Where pkexec is installed on the distributions rogg supports.
pub const PKEXEC: &str = "/usr/bin/pkexec";

/// The pkexec to run: `fixed` ([`PKEXEC`]) when it is an executable file, else the first one on `path` (`PATH`).
/// Like the fixed `/usr/bin/osascript` on macOS, the known location comes first, so what runs with admin rights
/// doesn't depend on the environment unless pkexec lives elsewhere.
pub fn find_pkexec(fixed: &Path, path: Option<&OsStr>) -> Option<PathBuf> {
    if is_executable(fixed) {
        return Some(fixed.to_path_buf());
    }
    path.and_then(|path| find_on_path("pkexec", path))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

const QUOTE: u16 = b'"' as u16;
const BACKSLASH: u16 = b'\\' as u16;

/// The UAC launch's parameters: `--apply-update "<staged>" "<target>"`, each path quoted for the Windows command
/// line. The paths are UTF-16 (`OsStrExt::encode_wide`).
pub fn windows_parameters(staged: &[u16], target: &[u16]) -> Vec<u16> {
    let mut params: Vec<u16> = "--apply-update ".encode_utf16().collect();
    quote_windows_arg(staged, &mut params);
    params.push(b' ' as u16);
    quote_windows_arg(target, &mut params);
    params
}

/// Appends `arg` in double quotes, escaped so the program's argument parser (the `CommandLineToArgvW` rules) reads
/// it back unchanged: a quote becomes `\"` with the backslashes before it doubled, and a trailing run of
/// backslashes is doubled so it doesn't escape the closing quote. Other backslashes are literal.
pub fn quote_windows_arg(arg: &[u16], out: &mut Vec<u16>) {
    out.push(QUOTE);
    let mut backslashes = 0;
    for &c in arg {
        if c == BACKSLASH {
            backslashes += 1;
        } else {
            if c == QUOTE {
                out.extend(std::iter::repeat_n(BACKSLASH, backslashes + 1));
            }
            backslashes = 0;
        }
        out.push(c);
    }
    out.extend(std::iter::repeat_n(BACKSLASH, backslashes));
    out.push(QUOTE);
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::process::{Command, Stdio};

    pub fn run(exe: &Path, staged: &Path, target: &Path) -> Result<(), Failure> {
        let out = Command::new("/usr/bin/osascript")
            .args(["-e", &applescript(exe, staged, target)])
            .stdin(Stdio::null())
            .output()
            .map_err(|_| Failure::RolledBack)?;
        map_status(Os::Mac, out.status.code(), &String::from_utf8_lossy(&out.stderr))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use super::*;
    use std::process::{Command, Stdio};

    pub fn run(exe: &Path, staged: &Path, target: &Path) -> Result<(), Failure> {
        let path = std::env::var_os("PATH");
        let pkexec = find_pkexec(Path::new(PKEXEC), path.as_deref()).ok_or(Failure::NoPkexec)?;
        let out = Command::new(pkexec)
            .args(pkexec_argv(exe, staged, target))
            .stdin(Stdio::null())
            .output()
            .map_err(|_| Failure::RolledBack)?;
        map_status(Os::Linux, out.status.code(), &String::from_utf8_lossy(&out.stderr))
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, GetLastError, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().collect()
    }

    fn nul_terminated(mut s: Vec<u16>) -> Vec<u16> {
        s.push(0);
        s
    }

    pub fn run(exe: &Path, staged: &Path, target: &Path) -> Result<(), Failure> {
        let verb = nul_terminated(wide(OsStr::new("runas")));
        let file = nul_terminated(wide(exe.as_os_str()));
        let params = nul_terminated(windows_parameters(&wide(staged.as_os_str()), &wide(target.as_os_str())));
        let mut info = SHELLEXECUTEINFOW {
            cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
            // NOASYNC: this runs on a worker thread without a message loop.
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: verb.as_ptr(),
            lpFile: file.as_ptr(),
            lpParameters: params.as_ptr(),
            nShow: SW_HIDE,
            ..Default::default()
        };
        // SAFETY: `info` and the strings it points to outlive the calls; the process handle is closed before
        // returning.
        unsafe {
            if ShellExecuteExW(&mut info) == 0 {
                return Err(if GetLastError() == ERROR_CANCELLED { Failure::Declined } else { Failure::RolledBack });
            }
            if info.hProcess.is_null() {
                return Err(Failure::RolledBack);
            }
            let mut code = 0u32;
            let exited = WaitForSingleObject(info.hProcess, INFINITE) == WAIT_OBJECT_0
                && GetExitCodeProcess(info.hProcess, &mut code) != 0;
            CloseHandle(info.hProcess);
            if !exited {
                return Err(Failure::RolledBack);
            }
            map_status(Os::Windows, Some(code as i32), "")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(parts: &[&str]) -> Vec<OsString> {
        parts.iter().map(OsString::from).collect()
    }

    #[test]
    fn applescript_quotes_every_path() {
        let script = applescript(
            Path::new("/Apps/r o g g.app/Contents/MacOS/rogg"),
            Path::new("/tmp/it's/rogg.app"),
            Path::new("/Apps/r o g g.app"),
        );
        assert_eq!(
            script,
            r#"do shell script quoted form of "/Apps/r o g g.app/Contents/MacOS/rogg" & " --apply-update " & quoted form of "/tmp/it's/rogg.app" & " " & quoted form of "/Apps/r o g g.app" with administrator privileges"#
        );
    }

    #[test]
    fn applescript_escapes_backslashes_and_double_quotes() {
        let script = applescript(Path::new(r#"/a\b"c"#), Path::new("/s"), Path::new("/t"));
        assert!(script.starts_with(r#"do shell script quoted form of "/a\\b\"c" & "#), "{script}");
    }

    #[test]
    fn pkexec_runs_the_hidden_command() {
        let argv = pkexec_argv(Path::new("/opt/rogg/rogg"), Path::new("/tmp/s p/rogg"), Path::new("/usr/bin/rogg"));
        assert_eq!(argv, os(&["/opt/rogg/rogg", "--apply-update", "/tmp/s p/rogg", "/usr/bin/rogg"]));
    }

    #[test]
    fn a_successful_helper_is_ok() {
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            assert_eq!(map_status(os, Some(0), ""), Ok(()));
        }
    }

    #[test]
    fn a_cancelled_mac_prompt_is_declined() {
        assert_eq!(map_status(Os::Mac, Some(1), "0:94: execution error: User canceled. (-128)\n"), Err(Failure::Declined));
    }

    #[test]
    fn a_dismissed_pkexec_prompt_is_declined() {
        assert_eq!(map_status(Os::Linux, Some(126), ""), Err(Failure::Declined));
        assert_eq!(map_status(Os::Linux, Some(127), ""), Err(Failure::Declined));
    }

    #[test]
    fn a_failed_or_refusing_helper_rolled_back() {
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            assert_eq!(map_status(os, Some(1), "rogg: update failed: busy"), Err(Failure::RolledBack));
            assert_eq!(map_status(os, Some(2), "rogg: refusing to update"), Err(Failure::RolledBack));
            assert_eq!(map_status(os, None, ""), Err(Failure::RolledBack));
        }
        // The cancel code means nothing outside osascript.
        assert_eq!(map_status(Os::Linux, Some(1), "(-128)"), Err(Failure::RolledBack));
        assert_eq!(map_status(Os::Windows, Some(1), "(-128)"), Err(Failure::RolledBack));
        // A helper failure osascript reports: the helper's own exit code, not the cancel code.
        assert_eq!(
            map_status(Os::Mac, Some(1), "0:94: execution error: rogg: update failed: busy (1)\n"),
            Err(Failure::RolledBack)
        );
    }

    #[test]
    fn missing_pkexec_is_reported() {
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(find_on_path("pkexec", empty.path().as_os_str()), None);
        assert_eq!(find_on_path("pkexec", OsStr::new("")), None);
    }

    #[test]
    fn pkexec_is_found_on_path() {
        let empty = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let pkexec = bin.path().join("pkexec");
        std::fs::write(&pkexec, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&pkexec, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = std::env::join_paths([empty.path(), bin.path()]).unwrap();
        assert_eq!(find_on_path("pkexec", &path), Some(pkexec));
    }

    fn executable(path: &Path) {
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn the_fixed_pkexec_comes_before_the_path() {
        let fixed_dir = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let fixed = fixed_dir.path().join("pkexec");
        let on_path = bin.path().join("pkexec");
        executable(&on_path);
        let path = bin.path().as_os_str();
        // Missing: the PATH one.
        assert_eq!(find_pkexec(&fixed, Some(path)), Some(on_path.clone()));
        executable(&fixed);
        assert_eq!(find_pkexec(&fixed, Some(path)), Some(fixed.clone()));
        assert_eq!(find_pkexec(&fixed, None), Some(fixed));
        assert_eq!(find_pkexec(&fixed_dir.path().join("none"), None), None);
        assert_eq!(find_pkexec(fixed_dir.path(), None), None, "a folder isn't pkexec");
        assert_eq!(Path::new(PKEXEC), Path::new("/usr/bin/pkexec"));
    }

    #[cfg(unix)]
    #[test]
    fn a_fixed_pkexec_that_isnt_executable_is_skipped() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let fixed = dir.path().join("pkexec");
        std::fs::write(&fixed, "").unwrap();
        std::fs::set_permissions(&fixed, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(find_pkexec(&fixed, None), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_isnt_executable_isnt_pkexec() {
        use std::os::unix::fs::PermissionsExt;
        let bin = tempfile::tempdir().unwrap();
        let pkexec = bin.path().join("pkexec");
        std::fs::write(&pkexec, "").unwrap();
        std::fs::set_permissions(&pkexec, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::create_dir(bin.path().join("dir")).unwrap();
        assert_eq!(find_on_path("pkexec", bin.path().as_os_str()), None);
        assert_eq!(find_on_path("dir", bin.path().as_os_str()), None);
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn windows_parameters_quote_each_path() {
        let params = windows_parameters(&wide(r"C:\Users\me\AppData\Local\Temp\rogg.exe"), &wide(r"C:\Program Files\rogg\rogg.exe"));
        assert_eq!(
            String::from_utf16(&params).unwrap(),
            r#"--apply-update "C:\Users\me\AppData\Local\Temp\rogg.exe" "C:\Program Files\rogg\rogg.exe""#
        );
    }

    #[test]
    fn windows_quoting_follows_the_command_line_rules() {
        let quote = |s: &str| {
            let mut out = Vec::new();
            quote_windows_arg(&wide(s), &mut out);
            String::from_utf16(&out).unwrap()
        };
        // Backslashes not followed by a quote are literal.
        assert_eq!(quote(r"a\\b\c"), r#""a\\b\c""#);
        // A trailing run is doubled so it doesn't escape the closing quote.
        assert_eq!(quote(r"C:\dir\"), r#""C:\dir\\""#);
        assert_eq!(quote(r"C:\dir\\"), r#""C:\dir\\\\""#);
        // An embedded quote is escaped, and the backslashes before it doubled.
        assert_eq!(quote(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote(r#"a\"b"#), r#""a\\\"b""#);
        assert_eq!(quote(""), r#""""#);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn osascript_passes_hostile_paths_through_unchanged() {
        // Runs the script with `return` in place of `do shell script … with administrator privileges` (no prompt),
        // then hands the shell command it built to a function that prints its arguments.
        let exe = "/Apps/r\"o\\g'g $(echo pwned) `echo pwned`.app/rogg";
        let staged = "/tmp/a\" & (do shell script \"echo pwned\") & \"b/rogg.app";
        let target = "/tmp/new\nline;'\\\"/rogg.app";
        let script = applescript(Path::new(exe), Path::new(staged), Path::new(target));
        let script = script.strip_prefix("do shell script ").and_then(|s| s.strip_suffix(" with administrator privileges")).unwrap();
        let out = std::process::Command::new("/usr/bin/osascript").args(["-e", &format!("return {script}")]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let command = String::from_utf8(out.stdout).unwrap();
        let command = command.strip_suffix('\n').unwrap();
        let shell = format!("f() {{ printf '%s\\0' \"$@\"; }}; f {command}");
        let printed = std::process::Command::new("/bin/sh").args(["-c", &shell]).output().unwrap();
        let args: Vec<&str> = std::str::from_utf8(&printed.stdout).unwrap().split_terminator('\0').collect();
        assert_eq!(args, [exe, "--apply-update", staged, target]);
    }
}
