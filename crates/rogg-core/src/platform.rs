//! Everything that differs between operating systems (spec §3): data folders, whether a process is alive, whether
//! stdin carries data, and stream paths. The folder rules are pure functions of `Os` and `Env`, so every platform's
//! rules are tested on every platform.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Linux,
    Windows,
}

impl Os {
    /// The OS this binary was built for (other Unixes use the Linux rules).
    pub const fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// The environment variables the folder rules read.
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub home: Option<OsString>,
    pub userprofile: Option<OsString>,
    pub xdg_config_home: Option<OsString>,
    pub xdg_data_home: Option<OsString>,
    pub xdg_cache_home: Option<OsString>,
    pub appdata: Option<OsString>,
    pub localappdata: Option<OsString>,
}

impl Env {
    pub fn from_process() -> Env {
        let get = |k: &str| std::env::var_os(k);
        Env {
            home: get("HOME"),
            userprofile: get("USERPROFILE"),
            xdg_config_home: get("XDG_CONFIG_HOME"),
            xdg_data_home: get("XDG_DATA_HOME"),
            xdg_cache_home: get("XDG_CACHE_HOME"),
            appdata: get("APPDATA"),
            localappdata: get("LOCALAPPDATA"),
        }
    }
}

const APP: &str = "rogg";

/// A set, non-empty, absolute path from `value` (XDG ignores relative values; empty means unset). Absoluteness is
/// `os`'s rule, not the host's, so every platform's rules can be tested on every platform.
fn absolute(os: Os, value: &Option<OsString>) -> Option<PathBuf> {
    let text = value.as_ref()?.to_str()?;
    let ok = match os {
        Os::Windows => {
            let b = text.as_bytes();
            let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/');
            drive || text.starts_with(r"\\")
        }
        Os::Mac | Os::Linux => text.starts_with('/'),
    };
    ok.then(|| PathBuf::from(text))
}

/// The user's home folder; the temp dir when unknown, so paths never become cwd-relative.
pub fn home(os: Os, env: &Env) -> PathBuf {
    let var = if os == Os::Windows { &env.userprofile } else { &env.home };
    absolute(os, var).unwrap_or_else(std::env::temp_dir)
}

/// Settings and profiles.
pub fn config_base(os: Os, env: &Env) -> PathBuf {
    match os {
        Os::Mac => home(os, env).join("Library/Application Support").join(APP),
        Os::Linux => absolute(os, &env.xdg_config_home).unwrap_or_else(|| home(os, env).join(".config")).join(APP),
        Os::Windows => roaming(env).join(APP),
    }
}

/// History, recent files and the session.
pub fn data_base(os: Os, env: &Env) -> PathBuf {
    match os {
        Os::Mac => config_base(os, env),
        Os::Linux => absolute(os, &env.xdg_data_home).unwrap_or_else(|| home(os, env).join(".local").join("share")).join(APP),
        Os::Windows => roaming(env).join(APP),
    }
}

/// Conversions, extracted members, stream captures.
pub fn cache_base(os: Os, env: &Env) -> PathBuf {
    match os {
        Os::Mac => home(os, env).join("Library/Caches").join(APP),
        Os::Linux => absolute(os, &env.xdg_cache_home).unwrap_or_else(|| home(os, env).join(".cache")).join(APP),
        Os::Windows => absolute(os, &env.localappdata).unwrap_or_else(|| home(os, env).join("AppData").join("Local")).join(APP).join("cache"),
    }
}

fn roaming(env: &Env) -> PathBuf {
    absolute(Os::Windows, &env.appdata).unwrap_or_else(|| home(Os::Windows, env).join("AppData").join("Roaming"))
}

/// Whether process `pid` exists (a process we may not signal still counts). Used to keep a running instance's cache
/// files during cleanup.
pub fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    imp::process_alive(pid)
}

/// stdin carries data: a pipe, FIFO, socket or regular file — not a terminal or a null device.
pub fn stdin_has_data() -> bool {
    imp::stdin_has_data()
}

/// A FIFO or socket on disk: read it as a stream, never map it. Windows has neither, so it is always false there.
pub fn is_stream_path(path: &Path) -> bool {
    imp::is_stream_path(path)
}

#[cfg(unix)]
mod imp {
    use std::os::unix::fs::FileTypeExt;
    use std::path::Path;

