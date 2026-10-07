//! Highlighting profiles (spec §6.1): a named, ordered list of line rules plus the filter-match palette, stored as
//! one TOML file per profile in `profiles/`, importable and exportable as `.roggprofile`.

use crate::error::CoreError;
use crate::persist::{read_toml_strict, save_toml, with_suffix};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_PROFILE: &str = "Default";
pub const SERVICES_PROFILE: &str = "Services";
pub const EXPORT_EXT: &str = "roggprofile";
/// Most filter-match colours a profile holds.
pub const MAX_PALETTE: usize = 12;
/// Alpha of a row tint made from a rule colour.
pub const TINT_ALPHA: u8 = 0x18;
/// A leading `YYYY-MM-DD HH:MM:SS[.fff]` timestamp: the built-ins paint it faint, as the reference does.
pub const TIMESTAMP_PATTERN: &str = r"^\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:[.,]\d{1,9})?";

/// Theme tokens a colour may name instead of a hex value; they follow the active theme. `hit-N` is the palette's
/// filter-match text colour N, `hit-N-bg` its background.
pub const COLOR_TOKENS: [&str; 14] = [
    "text", "muted", "faint", "accent", "error", "amber", "hit-0", "hit-1", "hit-2", "hit-3", "hit-0-bg", "hit-1-bg", "hit-2-bg", "hit-3-bg",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorValue {
    Token(&'static str),
    /// `0xRRGGBBAA`.
    Rgba(u32),
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa` → `0xRRGGBBAA`.
pub fn parse_hex(s: &str) -> Option<u32> {
    let hex = s.trim().strip_prefix('#')?;
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let full = match hex.len() {
        3 => format!("{}ff", hex.chars().flat_map(|c| [c, c]).collect::<String>()),
        6 => format!("{hex}ff"),
        8 => hex.to_string(),
        _ => return None,
    };
    u32::from_str_radix(&full, 16).ok()
}

/// A colour as written in a profile file: a theme token or a hex value.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ColorSpec(pub String);

impl ColorSpec {
    pub fn token(name: &str) -> Self {
        ColorSpec(name.to_string())
    }

    pub fn hex(rgba: u32) -> Self {
        if rgba & 0xff == 0xff { ColorSpec(format!("#{:06x}", rgba >> 8)) } else { ColorSpec(format!("#{rgba:08x}")) }
    }

    pub fn parse(&self) -> Option<ColorValue> {
        let s = self.0.trim();
        COLOR_TOKENS.iter().find(|t| **t == s).map(|t| ColorValue::Token(t)).or_else(|| parse_hex(s).map(ColorValue::Rgba))
    }
}

pub(crate) fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.;
    if max == min {
        return (0., 0., l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2. - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6. } else { 0. }
    } else if max == g {
        (b - r) / d + 2.
    } else {
        (r - g) / d + 4.
    };
    (h * 60., s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s == 0. {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1. + s) } else { l + s - l * s };
    let p = 2. * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.);
        if t < 1. / 6. {
            p + (q - p) * 6. * t
        } else if t < 0.5 {
            q
        } else if t < 2. / 3. {
            p + (q - p) * (2. / 3. - t) * 6.
        } else {
            p
        }
    };
    let h = h / 360.;
    (channel(h + 1. / 3.), channel(h), channel(h - 1. / 3.))
}

/// The light-theme variant of a dark-theme colour (spec §6.1: derived automatically, can be overridden): hue and alpha
/// are kept and the lightness mirrored — into 25–45% for text, so it reads on a light background, or 80–92% for
/// backgrounds, so marks stay soft.
pub fn derive_light(rgba: u32, background: bool) -> u32 {
    let [r, g, b, a] = rgba.to_be_bytes();
    let (h, s, l) = rgb_to_hsl(r as f32 / 255., g as f32 / 255., b as f32 / 255.);
    let l = if background { (1. - l).clamp(0.80, 0.92) } else { (1. - l).clamp(0.25, 0.45) };
    let (r, g, b) = hsl_to_rgb(h, s.min(0.8), l);
    let byte = |v: f32| (v * 255.).round().clamp(0., 255.) as u8;
    u32::from_be_bytes([byte(r), byte(g), byte(b), a])
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Panes {
    Full,
    Filtered,
    #[default]
    Both,
}

impl Panes {
    pub fn includes(self, filtered: bool) -> bool {
        match self {
            Panes::Both => true,
            Panes::Full => !filtered,
            Panes::Filtered => filtered,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Panes::Full => "full log",
            Panes::Filtered => "filtered log",
            Panes::Both => "both panels",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct LineRule {
    pub pattern: String,
    pub regex: bool,
    pub case_sensitive: bool,
    /// Effects (spec §6.1): colour the matched text, colour the whole line, tint the row, bold the matched text.
    pub match_color: bool,
    pub line_color: bool,
    pub row_tint: bool,
    pub bold: bool,
    pub panes: Panes,
    pub enabled: bool,
    /// Dark-theme colour (or a token, which follows both themes).
    pub color: ColorSpec,
    /// Light-theme override of a hex `color`; None derives it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<ColorSpec>,
}

impl Default for LineRule {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            regex: false,
            case_sensitive: false,
            match_color: true,
            line_color: false,
            row_tint: false,
            bold: false,
            panes: Panes::Both,
            enabled: true,
            color: ColorSpec::token("accent"),
            light: None,
        }
    }
}

impl LineRule {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self { pattern: pattern.into(), ..Self::default() }
    }

    /// `Line text + row tint · both panels`, as the editor lists rules (reference `.profile-rule`).
    pub fn summary(&self) -> String {
        let effects: Vec<&str> = [("match text", self.match_color), ("line text", self.line_color), ("row tint", self.row_tint), ("bold", self.bold)]
            .into_iter()
            .filter(|(_, on)| *on)
            .map(|(name, _)| name)
            .collect();
        let mut text = if effects.is_empty() { "no effect".to_string() } else { effects.join(" + ") };
        if let Some(first) = text.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        format!("{text} · {}", self.panes.label())
    }
}

/// One filter-match colour slot (spec §6.1 `hit_palette`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct HitEntry {
    pub fg: ColorSpec,
    pub bg: ColorSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_fg: Option<ColorSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_bg: Option<ColorSpec>,
}

impl HitEntry {
    /// The theme palette's slot `n` (0–3).
    pub fn slot(n: usize) -> Self {
        Self { fg: ColorSpec(format!("hit-{}", n % 4)), bg: ColorSpec(format!("hit-{}-bg", n % 4)), light_fg: None, light_bg: None }
    }
}

impl Default for HitEntry {
    fn default() -> Self {
        Self::slot(0)
    }
}

fn default_palette() -> Vec<HitEntry> {
    (0..4).map(HitEntry::slot).collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub rules: Vec<LineRule>,
    #[serde(default = "default_palette")]
    pub hit_palette: Vec<HitEntry>,
}

fn timestamp_rule() -> LineRule {
    LineRule { pattern: TIMESTAMP_PATTERN.into(), regex: true, case_sensitive: true, color: ColorSpec::token("faint"), ..LineRule::default() }
}

fn level_rule(word: &str, color: &str, tint: bool) -> LineRule {
    LineRule {
        pattern: format!(r"\b{word}\b"),
        regex: true,
        case_sensitive: true,
        match_color: false,
        line_color: true,
        row_tint: tint,
        color: ColorSpec::token(color),
        ..LineRule::default()
    }
}

/// Spec §6.1 Default: ERROR (error text + row tint), WARN (amber), DEBUG (muted); plus the faint timestamp.
pub fn builtin_default() -> Profile {
    Profile {
        name: DEFAULT_PROFILE.into(),
        rules: vec![timestamp_rule(), level_rule("ERROR", "error", true), level_rule("WARN", "amber", false), level_rule("DEBUG", "muted", false)],
        hit_palette: default_palette(),
    }
}

/// Spec §6.1 Services, the reference example: ERROR, WARN, `request_id=\S+`; plus the faint timestamp.
pub fn builtin_services() -> Profile {
    let request_id = LineRule { pattern: r"request_id=\S+".into(), regex: true, case_sensitive: true, color: ColorSpec::token("hit-2"), ..LineRule::default() };
    Profile {
        name: SERVICES_PROFILE.into(),
        rules: vec![timestamp_rule(), level_rule("ERROR", "error", true), level_rule("WARN", "amber", false), request_id],
        hit_palette: default_palette(),
    }
}

pub fn builtins() -> Vec<Profile> {
    vec![builtin_default(), builtin_services()]
}

/// The file a profile is stored in: its name with every character other than a letter, digit, space, `-` or `_`
/// replaced by `_`, plus `.toml`.
pub fn file_name(name: &str) -> String {
    let stem: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { '_' }).collect();
    format!("{}.toml", if stem.trim().is_empty() { "profile" } else { stem.as_str() })
}

/// `name`, or `name 2`, `name 3`… — the first one not in `taken`.
pub fn unique_name(name: &str, taken: &[String]) -> String {
    let name = if name.trim().is_empty() { "Profile" } else { name.trim() };
    if !taken.iter().any(|t| t == name) {
        return name.to_string();
    }
    (2..).map(|n| format!("{name} {n}")).find(|candidate| !taken.contains(candidate)).unwrap_or_default()
}

pub struct ProfilesLoad {
    /// Sorted by name with Default first; Default is always present.
    pub profiles: Vec<Profile>,
    /// Files that could not be read and were moved to `<file>.bak`.
    pub recovered: Vec<PathBuf>,
}

/// Reads every `*.toml` in `dir`. A missing `dir` is created and seeded with the built-ins; an unparseable file is
/// moved to `.bak` (spec §6.6); a file without a name takes its file stem; duplicate names keep the first file
/// (by file name); a missing Default is supplied from the built-in (not written back).
pub fn load_dir(dir: &Path) -> Result<ProfilesLoad, CoreError> {
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(CoreError::io(dir))?;
        for p in builtins() {
            save(dir, &p)?;
        }
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(CoreError::io(dir))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml") && p.is_file())
        .collect();
    files.sort();
    let mut profiles: Vec<Profile> = Vec::new();
    let mut recovered = Vec::new();
    for path in files {
        let parsed = std::fs::read(&path)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|s| toml::from_str::<Profile>(&s).ok());
        match parsed {
            Some(mut p) => {
                if p.name.trim().is_empty() {
                    p.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                }
                if p.hit_palette.is_empty() {
                    p.hit_palette = default_palette();
                }
                p.hit_palette.truncate(MAX_PALETTE);
                if !profiles.iter().any(|q| q.name == p.name) {
                    profiles.push(p);
                }
            }
            None => {
                std::fs::rename(&path, with_suffix(&path, ".bak")).map_err(CoreError::io(&path))?;
                recovered.push(path);
            }
        }
    }
    if !profiles.iter().any(|p| p.name == DEFAULT_PROFILE) {
        profiles.push(builtin_default());
    }
    profiles.sort_by_key(|p| (p.name != DEFAULT_PROFILE, p.name.to_lowercase()));
    Ok(ProfilesLoad { profiles, recovered })
}

/// Writes `profile` to `dir/<file_name>`; returns the path.
pub fn save(dir: &Path, profile: &Profile) -> Result<PathBuf, CoreError> {
    let path = dir.join(file_name(&profile.name));
    save_toml(&path, profile)?;
    Ok(path)
}

/// Removes the file of the profile named `name` (nothing to do when it is gone).
pub fn delete(dir: &Path, name: &str) -> Result<(), CoreError> {
    let path = dir.join(file_name(name));
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(CoreError::Io { path, source: e }),
        _ => Ok(()),
    }
}

/// Reads a `.roggprofile` the user picked; a missing name comes from the file stem. Never renames the file.
pub fn import(path: &Path) -> Result<Profile, CoreError> {
    let mut p: Profile = read_toml_strict(path)?;
    if p.name.trim().is_empty() {
        p.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Imported".into());
    }
    if p.hit_palette.is_empty() {
        p.hit_palette = default_palette();
    }
    p.hit_palette.truncate(MAX_PALETTE);
    Ok(p)
}

pub fn export(path: &Path, profile: &Profile) -> Result<(), CoreError> {
    save_toml(path, profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse_tokens_and_hex() {
        assert_eq!(ColorSpec::token("error").parse(), Some(ColorValue::Token("error")));
        assert_eq!(ColorSpec("#e0c283".into()).parse(), Some(ColorValue::Rgba(0xe0c283ff)));
        assert_eq!(ColorSpec("#8f352e10".into()).parse(), Some(ColorValue::Rgba(0x8f352e10)));
        assert_eq!(ColorSpec("#abc".into()).parse(), Some(ColorValue::Rgba(0xaabbccff)));
        assert_eq!(ColorSpec("purple".into()).parse(), None);
        assert_eq!(ColorSpec("#12345".into()).parse(), None);
        assert_eq!(ColorSpec::hex(0xe0c283ff).0, "#e0c283");
        assert_eq!(ColorSpec::hex(0x8f352e10).0, "#8f352e10");
    }

    fn hsl(rgba: u32) -> (f32, f32, f32) {
        let [r, g, b, _] = rgba.to_be_bytes();
        rgb_to_hsl(r as f32 / 255., g as f32 / 255., b as f32 / 255.)
    }

    #[test]
    fn light_variants_keep_hue_and_alpha_and_stay_readable() {
        let text = derive_light(0xefaaa2ff, false);
        let (h0, _, _) = hsl(0xefaaa2ff);
        let (h1, _, l1) = hsl(text);
        assert!((h0 - h1).abs() < 2., "hue {h0} vs {h1}");
        assert!((0.25..=0.45).contains(&l1), "text lightness {l1}");
        assert_eq!(text & 0xff, 0xff);
        let bg = derive_light(0x74403780, true);
        let (_, _, l2) = hsl(bg);
        assert!((0.80..=0.92).contains(&l2), "background lightness {l2}");
        assert_eq!(bg & 0xff, 0x80, "alpha is kept");
    }

    #[test]
    fn builtins_follow_the_spec() {
        let d = builtin_default();
        assert_eq!(d.name, "Default");
        let patterns: Vec<&str> = d.rules.iter().map(|r| r.pattern.as_str()).collect();
        assert_eq!(patterns, [TIMESTAMP_PATTERN, r"\bERROR\b", r"\bWARN\b", r"\bDEBUG\b"]);
        let error = &d.rules[1];
        assert!(error.line_color && error.row_tint && !error.match_color && error.color == ColorSpec::token("error"));
        assert_eq!(d.rules[2].color, ColorSpec::token("amber"));
        assert_eq!(d.rules[3].color, ColorSpec::token("muted"));
        assert_eq!(d.hit_palette, (0..4).map(HitEntry::slot).collect::<Vec<_>>());
        let s = builtin_services();
        assert_eq!(s.rules.last().unwrap().pattern, r"request_id=\S+");
        assert!(s.rules.last().unwrap().match_color);
        assert_eq!(s.rules[1].summary(), "Line text + row tint · both panels");
        assert_eq!(s.rules.last().unwrap().summary(), "Match text · both panels");
    }

    #[test]
    fn profiles_round_trip_through_toml() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = builtin_services();
        p.rules[0].light = Some(ColorSpec::hex(0x112233ff));
        p.rules[2].panes = Panes::Filtered;
        p.hit_palette.push(HitEntry { fg: ColorSpec::hex(0xffffffff), bg: ColorSpec::hex(0x000000ff), light_fg: None, light_bg: None });
        let path = save(dir.path(), &p).unwrap();
        assert_eq!(path, dir.path().join("Services.toml"));
        let loaded = load_dir(dir.path()).unwrap();
        assert_eq!(loaded.profiles.iter().find(|q| q.name == "Services"), Some(&p));
    }

    #[test]
    fn a_missing_dir_is_seeded_with_the_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = dir.path().join("profiles");
        let loaded = load_dir(&profiles).unwrap();
        assert_eq!(loaded.profiles.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Default", "Services"]);
        assert!(profiles.join("Default.toml").exists() && profiles.join("Services.toml").exists());
        std::fs::remove_file(profiles.join("Services.toml")).unwrap();
        let again = load_dir(&profiles).unwrap();
        assert_eq!(again.profiles.len(), 1, "a deleted built-in stays deleted once the folder exists");
        std::fs::remove_file(profiles.join("Default.toml")).unwrap();
        assert_eq!(load_dir(&profiles).unwrap().profiles[0], builtin_default(), "Default is always available");
    }

    #[test]
    fn a_broken_profile_file_is_set_aside_alone() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &builtin_default()).unwrap();
        std::fs::write(dir.path().join("Mine.toml"), "name = \"Mine\"\n[[rules]]\npattern = 3\n").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "ignored").unwrap();
        let loaded = load_dir(dir.path()).unwrap();
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.recovered, vec![dir.path().join("Mine.toml")]);
        assert!(dir.path().join("Mine.toml.bak").exists());
        assert!(dir.path().join("notes.txt").exists());
    }

    #[test]
    fn missing_fields_default_and_an_empty_name_comes_from_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Plain.toml"), "[[rules]]\npattern = \"x\"\n").unwrap();
        let p = load_dir(dir.path()).unwrap().profiles.into_iter().find(|p| p.name == "Plain").unwrap();
        let r = &p.rules[0];
        assert!(r.enabled && r.match_color && !r.regex && r.panes == Panes::Both);
        assert_eq!(p.hit_palette.len(), 4);
    }

    #[test]
    fn import_export_and_names() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("shared.roggprofile");
        export(&out, &builtin_services()).unwrap();
        assert_eq!(import(&out).unwrap(), builtin_services());
        std::fs::write(&out, "rules = 5").unwrap();
        assert!(matches!(import(&out), Err(CoreError::Parse { .. })));
        assert!(out.exists(), "an import never renames the user's file");
        let taken = vec!["Default".to_string(), "Services".to_string(), "Services 2".to_string()];
        assert_eq!(unique_name("Services", &taken), "Services 3");
        assert_eq!(unique_name("New", &taken), "New");
        assert_eq!(file_name("a/b: c"), "a_b_ c.toml");
        delete(dir.path(), "nothing here").unwrap();
    }
}
