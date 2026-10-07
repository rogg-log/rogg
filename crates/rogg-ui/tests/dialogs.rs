mod common;
use gpui_kit::prelude::*;
use gpui_kit::{SharedString, TestAppContext, TestSupportExt, div, point, px};
use rogg_ui::dialogs::{ModalLayer, all_tabs, shortcuts};
use rogg_ui::empty::{loading, no_matches};
use std::cell::Cell;
use std::rc::Rc;

#[gpui_kit::test]
fn dialog_frame_is_500_wide_and_centred(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1440., 900., |_, _| {
        ModalLayer::new(shortcuts(&[("Find in focused panel".into(), "⌘ F".into()), ("Edit filters".into(), "⌘ L".into())])).into_any_element()
    });
    let frame = common::bounds(cx, w, "dialog");
    assert_eq!(frame.size.width, px(500.));
    assert!((frame.center().x - px(720.)).abs() <= px(0.5));
    assert!(common::exists(cx, w, "shortcut-row-1"));
}

#[gpui_kit::test]
fn backdrop_click_dismisses_but_frame_click_does_not(cx: &mut TestAppContext) {
    common::setup(cx);
    let dismissed = Rc::new(Cell::new(0));
    let d = dismissed.clone();
    let w = common::open(cx, 1440., 900., move |_, _| {
        let d = d.clone();
        ModalLayer::new(shortcuts(&[])).on_dismiss(move |_, _| d.set(d.get() + 1)).into_any_element()
    });
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click_at("dialog", point(px(250.), px(10.)), cx);
        window.click_at("modal-layer", point(px(5.), px(5.)), cx);
    })
    .unwrap();
    assert_eq!(dismissed.get(), 1);
}

#[gpui_kit::test]
fn backdrop_click_does_not_reach_content_underneath(cx: &mut TestAppContext) {
    common::setup(cx);
    let dismissed = Rc::new(Cell::new(0));
    let under = Rc::new(Cell::new(0));
    let (d, u) = (dismissed.clone(), under.clone());
    let w = common::open(cx, 1440., 900., move |_, _| {
        let (d, u) = (d.clone(), u.clone());
        div()
            .relative()
            .size_full()
            .child(div().id("underneath").test_support().size_full().on_click(move |_, _, _| u.set(u.get() + 1)))
            .child(ModalLayer::new(shortcuts(&[])).on_dismiss(move |_, _| d.set(d.get() + 1)))
            .into_any_element()
    });
    cx.update_window(w, |_, window, cx| {
        use gpui_kit::test::TestWindowExt;
        window.render_frame(cx);
        window.click_at("modal-layer", point(px(5.), px(5.)), cx);
    })
    .unwrap();
    assert_eq!(dismissed.get(), 1);
    assert_eq!(under.get(), 0);
}

#[gpui_kit::test]
fn tall_dialog_is_capped_to_the_window_and_stays_centred(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 1440., 900., |_, _| {
        let tabs: Vec<(SharedString, bool)> = (0..40).map(|i| (SharedString::from(format!("file-{i}.log")), i == 3)).collect();
        ModalLayer::new(all_tabs(&tabs, Rc::new(|_, _, _| {}))).into_any_element()
    });
    let frame = common::bounds(cx, w, "dialog");
    assert_eq!(frame.size.height, px(820.), "capped to the window minus the 80px margin");
    assert!(frame.origin.y >= px(0.));
    assert!((frame.center().y - px(450.)).abs() <= px(0.5));
    assert!((frame.center().x - px(720.)).abs() <= px(0.5));
}

#[gpui_kit::test]
fn no_matches_fills_its_box(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 300., |_, _| no_matches("empty-results").into_any_element());
    assert_eq!(common::bounds(cx, w, "empty-results").size.height, px(300.));
}

#[gpui_kit::test]
fn loading_bar_fill_is_clamped_and_ignores_non_finite_fractions(cx: &mut TestAppContext) {
    common::setup(cx);
    // The bar is 220px wide; no fraction shows the indeterminate 30%.
    for (fraction, expected) in
        [(None, 66.), (Some(0.5), 110.), (Some(2.), 220.), (Some(-1.), 0.), (Some(f32::NAN), 0.), (Some(f32::INFINITY), 0.), (Some(f32::NEG_INFINITY), 0.)]
    {
        let w = common::open(cx, 600., 300., move |_, _| loading("Opening", fraction, Rc::new(|_, _| {})).into_any_element());
        assert_eq!(common::bounds(cx, w, "loading-fill").size.width, px(expected), "fraction {fraction:?}");
    }
}

#[gpui_kit::test]
fn history_picker_rows_are_clickable(cx: &mut gpui_kit::TestAppContext) {
    common::setup(cx);
    let picked = Rc::new(Cell::new(None));
    let p = picked.clone();
    let w = common::open(cx, 1000., 700., move |_, _| {
        let p = p.clone();
        let on_pick: rogg_ui::IndexCallback = Rc::new(move |i: usize, _: &mut gpui_kit::Window, _: &mut gpui_kit::App| p.set(Some(i)));
        rogg_ui::dialogs::history_picker(&[("a || b".into(), "2 include · 0 exclude · 0 off".into()), ("c".into(), "1 include · 0 exclude · 0 off".into())], on_pick)
            .into_any_element()
    });
    common::click(cx, w, "history-pick-1");
    assert_eq!(picked.get(), Some(1));
}

#[gpui_kit::test]
fn the_generic_picker_reports_picks(cx: &mut TestAppContext) {
    common::setup(cx);
    let picked = Rc::new(Cell::new(usize::MAX));
    let p = picked.clone();
    let w = common::open(cx, 900., 700., move |_, _| {
        let p = p.clone();
        rogg_ui::dialogs::picker("Choose a log font", &[("Menlo".into(), false), ("Monaco".into(), true)], Rc::new(move |i, _, _| p.set(i))).into_any_element()
    });
    common::click(cx, w, "picker-0");
    assert_eq!(picked.get(), 0);
}
