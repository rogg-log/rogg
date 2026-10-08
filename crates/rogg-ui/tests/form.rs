mod common;
use gpui_kit::prelude::*;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, TestAppContext, div, px, size};
use rogg_ui::form::{EditableRuleRow, FormRow, PaletteSlot, Stepper, TextField, Toggle};
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
fn a_narrow_rule_row_keeps_its_controls_full_size(cx: &mut TestAppContext) {
    common::setup(cx);
    // Narrower than the row's fixed parts (swatch, gaps, switch, move buttons): only the pattern may give way.
    let w = common::open(cx, 120., 100., |_, _| {
        div().w(px(120.)).child(EditableRuleRow::new(0, Color::rgb(0xefaaa2), r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3}", "Match text · both panels")).into_any_element()
    });
    assert_eq!(common::bounds(cx, w, "profile-rule-0-toggle").size, size(px(28.), px(16.)));
    assert_eq!(common::bounds(cx, w, "profile-rule-0-up").size, size(px(22.), px(22.)));
    assert_eq!(common::bounds(cx, w, "profile-rule-0-down").size, size(px(22.), px(22.)));
}

#[gpui_kit::test]
fn a_text_field_is_as_tall_as_a_button(cx: &mut TestAppContext) {
    common::setup(cx);
    let w = common::open(cx, 300., 100., |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx).default_value("#ffaa00"));
        div().flex().items_start().child(TextField::new("hex", input).mono(true).width(96.)).into_any_element()
    });
    assert_eq!(common::bounds(cx, w, "hex").size, size(px(96.), px(22.)));
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

#[gpui_kit::test]
fn custom_colour_renders_a_picker_trigger(cx: &mut TestAppContext) {
    use gpui_kit::component::color_picker::ColorPickerState;
    use rogg_ui::form::CustomColor;
    common::setup(cx);
    let slot: Rc<RefCell<Option<gpui_kit::Entity<ColorPickerState>>>> = Rc::default();
    let w = common::open(cx, 400., 200., move |window, cx| {
        let state = slot.borrow_mut().get_or_insert_with(|| cx.new(|cx| ColorPickerState::new(window, cx))).clone();
        CustomColor::new("rule-custom", &state).featured(vec![Color::rgb(0xff0000)]).into_any_element()
    });
    assert!(common::exists(cx, w, "rule-custom"));
}

#[gpui_kit::test]
fn custom_colour_is_a_swatch_sized_square_with_a_pen_and_still_opens_the_picker(cx: &mut TestAppContext) {
    use gpui_kit::component::color_picker::ColorPickerState;
    use rogg_ui::form::{CustomColor, Swatch};
    common::setup(cx);
    let slot: Rc<RefCell<Option<gpui_kit::Entity<ColorPickerState>>>> = Rc::default();
    let s = slot.clone();
    let w = common::open(cx, 400., 200., move |window, cx| {
        let state = s.borrow_mut().get_or_insert_with(|| cx.new(|cx| ColorPickerState::new(window, cx))).clone();
        div()
            .flex()
            .gap(px(8.))
            .child(Swatch::new("preset", Color::rgb(0xff0000)))
            .child(CustomColor::new("rule-custom", &state))
            .into_any_element()
    });
    let swatch = common::bounds(cx, w, "preset");
    let custom = common::bounds(cx, w, "rule-custom");
    assert_eq!(custom.size, swatch.size);
    assert_eq!(custom.size, size(px(21.), px(21.)));
    assert_eq!(custom.origin.y, swatch.origin.y, "lines up with the preset row");
    let pen = common::bounds(cx, w, "rule-custom-pen");
    assert_eq!(pen.size, size(px(11.), px(11.)));
    assert!(custom.contains(&pen.center()), "the pen sits inside the square");
    let state = slot.borrow().clone().unwrap();
    assert!(!cx.read_entity(&state, |s, _| s.is_open()));
    common::click(cx, w, "rule-custom");
    assert!(cx.read_entity(&state, |s, _| s.is_open()), "clicking the square opens the popover");
}
