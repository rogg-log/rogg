//! The sidebar's file tree (spec §5.3): open files grouped by folder, or a fixed tree (the demo bundle), and the
//! action behind each row.

use crate::document::Source;
use crate::format::format_size;
use crate::tab::TabInit;
use gpui_kit::{EntityId, SharedString};
use rogg_core::source::SourceRef;
use rogg_core::stats::{group_digits, noun};
use rogg_core::tree::{Listing, NodeId, NodeKind, SourceTree};
use rogg_ui::sidebar::{RecentItem, SidebarFooter, TreeRow};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct SidebarEntry {
    pub label: SharedString,
    pub size: SharedString,
    /// The open tab showing this entry.
    pub tab: Option<EntityId>,
    /// How to open the entry when no tab shows it (fixed trees only).
    pub open: Option<(Source, TabInit)>,
}

#[derive(Clone, Debug)]
pub struct SidebarGroup {
    /// Identifies the group (collapsed state): the folder's full path, "In memory", or a fixed group's label.
    pub key: SharedString,
    pub label: SharedString,
    pub entries: Vec<SidebarEntry>,
}

/// A sidebar that does not follow the open tabs (the demo's bundle).
#[derive(Clone, Debug)]
pub struct FixedSidebar {
    pub groups: Vec<SidebarGroup>,
    pub count: SharedString,
    pub footer: SidebarFooter,
    pub recent: Vec<RecentItem>,
}

// `Open` carries a whole `TabInit`; the actions live for one render and the interface is shared with the demo (Task 13).
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum RowAction {
    /// Toggles the collapsed state of the group with this `SidebarGroup::key`.
    ToggleGroup(SharedString),
    Activate(EntityId),
    Open(Source, TabInit),
    /// Expands or collapses a folder or archive row of the source tree.
    ToggleNode(NodeId),
    /// Opens (or activates the tab of) a file or member row of the source tree.
    OpenNode(NodeId),
    Nothing,
}

/// An open tab as the sidebar sees it.
pub struct OpenFile {
    pub tab: EntityId,
    pub title: SharedString,
    pub path: Option<PathBuf>,
    /// Text size once loaded.
    pub bytes: Option<u64>,
    pub failed: bool,
    /// What the tab shows, to match it with a row of the source tree.
    pub source: Option<SourceRef>,
}

/// Groups open files by their folder (full path; the label is the folder's name), in tab order; in-memory sources go
/// under "In memory".
pub fn group_open_files(files: &[OpenFile]) -> Vec<SidebarGroup> {
    let mut groups: Vec<SidebarGroup> = Vec::new();
    for f in files {
        let (key, label): (SharedString, SharedString) = match f.path.as_ref().and_then(|p| p.parent()) {
            Some(dir) => (
                dir.display().to_string().into(),
                dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| dir.display().to_string()).into(),
            ),
            None => ("In memory".into(), "In memory".into()),
        };
        let entry = SidebarEntry { label: f.title.clone(), size: f.bytes.map(format_size).unwrap_or_default().into(), tab: Some(f.tab), open: None };
        match groups.iter_mut().find(|g| g.key == key) {
            Some(group) => group.entries.push(entry),
            None => groups.push(SidebarGroup { key, label, entries: vec![entry] }),
        }
    }
    groups
}

/// Flattens `groups` into sidebar rows and the action of each row. Collapsed groups hide their entries; the
/// entry of the `active` tab is highlighted; `failed` marks entries whose tab failed to open.
pub fn rows(groups: &[SidebarGroup], active: Option<EntityId>, collapsed: &HashSet<SharedString>) -> (Vec<TreeRow>, Vec<RowAction>) {
    rows_with_failures(groups, active, collapsed, &HashSet::new())
}

