//! The Export dialog (spec §6.5): filtered lines to a `.log`, the session to a `.rogg`, the filter expression to the
//! clipboard.

use crate::workspace::{Workspace, act};
use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, div, px};
use rogg_ui::controls::{ActionButton, Segmented};
use rogg_ui::dialogs::DialogFrame;
use rogg_ui::form::{FormRow, SectionLabel, Toggle};
use rogg_ui::theme::ActiveRoggTheme;

/// `ws` is the workspace being rendered (`this` cannot be read while it renders).
pub fn render(ws: &Workspace, this: &Entity<Workspace>, cx: &App) -> DialogFrame {
    let o = ws.export_options;
    let expression = ws.active_tab().map(|t| t.read(cx).doc.read(cx).filter.spec.expression()).unwrap_or_default();
    let t = cx.rogg().clone();
    DialogFrame::new("Export")
        .description("Exports run in the background; the status bar shows their progress.")
        .child(SectionLabel::new("Filtered lines"))
        .child(FormRow::new("row-export-context", "Include context lines").control(Toggle::new("export-context", o.context).on_click(act(this, |ws, _, cx| {
            ws.export_options.context = !ws.export_options.context;
            cx.notify();
        }))))
        .child(FormRow::new("row-export-gaps", "Include gap markers").control(Toggle::new("export-gaps", o.gaps).on_click(act(this, |ws, _, cx| {
            ws.export_options.gaps = !ws.export_options.gaps;
            cx.notify();
        }))))
        .child(FormRow::new("row-export-numbers", "Prefix original line numbers").control(Toggle::new("export-numbers", o.numbers).on_click(act(this, |ws, _, cx| {
            ws.export_options.numbers = !ws.export_options.numbers;
            cx.notify();
        }))))
        .child(div().flex().justify_end().pt(px(8.)).child(ActionButton::new("export-lines", "Export lines…").primary().on_click(act(this, |ws, window, cx| ws.prompt_export_lines(window, cx)))))
        .child(SectionLabel::new("Session"))
        .child(FormRow::new("row-export-scope", "Tabs").control(Segmented::new("export-scope", vec!["This tab".into(), "All tabs".into()], ws.export_all_tabs as usize).on_select({
            let this = this.clone();
            move |i, _, cx| {
                this.update(cx, |ws, cx| {
                    ws.export_all_tabs = i == 1;
                    cx.notify();
                })
            }
        })))
        .child(div().flex().justify_end().pt(px(8.)).child(ActionButton::new("export-session", "Save session…").on_click(act(this, |ws, window, cx| ws.prompt_export_session(window, cx)))))
        .child(SectionLabel::new("Filter"))
        .child(FormRow::new("row-export-expression", "Filter expression").control(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().max_w(px(220.)).truncate().font_family(t.fonts.mono.clone()).text_size(t.text_scale().px(11.)).text_color(t.palette.muted).child(expression))
                .child(ActionButton::new("export-copy", "Copy").on_click(act(this, |ws, _, cx| ws.copy_expression(cx)))),
        ))
}
