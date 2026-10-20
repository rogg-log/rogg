mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::args::Args;
use rogg_app::document::{Document, DocumentInit, Source};
use rogg_app::settings::SettingsStore;
use rogg_core::filter::{FilterSpec, Term, TermState};
use std::sync::Arc;

fn doc(cx: &mut TestAppContext) -> Entity<Document> {
    let bytes = Arc::new(b"ok start\nERROR disk\nretry 1\nhealthcheck ERROR".to_vec());
    let d = cx.new(|cx| Document::new(Source::Memory { name: "mem.log".into(), bytes }, DocumentInit::default(), cx));
    d.update(cx, |d, cx| d.set_expression("error || retry || healthcheck", cx)).unwrap();
    d
}

fn states(doc: &Entity<Document>, cx: &mut TestAppContext) -> Vec<TermState> {
    doc.read_with(cx, |d, _| d.filter.spec.terms.iter().map(|t| t.state).collect())
}

fn allow_exclude(cx: &mut TestAppContext, on: bool) {
    cx.update(|cx| {
        SettingsStore::update(cx, |s| s.behavior.exclude_terms = on);
    });
}

#[gpui_kit::test]
fn by_default_a_click_toggles_on_and_off(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = doc(cx);
    doc.update(cx, |d, cx| d.cycle_term(0, cx));
    assert_eq!(states(&doc, cx)[0], TermState::Off);
    doc.update(cx, |d, cx| d.cycle_term(0, cx));
    assert_eq!(states(&doc, cx)[0], TermState::Include, "one click back on, no NOT in between");
}

#[gpui_kit::test]
fn an_incoming_not_becomes_off_and_the_next_click_turns_it_on(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = doc(cx);
    // A restored session or history entry arrives with an exclusion.
    let spec = FilterSpec { terms: vec![Term::include("error"), Term::new("healthcheck", TermState::Exclude)], ..Default::default() };
    doc.update(cx, |d, cx| d.apply_spec(spec, cx)).unwrap();
    assert_eq!(states(&doc, cx), [TermState::Include, TermState::Off]);
    assert_eq!(doc.read_with(cx, |d, _| d.filter.matches.as_ref().clone()), [1, 3], "healthcheck no longer excludes line 3");
    doc.update(cx, |d, cx| d.cycle_term(1, cx));
    assert_eq!(states(&doc, cx)[1], TermState::Include);
    // "Exclude …" from the log's menu (hidden while NOT is off) can't sneak one in either.
    doc.update(cx, |d, cx| d.add_term("retry", true, cx));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.terms.iter().find(|t| t.text == "retry").map(|t| t.state)), Some(TermState::Off));
}

#[gpui_kit::test]
fn the_setting_brings_back_the_three_states(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    allow_exclude(cx, true);
    let doc = doc(cx);
    doc.update(cx, |d, cx| {
        d.cycle_term(2, cx);
        d.cycle_term(2, cx);
    });
    assert_eq!(states(&doc, cx)[2], TermState::Exclude);
    doc.update(cx, |d, cx| d.cycle_term(2, cx));
    assert_eq!(states(&doc, cx)[2], TermState::Include);
}

#[gpui_kit::test]
fn turning_the_setting_off_converts_open_tabs(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    allow_exclude(cx, true);
    let path = common::write_log(dir.path(), "a.log", &["ok", "error one", "healthcheck error"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![path], ..Default::default() });
    let tab = ws.read_with(cx, |ws, _| ws.active_tab().expect("a tab"));
    common::wait_until(cx, |cx| tab.read_with(cx, |t, cx| t.is_ready(cx)));
    let doc = tab.read_with(cx, |t, _| t.doc.clone());
    doc.update(cx, |d, cx| d.set_expression("error || healthcheck", cx)).unwrap();
    doc.update(cx, |d, cx| {
        d.cycle_term(1, cx);
        d.cycle_term(1, cx);
    });
    assert_eq!(states(&doc, cx)[1], TermState::Exclude);
    assert!(tab.read_with(cx, |t, cx| t.full.read(cx).allow_exclude()), "the log menu offers Exclude while NOT is on");
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.change_settings(|s| s.behavior.exclude_terms = false, window, cx))).unwrap();
    assert_eq!(states(&doc, cx)[1], TermState::Off);
    assert_eq!(doc.read_with(cx, |d, _| d.filter.matches.as_ref().clone()), [1, 2], "healthcheck no longer excludes");
    assert!(!tab.read_with(cx, |t, cx| t.full.read(cx).allow_exclude() || t.filtered.read(cx).allow_exclude()), "both panes hide Exclude");
}

#[test]
fn the_hint_matches_the_setting() {
    assert_eq!(rogg_app::tab_view::filter_hint(false), "Click a term's icon to turn it on or off");
    assert_eq!(rogg_app::tab_view::filter_hint(true), "Click a state to cycle Include → Off → Exclude");
}

#[gpui_kit::test]
fn the_settings_toggle_turns_excluding_on(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let (w, _ws) = common::open_workspace(cx, Args::default());
    common::press(cx, w, "cmd-,");
    assert!(!cx.update(|cx| SettingsStore::get(cx).behavior.exclude_terms));
    common::click(cx, w, "setting-exclude-terms");
    assert!(cx.update(|cx| SettingsStore::get(cx).behavior.exclude_terms));
}
