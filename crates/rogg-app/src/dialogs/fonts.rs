//! "Other…" log fonts (spec §6.2): every installed monospace family. A family counts as monospace when `i`, `W` and
//! `.` have the same advance; measured once per run.

use crate::settings::SettingsStore;
use crate::workspace::Workspace;
use gpui_kit::{App, Entity, Global, SharedString, font, px};
use rogg_ui::dialogs::{DialogFrame, picker};
use rogg_ui::theme::{ActiveRoggTheme, ChromeStyle};
use std::rc::Rc;

pub struct MonoFonts(pub Vec<String>);

impl Global for MonoFonts {}

pub fn is_monospace(advances: [Option<f32>; 3]) -> bool {
    matches!(advances, [Some(a), Some(b), Some(c)] if a > 0. && (a - b).abs() < 0.01 && (b - c).abs() < 0.01)
}

/// The installed monospace families, measured on first use and cached.
pub fn monospace_fonts(cx: &mut App) -> Vec<String> {
    if let Some(fonts) = cx.try_global::<MonoFonts>() {
        return fonts.0.clone();
    }
    let ts = cx.text_system().clone();
    let fonts: Vec<String> = ts
        .all_font_names()
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .filter(|name| {
            let id = ts.resolve_font(&font(name.clone()));
            is_monospace(['i', 'W', '.'].map(|c| ts.advance(id, px(12.), c).ok().map(|s| f32::from(s.width))))
        })
        .collect();
    cx.set_global(MonoFonts(fonts.clone()));
    fonts
}

pub fn render(this: &Entity<Workspace>, cx: &App) -> DialogFrame {
    let fonts = cx.try_global::<MonoFonts>().map(|f| f.0.clone()).unwrap_or_default();
    let current = SettingsStore::get(cx).appearance.log_font;
    let items: Vec<(SharedString, bool)> = fonts.iter().map(|f| (SharedString::from(f.clone()), *f == current)).collect();
    let this = this.clone();
    picker(
        "Choose a log font",
        &items,
        Rc::new(move |i, window, cx| {
            if let Some(name) = fonts.get(i).cloned() {
                this.update(cx, |ws, cx| ws.pick_log_font(&name, window, cx));
            }
        }),
    )
    .description(picker_description(cx.rogg().chrome))
}

/// The font picker's description: "this Mac" in the Mac style, "this computer" in the others.
pub fn picker_description(chrome: ChromeStyle) -> &'static str {
    if chrome.is_mac() { "Monospace fonts installed on this Mac." } else { "Monospace fonts installed on this computer." }
}
