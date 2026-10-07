mod common;
use gpui_kit::TestAppContext;
use rogg_app::args::Args;
use rogg_app::demo;
use rogg_app::document::Pane;

#[gpui_kit::test]
fn demo_reproduces_the_reference_state(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, ws) = common::open_workspace(cx, Args { demo: true, paths: vec![] });
    let titles: Vec<String> = ws.read_with(cx, |ws, cx| ws.tabs().iter().map(|t| t.read(cx).title(cx).to_string()).collect());
    assert_eq!(titles, demo::TABS);
    assert_eq!(ws.read_with(cx, |ws, _| ws.active_index()), 0);
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().unwrap());
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.read_with(cx, |d, _| {
        assert_eq!(d.number_base(), demo::FIRST_LINE_NUMBER);
        let stats = d.filter.stats.clone().unwrap();
        assert_eq!((stats.matched, stats.total_lines, stats.pinned, stats.pinned_unmatched, stats.context_lines), (6, 40, 1, 1, 0));
        assert_eq!(d.filter.view.shown_lines().as_ref(), &vec![7, 12, 15, 20, 23, 28, 34]);
    });
    let selected = demo::FIRST_LINE_NUMBER + demo::SELECTED_SAMPLE;
    tab.read_with(cx, |t, cx| {
        assert_eq!(t.state(Pane::Full).read(cx).selected(), Some(selected));
        assert_eq!(t.state(Pane::Filtered).read(cx).selected(), Some(selected));
        assert_eq!(t.selection(), Some(selected), "the initial selection is the tab's selection (⌘B, Return, find act on it)");
        assert_eq!(t.announced_selection(), None, "the footer shows no `Line N` until the user selects");
    });
    assert!(common::exists(cx, w, "tree-row-10"), "bundle folder + 10 members");
    assert!(common::exists(cx, w, "recent-1"));
    common::click(cx, w, "tree-row-4");
    assert_eq!(ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).title(cx).to_string()), "auth.log");
    assert_eq!(ws.read_with(cx, |ws, _| ws.tabs().len()), 4);
}

#[test]
fn sample_lines_follow_the_reference_formula() {
    assert_eq!(demo::line_text(0), "2026-10-05 14:32:08.142  INFO   server       request started method=POST path=/v1/payments request_id=req_8f29");
    assert_eq!(demo::sample_bytes().iter().filter(|&&b| b == b'\n').count(), 39);
}
