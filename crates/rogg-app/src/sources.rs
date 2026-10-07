//! The sidebar's folders and archives (spec §4.1, §5.3): the source tree, which containers are expanded, and the
//! background scans and listings that fill it.

use rogg_core::archive::ArchiveEntry;
use rogg_core::cache::Cache;
use rogg_core::folder::{DEFAULT_SCAN_DEPTH, FolderScan, scan_folder};
use rogg_core::source::{SourceRef, list_archive};
use rogg_core::tree::{NodeId, SourceTree};
use rogg_core::{CancelToken, CoreError};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

pub enum TreeResult {
    Scanned(Result<FolderScan, CoreError>),
    Listed(Result<Vec<ArchiveEntry>, CoreError>),
    /// The worker ended without a result.
    Died,
}

/// A folder scan or archive listing running for one tree node.
pub struct TreeJob {
    pub node: NodeId,
    cancel: CancelToken,
    rx: Receiver<TreeResult>,
}

impl TreeJob {
    pub fn scan(node: NodeId, dir: PathBuf) -> TreeJob {
        Self::spawn(node, move |token| TreeResult::Scanned(scan_folder(&dir, DEFAULT_SCAN_DEPTH, token)))
    }

    /// Lists an archive (extracting it into the cache first when it is a member of another archive).
    pub fn list(node: NodeId, source: SourceRef, cache: Arc<Cache>) -> TreeJob {
        Self::spawn(node, move |token| TreeResult::Listed(list_archive(&source, &cache, token)))
    }

    fn spawn(node: NodeId, work: impl FnOnce(&CancelToken) -> TreeResult + Send + 'static) -> TreeJob {
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let (tx, rx) = mpsc::channel();
        let _detached = std::thread::Builder::new().name("rogg-tree".into()).spawn(move || {
            let _ = tx.send(work(&token));
        });
        TreeJob { node, cancel, rx }
    }

    /// The result once the job has finished.
    pub fn poll(&self) -> Option<TreeResult> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(TreeResult::Died),
        }
    }
}

impl Drop for TreeJob {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[derive(Default)]
pub struct SourcesState {
    pub tree: SourceTree,
    /// Expanded folders and archives (roots start expanded).
    pub expanded: HashSet<NodeId>,
    pub(crate) jobs: Vec<TreeJob>,
    /// Roots whose first log opens when their scan or listing arrives (the user opened the folder or archive itself).
    pub(crate) open_first: HashSet<NodeId>,
    pub(crate) polling: bool,
}
