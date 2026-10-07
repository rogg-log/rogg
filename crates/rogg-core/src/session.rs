//! Session restore and `.rogg` files (spec §6.4, §6.5): the window and every tab's state as JSON. `session.json` is
//! written on quit and every 30 s; a `.rogg` file is the same schema, exported by the user and restored when opened.

use crate::error::CoreError;
use crate::filter::{FilterSpec, MAX_CONTEXT};
use crate::persist::{load_json, read_json_strict, save_json};
use crate::source::SourceRef;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SESSION_VERSION: u32 = 1;
pub const ROGG_EXT: &str = "rogg";
/// The panes' default split (`rogg_ui::layout::MainWindowParts::DEFAULT_SPLIT`).
pub const DEFAULT_SPLIT: f32 = 1.1 / 2.1;

fn yes() -> bool {
    true
}

fn default_split() -> f32 {
    DEFAULT_SPLIT
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub maximized: bool,
    #[serde(default = "yes")]
    pub sidebar_visible: bool,
    #[serde(default)]
    pub sidebar_width: Option<f32>,
}

/// Where a restored tab's text comes from. Streams (stdin, FIFOs) and in-memory texts are never stored (spec §6.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SessionSource {
    File { path: PathBuf },
    Member { source: SourceRef },
    Derived(Box<DerivedSpec>),
}

/// An "Open filtered as tab" tab (spec §4.9, §6.4): rebuilt from its parent and filter, or — when the parent was live —
/// from the explicit list of the parent's 0-based lines.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedSpec {
    pub parent: SessionSource,
    pub filter: FilterSpec,
    #[serde(default)]
    pub pins: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<u64>>,
    /// `HH:MM:SS` when the parent was live (the `Snapshot · …` note).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabState {
    pub source: SessionSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub filter: FilterSpec,
    /// Pinned lines, 0-based.
    #[serde(default)]
    pub pins: Vec<u64>,
    /// Selected display number.
    #[serde(default)]
    pub selection: Option<u64>,
    /// Display number of the first visible line of the full and the filtered log.
    #[serde(default)]
    pub top: [Option<u64>; 2],
    #[serde(default = "default_split")]
    pub split: f32,
    #[serde(default)]
    pub wrap: [bool; 2],
    #[serde(default = "yes")]
    pub follow: bool,
}

impl TabState {
    pub fn new(source: SessionSource) -> Self {
        Self { source, profile: None, filter: FilterSpec::default(), pins: Vec::new(), selection: None, top: [None, None], split: DEFAULT_SPLIT, wrap: [false, false], follow: true }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub window: Option<WindowState>,
    #[serde(default)]
    pub tabs: Vec<TabState>,
    #[serde(default)]
    pub active: usize,
}

fn sanitize_filter(filter: &mut FilterSpec) {
    filter.context = filter.context.min(MAX_CONTEXT);
}

fn sanitize_source(source: &mut SessionSource) {
    if let SessionSource::Derived(spec) = source {
        sanitize_filter(&mut spec.filter);
        sanitize_source(&mut spec.parent);
    }
}

impl Session {
    /// Values a hand-edited file may hold out of range are brought back in: the active index, context radii, splits.
    /// A window frame with a non-finite value is dropped (JSON has no NaN: it would be written as `null` and make the
    /// next load fail), and so is a non-finite sidebar width.
    pub fn sanitized(mut self) -> Self {
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        if self.window.as_ref().is_some_and(|w| ![w.x, w.y, w.width, w.height].iter().all(|v| v.is_finite())) {
            self.window = None;
        }
        if let Some(w) = &mut self.window
            && w.sidebar_width.is_some_and(|v| !v.is_finite())
        {
            w.sidebar_width = None;
        }
        for tab in &mut self.tabs {
            sanitize_filter(&mut tab.filter);
            sanitize_source(&mut tab.source);
            if !tab.split.is_finite() {
                tab.split = DEFAULT_SPLIT;
            }
            tab.split = tab.split.clamp(0.05, 0.95);
        }
        self
    }

    /// `session.json`: a missing file is an empty session; an unparseable one is set aside as `.bak` (`true`).
    pub fn load(path: &Path) -> Result<(Session, bool), CoreError> {
        let (session, recovered) = load_json::<Session>(path)?;
        Ok((session.sanitized(), recovered))
    }

    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        save_json(path, &Session { version: SESSION_VERSION, ..self.clone() })
    }

