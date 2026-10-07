//! Opening a folder (spec §4.1): a recursive scan, up to a depth, of the regular files that are text, gzip or known
//! archives. Binaries (a NUL in the first 8 KB) and hidden entries are skipped, symlinked folders are not followed
//! (no cycles), and the scan stops after `MAX_SCAN_FILES` files.

use crate::archive::{Content, sniff};
use crate::cancel::CancelToken;
use crate::error::CoreError;
use std::path::{Path, PathBuf};

/// Spec §4.1 default (settings arrive in M4): files directly in the folder are depth 1.
pub const DEFAULT_SCAN_DEPTH: usize = 4;
pub const MAX_SCAN_FILES: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedFile {
    /// Path relative to the scanned folder.
    pub rel: PathBuf,
    pub size: u64,
    /// `Text`, `Gzip` or `Archive(..)` — binaries are never listed.
    pub content: Content,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderScan {
    /// Sorted by path within each folder (files and folders interleaved by name).
    pub files: Vec<ScannedFile>,
    /// The scan stopped at the file limit.
    pub truncated: bool,
}

pub fn scan_folder(dir: &Path, depth: usize, cancel: &CancelToken) -> Result<FolderScan, CoreError> {
    scan_folder_with(dir, depth, MAX_SCAN_FILES, cancel)
}

/// `scan_folder` with an explicit file limit (tests).
pub fn scan_folder_with(dir: &Path, depth: usize, max_files: usize, cancel: &CancelToken) -> Result<FolderScan, CoreError> {
    // The folder itself must be readable; unreadable subfolders are skipped.
    std::fs::read_dir(dir).map_err(CoreError::io(dir))?;
    let mut scan = FolderScan::default();
    walk(dir, Path::new(""), 1, depth.max(1), max_files, cancel, &mut scan)?;
    Ok(scan)
}

fn walk(dir: &Path, rel: &Path, depth: usize, max_depth: usize, max_files: usize, cancel: &CancelToken, scan: &mut FolderScan) -> Result<(), CoreError> {
    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }
    let Ok(read) = std::fs::read_dir(dir) else { return Ok(()) };
    let mut entries: Vec<std::fs::DirEntry> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if scan.truncated {
            return Ok(());
        }
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else { continue };
        let path = entry.path();
        if file_type.is_dir() {
            if depth < max_depth {
                walk(&path, &rel.join(&name), depth + 1, max_depth, max_files, cancel, scan)?;
            }
            continue;
        }
        // Regular files and symlinks to regular files; a symlink to a folder is not followed.
        let Ok(meta) = std::fs::metadata(&path) else { continue };
        if !meta.is_file() {
            continue;
        }
        let Ok(content) = sniff(&path) else { continue };
        if content == Content::Binary {
            continue;
        }
        if scan.files.len() >= max_files {
            scan.truncated = true;
            return Ok(());
        }
        scan.files.push(ScannedFile { rel: rel.join(&name), size: meta.len(), content });
    }
    Ok(())
}
