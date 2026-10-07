//! What a tab shows and where it comes from (spec §4.1): a file, or a member of an archive (possibly nested).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceRef {
    File(PathBuf),
    /// `path` (`/`-separated) inside the archive `archive`, which is itself a file or a member.
    Member { archive: Box<SourceRef>, path: String },
}

impl SourceRef {
    pub fn member(archive: SourceRef, path: impl Into<String>) -> SourceRef {
        SourceRef::Member { archive: Box::new(archive), path: path.into() }
    }

    /// The file on disk everything comes from (the outermost archive of a member).
    pub fn root_path(&self) -> &Path {
        match self {
            SourceRef::File(path) => path,
            SourceRef::Member { archive, .. } => archive.root_path(),
        }
    }

    /// Member paths from the outermost archive inwards; empty for a file.
    pub fn member_chain(&self) -> Vec<&str> {
        match self {
            SourceRef::File(_) => Vec::new(),
            SourceRef::Member { archive, path } => {
                let mut chain = archive.member_chain();
                chain.push(path.as_str());
                chain
            }
        }
    }

    /// The file's or member's own name.
    pub fn name(&self) -> String {
        match self {
            SourceRef::File(path) => path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string()),
            SourceRef::Member { path, .. } => path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(path.as_str()).to_string(),
        }
    }

    /// The info-bar breadcrumb (spec §5.3): a file's folder and name; for a member, the outer archive's name, each
    /// nested archive and folder, and the member's name (`incident.zip › logs › api.log`).
    pub fn crumbs(&self) -> Vec<String> {
        match self {
            SourceRef::File(path) => {
                let mut crumbs: Vec<String> = path.parent().and_then(Path::file_name).map(|d| d.to_string_lossy().into_owned()).into_iter().collect();
                crumbs.push(self.name());
                crumbs
            }
            SourceRef::Member { archive, path } => {
                let mut crumbs = match archive.as_ref() {
                    SourceRef::File(_) => vec![archive.name()],
                    member => member.crumbs(),
                };
                crumbs.extend(path.split('/').filter(|s| !s.is_empty()).map(str::to_string));
                crumbs
            }
        }
    }

    /// Tab tooltip: a file's full path; a member as `api.log · incident.zip` (the reference's tab title).
    pub fn tooltip(&self) -> String {
        match self {
            SourceRef::File(path) => path.display().to_string(),
            SourceRef::Member { .. } => {
                let root = self.root_path().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                format!("{} · {root}", self.name())
            }
        }
    }
}
