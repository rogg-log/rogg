use rogg_core::CancelToken;
use rogg_core::archive::{ArchiveEntry, ArchiveKind, Content};
use rogg_core::folder::{DEFAULT_SCAN_DEPTH, scan_folder, scan_folder_with};
use rogg_core::source::SourceRef;
use rogg_core::tree::{Listing, NodeKind, SourceTree, TreeSummary};
use rogg_genlog::bundle::{BundleKind, write_archive, write_gz};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn names(tree: &SourceTree, rows: &[(usize, usize)]) -> Vec<String> {
    rows.iter().map(|&(id, depth)| format!("{}{}", "  ".repeat(depth), tree.node(id).name)).collect()
}

fn entry(path: &str, size: u64) -> ArchiveEntry {
    ArchiveEntry { path: path.into(), size }
}

#[test]
fn source_refs_name_their_chain() {
    let zip = SourceRef::File(PathBuf::from("/b/incident.zip"));
    let inner = SourceRef::member(zip.clone(), "nested/inner.tgz");
    let log = SourceRef::member(inner.clone(), "app/x.log");
    assert_eq!(log.root_path(), Path::new("/b/incident.zip"));
    assert_eq!(log.member_chain(), ["nested/inner.tgz", "app/x.log"]);
    assert_eq!(log.name(), "x.log");
    assert_eq!(log.crumbs(), ["incident.zip", "nested", "inner.tgz", "app", "x.log"]);
    assert_eq!(log.tooltip(), "x.log · incident.zip");
    let file = SourceRef::File(PathBuf::from("/var/log/api.log"));
    assert_eq!(file.crumbs(), ["log", "api.log"]);
    assert_eq!(file.tooltip(), "/var/log/api.log");
    assert!(file.member_chain().is_empty());
    let json = serde_json::to_string(&log).unwrap();
    assert_eq!(serde_json::from_str::<SourceRef>(&json).unwrap(), log);
}

#[test]
fn listings_become_folders_members_and_nested_archives() {
    let mut tree = SourceTree::new();
    let zip = tree.add_root(Path::new("/b/incident.zip"), NodeKind::Archive(ArchiveKind::Zip), Some(900));
    assert_eq!(tree.node(zip).listing, Listing::Pending);
    tree.insert_listing(zip, &[entry("logs/worker.log", 20), entry("logs/api.log", 30), entry("inner.tgz", 50), entry("README", 5)]);
    assert_eq!(tree.node(zip).listing, Listing::Listed);
    let expanded: HashSet<usize> = tree.visible(|_| true).into_iter().map(|(id, _)| id).collect();
    let all = tree.visible(|id| expanded.contains(&id));
    assert_eq!(names(&tree, &all), ["incident.zip", "  logs", "    api.log", "    worker.log", "  inner.tgz", "  README"]);
    let collapsed = tree.visible(|id| id == zip);
    assert_eq!(names(&tree, &collapsed), ["incident.zip", "  logs", "  inner.tgz", "  README"], "only expanded containers open");
    let inner = all[4].0;
    assert_eq!(tree.node(inner).kind, NodeKind::Archive(ArchiveKind::TarGz));
    assert_eq!(tree.node(inner).listing, Listing::Pending);
    assert!(tree.is_container(inner) && !tree.is_openable(inner));
    let api = SourceRef::member(SourceRef::File(PathBuf::from("/b/incident.zip")), "logs/api.log");
    assert_eq!(tree.find(&api), Some(all[2].0));
    assert_eq!(tree.first_openable(zip), Some(all[5].0), "a file at the top comes before files in folders");
    assert_eq!(tree.first_openable(all[1].0), Some(all[2].0), "inside a folder: its first file");
    assert_eq!(tree.root_of(all[2].0), zip);
    assert_eq!(tree.summary(zip), TreeSummary { files: 3, bytes: 55 }, "nested archives are not files until listed");
    tree.insert_listing(inner, &[entry("x.log", 7)]);
    assert_eq!(tree.summary(zip), TreeSummary { files: 4, bytes: 62 });
    let x = tree.first_openable(inner).unwrap();
    assert_eq!(tree.node(x).source, Some(SourceRef::member(SourceRef::member(SourceRef::File(PathBuf::from("/b/incident.zip")), "inner.tgz"), "x.log")));
    tree.set_listing(inner, Listing::Failed("damaged".into()));
    assert_eq!(tree.node(inner).listing, Listing::Failed("damaged".into()));
    assert_eq!(tree.root_for(Path::new("/b/incident.zip")), Some(zip));
    assert_eq!(tree.root_for(Path::new("/b/other.zip")), None);
}

#[test]
fn root_summaries_follow_listings_of_nested_archives() {
    let mut tree = SourceTree::new();
    let zip = tree.add_root(Path::new("/b/incident.zip"), NodeKind::Archive(ArchiveKind::Zip), Some(900));
    assert_eq!(tree.root_summary(zip), TreeSummary::default());
    tree.insert_listing(zip, &[entry("inner.tgz", 50), entry("README", 5)]);
    assert_eq!(tree.root_summary(zip), TreeSummary { files: 1, bytes: 5 });
    let inner = tree.visible(|_| true).into_iter().map(|(id, _)| id).find(|&id| tree.node(id).name == "inner.tgz").unwrap();
    tree.insert_listing(inner, &[entry("x.log", 7), entry("y.log", 8)]);
    assert_eq!(tree.root_summary(zip), TreeSummary { files: 3, bytes: 20 }, "a nested listing updates its root's cached summary");
    assert_eq!(tree.root_summary(zip), tree.summary(zip));
}

