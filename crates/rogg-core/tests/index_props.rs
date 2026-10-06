use proptest::prelude::*;
use rogg_core::CancelToken;
use rogg_core::index::{LineIndex, build_index, build_index_into};
use std::ops::Range;

fn bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(prop_oneof![Just(b'a'), Just(b'x'), Just(b'\n'), Just(b'\r')], 0..400)
}

fn naive_lines(data: &[u8]) -> Vec<Range<u64>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, &b) in data.iter().enumerate() {
        if b == b'\n' {
            out.push(start as u64..i as u64);
            start = i + 1;
        }
    }
    if start < data.len() {
        out.push(start as u64..data.len() as u64);
    }
    out
}

proptest! {
    #[test]
    fn serial_matches_naive_split(data in bytes()) {
        let idx = LineIndex::from_bytes(&data);
        let naive = naive_lines(&data);
        prop_assert_eq!(idx.line_count(), naive.len() as u64);
        for (i, r) in naive.iter().enumerate() {
            prop_assert_eq!(idx.line_range(i as u64), r.clone());
        }
        let longest = naive.iter().map(|r| r.end - r.start).max().unwrap_or(0);
        prop_assert_eq!(idx.longest_line(), longest);
    }

    #[test]
    fn parallel_equals_serial(data in bytes(), chunk in 1usize..64, span in prop_oneof![Just(u32::MAX as u64), 1u64..16]) {
        let par = build_index_into(LineIndex::with_max_block_span(span), &data, chunk, &CancelToken::new(), |_, _| {}).unwrap();
        let mut serial = LineIndex::with_max_block_span(span);
        serial.extend_to(&data);
        prop_assert_eq!(par, serial);
    }

    #[test]
    fn incremental_equals_full(data in bytes(), mut cuts in proptest::collection::vec(0usize..400, 0..6)) {
        cuts.sort_unstable();
        let mut idx = LineIndex::new();
        for c in cuts {
            idx.extend_to(&data[..c.min(data.len())]);
        }
        idx.extend_to(&data);
        prop_assert_eq!(idx, LineIndex::from_bytes(&data));
    }

    #[test]
    fn parallel_build_default_entry_point(data in bytes(), chunk in 1usize..64) {
        prop_assert_eq!(build_index(&data, chunk, &CancelToken::new(), |_, _| {}).unwrap(), LineIndex::from_bytes(&data));
    }
}
