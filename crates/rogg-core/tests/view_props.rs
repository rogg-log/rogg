use proptest::prelude::*;
use rogg_core::view::{FilteredView, ViewRow};
use std::collections::BTreeSet;
use std::sync::Arc;

fn naive_rows(matches: &BTreeSet<u64>, pins: &BTreeSet<u64>, n: u64, total: u64, excluded: &BTreeSet<u64>) -> Vec<ViewRow> {
    let anchors: BTreeSet<u64> = matches.union(pins).copied().filter(|&l| l < total).collect();
    let shown: Vec<u64> = (0..total)
        .filter(|&j| anchors.contains(&j) || (!excluded.contains(&j) && anchors.iter().any(|&a| a.abs_diff(j) <= n)))
        .collect();
    let mut rows = Vec::new();
    for (k, &l) in shown.iter().enumerate() {
        if k > 0 && l > shown[k - 1] + 1 {
            rows.push(ViewRow::Gap { omitted: l - shown[k - 1] - 1 });
        }
        rows.push(ViewRow::Line { number: l, matched: matches.contains(&l), pinned: pins.contains(&l) });
    }
    rows
}

proptest! {
    #[test]
    fn view_equals_naive(
        total in 0u64..3_000,
        matches in proptest::collection::btree_set(0u64..3_000, 0..300),
        pins in proptest::collection::btree_set(0u64..3_100, 0..10),
        excluded in proptest::collection::btree_set(0u64..3_000, 0..300),
        n in 0u32..4,
    ) {
        let matches: BTreeSet<u64> = matches.into_iter().filter(|&m| m < total && !excluded.contains(&m)).collect();
        let view = FilteredView::build(Arc::new(matches.iter().copied().collect()), &pins, n, total, |l| excluded.contains(&l));
        let expected = naive_rows(&matches, &pins, n as u64, total, &excluded);
        prop_assert_eq!(view.row_count(), expected.len() as u64);
        let naive_context = expected.iter().filter(|r| matches!(r, ViewRow::Line { matched: false, pinned: false, .. })).count() as u64;
        prop_assert_eq!(view.context_lines(), naive_context);
        for (r, row) in expected.iter().enumerate() {
            prop_assert_eq!(view.row(r as u64), Some(*row));
            if let ViewRow::Line { number, .. } = row {
                prop_assert_eq!(view.row_for_line(*number), Some(r as u64));
            }
        }
    }
}
