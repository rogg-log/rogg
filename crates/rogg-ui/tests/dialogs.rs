mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, point, px};
use rogg_ui::dialogs::{ModalLayer, shortcuts};
use rogg_ui::empty::no_matches;
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
fn no_matches_fills_its_box(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 600., 300., |_, _| no_matches("empty-results").into_any_element());
    assert_eq!(common::bounds(cx, w, "empty-results").size.height, px(300.));
}
