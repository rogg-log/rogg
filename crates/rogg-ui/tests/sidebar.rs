mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, px};
use rogg_ui::sidebar::{RecentItem, Sidebar, SidebarFooter, TreeRow};
use std::cell::Cell;
use std::rc::Rc;

fn sidebar() -> Sidebar {
    Sidebar::new()
        .count("10 files".into())
        .rows(vec![
            TreeRow::Folder { label: "incident.zip".into(), expanded: true, depth: 0, note: None, failed: false },
            TreeRow::File { label: "api.log".into(), size: "972 MB".into(), active: true, depth: 1, failed: false },
            TreeRow::File { label: "worker.log".into(), size: "841 MB".into(), active: false, depth: 1, failed: false },
        ])
        .recent(vec![RecentItem { label: "staging / logs".into(), is_folder: true, missing: false }])
        .footer(Some(SidebarFooter { title: "Support bundle".into(), lines: vec!["10 members · 8.4 GB expanded".into()] }))
}

#[gpui_kit::test]
fn sidebar_geometry(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 700., |_, _| sidebar().into_any_element());
    assert_eq!(common::bounds(cx, w, "sidebar").size.width, px(211.));
    assert_eq!(common::bounds(cx, w, "side-top").size.height, px(43.));
    let folder = common::bounds(cx, w, "tree-row-0");
    let file = common::bounds(cx, w, "tree-row-1");
    assert_eq!(folder.size.height, px(30.));
    assert_eq!(file.size.height, px(30.));
    // .side-body padding 5px 7px: first row starts at 43 + 5, inset 7.
    assert_eq!(folder.origin.y, px(48.));
    assert_eq!(folder.origin.x, px(7.));
    assert!(common::exists(cx, w, "recent-0"));
    assert!(common::bounds(cx, w, "side-bottom").bottom() <= px(700.));
}

#[gpui_kit::test]
fn clicking_rows_reports_index(cx: &mut TestAppContext) {
    common::setup(cx);
    let hit = Rc::new(Cell::new(usize::MAX));
    let h = hit.clone();
    let w = common::open(cx, 600., 700., move |_, _| {
        let h = h.clone();
        sidebar().on_row(move |i, _, _| h.set(i)).into_any_element()
    });
    common::click(cx, w, "tree-row-2");
    assert_eq!(hit.get(), 2);
}

#[gpui_kit::test]
fn clicking_recent_reports_recent_index_only(cx: &mut TestAppContext) {
    common::setup(cx);
    let row_hit = Rc::new(Cell::new(usize::MAX));
    let recent_hit = Rc::new(Cell::new(usize::MAX));
    let (r, c) = (row_hit.clone(), recent_hit.clone());
    let w = common::open(cx, 600., 700., move |_, _| {
        let (r, c) = (r.clone(), c.clone());
        sidebar().on_row(move |i, _, _| r.set(i)).on_recent(move |i, _, _| c.set(i)).into_any_element()
    });
    common::click(cx, w, "recent-0");
    assert_eq!(recent_hit.get(), 0);
    assert_eq!(row_hit.get(), usize::MAX);
}

#[gpui_kit::test]
fn long_tree_scrolls_and_keeps_footer_pinned(cx: &mut TestAppContext) {
    common::setup(cx);
    let rows: Vec<TreeRow> = (0..60)
        .map(|i| TreeRow::File { label: format!("file-{i}.log").into(), size: "1 MB".into(), active: false, depth: 1, failed: false })
        .collect();
    let w = common::open(cx, 600., 700., move |_, _| sidebar().rows(rows.clone()).into_any_element());
    // 60 rows * 30px far exceed the window: the body scrolls instead of growing the sidebar.
    assert_eq!(common::bounds(cx, w, "sidebar").size.height, px(700.));
    assert_eq!(common::bounds(cx, w, "side-bottom").bottom(), px(700.));
    assert_eq!(common::bounds(cx, w, "side-top").size.height, px(43.));
}

#[gpui_kit::test]
fn width_override_and_missing_footer(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 700., |_, _| sidebar().width(px(260.)).footer(None).into_any_element());
    assert_eq!(common::bounds(cx, w, "sidebar").size.width, px(260.));
    assert!(!common::exists(cx, w, "side-bottom"));
}

#[gpui_kit::test]
fn folder_rows_show_a_note_and_a_failed_state(cx: &mut TestAppContext) {
    common::setup(cx);
    let rows = vec![
        TreeRow::Folder { label: "bundle".into(), expanded: true, depth: 0, note: None, failed: false },
        TreeRow::Folder { label: "inner.tgz".into(), expanded: true, depth: 1, note: Some("listing…".into()), failed: false },
        TreeRow::Folder { label: "broken.zip".into(), expanded: false, depth: 1, note: Some("unreadable".into()), failed: true },
    ];
    let w = common::open(cx, 211., 400., move |_, _| Sidebar::new().rows(rows.clone()).into_any_element());
    assert!(!common::exists(cx, w, "tree-note-0"));
    assert!(common::exists(cx, w, "tree-note-1") && common::exists(cx, w, "tree-note-2"));
    let row = common::bounds(cx, w, "tree-row-2");
    let note = common::bounds(cx, w, "tree-note-2");
    assert!(note.right() <= row.right() && note.right() >= row.right() - px(9.) - px(1.), "the note sits at the row's right padding");
}
