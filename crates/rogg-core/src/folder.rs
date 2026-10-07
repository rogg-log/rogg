//! Opening a folder (spec §4.1): a recursive scan, up to a depth, of the regular files that are text, gzip or known
//! archives. Binaries (a NUL in the first 8 KB) and hidden entries are skipped, symlinked folders are not followed
//! (no cycles), and the scan stops after `MAX_SCAN_FILES` listed files or `MAX_SCAN_ENTRIES` examined directory entries.

use crate::archive::{Content, sniff};
use crate::cancel::CancelToken;
use crate::error::CoreError;
use std::path::{Path, PathBuf};

/// Spec §4.1 default (settings arrive in M4): files directly in the folder are depth 1.
pub const DEFAULT_SCAN_DEPTH: usize = 4;
pub const MAX_SCAN_FILES: usize = 10_000;
/// Every directory entry examined counts (files, folders, hidden and binary ones), so a folder of a million binaries
/// stops too.
pub const MAX_SCAN_ENTRIES: usize = 100_000;
/// `cancel` is polled inside a directory every this many entries.
const CANCEL_POLL_ENTRIES: usize = 256;

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
    /// The scan stopped at the file or entry limit.
    pub truncated: bool,
}

pub fn scan_folder(dir: &Path, depth: usize, cancel: &CancelToken) -> Result<FolderScan, CoreError> {
    scan_folder_with(dir, depth, MAX_SCAN_FILES, MAX_SCAN_ENTRIES, cancel)
}

/// `scan_folder` with explicit file and entry limits (tests).
pub fn scan_folder_with(dir: &Path, depth: usize, max_files: usize, max_entries: usize, cancel: &CancelToken) -> Result<FolderScan, CoreError> {
    // The folder itself must be readable; unreadable subfolders are skipped.
    std::fs::read_dir(dir).map_err(CoreError::io(dir))?;
    let mut walker = Walker { max_depth: depth.max(1), max_files, max_entries, entries: 0, cancel, scan: FolderScan::default() };
    walker.walk(dir, Path::new(""), 1)?;
    Ok(walker.scan)
}

struct Walker<'a> {
    max_depth: usize,
    max_files: usize,
    max_entries: usize,
    /// Directory entries examined so far.
    entries: usize,
    cancel: &'a CancelToken,
    scan: FolderScan,
}

impl Walker<'_> {
    fn walk(&mut self, dir: &Path, rel: &Path, depth: usize) -> Result<(), CoreError> {
    if self.cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }
    let Ok(read) = std::fs::read_dir(dir) else { return Ok(()) };
    let mut entries: Vec<std::fs::DirEntry> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if self.scan.truncated {
            return Ok(());
        }
        if self.entries >= self.max_entries {
            self.scan.truncated = true;
            return Ok(());
        }
        self.entries += 1;
        if self.entries.is_multiple_of(CANCEL_POLL_ENTRIES) && self.cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else { continue };
        let path = entry.path();
        if file_type.is_dir() {
            if depth < self.max_depth {
                self.walk(&path, &rel.join(&name), depth + 1)?;
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
        if self.scan.files.len() >= self.max_files {
            self.scan.truncated = true;
            return Ok(());
        }
        self.scan.files.push(ScannedFile { rel: rel.join(&name), size: meta.len(), content });
    }
    Ok(())
    }
}
