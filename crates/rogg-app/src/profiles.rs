//! Highlighting profiles (spec §6.1) as an app global, and their colours resolved for the active theme.

use gpui_kit::{App, Global};
use rogg_core::profile::{self, ColorSpec, ColorValue, DEFAULT_PROFILE, LineRule, Profile, TINT_ALPHA, derive_light, names_collide, unique_name};
use rogg_core::style::{CompiledProfile, RuleError};
use rogg_ui::logview::{RowTint, Span, SpanStyle};
use rogg_ui::theme::{ActiveRoggTheme, Appearance, ChipColor, Color, HitColor, Palette};
use std::path::PathBuf;
use std::sync::Arc;

struct Entry {
    profile: Profile,
    compiled: Arc<CompiledProfile>,
    errors: Vec<RuleError>,
}

fn entry(profile: Profile) -> Entry {
    let (compiled, errors) = CompiledProfile::compile(&profile);
    Entry { profile, compiled: Arc::new(compiled), errors }
}

pub struct ProfileStore {
    dir: PathBuf,
    entries: Vec<Entry>,
    /// Bumped on every change; tabs restyle when it moves.
    revision: u64,
    /// Profile files that were unreadable and were moved to `.bak` (toasts at start).
    pub recovered: Vec<PathBuf>,
    /// Why the last save or delete failed, as a message for the user; None after a successful one.
    pub last_error: Option<String>,
    /// How many times a profile file was written.
    saves: u64,
    /// Profiles changed in memory only (a colour slider being dragged); `flush` writes them.
    unsaved: Vec<String>,
}

impl Global for ProfileStore {}

impl ProfileStore {
    pub fn load(dir: PathBuf) -> Self {
        match profile::load_dir(&dir) {
            Ok(loaded) => Self { dir, entries: loaded.profiles.into_iter().map(entry).collect(), revision: 0, recovered: loaded.recovered, last_error: None, saves: 0, unsaved: Vec::new() },
            Err(e) => Self { dir, entries: profile::builtins().into_iter().map(entry).collect(), revision: 0, recovered: Vec::new(), last_error: Some(e.to_string()), saves: 0, unsaved: Vec::new() },
        }
    }

    pub fn revision(cx: &App) -> u64 {
        cx.try_global::<ProfileStore>().map_or(0, |s| s.revision)
    }