    /// A `.rogg` file the user opened: errors are reported, the file is never renamed.
    pub fn read_rogg(path: &Path) -> Result<Session, CoreError> {
        read_json_strict::<Session>(path).map(Session::sanitized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{FilterSpec, Term, TermState};
    use crate::source::SourceRef;

    fn sample() -> Session {
        let member = SourceRef::member(SourceRef::File("/b/incident.zip".into()), "logs/api.log");
        let filter = FilterSpec { terms: vec![Term::include("error"), Term::new("health", TermState::Exclude)], regex: false, case_sensitive: true, context: 2 };
        let derived = DerivedSpec { parent: SessionSource::Member { source: member.clone() }, filter: filter.clone(), pins: vec![4], lines: None, snapshot: None };
        let mut file = TabState::new(SessionSource::File { path: "/var/log/app.log".into() });
        file.filter = filter;
        file.pins = vec![1, 9];
        file.selection = Some(12);
        file.top = [Some(3), None];
        file.wrap = [true, false];
        file.follow = false;
        file.profile = Some("Services".into());
        Session {
            version: SESSION_VERSION,
            window: Some(WindowState { x: 10., y: 20., width: 1440., height: 900., maximized: false, sidebar_visible: true, sidebar_width: Some(240.) }),
            tabs: vec![file, TabState::new(SessionSource::Member { source: member }), TabState::new(SessionSource::Derived(Box::new(derived)))],
            active: 2,
        }
    }

    #[test]
    fn sessions_round_trip_through_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        sample().save(&path).unwrap();
        assert_eq!(Session::load(&path).unwrap(), (sample(), false));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"kind\": \"derived\""), "{text}");
        assert_eq!(Session::read_rogg(&path).unwrap(), sample(), ".rogg uses the same schema");
    }

    #[test]
    fn corrupt_session_is_set_aside_but_rogg_files_are_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        std::fs::write(&path, "{\"tabs\": [").unwrap();
        assert_eq!(Session::load(&path).unwrap(), (Session::default(), true));
        assert!(dir.path().join("session.json.bak").exists());
        let rogg = dir.path().join("x.rogg");
        std::fs::write(&rogg, "{\"tabs\": [").unwrap();
        assert!(matches!(Session::read_rogg(&rogg), Err(CoreError::Parse { .. })));
        assert!(rogg.exists());
    }

    #[test]
    fn loading_sanitizes_out_of_range_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let mut s = sample();
        s.active = 9;
        s.tabs[0].filter.context = 99;
        s.tabs[0].split = 7.;
        s.save(&path).unwrap();
        let (loaded, _) = Session::load(&path).unwrap();
        assert_eq!(loaded.active, 2);
        assert_eq!(loaded.tabs[0].filter.context, crate::filter::MAX_CONTEXT);
        assert!((0.05..=0.95).contains(&loaded.tabs[0].split));
        let empty = Session { active: 3, ..Session::default() }.sanitized();
        assert_eq!(empty.active, 0);
    }

    #[test]
    fn non_finite_values_are_sanitized_away() {
        let mut s = sample();
        s.tabs[0].split = f32::NAN;
        s.window.as_mut().unwrap().sidebar_width = Some(f32::INFINITY);
        let clean = s.clone().sanitized();
        assert!((clean.tabs[0].split - DEFAULT_SPLIT).abs() < 1e-6);
        assert_eq!(clean.window.as_ref().unwrap().sidebar_width, None);
        assert_eq!(clean.window.as_ref().unwrap().width, 1440.);
        s.window.as_mut().unwrap().x = f32::NAN;
        assert_eq!(s.sanitized().window, None, "a frame with a non-finite value is not kept");
    }

    #[test]
    fn minimal_tab_entries_load_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("min.rogg");
        std::fs::write(&path, r#"{"tabs":[{"source":{"kind":"file","path":"/x.log"}}]}"#).unwrap();
        let s = Session::read_rogg(&path).unwrap();
        let t = &s.tabs[0];
        assert!(t.follow && t.pins.is_empty() && t.selection.is_none() && t.filter == FilterSpec::default());
        assert!((t.split - DEFAULT_SPLIT).abs() < 1e-6);
    }
}
