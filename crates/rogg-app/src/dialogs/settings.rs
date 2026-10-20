//! The Settings dialog (spec §6.2): appearance and behaviour. Every change is applied and saved at once.

use crate::profiles::ProfileStore;
use crate::settings::{SettingsStore, available_log_fonts, default_log_font_label};
use crate::workspace::{Dialog, Workspace};
use gpui_kit::{App, Entity, SharedString, Window};
use rogg_core::filter::MAX_CONTEXT;
use rogg_core::settings::{CACHE_LIMITS_GB, DEFAULT_LOG_FONT, DensitySetting, LOG_FONT_SIZES, SCAN_DEPTHS, Settings, ThemeSetting, UI_FONT_SIZES, step_cache_limit};
use rogg_core::stats::noun;
use rogg_ui::controls::{CompactSelect, Segmented};
use rogg_ui::dialogs::DialogFrame;
use rogg_ui::form::{FormRow, SectionLabel, Stepper, Toggle};
use rogg_ui::theme::{ActiveRoggTheme, ChromeStyle};

pub const OTHER_FONT: &str = "Other…";

/// `None`, `±1 line`, `±7 lines`.
pub fn context_label(n: u32) -> String {
    if n == 0 { "None".into() } else { format!("±{n} {}", noun(n as u64, "line", "lines")) }
}

/// The log font select: the curated fonts installed here, the current font when it is another one, then `Other…`.
pub fn log_font_options(available: &[String], current: &str) -> (Vec<SharedString>, usize) {
    let mut options: Vec<SharedString> = available.iter().map(|f| SharedString::from(f.clone())).collect();
    let selected = match options.iter().position(|o| o.as_ref() == current) {
        Some(i) => i,
        None => {
            options.push(current.to_string().into());
            options.len() - 1
        }
    };
    options.push(OTHER_FONT.into());
    (options, selected)
}

/// The select's labels for `options` (stored values): the default entry reads as the platform's default font, the
/// others as they are. Choosing an entry still stores its value.
pub fn log_font_labels(options: &[SharedString], chrome: ChromeStyle) -> Vec<SharedString> {
    options.iter().map(|o| if o.as_ref() == DEFAULT_LOG_FONT { default_log_font_label(chrome).into() } else { o.clone() }).collect()
}

/// The Theme row's hint: the Mac style names macOS, the others the desktop.
pub fn theme_hint(chrome: ChromeStyle) -> &'static str {
    if chrome.is_mac() { "System follows the macOS appearance" } else { "System follows the desktop appearance" }
}

fn change(this: &Entity<Workspace>, f: impl Fn(&mut Settings) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |ws, cx| ws.change_settings(|s| f(s), window, cx))
}

fn change_ix(this: &Entity<Workspace>, f: impl Fn(&mut Settings, usize) + 'static) -> impl Fn(usize, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |i, window, cx| this.update(cx, |ws, cx| ws.change_settings(|s| f(s, i), window, cx))
}

