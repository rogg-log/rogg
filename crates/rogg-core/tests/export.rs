use rogg_core::CancelToken;
use rogg_core::export::{ExportOptions, export_lines};
use rogg_core::text::LogText;
use rogg_core::view::FilteredView;
use std::collections::BTreeSet;
use std::sync::Arc;

fn fixture() -> (LogText, FilteredView) {
    let text = LogText::from_bytes((0..10).map(|i| format!("l{i}")).collect::<Vec<_>>().join("\n").into_bytes());
    let view = FilteredView::build(Arc::new(vec![2, 6]), &BTreeSet::from([8]), 1, 10, |_| false);
    (text, view)
}

fn export(options: ExportOptions) -> (u64, String) {
    let (text, view) = fixture();
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("out.log");
    let n = export_lines(&text, &view, |l| l + 1, options, &dst, &CancelToken::new(), |_, _| {}).unwrap();
    (n, std::fs::read_to_string(&dst).unwrap())
}

#[test]
fn exports_context_gaps_and_numbers() {
    let (n, out) = export(ExportOptions { context: true, gaps: true, numbers: true });
    assert_eq!(n, 8);
    assert_eq!(out, " 2  l1\n 3  l2\n 4  l3\n··· 1 line omitted\n 6  l5\n 7  l6\n 8  l7\n 9  l8\n10  l9\n");
}

#[test]
fn without_context_only_matches_and_pins_are_written() {
    let (n, out) = export(ExportOptions { context: false, gaps: true, numbers: false });
    assert_eq!(n, 3);
    assert_eq!(out, "l2\n··· 3 lines omitted\nl6\n··· 1 line omitted\nl8\n");
    let (_, plain) = export(ExportOptions { context: false, gaps: false, numbers: false });
    assert_eq!(plain, "l2\nl6\nl8\n");
}

#[test]
fn export_failure_leaves_no_partial() {
    use std::os::unix::fs::PermissionsExt;
    let (text, view) = fixture();
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let result = export_lines(&text, &view, |l| l + 1, ExportOptions { context: true, gaps: false, numbers: false }, &locked.join("out.log"), &CancelToken::new(), |_, _| {});
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(rogg_core::CoreError::Io { .. })));
    assert_eq!(std::fs::read_dir(&locked).unwrap().count(), 0);
    let cancel = CancelToken::new();
    cancel.cancel();
    let dst = dir.path().join("c.log");
    assert!(matches!(export_lines(&text, &view, |l| l, ExportOptions { context: true, gaps: true, numbers: true }, &dst, &cancel, |_, _| {}), Err(rogg_core::CoreError::Cancelled)));
    assert!(!dst.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "only `locked`: no partial is left");
}
