//! Tab management, the sidebar, recent files, open prompts, drag-and-drop and the split handles.

use super::{Workspace, act_ix};
use crate::document::{LoadState, Source};
use crate::history::HistoryStore;
use crate::menus;
use crate::recent::RecentStore;
use crate::sidebar_model::{self, FixedSidebar, OpenFile, RowAction, SidebarEntry, SidebarGroup};
use crate::tab::TabInit;
use gpui_kit::{AnyElement, App, BorrowAppContext, Context, Entity, EntityId, IntoElement, PathPromptOptions, Pixels, SharedString, Window, px};
use rogg_core::encoding::Encoding;
use rogg_core::recent::RecentEntry;
use rogg_core::stats::{group_digits, noun};
use rogg_ui::sidebar::{RecentItem, Sidebar, SidebarFooter};
use rogg_ui::theme::size;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Open Folder (M2) opens at most this many files: the regular, non-hidden text files directly inside the folder.
/// Recursive folder trees and archives arrive with folder sources (M3).
pub const FOLDER_FILE_LIMIT: usize = 50;

/// The regular, non-hidden files directly in `dir` that look like text (no NUL byte in the first 8 KB, unless the
/// file starts with a UTF-16 BOM), sorted by name, at most `FOLDER_FILE_LIMIT`.
pub fn folder_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') || !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if looks_like_text(&path) {
            files.push(path);
        }
    }
    files.sort();
    files.truncate(FOLDER_FILE_LIMIT);
    Ok(files)
}

fn looks_like_text(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let mut head = [0u8; 8192];
    let mut len = 0;
    while len < head.len() {
        match file.read(&mut head[len..]) {
            Ok(0) => break,
            Ok(n) => len += n,
            Err(_) => return false,
        }
    }
    Encoding::detect(&head[..len]).is_utf16() || !head[..len].contains(&0)
}

pub(crate) fn recent_items(cx: &App) -> Vec<RecentItem> {
    cx.try_global::<RecentStore>()
        .map(|s| s.entries().iter().map(|e| RecentItem { label: e.label().into(), is_folder: e.is_folder, missing: e.is_missing() }).collect())
        .unwrap_or_default()
}