fn step(this: &Entity<Workspace>, f: impl Fn(&mut Settings, bool) + 'static) -> impl Fn(bool, &mut Window, &mut App) + 'static {
    let this = this.clone();
    move |up, window, cx| this.update(cx, |ws, cx| ws.change_settings(|s| f(s, up), window, cx))
}

fn bump(v: u32, up: bool) -> u32 {
    if up { v + 1 } else { v.saturating_sub(1) }
}

const THEMES: [ThemeSetting; 3] = [ThemeSetting::System, ThemeSetting::Light, ThemeSetting::Dark];
const DENSITIES: [DensitySetting; 2] = [DensitySetting::Compact, DensitySetting::Comfortable];

pub fn render(this: &Entity<Workspace>, cx: &App) -> DialogFrame {
    let s = SettingsStore::get(cx);
    let (a, b) = (&s.appearance, &s.behavior);
    let (font_options, font_selected) = log_font_options(&available_log_fonts(cx), &a.log_font);
    let font_labels = log_font_labels(&font_options, cx.rogg().chrome);
    let other = font_options.len() - 1;
    let profiles: Vec<String> = cx.try_global::<ProfileStore>().map(|p| p.names()).unwrap_or_else(|| vec![b.default_profile.clone()]);
    let profile_selected = profiles.iter().position(|n| *n == b.default_profile).unwrap_or(0);
    let pick_font = {
        let this = this.clone();
        let options = font_options.clone();
        move |i: usize, window: &mut Window, cx: &mut App| {
            if i == other {
                crate::dialogs::fonts::monospace_fonts(cx);
                this.update(cx, |ws, cx| ws.open_dialog(Dialog::FontPicker, cx));
            } else if let Some(name) = options.get(i).map(|o| o.to_string()) {
                this.update(cx, |ws, cx| ws.change_settings(move |s| s.appearance.log_font = name, window, cx));
            }
        }
    };
    let names = profiles.clone();
    DialogFrame::new("Settings")
        .description("Changes apply immediately and are saved.")
        .child(SectionLabel::new("Appearance"))
        .child(FormRow::new("row-theme", "Theme").hint(theme_hint(cx.rogg().chrome)).control(
            Segmented::new("setting-theme", vec!["System".into(), "Light".into(), "Dark".into()], THEMES.iter().position(|t| *t == a.theme).unwrap_or(0))
                .on_select(change_ix(this, |s, i| s.appearance.theme = THEMES[i.min(2)])),
        ))
        .child(FormRow::new("row-ui-size", "UI font size").control(
            Stepper::new("setting-ui-size", format!("{} px", a.ui_font_size))
                .bounds(a.ui_font_size > *UI_FONT_SIZES.start(), a.ui_font_size < *UI_FONT_SIZES.end())
                .on_step(step(this, |s, up| s.appearance.ui_font_size = bump(s.appearance.ui_font_size, up))),
        ))
        .child(FormRow::new("row-log-font", "Log font").control(CompactSelect::new("setting-log-font", font_labels, font_selected).on_select(pick_font)))
        .child(FormRow::new("row-log-size", "Log font size").hint("Comfortable line spacing adds 1 px").control(
            Stepper::new("setting-log-size", format!("{} px", a.log_font_size))
                .bounds(a.log_font_size > *LOG_FONT_SIZES.start(), a.log_font_size < *LOG_FONT_SIZES.end())
                .on_step(step(this, |s, up| s.appearance.log_font_size = bump(s.appearance.log_font_size, up))),
        ))
        .child(FormRow::new("row-density", "Line spacing").control(
            Segmented::new("setting-density", vec!["Compact".into(), "Comfortable".into()], DENSITIES.iter().position(|d| *d == a.density).unwrap_or(0))
                .on_select(change_ix(this, |s, i| s.appearance.density = DENSITIES[i.min(1)])),
        ))
        .child(SectionLabel::new("Behavior"))
        .child(FormRow::new("row-wrap", "Wrap lines in new tabs").control(Toggle::new("setting-wrap", b.wrap).on_click(change(this, |s| s.behavior.wrap = !s.behavior.wrap))))
        .child(FormRow::new("row-follow", "Follow selection in new tabs").control(
            Toggle::new("setting-follow", b.follow_selection).on_click(change(this, |s| s.behavior.follow_selection = !s.behavior.follow_selection)),
        ))
        .child(FormRow::new("row-context", "Context lines in new tabs").control(
            Stepper::new("setting-context", context_label(b.context))
                .bounds(b.context > 0, b.context < MAX_CONTEXT)
                .on_step(step(this, |s, up| s.behavior.context = bump(s.behavior.context, up))),
        ))
        .child(FormRow::new("row-regex", "Regular expressions in new tabs").control(Toggle::new("setting-regex", b.regex).on_click(change(this, |s| s.behavior.regex = !s.behavior.regex))))
        .child(FormRow::new("row-case", "Case-sensitive filters in new tabs").control(
            Toggle::new("setting-case", b.case_sensitive).on_click(change(this, |s| s.behavior.case_sensitive = !s.behavior.case_sensitive)),
        ))
        .child(FormRow::new("row-exclude", "Allow excluding filter terms (NOT)").control(
            Toggle::new("setting-exclude-terms", b.exclude_terms).on_click(change(this, |s| s.behavior.exclude_terms = !s.behavior.exclude_terms)),
        ))
        .child(FormRow::new("row-omitted", "Show omitted-line markers in the filtered log").control(
            Toggle::new("setting-omitted", b.omitted_markers).on_click(change(this, |s| s.behavior.omitted_markers = !s.behavior.omitted_markers)),
        ))
        .child(FormRow::new("row-restore", "Restore session on launch").control(
            Toggle::new("setting-restore", b.restore_session).on_click(change(this, |s| s.behavior.restore_session = !s.behavior.restore_session)),
        ))
        .child(FormRow::new("row-check-updates", "Check for updates on start").control(
            Toggle::new("setting-check-updates", b.check_updates).on_click(change(this, |s| s.behavior.check_updates = !s.behavior.check_updates)),
        ))
        .child(FormRow::new("row-cache", "Cache size limit").hint("Extracted archive members, decompressed files and conversions").control(
            Stepper::new("setting-cache", format!("{} GB", b.cache_limit_gb))
                .bounds(b.cache_limit_gb > CACHE_LIMITS_GB[0], b.cache_limit_gb < CACHE_LIMITS_GB[CACHE_LIMITS_GB.len() - 1])
                .on_step(step(this, |s, up| s.behavior.cache_limit_gb = step_cache_limit(s.behavior.cache_limit_gb, up))),
        ))
        .child(FormRow::new("row-depth", "Folder scan depth").hint("Levels of subfolders listed when a folder opens").control(
            Stepper::new("setting-depth", format!("{} {}", b.scan_depth, noun(b.scan_depth as u64, "level", "levels")))
                .bounds(b.scan_depth > *SCAN_DEPTHS.start(), b.scan_depth < *SCAN_DEPTHS.end())
                .on_step(step(this, |s, up| s.behavior.scan_depth = bump(s.behavior.scan_depth, up))),
        ))
        .child(FormRow::new("row-profile", "Default highlighting profile").control(
            CompactSelect::new("setting-profile", profiles.iter().map(|n| SharedString::from(n.clone())).collect(), profile_selected).on_select(change_ix(this, move |s, i| {
                if let Some(name) = names.get(i) {
                    s.behavior.default_profile = name.clone();
                }
            })),
        ))
}
