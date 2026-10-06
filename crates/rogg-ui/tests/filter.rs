mod common;
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, px};
use rogg_ui::filter::{ChipState, FilterBand};
use std::cell::Cell;
use std::rc::Rc;

fn terms(n: usize) -> Vec<(gpui_kit::SharedString, ChipState)> {
    (0..n).map(|i| (format!("term{i}").into(), [ChipState::Include, ChipState::Off, ChipState::Exclude][i % 3])).collect()
}

#[test]
fn state_cycles_include_off_exclude() {
    assert_eq!(ChipState::Include.next(), ChipState::Off);
    assert_eq!(ChipState::Off.next(), ChipState::Exclude);
    assert_eq!(ChipState::Exclude.next(), ChipState::Include);
    assert_eq!([ChipState::Include.label(), ChipState::Off.label(), ChipState::Exclude.label()], ["IN", "OFF", "NOT"]);
}

#[gpui_kit::test]
fn chips_and_flags_use_reference_heights(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1200., 200., |_, _| FilterBand::new(terms(4)).into_any_element());
    assert_eq!(common::bounds(cx, w, "token-0").size.height, px(28.));
    assert_eq!(common::bounds(cx, w, "flag-regex").size.height, px(27.));
    let band = common::bounds(cx, w, "filter-band");
    let line = common::bounds(cx, w, "filters");
    // 1px top border + 10px padding-top, then the 28px chip row is centered in the 30px filter line (+1).
    assert_eq!(line.size.height, px(28.));
    assert_eq!(line.origin.y - band.origin.y, px(12.));
}

#[gpui_kit::test]
fn twenty_chips_keep_actions_visible(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 900., 200., |_, _| FilterBand::new(terms(20)).into_any_element());
    let actions = common::bounds(cx, w, "filter-actions");
    assert!(actions.right() <= px(900. - 12.) + px(0.5), "actions pushed out: {actions:?}");
}

#[gpui_kit::test]
fn overflowing_chip_row_stays_left_of_actions(cx: &mut TestAppContext) {
    common::setup(cx);
    // 60 chips overflow 900px even with zero-size text (each chip has fixed paddings, borders and gaps);
    // the row must clip/scroll inside its own box instead of pushing the actions out or overlapping them.
    let w = common::open(cx, 900., 200., |_, _| FilterBand::new(terms(60)).into_any_element());
    let row = common::bounds(cx, w, "filters");
    let actions = common::bounds(cx, w, "filter-actions");
    assert!(row.right() <= actions.left() + px(0.5), "chip row {row:?} overlaps actions {actions:?}");
    assert!(actions.right() <= px(900. - 12.) + px(0.5), "actions pushed out: {actions:?}");
    let (edit, case) = (common::bounds(cx, w, "edit-filters"), common::bounds(cx, w, "flag-case"));
    assert!(edit.left() >= actions.left() && case.right() <= actions.right() + px(0.5), "actions clipped: {actions:?}");
}

#[gpui_kit::test]
fn overflowing_chip_row_scrolls_horizontally(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 900., 200., |_, _| FilterBand::new(terms(60)).into_any_element());
    let before = common::bounds(cx, w, "token-0").left();
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.scroll("filters", gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(-120.), px(0.))), cx);
    })
    .unwrap();
    let after = common::bounds(cx, w, "token-0").left();
    assert!(after < before - px(50.), "chip row did not scroll: token-0 x {before:?} -> {after:?}");
}

#[gpui_kit::test]
fn edit_mode_swaps_chips_for_expression(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1200., 200., |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx).default_value("error || timeout"));
        FilterBand::new(terms(2)).editing(Some(input)).error(Some("regex parse error".into())).into_any_element()
    });
    assert!(common::exists(cx, w, "expression"));
    assert!(common::exists(cx, w, "apply"));
    assert!(common::exists(cx, w, "filter-error"));
    assert!(!common::exists(cx, w, "edit-filters"));
    assert!(!common::exists(cx, w, "token-0"));
}

#[gpui_kit::test]
fn clicking_state_reports_chip_index(cx: &mut TestAppContext) {
    common::setup(cx);
    let hit = Rc::new(Cell::new(usize::MAX));
    let h = hit.clone();
    let w = common::open(cx, 1200., 200., move |_, _| {
        let h = h.clone();
        FilterBand::new(terms(3)).on_cycle(move |i, _, _| h.set(i)).into_any_element()
    });
    common::click(cx, w, "token-state-1");
    assert_eq!(hit.get(), 1);
}
