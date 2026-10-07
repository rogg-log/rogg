//! Small JSON state files: load with corruption recovery and save atomically (spec §6.6).

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

/// Reads `path` as JSON. A missing file yields `T::default()`. An unparseable file (invalid JSON or invalid
/// UTF-8) is renamed to `<file>.bak` and yields `T::default()` with `recovered = true`; other I/O errors are
/// returned.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> Result<(T, bool), CoreError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((T::default(), false)),
        Err(e) => return Err(CoreError::Io { path: path.to_path_buf(), source: e }),
    };
    match serde_json::from_slice::<T>(&bytes) {
        Ok(value) => Ok((value, false)),
        Err(_) => {
            std::fs::rename(path, with_suffix(path, ".bak")).map_err(CoreError::io(path))?;
            Ok((T::default(), true))
        }
    }
}

/// Writes `value` as pretty JSON to `<file>.partial`, then renames it over `path`; creates the parent directory.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(CoreError::io(dir))?;
    }
    // Not every state serializes: a path that is not valid UTF-8 has no JSON form.
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| CoreError::Io { path: path.to_path_buf(), source: std::io::Error::new(std::io::ErrorKind::InvalidData, e) })?;
    let partial = with_suffix(path, ".partial");
    std::fs::write(&partial, json).map_err(CoreError::io(&partial))?;
    std::fs::rename(&partial, path).map_err(CoreError::io(path))
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
}