pub fn rows_with_failures(
    groups: &[SidebarGroup],
    active: Option<EntityId>,
    collapsed: &HashSet<SharedString>,
    failed: &HashSet<EntityId>,
) -> (Vec<TreeRow>, Vec<RowAction>) {
    let mut rows = Vec::new();
    let mut actions = Vec::new();
    for group in groups {
        let expanded = !collapsed.contains(&group.key);
        rows.push(TreeRow::Folder { label: group.label.clone(), expanded, depth: 0, note: None, failed: false });
        actions.push(RowAction::ToggleGroup(group.key.clone()));
        if !expanded {
            continue;
        }
        for entry in &group.entries {
            rows.push(TreeRow::File {
                label: entry.label.clone(),
                size: entry.size.clone(),
                active: entry.tab.is_some() && entry.tab == active,
                depth: 1,
                failed: entry.tab.is_some_and(|t| failed.contains(&t)),
            });
            actions.push(match (&entry.tab, &entry.open) {
                (Some(tab), _) => RowAction::Activate(*tab),
                (None, Some((source, init))) => RowAction::Open(source.clone(), init.clone()),
                (None, None) => RowAction::Nothing,
            });
        }
    }
    (rows, actions)
}

/// Rows for the tree's roots (folders and archives) and their expanded contents, with the action of each row. A file
/// or member row is active and failed like the tab showing it.
pub fn tree_rows(tree: &SourceTree, expanded: &HashSet<NodeId>, files: &[OpenFile], active: Option<EntityId>) -> (Vec<TreeRow>, Vec<RowAction>) {
    let mut rows = Vec::new();
    let mut actions = Vec::new();
    let tabs: HashMap<&SourceRef, &OpenFile> = files.iter().filter_map(|f| Some((f.source.as_ref()?, f))).collect();
    for (id, depth) in tree.visible(|id| expanded.contains(&id)) {
        let node = tree.node(id);
        if tree.is_container(id) {
            let (note, failed) = match &node.listing {
                Listing::Listing => (Some("listing…"), false),
                Listing::Failed(_) => (Some("unreadable"), true),
                Listing::Pending | Listing::Listed => (None, false),
            };
            rows.push(TreeRow::Folder { label: node.name.clone().into(), expanded: expanded.contains(&id), depth, note: note.map(SharedString::from), failed });
            actions.push(RowAction::ToggleNode(id));
        } else {
            let tab = node.source.as_ref().and_then(|s| tabs.get(s).copied());
            rows.push(TreeRow::File {
                label: node.name.clone().into(),
                size: node.size.map(format_size).unwrap_or_default().into(),
                active: tab.is_some_and(|f| Some(f.tab) == active),
                depth: depth.max(1),
                failed: tab.is_some_and(|f| f.failed),
            });
            actions.push(RowAction::OpenNode(id));
        }
    }
    (rows, actions)
}

