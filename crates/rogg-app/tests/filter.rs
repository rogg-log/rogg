mod common;
use gpui_kit::{AppContext, Entity, TestAppContext};
use rogg_app::document::{Document, DocumentInit, EngineConfig, Pane, Source};
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
fn add_term_matches_the_literal_word_in_regex_mode(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    let doc = doc_from(cx, ["x a.b{ y", "axb", "a.b{", "other"].join("\n"), DocumentInit::default());
    doc.update(cx, |d, cx| d.toggle_regex(cx));
    doc.update(cx, |d, cx| d.add_term("a.b{", false, cx));
    doc.read_with(cx, |d, _| assert!(d.filter.error.is_none(), "the escaped word is a valid regex: {:?}", d.filter.error));
    assert_eq!(matches(&doc, cx), [0, 2], "only lines with the literal text");
    doc.update(cx, |d, cx| d.add_term("a.b{", true, cx));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.terms.len()), 1, "the escaped term is found again");
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.terms[0].state), TermState::Exclude);
    assert_eq!(doc.read_with(cx, |d, _| d.filter.spec.terms[0].text.clone()), r"a\.b\{");
}

#[gpui_kit::test]
fn applied_filters_are_recorded_in_history(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let doc = small_doc(cx);
    doc.update(cx, |d, cx| d.set_expression("error || timeout", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_context(2, cx));
    cx.update(|cx| {
        let store = cx.global::<HistoryStore>();
        assert_eq!(store.entries().len(), 1, "a context-only change updates the entry instead of adding one");
        assert_eq!(store.entries()[0].expression(), "error || timeout");
        assert_eq!(store.entries()[0].context, 2);
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
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let (text, expected) = numbered(20_000, 7, "needle here");
    let doc = doc_from(cx, text, DocumentInit { spec: FilterSpec::from_expression("needle"), ..Default::default() });
    assert!(doc.read_with(cx, |d, _| d.filter.progress.is_some()), "large texts scan in the background");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    assert_eq!(matches(&doc, cx), expected);
}

#[gpui_kit::test]
fn superseded_async_scan_never_leaks_results(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let text: String = (0..200_000).map(|i| if i % 2 == 0 { "alpha\n" } else { "beta\n" }).collect();
    let doc = doc_from(cx, text, DocumentInit::default());
    doc.update(cx, |d, cx| d.set_expression("alpha", cx)).unwrap();
    doc.update(cx, |d, cx| d.set_expression("beta", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    let found = matches(&doc, cx);
    assert_eq!(found.len(), 100_000);
    assert!(found.iter().all(|l| l % 2 == 1), "only the newest scan's matches appear");
}

/// A smoke test: a cancelled worker winds down quickly anyway, so this cannot catch a synchronous drop. The
/// no-blocking guarantee rests on `retire` (cancel, then join on a short-lived thread) in `Drop for Document`.
#[gpui_kit::test]
fn dropping_a_document_mid_scan_does_not_block(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
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
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
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

#[gpui_kit::test]
fn progressive_views_skip_context_until_the_scan_finishes(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let text: String = (0..10_000_000).map(|i| if i % 2 == 0 { "needle\n" } else { "filler\n" }).collect();
    let spec = FilterSpec { context: 1, ..FilterSpec::from_expression("needle") };
    let doc = doc_from(cx, text, DocumentInit { spec, ..Default::default() });
    common::wait_until(cx, |cx| !matches(&doc, cx).is_empty());
    assert!(doc.read_with(cx, |d, _| d.filter.progress.is_some()), "the scan must still be running");
    assert_eq!(doc.read_with(cx, |d, _| d.filter.view.context_lines()), 0, "no context while the scan runs");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none()));
    // The finished view is large, so it is composed off the UI thread and lands a little after the scan.
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.context_lines() == 5_000_000));
    assert!(doc.read_with(cx, |d, _| d.filter.view_builds_off_thread >= 1), "the final view was built off the UI thread");
}

fn every_tenth(n: usize) -> String {
    (0..n).map(|i| if i % 10 == 5 { "match here\n" } else { "filler\n" }).collect()
}

#[gpui_kit::test]
fn large_view_rebuilds_run_off_the_ui_thread_and_keep_the_old_view(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    // 100 matches x max(context, 1): the context-free view (100) is cheap, the +-2 view (200) is not.
    cx.update(|cx| cx.set_global(EngineConfig { view_sync_work: 150, ..Default::default() }));
    let doc = doc_from(cx, every_tenth(1000), DocumentInit { spec: FilterSpec::from_expression("match"), ..Default::default() });
    assert_eq!(doc.read_with(cx, |d, _| d.filter.view.shown_lines().len()), 100, "a context-free view is built at once");
    doc.update(cx, |d, cx| d.set_context(2, cx));
    assert_eq!(doc.read_with(cx, |d, _| (d.filter.view.context_lines(), d.filter.view_builds_off_thread)), (0, 1), "the old view stays meanwhile");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.context_lines() == 400));
    assert_eq!(doc.read_with(cx, |d, _| (d.filter.scans, d.filter.stats.as_ref().unwrap().context_lines)), (1, 400), "no rescan; stats follow the view");
}

