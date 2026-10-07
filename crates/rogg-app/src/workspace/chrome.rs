//! Tab management, the sidebar, recent files, open prompts, drag-and-drop and the split handles.

use super::{Workspace, act_ix};
use crate::cache::CacheStore;
use crate::document::{LoadState, POLL_INTERVAL, Source};
use crate::history::HistoryStore;
use crate::menus;
use crate::recent::RecentStore;
use crate::sidebar_model::{self, FixedSidebar, OpenFile, RowAction, SidebarEntry, SidebarGroup};
use crate::sources::{TreeJob, TreeResult};
use crate::tab::{Tab, TabInit};
use gpui_kit::{AnyElement, App, BorrowAppContext, Context, Entity, EntityId, IntoElement, PathPromptOptions, Pixels, SharedString, Window, px};
use rogg_core::archive::{ArchiveKind, Content, sniff};
use rogg_core::folder::MAX_SCAN_FILES;
use rogg_core::recent::RecentEntry;
use rogg_core::source::SourceRef;
use rogg_core::stats::{group_digits, noun};
use rogg_core::tree::{Listing, NodeId, NodeKind};
use rogg_ui::sidebar::{RecentItem, Sidebar, SidebarFooter};
use rogg_ui::theme::size;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The Recent list as the sidebar and welcome view show it. Reads only the store's cached missing flags, so a
/// render never touches the filesystem.
pub fn recent_items(cx: &App) -> Vec<RecentItem> {
    cx.try_global::<RecentStore>()
        .map(|s| {
            s.entries()
                .iter()
                .enumerate()
                .map(|(i, e)| RecentItem { label: e.label().into(), is_folder: e.is_folder, missing: s.is_missing(i) })
                .collect()
        })
        .unwrap_or_default()
}

/// How often unfinished stream captures are checked for errors.
const STREAM_POLL: std::time::Duration = std::time::Duration::from_millis(500);

/// What an opened or dropped path holds.
enum PathKind {
    Folder,
    Archive(ArchiveKind, u64),
    File,
    /// A FIFO or socket: read as a stream, never mapped or sniffed.
    Stream,
}

fn classify(path: &Path) -> PathKind {
    let Ok(meta) = std::fs::metadata(path) else { return PathKind::File };
    if meta.is_dir() {
        return PathKind::Folder;
    }
    // Before anything opens the file: opening a FIFO blocks until a writer appears.
    if rogg_core::stream::is_stream_path(path) {
        return PathKind::Stream;
    }
    match sniff(path) {
        Ok(Content::Archive(kind)) => PathKind::Archive(kind, meta.len()),
        _ => PathKind::File,
    }
}

/// Two spellings of one file: equal paths, or equal canonical paths once both exist.
fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

