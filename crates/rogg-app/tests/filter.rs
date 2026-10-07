mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::document::{Document, DocumentInit, EngineConfig, Source};
use rogg_app::history::HistoryStore;
use rogg_core::filter::{FilterSpec, TermState};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

const LINES: [&str; 8] = ["ok start", "ERROR disk full", "retry 1", "timeout talking to db", "ok", "ERROR timeout again", "healthcheck ERROR", "done"];

fn doc_from(cx: &mut TestAppContext, text: String, init: DocumentInit) -> Entity<Document> {
    let bytes = Arc::new(text.into_bytes());
    cx.new(|cx| Document::new(Source::Memory { name: "mem.log".into(), bytes }, init, cx))
}

fn small_doc(cx: &mut TestAppContext) -> Entity<Document> {
    doc_from(cx, LINES.join("\n"), DocumentInit::default())
}

fn matches(doc: &Entity<Document>, cx: &mut TestAppContext) -> Vec<u64> {
    doc.read_with(cx, |d, _| d.filter.matches.as_ref().clone())
}

fn shown(doc: &Entity<Document>, cx: &mut TestAppContext) -> Vec<u64> {
    doc.read_with(cx, |d, _| d.filter.view.shown_lines().as_ref().clone())
}

#[gpui_kit::test]
fn expression_states_and_flags_filter_the_text(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("error || timeout", cx)).unwrap();
    assert_eq!(matches(&doc, cx), [1, 3, 5, 6]);
    doc.update(cx, |d, cx| d.set_expression("error || timeout || healthcheck", cx)).unwrap();
    doc.update(cx, |d, cx| {
        d.cycle_term(2, cx); // Include → Off
        d.cycle_term(2, cx); // Off → Exclude
    });
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.terms[2].state), TermState::Exclude);
    assert_eq!(matches(&doc, cx), [1, 3, 5]);
    doc.update(cx, |d, cx| d.toggle_case(cx)); // "error" no longer matches "ERROR"
    assert_eq!(matches(&doc, cx), [3, 5]);
    let stats = doc.read_with(cx, |d, _| d.filter.stats.clone().unwrap());
    assert_eq!((stats.matched, stats.total_lines), (2, 8));
}

#[gpui_kit::test]
fn no_active_terms_match_nothing(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    assert!(matches(&doc, cx).is_empty());
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    doc.update(cx, |d, cx| d.cycle_term(0, cx)); // Off
    assert!(matches(&doc, cx).is_empty());
}

#[gpui_kit::test]
fn pins_and_context_recompose_without_rescanning(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("disk", cx)).unwrap();
    let scans = doc.read_with(cx, |d, _| d.filter.scans);
    doc.update(cx, |d, cx| d.toggle_pin(7, cx));
    assert_eq!(shown(&doc, cx), [1, 7]);
    doc.update(cx, |d, cx| d.set_context(1, cx));
    assert_eq!(shown(&doc, cx), [0, 1, 2, 6, 7]);
    assert_eq!(doc.read_with(cx, |d, _| d.filter.stats.as_ref().unwrap().context_lines), 3);
    doc.update(cx, |d, cx| d.toggle_pin(7, cx));
    assert_eq!(shown(&doc, cx), [0, 1, 2]);
    assert_eq!(doc.read_with(cx, |d, _| d.filter.scans), scans, "pins and context never rescan");
}

#[gpui_kit::test]
fn invalid_regex_is_rejected_and_previous_results_stay(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    doc.update(cx, |d, cx| d.toggle_regex(cx));
    let err = doc.update(cx, |d, cx| d.set_expression("error || (", cx)).unwrap_err();
    assert!(err.contains("invalid regex"), "{err}");
    assert_eq!(matches(&doc, cx), [1, 5, 6]);
    doc.read_with(cx, |d, _| {
        assert_eq!(d.filter.spec.expression(), "error");
        assert_eq!(d.filter.error.as_ref(), Some(&err));
    });
    doc.update(cx, |d, cx| d.clear_filter_error(cx));
    assert!(doc.read_with(cx, |d, _| d.filter.error.is_none()));
}

