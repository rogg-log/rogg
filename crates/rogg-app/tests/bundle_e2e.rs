mod common;
use gpui_kit::{Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::tab::Tab;
use rogg_app::workspace::Workspace;
use rogg_genlog::GenOptions;
use rogg_genlog::bundle::{BundleKind, generate_bundle};
use std::io::Write;

fn active(ws: &Entity<Workspace>, cx: &mut TestAppContext) -> Entity<Tab> {
    ws.read_with(cx, |ws, _| ws.active_tab().unwrap())
}

fn loaded(tab: &Entity<Tab>, cx: &mut TestAppContext) -> bool {
    tab.read_with(cx, |t, cx| t.doc.read(cx).text().is_some())
}

/// Spec §8 rogg-app flow on an archive: open a bundle → its first member opens → filter → pin → open another member
/// → come back: the first tab kept its filter and pin.
#[gpui_kit::test]
fn a_bundle_opens_filters_and_keeps_tab_state(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let bundle = dir.path().join("support.tgz");
    generate_bundle(&bundle, BundleKind::TarGz, 3, &GenOptions { size_bytes: 200_000, ..Default::default() }).unwrap();
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![bundle] });
    common::wait_until(cx, |cx| ws.read_with(cx, |ws, _| ws.tabs().len()) == 1);
    let first = active(&ws, cx);
    common::wait_until(cx, |cx| loaded(&first, cx));
    assert_eq!(first.read_with(cx, |t, cx| t.title(cx).to_string()), "service-00.log");
    let doc = first.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("ERROR", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none() && !d.filter.matches.is_empty()));
    doc.update(cx, |d, cx| d.toggle_pin(3, cx));
    // support.tgz, service-00.log, service-01.log, service-02.log
    common::click(cx, w, "tree-row-2");
    let second = active(&ws, cx);
    common::wait_until(cx, |cx| loaded(&second, cx));
    assert_eq!(second.read_with(cx, |t, cx| t.title(cx).to_string()), "service-01.log");
    common::click(cx, w, "tree-row-1");
    assert_eq!(ws.read_with(cx, |ws, _| ws.active_index()), 0, "the open member's tab is activated, not reopened");
    assert_eq!(doc.read_with(cx, |d, _| (d.filter.spec.expression(), d.filter.pins.contains(&3))), ("ERROR".to_string(), true));
    let footer = ws.read_with(cx, |ws, cx| ws.sidebar_footer(cx)).unwrap();
    assert_eq!(footer.title.as_ref(), "support.tgz");
    assert!(footer.lines[0].starts_with("3 members · "), "{:?}", footer.lines);
}

/// Spec §8: live append and truncate through the window.
#[gpui_kit::test]
fn a_live_log_appends_filters_and_reloads(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let path = common::write_log(dir.path(), "app.log", &["start", ""]);
    let (_w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path.clone()] });
    let tab = active(&ws, cx);
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("ERROR", cx)).unwrap();
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    for i in 0..5 {
        writeln!(f, "line {i} ERROR").unwrap();
    }
    drop(f);
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.matches.len() == 5));
    assert!(tab.read_with(cx, |t, cx| t.doc.read(cx).is_live()));
    std::fs::write(&path, "fresh\n").unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.load_revision() == 2 && d.text().is_some_and(|t| t.line_count() == 1)));
    assert!(doc.read_with(cx, |d, _| d.filter.matches.is_empty()), "the reloaded file is filtered again");
}