/// The sidebar footer for a root (spec §5.3: `10 members · 8.4 GB expanded` / `Members open individually`).
pub fn tree_footer(tree: &SourceTree, root: NodeId) -> SidebarFooter {
    let node = tree.node(root);
    let sum = tree.root_summary(root);
    let lines = match node.kind {
        NodeKind::Archive(_) => vec![
            SharedString::from(format!("{} {} · {} expanded", group_digits(sum.files), noun(sum.files, "member", "members"), format_size(sum.bytes))),
            SharedString::from("Members open individually"),
        ],
        _ => vec![SharedString::from(format!("{} {} · {}", group_digits(sum.files), noun(sum.files, "file", "files"), format_size(sum.bytes)))],
    };
    SidebarFooter { title: node.name.clone().into(), lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_by_folder_and_flattens_with_actions() {
        let a = EntityId::from(1u64);
        let b = EntityId::from(2u64);
        let c = EntityId::from(3u64);
        let files = [
            OpenFile { tab: a, title: "a.log".into(), path: Some("/var/log/a.log".into()), bytes: Some(14_200), failed: false, source: None },
            OpenFile { tab: b, title: "mem".into(), path: None, bytes: None, failed: false, source: None },
            OpenFile { tab: c, title: "c.log".into(), path: Some("/var/log/c.log".into()), bytes: None, failed: true, source: None },
        ];
        let groups = group_open_files(&files);
        assert_eq!(groups.iter().map(|g| g.label.as_ref()).collect::<Vec<_>>(), ["log", "In memory"]);
        assert_eq!(groups[0].entries[0].size.as_ref(), "14 KB");
        let failed = HashSet::from([c]);
        let (rows, actions) = rows_with_failures(&groups, Some(c), &HashSet::new(), &failed);
        assert_eq!(rows.len(), 5);
        assert!(matches!(&rows[2], TreeRow::File { active: true, failed: true, .. }));
        assert!(matches!(actions[1], RowAction::Activate(t) if t == a));
        let collapsed = HashSet::from([SharedString::from("/var/log")]);
        let (rows, _) = super::rows(&groups, None, &collapsed);
        assert_eq!(rows.len(), 3, "collapsed folder hides its two files");
    }

    #[test]
    fn same_named_folders_in_different_places_are_separate_groups() {
        let (a, b) = (EntityId::from(1u64), EntityId::from(2u64));
        let files = [
            OpenFile { tab: a, title: "x.log".into(), path: Some("/a/logs/x.log".into()), bytes: None, failed: false, source: None },
            OpenFile { tab: b, title: "y.log".into(), path: Some("/b/logs/y.log".into()), bytes: None, failed: false, source: None },
        ];
        let groups = group_open_files(&files);
        assert_eq!(groups.iter().map(|g| g.label.as_ref()).collect::<Vec<_>>(), ["logs", "logs"]);
        assert_eq!(groups.iter().map(|g| g.key.as_ref()).collect::<Vec<_>>(), ["/a/logs", "/b/logs"]);
        let collapsed = HashSet::from([groups[0].key.clone()]);
        let (rows, actions) = super::rows(&groups, None, &collapsed);
        assert_eq!(rows.len(), 3, "collapsing one `logs` folder leaves the other open");
        assert!(matches!(&actions[0], RowAction::ToggleGroup(k) if k.as_ref() == "/a/logs"));
        assert!(matches!(&actions[2], RowAction::Activate(t) if *t == b));
    }

    #[test]
    fn tree_rows_follow_listing_states_and_tabs() {
        use rogg_core::archive::{ArchiveEntry, ArchiveKind};
        let mut tree = SourceTree::new();
        let zip = tree.add_root(std::path::Path::new("/b/x.zip"), NodeKind::Archive(ArchiveKind::Zip), Some(10));
        tree.set_listing(zip, Listing::Listing);
        let expanded = HashSet::from([zip]);
        let (rows, _) = tree_rows(&tree, &expanded, &[], None);
        assert!(matches!(&rows[0], TreeRow::Folder { note: Some(n), failed: false, .. } if n.as_ref() == "listing…"));
        tree.insert_listing(zip, &[ArchiveEntry { path: "a.log".into(), size: 1200 }]);
        let tab = EntityId::from(7u64);
        let a = SourceRef::member(SourceRef::File("/b/x.zip".into()), "a.log");
        let files = [OpenFile { tab, title: "a.log".into(), path: None, bytes: None, failed: true, source: Some(a) }];
        let (rows, actions) = tree_rows(&tree, &expanded, &files, Some(tab));
        assert!(matches!(&rows[1], TreeRow::File { active: true, failed: true, depth: 1, .. }));
        assert!(matches!(actions[1], RowAction::OpenNode(_)));
        tree.set_listing(zip, Listing::Failed("damaged".into()));
        let (rows, _) = tree_rows(&tree, &expanded, &files, None);
        assert!(matches!(&rows[0], TreeRow::Folder { failed: true, .. }));
        let footer = tree_footer(&tree, zip);
        assert_eq!(footer.lines[0].as_ref(), "1 member · 1 KB expanded");
    }
}
