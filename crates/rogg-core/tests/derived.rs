use rogg_core::CancelToken;
use rogg_core::cache::Cache;
use rogg_core::derived::{materialize, rebuild, view_lines};
use rogg_core::filter::FilterSpec;
use rogg_core::session::{DerivedSpec, SessionSource};
use rogg_core::text::LogText;
use std::collections::BTreeSet;

fn lines(text: &LogText) -> Vec<String> {
    (0..text.line_count()).map(|i| text.line_text(i).into_owned()).collect()
}

#[test]
fn view_lines_are_matches_pins_and_context() {
    let text = LogText::from_bytes(b"a0\nb1\na2\nb3\nb4\nb5\na6\nb7".to_vec());
    let spec = FilterSpec { context: 1, ..FilterSpec::from_expression("a") };
    let shown = view_lines(&text, &spec, &BTreeSet::from([4]), &CancelToken::new()).unwrap();
    assert_eq!(shown, vec![0, 1, 2, 3, 4, 5, 6, 7]);
    let no_context = view_lines(&text, &FilterSpec::from_expression("a"), &BTreeSet::from([4]), &CancelToken::new()).unwrap();
    assert_eq!(no_context, vec![0, 2, 4, 6]);
}

#[test]
fn materialize_copies_lines() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path().join("cache"), 1 << 30);
    let text = LogText::from_bytes(b"zero\r\none\ntwo\nthree".to_vec());
    let mut seen = Vec::new();
    let d = materialize(&text, &[1, 3], &cache, &CancelToken::new(), |done, total| seen.push((done, total))).unwrap();
    assert_eq!(lines(&d.text), ["one", "three"]);
    assert!(d.path.starts_with(cache.streams_dir()));
    assert_eq!(seen.last(), Some(&(2, 2)));
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(materialize(&text, &[0], &cache, &cancel, |_, _| {}), Err(rogg_core::CoreError::Cancelled)));
    let leftovers = std::fs::read_dir(cache.streams_dir()).unwrap().count();
    assert_eq!(leftovers, 1, "only the finished derived file; the cancelled one left no partial");
}

#[test]
fn nested_rebuild_applies_each_level() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path().join("cache"), 1 << 30);
    let log = dir.path().join("app.log");
    std::fs::write(&log, "x error a\nok\nerror b\nfine\nerror a again\n").unwrap();
    let first = DerivedSpec { parent: SessionSource::File { path: log.clone() }, filter: FilterSpec::from_expression("error"), pins: vec![], lines: None, snapshot: None };
    let nested = DerivedSpec { parent: SessionSource::Derived(Box::new(first)), filter: FilterSpec::from_expression("a"), pins: vec![], lines: None, snapshot: None };
    let r = rebuild(&nested, &cache, &CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(lines(&r.derived.text), ["x error a", "error a again"]);
    let derived_files = std::fs::read_dir(cache.streams_dir()).unwrap().count();
    assert_eq!(derived_files, 1, "the intermediate level's file is removed once the nested one is written");
}

#[test]
fn explicit_lines_win_over_the_filter_and_missing_parents_fail() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path().join("cache"), 1 << 30);
    let log = dir.path().join("live.log");
    std::fs::write(&log, "a\nb\nc\n").unwrap();
    let spec = DerivedSpec { parent: SessionSource::File { path: log.clone() }, filter: FilterSpec::from_expression("zzz"), pins: vec![], lines: Some(vec![2, 0, 9]), snapshot: Some("14:32:08".into()) };
    let r = rebuild(&spec, &cache, &CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(lines(&r.derived.text), ["a", "c"], "sorted, and lines past the end are dropped");
    let gone = DerivedSpec { parent: SessionSource::File { path: dir.path().join("gone.log") }, ..spec };
    assert!(matches!(rebuild(&gone, &cache, &CancelToken::new(), |_, _| {}), Err(rogg_core::CoreError::Io { .. })));
}

#[test]
fn rebuild_reads_a_whole_gz_parent() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path().join("cache"), 1 << 30);
    let body: String = (0..50_000).map(|i| format!("line {i} {}\n", if i % 10_000 == 0 { "hit" } else { "-" })).collect();
    let gz = dir.path().join("big.log.gz");
    let mut enc = flate2::write::GzEncoder::new(std::fs::File::create(&gz).unwrap(), flate2::Compression::fast());
    std::io::Write::write_all(&mut enc, body.as_bytes()).unwrap();
    enc.finish().unwrap();
    let spec = DerivedSpec { parent: SessionSource::File { path: gz }, filter: FilterSpec::from_expression("hit"), pins: vec![], lines: None, snapshot: None };
    let r = rebuild(&spec, &cache, &CancelToken::new(), |_, _| {}).unwrap();
    assert_eq!(lines(&r.derived.text), ["line 0 hit", "line 10000 hit", "line 20000 hit", "line 30000 hit", "line 40000 hit"]);
}
