//! Small state files: load with corruption recovery and save atomically (spec §6.6). JSON for history, recent files
//! and sessions; TOML for settings and profiles (spec §3.4).

use crate::error::CoreError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

/// `dir/name.json` + `".bak"` → `dir/name.json.bak`.
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Writes `bytes` to `<file>.partial`, then renames it over `path`; creates the parent directory. State files have
/// one writer (the UI thread), so the fixed partial name is never shared.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(CoreError::io(dir))?;
    }
    let partial = with_suffix(path, ".partial");
    std::fs::write(&partial, bytes).map_err(CoreError::io(&partial))?;
    std::fs::rename(&partial, path).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        CoreError::Io { path: path.to_path_buf(), source: e }
    })
}

/// Reads `path` with `parse`. A missing file yields `T::default()`; an unparseable one (including invalid UTF-8) is
/// renamed to `<file>.bak` and yields `T::default()` with `recovered = true`; other I/O errors are returned.
fn load_with<T: Default>(path: &Path, parse: impl FnOnce(&[u8]) -> Option<T>) -> Result<(T, bool), CoreError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((T::default(), false)),
        Err(e) => return Err(CoreError::Io { path: path.to_path_buf(), source: e }),
    };
    match parse(&bytes) {
        Some(value) => Ok((value, false)),
        None => {
            std::fs::rename(path, with_suffix(path, ".bak")).map_err(CoreError::io(path))?;
            Ok((T::default(), true))
        }
    }
}

pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> Result<(T, bool), CoreError> {
    load_with(path, |bytes| serde_json::from_slice(bytes).ok())
}

pub fn load_toml<T: DeserializeOwned + Default>(path: &Path) -> Result<(T, bool), CoreError> {
    load_with(path, |bytes| std::str::from_utf8(bytes).ok().and_then(|s| toml::from_str(s).ok()))
}

fn invalid_data(path: &Path, e: impl std::fmt::Display) -> CoreError {
    CoreError::Io { path: path.to_path_buf(), source: std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()) }
}

/// Writes `value` as pretty JSON, atomically.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    // Not every state serializes: a path that is not valid UTF-8 has no JSON form.
    let json = serde_json::to_string_pretty(value).map_err(|e| invalid_data(path, e))?;
    write_atomic(path, json.as_bytes())
}

/// Writes `value` as TOML, atomically.
pub fn save_toml<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    let text = toml::to_string_pretty(value).map_err(|e| invalid_data(path, e))?;
    write_atomic(path, text.as_bytes())
}

fn parse_error(path: &Path, message: impl std::fmt::Display) -> CoreError {
    CoreError::Parse { path: path.to_path_buf(), message: message.to_string() }
}

/// Reads a JSON file the user chose; never renames it.
pub fn read_json_strict<T: DeserializeOwned>(path: &Path) -> Result<T, CoreError> {
    let bytes = std::fs::read(path).map_err(CoreError::io(path))?;
    serde_json::from_slice(&bytes).map_err(|e| parse_error(path, e))
}

/// Reads a TOML file the user chose; never renames it.
pub fn read_toml_strict<T: DeserializeOwned>(path: &Path) -> Result<T, CoreError> {
    let bytes = std::fs::read(path).map_err(CoreError::io(path))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| parse_error(path, e))?;
    toml::from_str(text).map_err(|e| parse_error(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct State {
        #[serde(default)]
        items: Vec<String>,
    }

    #[test]
    fn missing_file_loads_default_without_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let (state, recovered) = load_json::<State>(&dir.path().join("none.json")).unwrap();
        assert_eq!(state, State::default());
        assert!(!recovered);
    }

    #[test]
    fn save_then_load_round_trips_and_leaves_no_partial() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/state.json");
        let state = State { items: vec!["a".into(), "b".into()] };
        save_json(&path, &state).unwrap();
        assert_eq!(load_json::<State>(&path).unwrap(), (state, false));
        assert!(!with_suffix(&path, ".partial").exists());
    }

    #[test]
    fn unparseable_file_is_moved_to_bak() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, [0x7b, 0xff, 0xfe]).unwrap();
        let (state, recovered) = load_json::<State>(&path).unwrap();
        assert_eq!(state, State::default());
        assert!(recovered);
        assert!(!path.exists());
        assert_eq!(std::fs::read(with_suffix(&path, ".bak")).unwrap(), [0x7b, 0xff, 0xfe]);
    }

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Conf {
        #[serde(default)]
        name: String,
        #[serde(default)]
        size: u32,
    }

    #[test]
    fn toml_round_trips_and_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/conf.toml");
        assert_eq!(load_toml::<Conf>(&path).unwrap(), (Conf::default(), false));
        let conf = Conf { name: "x".into(), size: 3 };
        save_toml(&path, &conf).unwrap();
        assert_eq!(load_toml::<Conf>(&path).unwrap(), (conf, false));
        std::fs::write(&path, "name = [unclosed").unwrap();
        assert_eq!(load_toml::<Conf>(&path).unwrap(), (Conf::default(), true));
        assert!(with_suffix(&path, ".bak").exists() && !path.exists());
    }

    #[test]
    fn strict_readers_report_and_never_rename() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.rogg");
        std::fs::write(&path, "{ nope").unwrap();
        match read_json_strict::<State>(&path) {
            Err(CoreError::Parse { path: p, .. }) => assert_eq!(p, path),
            other => panic!("expected a parse error, got {other:?}"),
        }
        assert!(path.exists(), "a file the user opened is never renamed");
        std::fs::write(&path, "size = \"big\"").unwrap();
        assert!(matches!(read_toml_strict::<Conf>(&path), Err(CoreError::Parse { .. })));
        assert!(matches!(read_toml_strict::<Conf>(&dir.path().join("missing.toml")), Err(CoreError::Io { .. })));
    }

    #[test]
    fn write_atomic_replaces_and_leaves_no_partial() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deep/f.txt");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        assert!(!with_suffix(&path, ".partial").exists());
    }
}