impl Workspace {
    /// Opens each path: folders and archives as sidebar roots (spec §4.1), files in a tab (or by activating the tab
    /// already showing them). Opened files, folders and archives are recorded in Recent.
    pub fn open_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        for path in paths {
            if path.as_os_str() == "-" {
                self.open_stdin(window, cx);
                continue;
            }
            let path = path.canonicalize().unwrap_or(path);
            if path.extension().is_some_and(|e| e == rogg_core::session::ROGG_EXT) {
                self.open_rogg(&path, window, cx);
                continue;
            }
            match classify(&path) {
                PathKind::Stream => self.open_fifo(path, window, cx),
                PathKind::Folder => self.open_folder(path, window, cx),
                PathKind::Archive(kind, size) => self.open_archive(path, kind, size, true, window, cx),
                PathKind::File => {
                    if path.exists() {
                        self.record_recent(RecentEntry::file(&path), cx);
                    }
                    self.open_file(path, window, cx);
                }
            }
        }
    }

    /// stdin (spec §4.1): captured into the cache as it arrives, in a live tab. Opened once per run.
    pub fn open_stdin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.stdin_opened, true) {
            return;
        }
        self.open_stream("stdin", || Ok(std::io::stdin()), window, cx);
    }

    /// A FIFO or socket (`mkfifo`, `<(cmd)`): read as a stream, never mapped. Opening it waits for a writer, which
    /// happens on the capture thread.
    pub fn open_fifo(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "pipe".into());
        self.open_stream(&name, move || std::fs::File::open(&path), window, cx);
    }

    /// Starts copying `open`'s data into the cache and opens a live tab on the copy. Streams are not recent files.
    pub fn open_stream<R: std::io::Read + Send + 'static>(
        &mut self,
        name: &str,
        open: impl FnOnce() -> std::io::Result<R> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<Tab>> {
        match rogg_core::stream::StreamCapture::start(open, &CacheStore::get(cx)) {
            Ok(capture) => {
                let source = Source::Stream { name: name.to_string().into(), path: capture.path().to_path_buf() };
                self.streams.push((name.to_string().into(), capture));
                self.ensure_stream_polling(window, cx);
                Some(self.open_source(source, TabInit::from_settings(&crate::settings::SettingsStore::get(cx)), window, cx))
            }
            Err(e) => {
                self.toast(format!("Could not read {name}: {e}"), cx);
                None
            }
        }
    }

    /// The capture files of the streams still being read (tests).
    pub fn stream_paths(&self) -> Vec<PathBuf> {
        self.streams.iter().map(|(_, capture)| capture.path().to_path_buf()).collect()
    }

    /// While any capture is unfinished, checks every 500 ms for ones that ended.
    fn ensure_stream_polling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.streams_polling {
            return;
        }
        self.streams_polling = true;
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(STREAM_POLL).await;
                match this.update_in(cx, |ws, _, cx| ws.poll_streams(cx)) {
                    Ok(true) => continue,
                    _ => break,
                }
            }
        })
        .detach();
    }

    /// Toasts the failure of each capture that has just finished (once: finished captures are dropped, the tab keeps
    /// what was captured); returns whether any capture is still running.
    fn poll_streams(&mut self, cx: &mut Context<Self>) -> bool {
        let mut failures = Vec::new();
        self.streams.retain(|(name, capture)| {
            if !capture.is_done() {
                return true;
            }
            if let Some(error) = capture.error() {
                failures.push(format!("Could not read {name}: {error}"));
            }
            false
        });
        for message in failures {
            self.toast(message, cx);
        }
        self.streams_polling = !self.streams.is_empty();
        self.streams_polling
    }

    fn open_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let existing = self.tabs.iter().position(|t| t.read(cx).doc.read(cx).source.path().is_some_and(|p| same_file(p, &path)));
        match existing {
            Some(index) => self.activate(index, window, cx),
            None => {
                self.open_source(Source::File(path), TabInit::from_settings(&crate::settings::SettingsStore::get(cx)), window, cx);
            }
        }
    }

    /// A folder becomes a tree root, scanned on a background thread; its first log opens when the scan arrives.
    pub fn open_folder(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.record_recent(RecentEntry::folder(&dir), cx);
        if let Some(root) = self.sources.tree.root_for(&dir) {
            self.sources.expanded.insert(root);
            cx.notify();
            return;
        }
        let root = self.sources.tree.add_root(&dir, NodeKind::Folder, None);
        self.sources.tree.set_listing(root, Listing::Listing);
        self.sources.expanded.insert(root);
        self.sources.open_first.insert(root);
        self.sources.jobs.push(TreeJob::scan(root, dir, crate::settings::SettingsStore::get(cx).behavior.scan_depth as usize));
        self.ensure_tree_polling(window, cx);
        cx.notify();
    }

    /// An archive becomes a tree root, listed on a background thread; with `open_first`, its first log opens when the
    /// listing arrives.
    pub fn open_archive(&mut self, path: PathBuf, kind: ArchiveKind, size: u64, open_first: bool, window: &mut Window, cx: &mut Context<Self>) {
        if open_first {
            self.record_recent(RecentEntry::file(&path), cx);
        }
        if let Some(root) = self.sources.tree.root_for(&path) {
            self.sources.expanded.insert(root);
            cx.notify();
            return;
        }
        let root = self.sources.tree.add_root(&path, NodeKind::Archive(kind), Some(size));
        self.sources.expanded.insert(root);
        if open_first {
            self.sources.open_first.insert(root);
        }
        self.start_listing(root, window, cx);
        cx.notify();
    }

    /// Puts the outermost archive of a restored member in the sidebar (listed, not opening its first member).
    pub(crate) fn show_archive_of(&mut self, source: &SourceRef, window: &mut Window, cx: &mut Context<Self>) {
        let root = source.root_path().to_path_buf();
        if let PathKind::Archive(kind, size) = classify(&root) {
            self.open_archive(root, kind, size, false, window, cx);
        }
    }

    fn start_listing(&mut self, node: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.sources.tree.node(node).source.clone() else { return };
        self.sources.tree.set_listing(node, Listing::Listing);
        self.sources.jobs.push(TreeJob::list(node, source, CacheStore::get(cx)));
        self.ensure_tree_polling(window, cx);
    }

    fn ensure_tree_polling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sources.polling {
            return;
        }
        self.sources.polling = true;
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                match this.update_in(cx, |ws, window, cx| ws.poll_tree(window, cx)) {
                    Ok(true) => continue,
                    _ => break,
                }
            }
        })
        .detach();
    }

    /// Applies finished scans and listings; returns whether any job is still running.
    fn poll_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut finished = Vec::new();
        self.sources.jobs.retain(|job| match job.poll() {
            Some(result) => {
                finished.push((job.node, result));
                false
            }
            None => true,
        });
        let finished_any = !finished.is_empty();
        for (node, result) in finished {
            let name = self.sources.tree.node(node).name.clone();
            match result {
                TreeResult::Scanned(Ok(scan)) => {
                    self.sources.tree.insert_folder(node, &scan);
                    if scan.files.is_empty() {
                        self.toast(format!("No log files in {name}"), cx);
                    }
                    if scan.truncated {
                        self.toast(format!("{name}: showing the first {} files", group_digits(MAX_SCAN_FILES as u64)), cx);
                    }
                }
                TreeResult::Listed(Ok(entries)) => self.sources.tree.insert_listing(node, &entries),
                TreeResult::Scanned(Err(e)) | TreeResult::Listed(Err(e)) => {
                    self.sources.tree.set_listing(node, Listing::Failed(e.to_string()));
                    self.toast(format!("Could not read {name}: {e}"), cx);
                }
                TreeResult::Died => {
                    self.sources.tree.set_listing(node, Listing::Failed("stopped unexpectedly".into()));
                    self.toast(format!("Could not read {name}: stopped unexpectedly"), cx);
                }
            }
            if self.sources.open_first.remove(&node)
                && let Some(first) = self.sources.tree.first_openable(node)
            {
                self.open_node(first, false, window, cx);
            }
        }
        if finished_any {
            cx.notify();
        }
        let active = !self.sources.jobs.is_empty();
        if !active {
            self.sources.polling = false;
        }
        active
    }

    /// The tab showing `source`.
    fn tab_for(&self, source: &SourceRef, cx: &App) -> Option<usize> {
        self.tabs.iter().position(|t| t.read(cx).doc.read(cx).source.source_ref().as_ref() == Some(source))
    }

    /// Opens a file or member row (or activates the tab showing it). `record` puts it in Recent: true for the user's
    /// clicks, false when a folder or archive opens its first log by itself.
    pub fn open_node(&mut self, id: NodeId, record: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.sources.tree.node(id).source.clone() else { return };
        self.open_source_ref(source, record, window, cx);
    }

    fn open_source_ref(&mut self, source: SourceRef, record: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.tab_for(&source, cx) {
            self.activate(index, window, cx);
            return;
        }
        if record {
            let entry = match &source {
                SourceRef::File(path) => RecentEntry::file(path),
                SourceRef::Member { .. } => RecentEntry::member(source.root_path(), source.member_chain().iter().map(|m| m.to_string()).collect()),
            };
            self.record_recent(entry, cx);
        }
        let document = match source {
            SourceRef::File(path) => Source::File(path),
            member => Source::Member(member),
        };
        self.open_source(document, TabInit::from_settings(&crate::settings::SettingsStore::get(cx)), window, cx);
    }

    /// Folder and archive rows expand and collapse; an archive is listed the first time it opens, and again when its
    /// last listing failed.
    pub fn toggle_node(&mut self, id: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        if !self.sources.expanded.remove(&id) {
            self.sources.expanded.insert(id);
            let node = self.sources.tree.node(id);
            if matches!(node.kind, NodeKind::Archive(_)) && matches!(node.listing, Listing::Pending | Listing::Failed(_)) {
                self.start_listing(id, window, cx);
            }
        }
        cx.notify();
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
            return;
        }
        match (entry.source(), classify(&entry.path)) {
            // An archive member: the archive appears in the sidebar and the member opens.
            (Some(member), PathKind::Archive(kind, size)) => {
                self.open_archive(entry.path.clone(), kind, size, false, window, cx);
                self.open_source_ref(member, true, window, cx);
            }
            _ => self.open_paths(vec![entry.path], window, cx),
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
        self.end_capture_of(&tab, cx);
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

    /// A stream tab owns its capture: closing the tab stops the copy and deletes the capture file (best effort — a
    /// snapshot of it that is still alive stays readable through its mapping).
    fn end_capture_of(&mut self, tab: &Entity<Tab>, cx: &App) {
        let Source::Stream { path, .. } = &tab.read(cx).doc.read(cx).source else { return };
        if let Some(at) = self.streams.iter().position(|(_, capture)| capture.path() == path) {
            self.streams.remove(at).1.stop();
        }
        let _ = std::fs::remove_file(path);
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

    /// Sidebar edge drag: sets the sidebar's width, clamped to 155–360 px.
    pub fn drag_sidebar(&mut self, width: Pixels, cx: &mut Context<Self>) {
        self.sidebar_width = width.clamp(px(size::SIDEBAR_MIN_W), px(size::SIDEBAR_MAX_W));
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
                    source: d.source.source_ref(),
                    stream: matches!(d.source, Source::Stream { .. }),
                    derived: matches!(d.source, Source::Derived(_)),
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
                    key: g.key.clone(),
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
        if let Some(node) = d.source.source_ref().and_then(|s| self.sources.tree.find(&s)) {
            return Some(sidebar_model::tree_footer(&self.sources.tree, self.sources.tree.root_of(node)));
        }
        let mut lines: Vec<SharedString> = vec![match d.source.path().and_then(Path::parent) {
            Some(dir) => dir.display().to_string().into(),
            None => d.source.location(),
        }];
        if let Some(text) = d.text() {
            let n = text.line_count();
            lines.push(format!("{} {} · {}", group_digits(n), noun(n, "line", "lines"), crate::format::format_size(text.bytes().len() as u64)).into());
        }
        Some(SidebarFooter { title: d.title(), lines })
    }

    /// The footer the sidebar shows now (tests).
    pub fn sidebar_footer(&self, cx: &App) -> Option<SidebarFooter> {
        match &self.fixed_sidebar {
            Some(fixed) => Some(fixed.footer.clone()),
            None => self.active_footer(cx),
        }
    }

    pub(crate) fn sidebar(&self, this: &Entity<Self>, cx: &App) -> Option<AnyElement> {
        if !self.sidebar_visible {
            return None;
        }
        let files = self.open_files(cx);
        let active = self.active_tab().map(|t| t.entity_id());
        let failed: HashSet<EntityId> = files.iter().filter(|f| f.failed).map(|f| f.tab).collect();
        let (rows, actions, count, recent) = match &self.fixed_sidebar {
            Some(fixed) => {
                let groups = self.sidebar_groups(&files);
                let (rows, actions) = sidebar_model::rows_with_failures(&groups, active, &self.collapsed, &failed);
                (rows, actions, fixed.count.clone(), fixed.recent.clone())
            }
            None => {
                let tree = &self.sources.tree;
                let (mut rows, mut actions) = sidebar_model::tree_rows(tree, &self.sources.expanded, &files, active);
                // Files opened on their own (not from a folder or archive) keep the M2 grouping by folder.
                let loose: Vec<OpenFile> = files.into_iter().filter(|f| f.source.as_ref().is_none_or(|s| tree.find(s).is_none())).collect();
                let (more_rows, more_actions) = sidebar_model::rows_with_failures(&sidebar_model::group_open_files(&loose), active, &self.collapsed, &failed);
                rows.extend(more_rows);
                actions.extend(more_actions);
                let total = tree.roots().iter().map(|&r| tree.root_summary(r).files).sum::<u64>() + loose.len() as u64;
                (rows, actions, format!("{total} {}", noun(total, "file", "files")).into(), recent_items(cx))
            }
        };
        let actions = Rc::new(actions);
        Some(
            Sidebar::new()
                .width(self.sidebar_width)
                .count(count)
                .rows(rows)
                .recent(recent)
                .footer(self.sidebar_footer(cx))
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
            RowAction::ToggleGroup(key) => {
                if !self.collapsed.remove(&key) {
                    self.collapsed.insert(key);
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
            RowAction::ToggleNode(id) => self.toggle_node(id, window, cx),
            RowAction::OpenNode(id) => self.open_node(id, true, window, cx),
            RowAction::Nothing => {}
        }
    }
}