    pub fn process_alive(pid: u32) -> bool {
        let Ok(pid) = i32::try_from(pid) else { return false };
        // SAFETY: signal 0 delivers nothing; it only checks that the process exists.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        alive || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }

    pub fn stdin_has_data() -> bool {
        crate::stream::fd_has_data(libc::STDIN_FILENO)
    }

    pub fn is_stream_path(path: &Path) -> bool {
        std::fs::metadata(path).is_ok_and(|m| m.file_type().is_fifo() || m.file_type().is_socket())
    }
}

#[cfg(windows)]
mod imp {
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::Storage::FileSystem::{FILE_TYPE_DISK, FILE_TYPE_PIPE, GetFileType};
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    pub fn process_alive(pid: u32) -> bool {
        // SAFETY: plain Win32 calls; the handle is closed before returning.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                // Access denied still means the process exists.
                return std::io::Error::last_os_error().raw_os_error() == Some(5);
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut code) != 0;
            CloseHandle(handle);
            ok && code == STILL_ACTIVE as u32
        }
    }

    pub fn stdin_has_data() -> bool {
        // SAFETY: GetStdHandle/GetFileType only read the handle table.
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            // Null: no stdin (a GUI-subsystem process); -1 is INVALID_HANDLE_VALUE.
            if handle.is_null() || handle as isize == -1 {
                return false;
            }
            matches!(GetFileType(handle), FILE_TYPE_PIPE | FILE_TYPE_DISK)
        }
    }

    pub fn is_stream_path(_: &Path) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn env(pairs: &[(&str, &str)]) -> Env {
        let get = |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| std::ffi::OsString::from(*v));
        Env {
            home: get("HOME"),
            userprofile: get("USERPROFILE"),
            xdg_config_home: get("XDG_CONFIG_HOME"),
            xdg_data_home: get("XDG_DATA_HOME"),
            xdg_cache_home: get("XDG_CACHE_HOME"),
            appdata: get("APPDATA"),
            localappdata: get("LOCALAPPDATA"),
        }
    }

    #[test]
    fn macos_folders_are_unchanged() {
        let e = env(&[("HOME", "/Users/u")]);
        assert_eq!(config_base(Os::Mac, &e), PathBuf::from("/Users/u/Library/Application Support/rogg"));
        assert_eq!(data_base(Os::Mac, &e), PathBuf::from("/Users/u/Library/Application Support/rogg"));
        assert_eq!(cache_base(Os::Mac, &e), PathBuf::from("/Users/u/Library/Caches/rogg"));
    }

    #[test]
    fn linux_follows_xdg_with_home_fallbacks() {
        let e = env(&[("HOME", "/home/u")]);
        assert_eq!(config_base(Os::Linux, &e), PathBuf::from("/home/u/.config/rogg"));
        assert_eq!(data_base(Os::Linux, &e), PathBuf::from("/home/u/.local/share/rogg"));
        assert_eq!(cache_base(Os::Linux, &e), PathBuf::from("/home/u/.cache/rogg"));
        let x = env(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "/cfg"), ("XDG_DATA_HOME", "/data"), ("XDG_CACHE_HOME", "/cache")]);
        assert_eq!(config_base(Os::Linux, &x), PathBuf::from("/cfg/rogg"));
        assert_eq!(data_base(Os::Linux, &x), PathBuf::from("/data/rogg"));
        assert_eq!(cache_base(Os::Linux, &x), PathBuf::from("/cache/rogg"));
    }

    #[test]
    fn relative_xdg_values_are_ignored() {
        let e = env(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "rel/cfg"), ("XDG_DATA_HOME", ""), ("XDG_CACHE_HOME", "./c")]);
        assert_eq!(config_base(Os::Linux, &e), PathBuf::from("/home/u/.config/rogg"));
        assert_eq!(data_base(Os::Linux, &e), PathBuf::from("/home/u/.local/share/rogg"));
        assert_eq!(cache_base(Os::Linux, &e), PathBuf::from("/home/u/.cache/rogg"));
    }

    #[test]
    fn windows_uses_appdata_and_localappdata() {
        let e = env(&[("USERPROFILE", r"C:\Users\u"), ("APPDATA", r"C:\Users\u\AppData\Roaming"), ("LOCALAPPDATA", r"C:\Users\u\AppData\Local")]);
        assert_eq!(config_base(Os::Windows, &e), PathBuf::from(r"C:\Users\u\AppData\Roaming").join("rogg"));
        assert_eq!(data_base(Os::Windows, &e), PathBuf::from(r"C:\Users\u\AppData\Roaming").join("rogg"));
        assert_eq!(cache_base(Os::Windows, &e), PathBuf::from(r"C:\Users\u\AppData\Local").join("rogg").join("cache"));
        assert_eq!(home(Os::Windows, &e), PathBuf::from(r"C:\Users\u"));
        let bare = env(&[("USERPROFILE", r"C:\Users\u")]);
        assert_eq!(config_base(Os::Windows, &bare), PathBuf::from(r"C:\Users\u").join("AppData").join("Roaming").join("rogg"));
        assert_eq!(cache_base(Os::Windows, &bare), PathBuf::from(r"C:\Users\u").join("AppData").join("Local").join("rogg").join("cache"));
    }

    #[test]
    fn absoluteness_follows_the_target_os_not_the_host() {
        let some = |s: &str| Some(OsString::from(s));
        for win in [r"C:\Users\u", "D:/data", r"\\server\share", r"\\?\C:\x"] {
            assert!(absolute(Os::Windows, &some(win)).is_some(), "{win}");
            assert!(absolute(Os::Linux, &some(win)).is_none(), "{win}");
        }
        for rel in ["C:", "C:rel", r"rel\x", "/x"] {
            assert!(absolute(Os::Windows, &some(rel)).is_none(), "{rel}");
        }
        for os in [Os::Mac, Os::Linux] {
            assert!(absolute(os, &some("/x")).is_some());
            assert!(absolute(os, &some("x/y")).is_none());
            assert!(absolute(os, &some("")).is_none());
            assert!(absolute(os, &None).is_none());
        }
    }

    #[test]
    fn empty_or_unset_home_falls_back_to_the_temp_dir() {
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            assert_eq!(home(os, &env(&[])), std::env::temp_dir());
            assert_eq!(home(os, &env(&[("HOME", ""), ("USERPROFILE", "")])), std::env::temp_dir());
        }
    }

    #[test]
    fn no_home_falls_back_to_the_temp_dir_never_cwd() {
        let e = env(&[]);
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            for p in [config_base(os, &e), data_base(os, &e), cache_base(os, &e)] {
                assert!(p.is_absolute(), "{os:?}: {p:?}");
                assert!(p.starts_with(std::env::temp_dir()), "{os:?}: {p:?}");
            }
        }
    }

    #[test]
    fn this_process_is_alive_and_a_bogus_pid_is_not() {
        assert!(process_alive(std::process::id()));
        assert!(!process_alive(0));
        assert!(!process_alive(u32::MAX - 7));
    }
}