#[gpui_kit::test]
fn add_term_appends_or_restates(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    doc.update(cx, |d, cx| d.add_term("timeout", false, cx));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.expression()), "error || timeout");
    assert_eq!(matches(&doc, cx), [1, 3, 5, 6]);
    doc.update(cx, |d, cx| d.add_term("error", true, cx));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.terms[0].state), TermState::Exclude);
    assert_eq!(matches(&doc, cx), [3]);
}

#[gpui_kit::test]
fn applied_filters_are_recorded_in_history(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("error || timeout", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_context(2, cx));
    cx.update(|cx| {
        let store = cx.global::<HistoryStore>();
        assert_eq!(store.entries().len(), 1, "a context-only change is not a new entry");
        assert_eq!(store.entries()[0].expression(), "error || timeout");
    });
    assert!(dir.path().join("history.json").exists());
}

#[gpui_kit::test]
fn initial_spec_and_pins_apply_when_the_text_loads(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let init = DocumentInit { number_base: 1, spec: FilterSpec::from_expression("timeout"), pins: BTreeSet::from([0]) };
    let doc = doc_from(cx, LINES.join("\n"), init);
    assert_eq!(matches(&doc, cx), [3, 5]);
    assert_eq!(shown(&doc, cx), [0, 3, 5]);
}

fn numbered(n: usize, every: usize, word: &str) -> (String, Vec<u64>) {
    let mut text = String::new();
    let mut expected = Vec::new();
    for i in 0..n {
        if i % every == 0 {
            text.push_str(word);
            expected.push(i as u64);
        } else {
            text.push_str("filler line");
        }
        text.push('\n');
    }
    (text, expected)
}

#[gpui_kit::test]
fn background_scan_matches_the_synchronous_result(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let (text, expected) = numbered(20_000, 7, "needle here");
    let doc = doc_from(cx, text, DocumentInit { spec: FilterSpec::from_expression("needle"), ..Default::default() });
    assert!(doc.read_with(cx, |d, _| d.filter.progress.is_some()), "large texts scan in the background");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    assert_eq!(matches(&doc, cx), expected);
}

#[gpui_kit::test]
fn superseded_async_scan_never_leaks_results(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let text: String = (0..200_000).map(|i| if i % 2 == 0 { "alpha\n" } else { "beta\n" }).collect();
    let doc = doc_from(cx, text, DocumentInit::default());
    doc.update(cx, |d, cx| d.set_expression("alpha", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_expression("beta", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    let found = matches(&doc, cx);
    assert_eq!(found.len(), 100_000);
    assert!(found.iter().all(|l| l % 2 == 1), "only the newest scan's matches appear");
}

#[gpui_kit::test]
fn dropping_a_document_mid_scan_does_not_block(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let text: String = (0..2_000_000).map(|i| if i % 3 == 0 { "x marks\n" } else { "nothing\n" }).collect();
    let doc = doc_from(cx, text, DocumentInit::default());
    doc.update(cx, |d, cx| d.set_expression("marks", cx)).unwrap();
    let started = Instant::now();
    drop(doc);
    cx.run_until_parked();
    assert!(started.elapsed() < Duration::from_millis(50), "closing waited {:?} for the scan", started.elapsed());
}

#[gpui_kit::test]
fn restarting_a_scan_discards_partial_results_of_the_previous_one(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0 }));
    let text: String = (0..10_000_000).map(|i| if i % 2 == 0 { "alpha\n" } else { "beta\n" }).collect();
    let doc = doc_from(cx, text, DocumentInit::default());
    doc.update(cx, |d, cx| d.set_expression("alpha", cx)).unwrap();
    // Let the first scan publish partial results before it is replaced.
    common::wait_until(cx, |cx| !matches(&doc, cx).is_empty());
    assert!(doc.read_with(cx, |d, _| d.filter.progress.is_some()), "the first scan must still be running when it is replaced");
    doc.update(cx, |d, cx| d.set_expression("beta", cx)).unwrap();
    assert!(matches(&doc, cx).is_empty(), "the new scan starts from no results");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    let found = matches(&doc, cx);
    assert_eq!(found.len(), 5_000_000);
    assert!(found.iter().all(|l| l % 2 == 1), "only the newest scan's matches appear");
}