    /// How many times a profile file was written (a slider drag writes once, when it ends).
    pub fn saves(&self) -> u64 {
        self.saves
    }

    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.profile.name.clone()).collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.profile.name == name)
    }

    /// The index of the entry named `name`, or of Default (always first) when there is none.
    fn position(&self, name: &str) -> usize {
        let named = |n: &str| self.entries.iter().position(|e| e.profile.name == n);
        named(name).or_else(|| named(DEFAULT_PROFILE)).unwrap_or(0)
    }

    fn find(&self, name: &str) -> &Entry {
        &self.entries[self.position(name)]
    }

    pub fn get(&self, name: &str) -> &Profile {
        &self.find(name).profile
    }

    pub fn compiled(&self, name: &str) -> Arc<CompiledProfile> {
        self.find(name).compiled.clone()
    }

    pub fn errors(&self, name: &str) -> &[RuleError] {
        &self.find(name).errors
    }

    /// Replaces the profile `old_name` with `profile` (a rename moves its file), recompiles and saves it. An empty
    /// filter-match palette is refilled with the default slots (`RowStyler::hit` needs at least one).
    pub fn put(&mut self, old_name: &str, mut profile: Profile) {
        if profile.hit_palette.is_empty() {
            profile.hit_palette = profile::default_palette();
        }
        let saved = self.save(&profile);
        self.unsaved.retain(|n| n != old_name && *n != profile.name);
        // A rename whose names share a file on the case-insensitive disk ("Ops" → "ops") was saved over the old file:
        // deleting the old name would delete the profile.
        if !names_collide(old_name, &profile.name) && saved.is_ok() {
            let _ = profile::delete(&self.dir, old_name);
        }
        self.last_error = saved.err().map(|e| format!("Could not save profile {}: {e}", profile.name));
        match self.entries.iter().position(|e| e.profile.name == old_name) {
            Some(i) => self.entries[i] = entry(profile),
            None => self.entries.push(entry(profile)),
        }
        self.revision += 1;
    }

    /// Adds `profile` under a unique name (`New profile 2`…); returns that name.
    pub fn add(&mut self, mut profile: Profile) -> String {
        profile.name = unique_name(&profile.name, &self.names());
        let name = profile.name.clone();
        self.put(&name, profile);
        name
    }

    /// Deletes a profile and its file; Default cannot be deleted.
    pub fn remove(&mut self, name: &str) -> bool {
        let Some(i) = self.entries.iter().position(|e| e.profile.name == name) else { return false };
        if name == DEFAULT_PROFILE {
            return false;
        }
        self.entries.remove(i);
        self.unsaved.retain(|n| n != name);
        self.last_error = profile::delete(&self.dir, name).err().map(|e| format!("Could not delete profile {name}: {e}"));
        self.revision += 1;
        true
    }

    /// Edits the profile named `name` (Default when there is none) and saves it.
    pub fn edit(&mut self, name: &str, f: impl FnOnce(&mut Profile)) {
        let mut profile = self.get(name).clone();
        let old = profile.name.clone();
        f(&mut profile);
        self.put(&old, profile);
    }

    /// Edits like `edit`, in memory only: tabs restyle at once and `flush` writes the file (a colour slider changes on
    /// every mouse move). A change of colours alone keeps the compiled patterns. Returns whether anything changed.
    pub fn edit_unsaved(&mut self, name: &str, f: impl FnOnce(&mut Profile)) -> bool {
        let i = self.position(name);
        let mut profile = self.entries[i].profile.clone();
        f(&mut profile);
        let current = &self.entries[i].profile;
        if profile == *current {
            return false;
        }
        if profile.name != current.name || profile.hit_palette.is_empty() {
            let old = current.name.clone();
            self.put(&old, profile);
            return true;
        }
        if !self.unsaved.contains(&profile.name) {
            self.unsaved.push(profile.name.clone());
        }
        if same_patterns(current, &profile) {
            self.entries[i].profile = profile;
        } else {
            self.entries[i] = entry(profile);
        }
        self.revision += 1;
        true
    }

    /// Writes `name` if it has changes made by `edit_unsaved`; returns whether it did.
    pub fn flush(&mut self, name: &str) -> bool {
        let Some(k) = self.unsaved.iter().position(|n| n == name) else { return false };
        self.unsaved.remove(k);
        let Some(entry) = self.entries.iter().find(|e| e.profile.name == name) else { return false };
        let profile = entry.profile.clone();
        let saved = self.save(&profile);
        self.last_error = saved.err().map(|e| format!("Could not save profile {}: {e}", profile.name));
        true
    }

    /// Writes every profile with unsaved changes; returns whether any was written.
    pub fn flush_all(&mut self) -> bool {
        let mut any = false;
        for name in self.unsaved.clone() {
            any |= self.flush(&name);
        }
        any
    }

    fn save(&mut self, profile: &Profile) -> Result<PathBuf, rogg_core::error::CoreError> {
        self.saves += 1;
        profile::save(&self.dir, profile)
    }
}

/// Whether `a` and `b` differ at most in colours, which do not affect what the rules match.
fn same_patterns(a: &Profile, b: &Profile) -> bool {
    a.rules.len() == b.rules.len()
        && a.rules.iter().zip(&b.rules).all(|(x, y)| LineRule { color: y.color.clone(), light: y.light.clone(), ..x.clone() } == *y)
}

fn token_color(token: &str, p: &Palette) -> Color {
    match token {
        "text" => p.log_text,
        "muted" => p.muted,
        "faint" => p.faint,
        "accent" => p.accent,
        "error" => p.error,
        "amber" => p.amber,
        other => {
            let slot = hit_slot(other).unwrap_or(0);
            if other.ends_with("-bg") { p.hits[slot].bg } else { p.hits[slot].fg }
        }
    }
}

/// `hit-2` / `hit-2-bg` → 2.
fn hit_slot(token: &str) -> Option<usize> {
    token.strip_prefix("hit-")?.trim_end_matches("-bg").parse().ok().filter(|n| *n < 4)
}