impl Workspace {
    /// Opens each path: folders through `open_folder`, files in a tab or by activating the tab already showing them.
    /// Files that exist are recorded in Recent.
    pub fn open_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        for path in paths {
            let path = path.canonicalize().unwrap_or(path);
            if path.is_dir() {
                self.open_folder(path, window, cx);
            } else {
                if path.exists() {
                    self.record_recent(RecentEntry::file(&path), cx);
                }
                self.open_file(path, window, cx);
            }
        }
    }

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let existing = self.tabs.iter().position(|t| t.read(cx).doc.read(cx).source.path() == Some(path.as_path()));
        match existing {
            Some(index) => self.activate(index, window, cx),
            None => {
                self.open_source(Source::File(path), TabInit::default(), window, cx);
            }
        }
    }

    pub fn open_folder(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match folder_files(&dir) {
            Ok(files) if files.is_empty() => self.toast(format!("No text files in {}", dir.display()), cx),
            Ok(files) => {
                self.record_recent(RecentEntry::folder(&dir), cx);
                for file in files {
                    self.open_file(file, window, cx);
                }
            }
            Err(e) => self.toast(format!("Could not read {}: {e}", dir.display()), cx),
        }
    }

    /// ⌘O / ⇧⌘O: the native open panel.
    pub(crate) fn prompt_open(&mut self, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: !folder, directories: folder, multiple: !folder, prompt: Some("Open".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                this.update_in(cx, |ws, window, cx| ws.open_paths(paths, window, cx)).ok();
            }
        })
        .detach();
    }

    pub fn open_recent(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.fixed_sidebar.is_some() {
            self.toast("Recent entries are examples in the demo", cx);
            return;
        }
        let Some(entry) = cx.try_global::<RecentStore>().and_then(|s| s.entries().get(index).cloned()) else { return };
        if entry.is_missing() {
            cx.update_global::<RecentStore, _>(|s, _| s.remove(&entry.path));
            self.refresh_menus(cx);
            self.toast(format!("{} no longer exists and was removed from Recent", entry.label()), cx);
        } else {
            self.open_paths(vec![entry.path], window, cx);
        }
    }

    fn record_recent(&mut self, entry: RecentEntry, cx: &mut Context<Self>) {
        if cx.has_global::<RecentStore>() {
            cx.update_global::<RecentStore, _>(|s, _| s.record(entry));
            self.refresh_menus(cx);
        }
    }

    /// Rebuilds the menu bar so File → Open Recent ▸ lists the current entries.
    pub fn refresh_menus(&self, cx: &mut App) {
        let entries = cx.try_global::<RecentStore>().map(|s| s.entries().to_vec()).unwrap_or_default();
        cx.set_menus(menus::app_menus(&entries));
    }

    /// Closes a tab; closing the active tab activates the tab now at its index (reference `closeTab`).
    pub fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(index);
        self.tab_subscriptions.remove(&tab.entity_id());
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        if self.tabs.is_empty() {
            window.set_window_title("rogg");
        } else {
            self.activate(self.active, window, cx);
        }
        cx.notify();
    }

    pub fn cycle_tab(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.tabs.len();
        if n == 0 {
            return;
        }
        let next = if forward { (self.active + 1) % n } else { (self.active + n - 1) % n };
        self.activate(next, window, cx);
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    /// Band handle drag: puts the band's top edge at window `y`.
    pub fn drag_band(&mut self, y: Pixels, cx: &mut Context<Self>) {
        let (Some(split), Some(tab)) = (self.measure.split_at(y), self.active_tab()) else { return };
        tab.update(cx, |t, cx| {
            t.split = split;
            cx.notify();
        });
    }

    /// Sidebar edge drag: the sidebar starts at the window's left edge, so its width is the pointer's x.
    pub fn drag_sidebar(&mut self, x: Pixels, cx: &mut Context<Self>) {
        self.sidebar_width = x.clamp(px(size::SIDEBAR_MIN_W), px(size::SIDEBAR_MAX_W));
        cx.notify();
    }

    /// History dialog pick: restores the entry's terms, states, flags and context on the active tab.
    pub fn restore_history(&mut self, index: usize, cx: &mut Context<Self>) {
        let spec = cx.try_global::<HistoryStore>().and_then(|h| h.entries().get(index)).map(|e| e.to_spec());
        if let (Some(spec), Some(tab)) = (spec, self.active_tab()) {
            let doc = tab.read(cx).doc.clone();
            let _ = doc.update(cx, |d, cx| d.apply_spec(spec, cx));
        }
        self.close_dialog(cx);
    }

    pub fn set_fixed_sidebar(&mut self, sidebar: FixedSidebar, cx: &mut Context<Self>) {
        self.fixed_sidebar = Some(sidebar);
        cx.notify();
    }

    fn open_files(&self, cx: &App) -> Vec<OpenFile> {
        self.tabs
            .iter()
            .map(|tab| {
                let t = tab.read(cx);
                let d = t.doc.read(cx);
                OpenFile {
                    tab: tab.entity_id(),
                    title: d.title(),
                    path: d.source.path().map(Path::to_path_buf),
                    bytes: d.text().map(|text| text.bytes().len() as u64),
                    failed: matches!(d.load, LoadState::Failed { .. }),
                }
            })
            .collect()
    }

    fn sidebar_groups(&self, files: &[OpenFile]) -> Vec<SidebarGroup> {
        match &self.fixed_sidebar {
            Some(fixed) => fixed
                .groups
                .iter()
                .map(|g| SidebarGroup {
                    label: g.label.clone(),
                    entries: g.entries.iter().map(|e| SidebarEntry { tab: files.iter().find(|f| f.title == e.label).map(|f| f.tab), ..e.clone() }).collect(),
                })
                .collect(),
            None => sidebar_model::group_open_files(files),
        }
    }

    fn active_footer(&self, cx: &App) -> Option<SidebarFooter> {
        let tab = self.active_tab()?;
        let d = tab.read(cx).doc.read(cx);
        let mut lines: Vec<SharedString> = vec![d.source.path().and_then(Path::parent).map(|p| p.display().to_string()).unwrap_or_else(|| "In memory".into()).into()];
        if let Some(text) = d.text() {
            let n = text.line_count();
            lines.push(format!("{} {} · {}", group_digits(n), noun(n, "line", "lines"), crate::format::format_size(text.bytes().len() as u64)).into());
        }
        Some(SidebarFooter { title: d.title(), lines })
    }

    pub(crate) fn sidebar(&self, this: &Entity<Self>, cx: &App) -> Option<AnyElement> {
        if !self.sidebar_visible {
            return None;
        }
        let files = self.open_files(cx);
        let groups = self.sidebar_groups(&files);
        let failed: HashSet<EntityId> = files.iter().filter(|f| f.failed).map(|f| f.tab).collect();
        let active = self.active_tab().map(|t| t.entity_id());
        let (rows, actions) = sidebar_model::rows_with_failures(&groups, active, &self.collapsed, &failed);
        let file_count: u64 = groups.iter().map(|g| g.entries.len() as u64).sum();
        let (count, footer, recent) = match &self.fixed_sidebar {
            Some(fixed) => (fixed.count.clone(), Some(fixed.footer.clone()), fixed.recent.clone()),
            None => (format!("{file_count} {}", noun(file_count, "file", "files")).into(), self.active_footer(cx), recent_items(cx)),
        };
        let actions = Rc::new(actions);
        Some(
            Sidebar::new()
                .width(self.sidebar_width)
                .count(count)
                .rows(rows)
                .recent(recent)
                .footer(footer)
                .on_row(act_ix(this, move |ws, i, window, cx| {
                    if let Some(action) = actions.get(i).cloned() {
                        ws.sidebar_action(action, window, cx);
                    }
                }))
                .on_recent(act_ix(this, |ws, i, window, cx| ws.open_recent(i, window, cx)))
                .into_any_element(),
        )
    }

    fn sidebar_action(&mut self, action: RowAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            RowAction::ToggleGroup(label) => {
                if !self.collapsed.remove(&label) {
                    self.collapsed.insert(label);
                }
                cx.notify();
            }
            RowAction::Activate(id) => {
                if let Some(index) = self.tabs.iter().position(|t| t.entity_id() == id) {
                    self.activate(index, window, cx);
                }
            }
            RowAction::Open(source, init) => {
                self.open_source(source, init, window, cx);
            }
            RowAction::Nothing => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_files_skips_hidden_binary_and_nested() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.log"), "text").unwrap();
        std::fs::write(dir.path().join("a.log"), "text").unwrap();
        std::fs::write(dir.path().join(".hidden"), "text").unwrap();
        std::fs::write(dir.path().join("image.png"), [137, 80, 78, 71, 0, 0]).unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        let names: Vec<String> = folder_files(dir.path()).unwrap().iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["a.log", "b.log"]);
    }
}
