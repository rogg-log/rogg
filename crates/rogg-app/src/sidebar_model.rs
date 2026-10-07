//! The sidebar's file tree (spec §5.3): open files grouped by folder, or a fixed tree (the demo bundle), and the
//! action behind each row.

use crate::document::Source;
use crate::format::format_size;
use crate::tab::TabInit;
use gpui_kit::{EntityId, SharedString};
use rogg_ui::sidebar::{RecentItem, SidebarFooter, TreeRow};
use std::collections::HashSet;
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
        rows.push(TreeRow::Folder { label: group.label.clone(), expanded, depth: 0 });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_by_folder_and_flattens_with_actions() {
        let a = EntityId::from(1u64);
        let b = EntityId::from(2u64);
        let c = EntityId::from(3u64);
        let files = [
            OpenFile { tab: a, title: "a.log".into(), path: Some("/var/log/a.log".into()), bytes: Some(14_200), failed: false },
            OpenFile { tab: b, title: "mem".into(), path: None, bytes: None, failed: false },
            OpenFile { tab: c, title: "c.log".into(), path: Some("/var/log/c.log".into()), bytes: None, failed: true },
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
            OpenFile { tab: a, title: "x.log".into(), path: Some("/a/logs/x.log".into()), bytes: None, failed: false },
            OpenFile { tab: b, title: "y.log".into(), path: Some("/b/logs/y.log".into()), bytes: None, failed: false },
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
}