/// A profile colour for the active theme (spec §6.1): tokens follow the theme; a hex value is used as is in dark,
/// and in light through its override or else its derived variant. An unreadable colour falls back to the log text (or
/// the first hit background).
pub fn resolve_color(spec: &ColorSpec, light: Option<&ColorSpec>, background: bool, p: &Palette, appearance: Appearance) -> Color {
    let value = |c: ColorValue| match c {
        ColorValue::Token(t) => token_color(t, p),
        ColorValue::Rgba(v) => Color(v),
    };
    match (appearance, spec.parse()) {
        (_, None) => {
            if background {
                p.hits[0].bg
            } else {
                p.log_text
            }
        }
        (Appearance::Dark, Some(c)) => value(c),
        (Appearance::Light, Some(c)) => match light.and_then(ColorSpec::parse) {
            Some(l) => value(l),
            None => match c {
                ColorValue::Token(t) => token_color(t, p),
                ColorValue::Rgba(v) => Color(derive_light(v, background)),
            },
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RulePaint {
    pub color: Color,
    pub tint: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfilePaint {
    pub rules: Vec<RulePaint>,
    pub hits: Vec<HitColor>,
    pub chips: Vec<ChipColor>,
}

impl ProfilePaint {
    pub fn resolve(profile: &Profile, p: &Palette, appearance: Appearance) -> Self {
        let rules = profile
            .rules
            .iter()
            .map(|r| {
                let color = resolve_color(&r.color, r.light.as_ref(), false, p, appearance);
                let error_token = r.color.parse() == Some(ColorValue::Token("error")) && r.light.is_none();
                RulePaint { color, tint: if error_token { p.error_row_tint } else { color.with_alpha(TINT_ALPHA) } }
            })
            .collect();
        let hits: Vec<HitColor> = profile
            .hit_palette
            .iter()
            .map(|h| HitColor { fg: resolve_color(&h.fg, h.light_fg.as_ref(), false, p, appearance), bg: resolve_color(&h.bg, h.light_bg.as_ref(), true, p, appearance) })
            .collect();
        let chips = profile
            .hit_palette
            .iter()
            .zip(&hits)
            .map(|(h, c)| match (h.fg.parse(), h.bg.parse()) {
                (Some(ColorValue::Token(f)), Some(ColorValue::Token(b))) if hit_slot(f).is_some() && hit_slot(f) == hit_slot(b) && b.ends_with("-bg") => {
                    p.chips[hit_slot(f).unwrap_or(0)]
                }
                _ => ChipColor { border: c.bg, bg: c.bg.with_alpha(0x66), term: c.fg },
            })
            .collect();
        Self { rules, hits, chips }
    }
}

/// A profile ready to style rows: the compiled rules and their colours for the active theme.
pub struct RowStyler {
    compiled: Arc<CompiledProfile>,
    paint: ProfilePaint,
}

impl RowStyler {
    pub fn new(profile: &Profile, compiled: Arc<CompiledProfile>, p: &Palette, appearance: Appearance) -> Self {
        Self { compiled, paint: ProfilePaint::resolve(profile, p, appearance) }
    }

    pub fn from_profile(profile: &Profile, p: &Palette, appearance: Appearance) -> Self {
        Self::new(profile, Arc::new(CompiledProfile::compile(profile).0), p, appearance)
    }

    /// The styler for the profile named `name` under the active theme.
    pub fn for_profile(cx: &App, name: &str) -> Self {
        let t = cx.rogg();
        match cx.try_global::<ProfileStore>() {
            Some(store) => Self::new(store.get(name), store.compiled(name), &t.palette, t.appearance),
            None => Self::from_profile(&profile::builtin_default(), &t.palette, t.appearance),
        }
    }

    /// Spans and row tint for a row's display text: the line colour first (later spans override it), then match
    /// colours, then bold ranges.
    pub fn decorate(&self, display: &str, filtered: bool) -> (Vec<Span>, Option<RowTint>) {
        let style = self.compiled.style(display, filtered);
        let color = |rule: usize| self.paint.rules.get(rule).map(|r| r.color).unwrap_or_default();
        let mut spans = Vec::with_capacity(1 + style.match_spans.len() + style.bold.len());
        if let Some(rule) = style.line_color {
            spans.push(Span { range: 0..display.len(), style: SpanStyle::Fg(color(rule)) });
        }
        spans.extend(style.match_spans.into_iter().map(|(range, rule)| Span { range, style: SpanStyle::Fg(color(rule)) }));
        spans.extend(style.bold.into_iter().map(|range| Span { range, style: SpanStyle::Bold }));
        let tint = style.tint.and_then(|rule| self.paint.rules.get(rule)).map(|r| RowTint::Color(r.tint));
        (spans, tint)
    }

    /// The mark style of filter term slot `slot`.
    pub fn hit(&self, slot: usize) -> SpanStyle {
        let h = self.paint.hits[slot % self.paint.hits.len().max(1)];
        SpanStyle::Mark { fg: h.fg, bg: h.bg }
    }

    pub fn palette_len(&self) -> usize {
        self.paint.hits.len().max(1)
    }

    pub fn chips(&self) -> Vec<ChipColor> {
        self.paint.chips.clone()
    }

    /// The swatch colour of rule `i` (the editor).
    pub fn rule_color(&self, i: usize) -> Color {
        self.paint.rules.get(i).map(|r| r.color).unwrap_or_default()
    }

    pub fn hit_colors(&self) -> &[HitColor] {
        &self.paint.hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rogg_core::profile::{HitEntry, builtin_services};

    #[test]
    fn built_in_colours_follow_the_palette_exactly() {
        let p = Palette::dark();
        let paint = ProfilePaint::resolve(&builtin_services(), &p, Appearance::Dark);
        assert_eq!(paint.rules[1].color, p.error);
        assert_eq!(paint.rules[1].tint, p.error_row_tint, "the error tint is the palette's");
        assert_eq!(paint.rules[3].color, p.hits[2].fg);
        assert_eq!(paint.hits, p.hits.to_vec());
        assert_eq!(paint.chips, p.chips.to_vec());
        let light = Palette::light();
        assert_eq!(ProfilePaint::resolve(&builtin_services(), &light, Appearance::Light).rules[2].color, light.amber);
    }

    #[test]
    fn hex_colours_tint_and_derive() {
        let p = Palette::dark();
        let spec = ColorSpec::hex(0x336699ff);
        assert_eq!(resolve_color(&spec, None, false, &p, Appearance::Dark), Color(0x336699ff));
        assert_eq!(resolve_color(&spec, None, true, &p, Appearance::Light), Color(derive_light(0x336699ff, true)));
        assert_eq!(resolve_color(&ColorSpec("bad".into()), None, false, &p, Appearance::Dark), p.log_text);
        let custom = HitEntry { fg: ColorSpec::hex(0xffffffff), bg: ColorSpec::hex(0x000000ff), light_fg: None, light_bg: None };
        let profile = Profile { name: "x".into(), rules: vec![], hit_palette: vec![custom] };
        let paint = ProfilePaint::resolve(&profile, &p, Appearance::Dark);
        assert_eq!(paint.chips[0], ChipColor { border: Color(0x000000ff), bg: Color(0x00000066), term: Color(0xffffffff) });
    }

    #[test]
    fn put_refills_an_empty_palette() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::load(dir.path().to_path_buf());
        store.put("Bare", Profile { name: "Bare".into(), rules: vec![], hit_palette: vec![] });
        assert_eq!(store.get("Bare").hit_palette, profile::default_palette());
        let styler = RowStyler::from_profile(store.get("Bare"), &Palette::dark(), Appearance::Dark);
        assert_eq!(styler.hit(5), SpanStyle::Mark { fg: Palette::dark().hits[1].fg, bg: Palette::dark().hits[1].bg });
    }

    #[test]
    fn a_rename_that_only_changes_case_keeps_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::load(dir.path().to_path_buf());
        let name = store.add(Profile { name: "Ops".into(), rules: vec![], hit_palette: vec![] });
        store.edit(&name, |p| p.name = "ops".into());
        assert_eq!(store.names().iter().filter(|n| n.eq_ignore_ascii_case("ops")).count(), 1);
        let reloaded = ProfileStore::load(dir.path().to_path_buf());
        assert!(reloaded.contains("ops"), "{:?}", reloaded.names());
    }

    #[test]
    fn unsaved_edits_restyle_and_wait_for_a_flush() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::load(dir.path().to_path_buf());
        let name = store.add(Profile { name: "Ops".into(), ..rogg_core::profile::builtin_default() });
        let (saves, revision, compiled) = (store.saves(), store.revision, store.compiled(&name));
        assert!(store.edit_unsaved(&name, |p| p.rules[0].color = ColorSpec::hex(0x336699ff)));
        assert!(!store.edit_unsaved(&name, |p| p.rules[0].color = ColorSpec::hex(0x336699ff)), "no change, nothing to save");
        assert_eq!((store.saves(), store.revision), (saves, revision + 1), "applied in memory, not written");
        assert!(Arc::ptr_eq(&compiled, &store.compiled(&name)), "a colour change keeps the compiled patterns");
        assert!(store.edit_unsaved(&name, |p| p.rules[0].pattern = "x".into()));
        assert!(!Arc::ptr_eq(&compiled, &store.compiled(&name)), "a pattern change recompiles");
        assert!(store.flush(&name) && !store.flush(&name), "written once");
        assert_eq!(store.saves(), saves + 1);
        assert_eq!(ProfileStore::load(dir.path().to_path_buf()).get(&name).rules[0].color, ColorSpec::hex(0x336699ff));
    }

    #[test]
    fn decorate_orders_line_colour_before_match_colours() {
        let p = Palette::dark();
        let styler = RowStyler::from_profile(&rogg_core::profile::builtin_default(), &p, Appearance::Dark);
        let (spans, tint) = styler.decorate("2026-10-05 14:32:08.142  ERROR  x", false);
        assert_eq!(spans[0], Span { range: 0..33, style: SpanStyle::Fg(p.error) });
        assert_eq!(spans[1], Span { range: 0..23, style: SpanStyle::Fg(p.faint) });
        assert_eq!(tint, Some(RowTint::Color(p.error_row_tint)));
    }
}
