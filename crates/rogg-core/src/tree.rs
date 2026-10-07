//! The sidebar's tree of opened folders and archives (spec §4.1 `SourceTree`): folder, archive, member and file
//! nodes with sizes. Archives are listed lazily (on expand); nodes are never removed, so ids stay valid.

use crate::archive::{ArchiveEntry, ArchiveKind, Content, kind_from_name};
use crate::folder::FolderScan;
use crate::source::SourceRef;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub type NodeId = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Folder,
    Archive(ArchiveKind),
    /// A file inside an archive.
    Member,
    /// A file on disk (plain or `.gz`).
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listing {
    /// Not listed yet (archives until first expanded, a folder until its scan finishes).
    Pending,
    Listing,
    Listed,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub kind: NodeKind,
    /// Size on disk (files, archives) or uncompressed size (members); None for folders.
    pub size: Option<u64>,
    /// What opening (files, members) or listing (archives) this node uses; roots also carry their path. None for
    /// folders inside a root.
    pub source: Option<SourceRef>,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub listing: Listing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeSummary {
    /// Openable files and members.
    pub files: u64,
    /// Their sizes (uncompressed for members).
    pub bytes: u64,
}

#[derive(Default)]
pub struct SourceTree {
    nodes: Vec<Node>,
    roots: Vec<NodeId>,
    by_source: HashMap<SourceRef, NodeId>,
}

impl SourceTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn is_container(&self, id: NodeId) -> bool {
        matches!(self.nodes[id].kind, NodeKind::Folder | NodeKind::Archive(_))
    }

    pub fn is_openable(&self, id: NodeId) -> bool {
        matches!(self.nodes[id].kind, NodeKind::Member | NodeKind::File)
    }

    fn push(&mut self, node: Node) -> NodeId {
        let id = self.nodes.len();
        if let Some(parent) = node.parent {
            self.nodes[parent].children.push(id);
        }
        if let Some(source) = &node.source {
            self.by_source.entry(source.clone()).or_insert(id);
        }
        self.nodes.push(node);
        id
    }

    /// Adds an opened folder or archive at the top level, `Pending` until it is scanned or listed.
    pub fn add_root(&mut self, path: &Path, kind: NodeKind, size: Option<u64>) -> NodeId {
        let source = SourceRef::File(path.to_path_buf());
        let id = self.push(Node { name: source.name(), kind, size, source: Some(source), parent: None, children: Vec::new(), listing: Listing::Pending });
        self.roots.push(id);
        id
    }

    /// The root opened from `path`, if any.
    pub fn root_for(&self, path: &Path) -> Option<NodeId> {
        self.roots.iter().copied().find(|&r| matches!(&self.nodes[r].source, Some(SourceRef::File(p)) if p == path))
    }

    pub fn set_listing(&mut self, id: NodeId, listing: Listing) {
        self.nodes[id].listing = listing;
    }

    /// The child folder `name` of `parent`, created when missing. `index` remembers the folders found or created
    /// during one insert call, so wide listings do not rescan a parent's children for every entry.
    fn folder_child(&mut self, index: &mut FolderIndex, parent: NodeId, name: &str) -> NodeId {
        if index.seeded.insert(parent) {
            for &c in &self.nodes[parent].children {
                if self.nodes[c].kind == NodeKind::Folder {
                    index.folders.entry((parent, self.nodes[c].name.clone())).or_insert(c);
                }
            }
        }
        if let Some(&id) = index.folders.get(&(parent, name.to_string())) {
            return id;
        }
        let id = self.push(Node { name: name.to_string(), kind: NodeKind::Folder, size: None, source: None, parent: Some(parent), children: Vec::new(), listing: Listing::Listed });
        index.seeded.insert(id);
        index.folders.insert((parent, name.to_string()), id);
        id
    }

    /// Sorts `id`'s subtree: folders first, then by name (case-insensitive). Iterative: member paths can nest
    /// arbitrarily deep.
    fn sort(&mut self, id: NodeId) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            let mut children = std::mem::take(&mut self.nodes[id].children);
            children.sort_by_cached_key(|&c| (self.nodes[c].kind != NodeKind::Folder, self.nodes[c].name.to_lowercase()));
            stack.extend(children.iter().copied().filter(|&c| !self.nodes[c].children.is_empty()));
            self.nodes[id].children = children;
        }
    }

    /// Fills a folder root from its scan: subfolders, files (plain and `.gz`) and archives (`Pending`).
    pub fn insert_folder(&mut self, root: NodeId, scan: &FolderScan) {
        let Some(SourceRef::File(base)) = self.nodes[root].source.clone() else { return };
        let mut index = FolderIndex::default();
        for file in &scan.files {
            let parts: Vec<String> = file.rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            let Some((leaf, dirs)) = parts.split_last() else { continue };
            let mut parent = root;
            for dir in dirs {
                parent = self.folder_child(&mut index, parent, dir);
            }
            let (kind, listing) = match file.content {
                Content::Archive(kind) => (NodeKind::Archive(kind), Listing::Pending),
                _ => (NodeKind::File, Listing::Listed),
            };
            let source = SourceRef::File(base.join(&file.rel));
            self.push(Node { name: leaf.clone(), kind, size: Some(file.size), source: Some(source), parent: Some(parent), children: Vec::new(), listing });
        }
        self.sort(root);
        self.nodes[root].listing = Listing::Listed;
    }

    /// Fills an archive from its listing: folders from the member paths, members, and nested archives (`Pending`).
    pub fn insert_listing(&mut self, archive: NodeId, entries: &[ArchiveEntry]) {
        let Some(archive_source) = self.nodes[archive].source.clone() else { return };
        let mut index = FolderIndex::default();
        for entry in entries {
            let parts: Vec<&str> = entry.path.split('/').filter(|p| !p.is_empty()).collect();
            let Some((leaf, dirs)) = parts.split_last() else { continue };
            let mut parent = archive;
            for dir in dirs {
                parent = self.folder_child(&mut index, parent, dir);
            }
            let (kind, listing) = match kind_from_name(leaf) {
                Some(kind) => (NodeKind::Archive(kind), Listing::Pending),
                None => (NodeKind::Member, Listing::Listed),
            };
            let source = SourceRef::member(archive_source.clone(), entry.path.clone());
            self.push(Node { name: leaf.to_string(), kind, size: Some(entry.size), source: Some(source), parent: Some(parent), children: Vec::new(), listing });
        }
        self.sort(archive);
        self.nodes[archive].listing = Listing::Listed;
    }

    /// Rows in display order with their depth: every root, and the children of each container `expanded` accepts.
    pub fn visible(&self, expanded: impl Fn(NodeId) -> bool) -> Vec<(NodeId, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(NodeId, usize)> = self.roots.iter().rev().map(|&r| (r, 0)).collect();
        while let Some((id, depth)) = stack.pop() {
            out.push((id, depth));
            if self.is_container(id) && expanded(id) {
                stack.extend(self.nodes[id].children.iter().rev().map(|&c| (c, depth + 1)));
            }
        }
        out
    }

    /// The openable node showing `source`.
    pub fn find(&self, source: &SourceRef) -> Option<NodeId> {
        self.by_source.get(source).copied().filter(|&id| self.is_openable(id))
    }

    /// The file or member to open first under `id`: its first direct file, else the first one found in its folders and
    /// archives (in display order). Iterative: member paths can nest arbitrarily deep.
    pub fn first_openable(&self, id: NodeId) -> Option<NodeId> {
        let mut stack = vec![id];
        while let Some(node) = stack.pop() {
            let children = &self.nodes[node].children;
            if let Some(&found) = children.iter().find(|&&c| self.is_openable(c)) {
                return Some(found);
            }
            stack.extend(children.iter().rev().copied());
        }
        None
    }

    pub fn root_of(&self, mut id: NodeId) -> NodeId {
        while let Some(parent) = self.nodes[id].parent {
            id = parent;
        }
        id
    }

    /// Openable files under `id` and their total size (spec §5.3 footer `10 members · 8.4 GB expanded`).
    pub fn summary(&self, id: NodeId) -> TreeSummary {
        let mut total = TreeSummary::default();
        let mut stack = vec![id];
        while let Some(node) = stack.pop() {
            for &c in &self.nodes[node].children {
                if self.is_openable(c) {
                    total.files += 1;
                    total.bytes += self.nodes[c].size.unwrap_or(0);
                } else {
                    stack.push(c);
                }
            }
        }
        total
    }
}

/// Folders found or created during one `insert_folder`/`insert_listing` call, keyed by parent and name.
#[derive(Default)]
struct FolderIndex {
    folders: HashMap<(NodeId, String), NodeId>,
    /// Parents whose existing folder children are already in `folders`.
    seeded: HashSet<NodeId>,
}
