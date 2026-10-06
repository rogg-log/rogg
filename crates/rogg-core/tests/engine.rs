use rogg_core::CancelToken;
use rogg_core::CoreError;
use rogg_core::engine::{FilterEvent, FilterJob, MAX_CHUNK_BYTES, run_filter, scan_lines};
use rogg_core::filter::{CompiledFilter, FilterSpec, Term, TermState};
use rogg_core::text::LogText;
use std::sync::Arc;

fn sample(lines: usize) -> LogText {
    let words = ["error", "timeout", "ok", "retry", "healthcheck"];
    let text: Vec<String> = (0..lines).map(|i| format!("{i} {} {}", words[i % 5], words[(i * 7) % 5])).collect();
    LogText::from_bytes(text.join("\n").into_bytes())
}

fn filter() -> CompiledFilter {
    CompiledFilter::compile(&FilterSpec {
        terms: vec![Term::include("error"), Term::include("retry"), Term::new("healthcheck", TermState::Exclude)],
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn run_filter_equals_per_line_evaluation() {
    let text = sample(1_000);
    let f = filter();
    let result = run_filter(&text, &f, 7, &CancelToken::new()).unwrap();
    let expected: Vec<u64> = (0..text.line_count()).filter(|&i| f.line_matches(text.line(i))).collect();
    assert_eq!(result.matches, expected);
    assert_eq!(result.total_lines, 1_000);
    assert_eq!(result.term_hits.len(), 3);
}

#[test]
fn events_are_ordered_contiguous_and_end_with_finished() {
    let text = sample(500);
    let mut events = Vec::new();
    scan_lines(&text, &filter(), 0..500, 9, &CancelToken::new(), |e| events.push(e));
    let mut next = 0;
    let mut last_progress = 0;
    for e in &events[..events.len() - 1] {
        match e {
            FilterEvent::Chunk { lines, matches } => {
                assert_eq!(lines.start, next);
                assert!(matches.iter().all(|m| lines.contains(m)));
                next = lines.end;
            }
            FilterEvent::Progress { lines_done, total_lines } => {
                assert!(*lines_done >= last_progress);
                assert_eq!(*total_lines, 500);
                last_progress = *lines_done;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(next, 500);
    assert!(matches!(events.last(), Some(FilterEvent::Finished { total_lines: 500, .. })));
}

#[test]
fn pre_cancelled_scan_emits_only_cancelled() {
    let text = sample(100);
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut events = Vec::new();
    scan_lines(&text, &filter(), 0..100, 3, &cancel, |e| events.push(e));
    assert_eq!(events, vec![FilterEvent::Cancelled]);
    assert!(run_filter(&text, &filter(), 3, &cancel).is_err());
}

#[test]
fn cancel_mid_scan_stops_without_finished() {
    let text = sample(20_000);
    let cancel = CancelToken::new();
    let mut events = Vec::new();
    scan_lines(&text, &filter(), 0..20_000, 2, &cancel, |e| {
        if matches!(e, FilterEvent::Chunk { .. }) {
            cancel.cancel();
        }
        events.push(e);
    });
    assert_eq!(events.last(), Some(&FilterEvent::Cancelled));
    assert!(!events.iter().any(|e| matches!(e, FilterEvent::Finished { .. })));
    let chunks = events.iter().filter(|e| matches!(e, FilterEvent::Chunk { .. })).count();
    assert!(chunks <= 4 * rayon::current_num_threads(), "stopped within one wave ({chunks} chunks)");
}

#[test]
fn inactive_filter_finishes_with_no_matches() {
    let text = sample(50);
    let none = CompiledFilter::compile(&FilterSpec::default()).unwrap();
    let r = run_filter(&text, &none, 8, &CancelToken::new()).unwrap();
    assert!(r.matches.is_empty());
    assert_eq!(r.total_lines, 50);
}

#[test]
fn job_wait_matches_run_filter_and_drop_joins() {
    let text = Arc::new(sample(3_000));
    let f = Arc::new(filter());
    let expected = run_filter(&text, &f, 64, &CancelToken::new()).unwrap();
    let job = FilterJob::spawn(text.clone(), f.clone(), 64);
    let got = job.wait().unwrap();
    assert_eq!(got.matches, expected.matches);
    assert_eq!(got.term_hits, expected.term_hits);
}

#[test]
fn dropping_a_job_cancels_and_joins_the_worker() {
    let big = Arc::new(sample(200_000));
    let probe = big.clone();
    let job = FilterJob::spawn(big, Arc::new(filter()), 16);
    drop(job);
    // The worker owns the other Arc clone; it is released only once the thread has finished.
    assert_eq!(Arc::strong_count(&probe), 1, "Drop must join the worker thread");
}

#[test]
fn cancelled_job_wait_returns_cancelled() {
    let job = FilterJob::spawn(Arc::new(sample(400_000)), Arc::new(filter()), 16);
    job.cancel();
    assert!(matches!(job.wait(), Err(CoreError::Cancelled)));
}

#[test]
fn cancelled_job_events_end_at_first_cancelled() {
    let job = FilterJob::spawn(Arc::new(sample(400_000)), Arc::new(filter()), 16);
    job.cancel();
    let events: Vec<FilterEvent> = job.events().iter().collect(); // ends when the worker drops its sender
    let first = events.iter().position(|e| *e == FilterEvent::Cancelled).expect("a Cancelled event");
    assert_eq!(first, events.len() - 1, "nothing may follow Cancelled");
    assert!(!events.iter().any(|e| matches!(e, FilterEvent::Finished { .. })));
}

#[test]
fn long_lines_are_split_by_bytes_not_just_lines() {
    let line = "x".repeat(1024 * 1024 - 8);
    let rows: Vec<String> = (0..100).map(|i| if i % 3 == 0 { format!("{i} error {line}") } else { format!("{i} ok {line}") }).collect();
    let text = LogText::from_bytes(rows.join("\n").into_bytes());
    let f = filter();
    let mut events = Vec::new();
    scan_lines(&text, &f, 0..100, 65_536, &CancelToken::new(), |e| events.push(e));
    let chunks: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            FilterEvent::Chunk { lines, .. } => Some(lines.clone()),
            _ => None,
        })
        .collect();
    assert!(chunks.len() > 1, "expected several chunks, got {}", chunks.len());
    let mut next = 0;
    for r in &chunks {
        assert_eq!(r.start, next);
        let end = if r.end < 100 { text.index().line_start(r.end) } else { text.index().data_len() };
        let bytes = end - text.index().line_start(r.start);
        assert!(r.end - r.start == 1 || bytes <= MAX_CHUNK_BYTES, "chunk {r:?} too large ({bytes} bytes)");
        next = r.end;
    }
    assert_eq!(next, 100);
    let result = run_filter(&text, &f, 65_536, &CancelToken::new()).unwrap();
    let expected: Vec<u64> = (0..text.line_count()).filter(|&i| f.line_matches(text.line(i))).collect();
    assert_eq!(result.matches, expected);
}

#[test]
fn huge_chunk_lines_does_not_overflow() {
    let text = sample(100);
    let f = filter();
    let r = run_filter(&text, &f, u64::MAX, &CancelToken::new()).unwrap();
    let expected: Vec<u64> = (0..text.line_count()).filter(|&i| f.line_matches(text.line(i))).collect();
    assert_eq!(r.matches, expected);
}