#[gpui_kit::test]
fn a_newer_view_wins_over_an_older_one(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { view_sync_work: 150, ..Default::default() }));
    let doc = doc_from(cx, every_tenth(1000), DocumentInit { spec: FilterSpec::from_expression("match"), ..Default::default() });
    doc.update(cx, |d, cx| {
        d.set_context(2, cx);
        d.set_context(4, cx);
    });
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.context_lines() > 0));
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.context_lines() == 800));
    for _ in 0..20 {
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        cx.run_until_parked();
    }
    assert_eq!(doc.read_with(cx, |d, _| d.filter.view.context_lines()), 800, "the ±2 result never replaces the ±4 one");
}

#[gpui_kit::test]
fn the_filtered_search_waits_for_the_final_view_of_a_scan(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { sync_bytes: 0, ..Default::default() }));
    let text: String = (0..200_000).map(|i| if i % 2 == 0 { "alpha\n" } else { "beta\n" }).collect();
    let doc = doc_from(cx, text, DocumentInit::default());
    doc.update(cx, |d, cx| d.set_search(Pane::Filtered, "alpha", cx));
    let before = doc.read_with(cx, |d, _| d.search(Pane::Filtered).runs);
    doc.update(cx, |d, cx| d.set_expression("alpha", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.progress.is_none() && !d.search(Pane::Filtered).is_running()));
    assert_eq!(doc.read_with(cx, |d, _| d.search(Pane::Filtered).runs), before + 1, "one count, on the final view");
    let hits = doc.read_with(cx, |d, _| d.search(Pane::Filtered).result.as_ref().map(|r| r.count()));
    assert_eq!(hits, Some(100_000));
}

#[gpui_kit::test]
fn pin_toggles_on_many_matches_rebuild_off_the_ui_thread(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { view_sync_work: 10, ..Default::default() }));
    let doc = doc_from(cx, every_tenth(1000), DocumentInit { spec: FilterSpec::from_expression("match"), ..Default::default() });
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.shown_lines().len() == 100));
    let before = doc.read_with(cx, |d, _| d.filter.view_builds_off_thread);
    doc.update(cx, |d, cx| d.toggle_pin(0, cx));
    assert_eq!(doc.read_with(cx, |d, _| d.filter.view_builds_off_thread), before + 1, "the rebuild runs on a thread even at context 0");
    assert_eq!(doc.read_with(cx, |d, _| d.filter.view.shown_lines().len()), 100, "the old view stays meanwhile");
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.shown_lines().len() == 101));
    assert!(doc.read_with(cx, |d, _| d.filter.view.shown_lines().contains(&0)), "the pinned line is shown");
}

#[gpui_kit::test]
fn one_view_build_runs_at_a_time_and_the_newest_state_wins(cx: &mut TestAppContext) {
    let _dir = common::setup(cx);
    cx.update(|cx| cx.set_global(EngineConfig { view_sync_work: 0, ..Default::default() }));
    let doc = doc_from(cx, every_tenth(1000), DocumentInit { spec: FilterSpec::from_expression("match"), ..Default::default() });
    // Even the context-free view is built off the UI thread here; let the first one land.
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.shown_lines().len() == 100));
    let before = doc.read_with(cx, |d, _| d.filter.view_builds_off_thread);
    doc.update(cx, |d, cx| {
        for context in 1..=4 {
            d.set_context(context, cx);
        }
    });
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.view.context_lines() == 800));
    for _ in 0..20 {
        cx.executor().advance_clock(std::time::Duration::from_millis(20));
        cx.run_until_parked();
    }
    let (builds, context) = doc.read_with(cx, |d, _| (d.filter.view_builds_off_thread - before, d.filter.view.context_lines()));
    assert_eq!(context, 800, "the final view has the newest context");
    assert!((1..=2).contains(&builds), "one build in flight plus one for the newest state, not one per request ({builds})");
}
