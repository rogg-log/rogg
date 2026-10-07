mod common;
use gpui_kit::TestAppContext;
use rogg_app::args::Args;
use rogg_app::dialogs::stats::{duration_label, percent, rows};
use rogg_core::filter::TermState;
use rogg_core::stats::{FilterStats, TermHits};
use std::time::Duration;

fn sample() -> FilterStats {
    FilterStats {
        total_lines: 40,
        matched: 6,
        pinned: 1,
        pinned_unmatched: 1,
        context_lines: 3,
        term_hits: vec![
            TermHits { text: "error".into(), state: TermState::Include, lines: 7 },
            TermHits { text: "healthcheck".into(), state: TermState::Exclude, lines: 4 },
            TermHits { text: "retry".into(), state: TermState::Off, lines: 0 },
        ],
        duration: Duration::from_micros(3_400),
    }
}

#[test]
fn statistics_rows_read_like_the_spec() {
    let r: Vec<(String, String)> = rows(Some(&sample()), None).into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    assert_eq!(
        r,
        [
            ("Matched lines", "6 of 40 (15%)"),
            ("Pinned lines", "1 (1 not matching)"),
            ("Context lines", "3"),
            ("error", "include · 7 lines"),
            ("healthcheck", "exclude · 4 lines"),
            ("retry", "off"),
            ("Scan time", "3.4 ms"),
        ]
        .map(|(a, b)| (a.to_string(), b.to_string()))
    );
    let running = rows(Some(&sample()), Some(0.45));
    assert_eq!(running[0].1.as_ref(), "6 of 40 (15%) · scanning 45%");
    assert_eq!(rows(None, None)[0].0.as_ref(), "No filter has run yet");
    assert_eq!([percent(1, 3), percent(0, 0), percent(1, 10_000_000), percent(40, 40)], ["33.3%", "0%", "<0.1%", "100%"]);
    assert_eq!([duration_label(Duration::from_micros(400)), duration_label(Duration::from_millis(1_240))], ["0.4 ms", "1.24 s"]);
}

#[gpui_kit::test]
fn the_footer_opens_live_statistics(cx: &mut TestAppContext) {
    let dir = common::setup(cx);
    let log = common::write_log(dir.path(), "a.log", &["error one", "ok", "error two"]);
    let (w, ws) = common::open_workspace(cx, Args { demo: false, paths: vec![log] });
    let doc = ws.read_with(cx, |ws, cx| ws.active_tab().unwrap().read(cx).doc.clone());
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.text().is_some()));
    doc.update(cx, |d, cx| d.set_expression("error", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.stats.is_some()));
    common::click(cx, w, "footer-stats");
    assert!(common::exists(cx, w, "stat-0") && common::exists(cx, w, "stat-3"));
    doc.update(cx, |d, cx| d.set_expression("error || ok", cx)).unwrap();
    common::wait_until(cx, |cx| doc.read_with(cx, |d, _| d.filter.stats.as_ref().is_some_and(|s| s.matched == 3)));
    assert!(common::exists(cx, w, "stat-4"), "the open popover follows the new filter");
}