#[test]
fn folder_scans_list_text_gz_and_archives_but_not_binaries() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bundle");
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::write(root.join("top.log"), "t\n").unwrap();
    std::fs::write(root.join("a/mid.log"), "m\n").unwrap();
    std::fs::write(root.join("a/b/deep.log"), "d\n").unwrap();
    std::fs::write(root.join(".hidden.log"), "h\n").unwrap();
    std::fs::write(root.join("image.png"), [0x89, b'P', b'N', b'G', 0, 0]).unwrap();
    write_gz(&root.join("old.log.1.gz"), b"o\n").unwrap();
    write_archive(&root.join("a/more.zip"), BundleKind::Zip, &[("x.log", b"x\n")]).unwrap();
    let scan = scan_folder(&root, DEFAULT_SCAN_DEPTH, &CancelToken::new()).unwrap();
    let got: Vec<(String, Content)> = scan.files.iter().map(|f| (f.rel.display().to_string(), f.content)).collect();
    assert_eq!(
        got,
        [
            ("a/b/deep.log".to_string(), Content::Text),
            ("a/mid.log".to_string(), Content::Text),
            ("a/more.zip".to_string(), Content::Archive(ArchiveKind::Zip)),
            ("old.log.1.gz".to_string(), Content::Gzip),
            ("top.log".to_string(), Content::Text),
        ]
    );
    assert!(!scan.truncated);

    let mut tree = SourceTree::new();
    let id = tree.add_root(&root, NodeKind::Folder, None);
    tree.insert_folder(id, &scan);
    let rows = tree.visible(|_| true);
    assert_eq!(names(&tree, &rows), ["bundle", "  a", "    b", "      deep.log", "    mid.log", "    more.zip", "  old.log.1.gz", "  top.log"]);
    let deep = rows[3].0;
    assert_eq!(tree.node(deep).source, Some(SourceRef::File(root.join("a/b/deep.log"))));
    assert_eq!(tree.node(rows[5].0).kind, NodeKind::Archive(ArchiveKind::Zip));
    assert_eq!(tree.node(rows[6].0).kind, NodeKind::File, "a .gz log opens like a file");
    assert_eq!(tree.summary(id).files, 4);
}

#[test]
#[cfg(unix)]
fn scan_is_bounded_and_does_not_follow_symlinked_folders() {
    use rogg_core::folder::MAX_SCAN_ENTRIES;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("r");
    let mut deep = root.clone();
    for level in 1..=6 {
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join(format!("level{level}.log")), "x\n").unwrap();
        deep = deep.join("sub");
    }
    std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
    std::os::unix::fs::symlink(root.join("level1.log"), root.join("alias.log")).unwrap();
    let scan = scan_folder(&root, DEFAULT_SCAN_DEPTH, &CancelToken::new()).unwrap();
    let got: Vec<String> = scan.files.iter().map(|f| f.rel.display().to_string()).collect();
    assert_eq!(got, ["alias.log", "level1.log", "sub/level2.log", "sub/sub/level3.log", "sub/sub/sub/level4.log"], "depth 4, no loop");
    let few = scan_folder_with(&root, DEFAULT_SCAN_DEPTH, 2, MAX_SCAN_ENTRIES, &CancelToken::new()).unwrap();
    assert_eq!(few.files.len(), 2);
    assert!(few.truncated);
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(scan_folder(&root, DEFAULT_SCAN_DEPTH, &cancel).is_err());
    assert!(scan_folder(&dir.path().join("missing"), DEFAULT_SCAN_DEPTH, &CancelToken::new()).is_err());
}

#[test]
fn many_unlisted_entries_stop_the_scan() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bin");
    std::fs::create_dir_all(&root).unwrap();
    for n in 0..50 {
        std::fs::write(root.join(format!("blob{n:02}.bin")), [0u8, 1, 2, 3]).unwrap();
    }
    let scan = scan_folder_with(&root, DEFAULT_SCAN_DEPTH, 10_000, 20, &CancelToken::new()).unwrap();
    assert!(scan.files.is_empty());
    assert!(scan.truncated, "binaries count towards the entry limit");
    let all = scan_folder_with(&root, DEFAULT_SCAN_DEPTH, 10_000, 50, &CancelToken::new()).unwrap();
    assert!(!all.truncated, "exactly the limit is not truncated");
}

#[test]
fn a_member_path_of_ten_thousand_components_does_not_overflow() {
    let mut tree = SourceTree::new();
    let zip = tree.add_root(Path::new("/b/deep.zip"), NodeKind::Archive(ArchiveKind::Zip), Some(1));
    let path = "d/".repeat(9_999) + "x.log";
    tree.insert_listing(zip, &[entry(&path, 7)]);
    let rows = tree.visible(|_| true);
    assert_eq!(rows.len(), 10_001);
    assert_eq!(rows.last().map(|&(_, depth)| depth), Some(10_000));
    let x = tree.first_openable(zip).unwrap();
    assert_eq!(tree.node(x).name, "x.log");
    assert_eq!(tree.summary(zip), TreeSummary { files: 1, bytes: 7 });
    assert_eq!(tree.root_of(x), zip);
}

#[test]
fn many_sibling_folders_are_shared_by_their_files() {
    let mut tree = SourceTree::new();
    let zip = tree.add_root(Path::new("/b/wide.zip"), NodeKind::Archive(ArchiveKind::Zip), Some(1));
    let entries: Vec<ArchiveEntry> = (0..2_000).flat_map(|n| [entry(&format!("dir{n:04}/a.log"), 1), entry(&format!("dir{n:04}/b.log"), 1)]).collect();
    tree.insert_listing(zip, &entries);
    assert_eq!(tree.node(zip).children.len(), 2_000, "one folder per directory, shared by its files");
    assert_eq!(tree.summary(zip).files, 4_000);
}
