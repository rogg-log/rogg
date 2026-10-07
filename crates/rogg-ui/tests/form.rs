mod common;
use gpui_kit::prelude::*;
use gpui_kit::{TestAppContext, div};
use rogg_ui::form::{EditableRuleRow, FormRow, PaletteSlot, Stepper, Toggle};
use rogg_ui::theme::Color;
use std::cell::RefCell;
use std::rc::Rc;

fn log() -> Rc<RefCell<Vec<String>>> {
    Rc::new(RefCell::new(Vec::new()))
}

#[gpui_kit::test]
fn toggle_and_stepper_report_clicks_and_respect_bounds(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = log();
    let e = events.clone();
    let w = common::open(cx, 600., 300., move |_, _| {
        let (a, b) = (e.clone(), e.clone());
        div()
            .child(FormRow::new("row-wrap", "Wrap lines").control(Toggle::new("wrap", false).on_click(move |_, _| a.borrow_mut().push("toggle".into()))))
            .child(FormRow::new("row-size", "Log font size").control(Stepper::new("size", "20 px").bounds(true, false).on_step(move |up, _, _| b.borrow_mut().push(format!("step {up}")))))
            .into_any_element()
    });
    common::click(cx, w, "wrap");
    common::click(cx, w, "size-down");
    common::click(cx, w, "size-up");
    assert_eq!(*events.borrow(), ["toggle", "step false"], "the up button at the bound does nothing");
    assert!(common::exists(cx, w, "row-size"));
}

#[gpui_kit::test]
fn rule_rows_select_toggle_move_and_show_errors(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = log();
    let e = events.clone();
    let w = common::open(cx, 600., 300., move |_, _| {
        let (a, b, c) = (e.clone(), e.clone(), e.clone());
        EditableRuleRow::new(1, Color::rgb(0xefaaa2), "ERROR", "Line text + row tint · both panels")
            .enabled(true)
            .selected(true)
            .error(Some("invalid regex".into()))
            .can_move(true, false)
            .on_select(move |_, _| a.borrow_mut().push("select".into()))
            .on_toggle(move |_, _| b.borrow_mut().push("toggle".into()))
            .on_move(move |up, _, _| c.borrow_mut().push(format!("move {up}")))
            .into_any_element()
    });
    common::click(cx, w, "profile-rule-1-toggle");
    common::click(cx, w, "profile-rule-1-up");
    common::click(cx, w, "profile-rule-1-down");
    common::click(cx, w, "profile-rule-1");
    assert_eq!(*events.borrow(), ["toggle", "move true", "select"], "controls do not also select the row; down is disabled");
    assert!(common::exists(cx, w, "profile-rule-1-error"));
}

#[gpui_kit::test]
fn palette_slots_are_clickable(cx: &mut TestAppContext) {
    common::setup(cx);
    let events = log();
    let e = events.clone();
    let w = common::open(cx, 400., 100., move |_, _| {
        let a = e.clone();
        PaletteSlot::new(2, Color::rgb(0xacd7ea), Color::rgb(0x315064)).selected(true).on_click(move |_, _| a.borrow_mut().push("slot".into())).into_any_element()
    });
    common::click(cx, w, "palette-slot-2");
    assert_eq!(*events.borrow(), ["slot"]);
}
