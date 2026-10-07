//! The window's keyboard actions (spec §5.9): focus cycling, Go to line, theme and density.

use super::{Dialog, Workspace};
use crate::actions::*;
use crate::document::Document;
use crate::tab::Tab;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, Stateful, Window, WindowAppearance};
use rogg_ui::theme::{ActiveRoggTheme, Appearance, Density, RoggTheme};

/// View → Theme. System follows the macOS appearance live (spec §6.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

pub fn appearance_for(choice: ThemeChoice, window: WindowAppearance) -> Appearance {
    match choice {
        ThemeChoice::Dark => Appearance::Dark,
        ThemeChoice::Light => Appearance::Light,
        ThemeChoice::System => match window {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
            _ => Appearance::Light,
        },
    }
}

/// A window action's listener that does nothing while a dialog is open (the dialog is modal: only `Dismiss`,
/// `Quit` and the dialog's own input stay live; Open / Open Folder / Open Recent open native panels or tabs and are
/// registered without this guard).
pub(super) fn unless_dialog<A: gpui_kit::Action>(
    cx: &mut Context<Workspace>,
    f: impl Fn(&mut Workspace, &A, &mut Window, &mut Context<Workspace>) + 'static,
) -> impl Fn(&A, &mut Window, &mut App) + 'static {
    cx.listener(move |ws, action: &A, window, cx| {
        if ws.dialog.is_none() {
            f(ws, action, window, cx);
        }
    })
}

/// Registers the keyboard actions on the workspace root.
pub(super) fn bind(root: Stateful<Div>, cx: &mut Context<Workspace>) -> Stateful<Div> {
    root.on_action(unless_dialog(cx, |ws, _: &Find, window, cx| ws.with_tab(cx, |t, cx| t.focus_find(window, cx))))
        .on_action(unless_dialog(cx, |ws, _: &EditFilters, window, cx| ws.with_tab(cx, |t, cx| t.start_edit(window, cx))))
        .on_action(unless_dialog(cx, |ws, _: &ToggleRegex, _, cx| ws.with_doc(cx, |d, cx| d.toggle_regex(cx))))
        .on_action(unless_dialog(cx, |ws, _: &ToggleCase, _, cx| ws.with_doc(cx, |d, cx| d.toggle_case(cx))))
        .on_action(unless_dialog(cx, |ws, _: &TogglePin, _, cx| ws.with_tab(cx, |t, cx| t.pin_selected(cx))))
        .on_action(unless_dialog(cx, |ws, _: &RevealInFullLog, _, cx| ws.with_tab(cx, |t, cx| t.reveal_selected(cx))))
        .on_action(unless_dialog(cx, |ws, _: &ToggleFollow, _, cx| ws.with_tab(cx, |t, cx| t.toggle_follow(cx))))
        .on_action(unless_dialog(cx, |ws, _: &ToggleLive, _, cx| ws.with_tab(cx, |t, cx| t.toggle_live(cx))))
        .on_action(unless_dialog(cx, |ws, _: &ToggleWrap, _, cx| {
            ws.with_tab(cx, |t, cx| {
                let pane = t.last_pane;
                t.toggle_wrap(pane, cx);
            })
        }))
        .on_action(unless_dialog(cx, |ws, _: &FocusNext, window, cx| ws.with_tab(cx, |t, cx| t.cycle_focus(true, window, cx))))
        .on_action(unless_dialog(cx, |ws, _: &FocusPrev, window, cx| ws.with_tab(cx, |t, cx| t.cycle_focus(false, window, cx))))
        .on_action(unless_dialog(cx, |ws, _: &GoToLine, window, cx| ws.open_go_to_line(window, cx)))
        .on_action(unless_dialog(cx, |ws, _: &ThemeSystem, window, cx| ws.set_theme(ThemeChoice::System, window, cx)))
        .on_action(unless_dialog(cx, |ws, _: &ThemeLight, window, cx| ws.set_theme(ThemeChoice::Light, window, cx)))
        .on_action(unless_dialog(cx, |ws, _: &ThemeDark, window, cx| ws.set_theme(ThemeChoice::Dark, window, cx)))
        .on_action(unless_dialog(cx, |ws, _: &DensityCompact, window, cx| ws.set_density(Density::Compact, window, cx)))
        .on_action(unless_dialog(cx, |ws, _: &DensityComfortable, window, cx| ws.set_density(Density::Comfortable, window, cx)))
        .on_action(cx.listener(|_, _: &Minimize, window, _| window.minimize_window()))
        .on_action(cx.listener(|_, _: &Zoom, window, _| window.zoom_window()))
}

impl Workspace {
    fn with_tab(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Tab, &mut Context<Tab>)) {
        if let Some(tab) = self.active_tab() {
            tab.update(cx, f);
        }
    }

    fn with_doc(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Document, &mut Context<Document>)) {
        if let Some(tab) = self.active_tab() {
            let doc = tab.read(cx).doc.clone();
            doc.update(cx, f);
        }
    }

    /// ⌘G: a dialog with a line-number field; Enter selects and centres the line in the full log.
    pub fn open_go_to_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_tab().is_none() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Line number, e.g. 120492"));
        input.update(cx, |i, cx| i.focus(window, cx));
        self.goto_subscription = Some(cx.subscribe_in(&input, window, |ws, input, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let text = input.read(cx).value().to_string();
                ws.go_to_line(&text, window, cx);
            }
        }));
        self.open_dialog(Dialog::GoToLine(input), cx);
    }

    fn go_to_line(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab() else { return };
        match text.trim().replace(',', "").parse::<u64>() {
            Ok(number) => {
                if tab.update(cx, |t, cx| t.go_to_line(number, cx)) {
                    self.close_dialog(cx);
                    tab.read(cx).focus[0].clone().focus(window, cx);
                } else {
                    self.toast(format!("Line {number} is not in this log"), cx);
                }
            }
            Err(_) => self.toast("Type a line number", cx),
        }
    }

    pub fn set_theme(&mut self, choice: ThemeChoice, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = choice;
        self.apply_theme(window, cx);
    }

    pub fn set_density(&mut self, density: Density, window: &mut Window, cx: &mut Context<Self>) {
        self.density = density;
        self.apply_theme(window, cx);
    }

    /// Installs the theme for the current choice and density (only if it changes).
    pub(crate) fn apply_theme(&self, window: &Window, cx: &mut App) {
        let appearance = appearance_for(self.theme, window.appearance());
        let current = cx.rogg();
        if current.appearance != appearance || current.density != self.density {
            RoggTheme::new(appearance, self.density).install(cx);
        }
    }
}
