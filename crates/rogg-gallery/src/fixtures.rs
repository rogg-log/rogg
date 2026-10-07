//! Canned data. Bundle, tabs and dialog contents are copied verbatim from the reference mockup.
use gpui_kit::SharedString;
use rogg_ui::dialogs::ProfileRuleView;
use rogg_ui::filter::ChipState;
use rogg_ui::logview::{LogLine, LogRow, RowTint, Span, SpanStyle};
use rogg_ui::sidebar::{RecentItem, SidebarFooter, TreeRow};
use rogg_ui::theme::Color;

pub use rogg_app::demo::{BUNDLE, FILES};

pub fn sidebar_rows(active: &str) -> Vec<TreeRow> {
    let mut rows = vec![TreeRow::Folder { label: BUNDLE.into(), expanded: true, depth: 0, note: None, failed: false }];
    rows.extend(FILES.iter().map(|(name, size)| TreeRow::File {
        label: (*name).into(),
        size: (*size).into(),
        active: *name == active,
        depth: 1,
        failed: false,
    }));
    rows
}

/// A folder root with a nested archive being listed, an unreadable archive and a compressed log (M3 states).
pub fn sidebar_tree_rows() -> Vec<TreeRow> {
    vec![
        TreeRow::Folder { label: "support-2026-10-05".into(), expanded: true, depth: 0, note: None, failed: false },
        TreeRow::Folder { label: "nodes".into(), expanded: true, depth: 1, note: None, failed: false },
        TreeRow::File { label: "node-1.log".into(), size: "412 MB".into(), active: false, depth: 2, failed: false },
        TreeRow::Folder { label: "bundle-node-2.tgz".into(), expanded: true, depth: 1, note: Some("listing…".into()), failed: false },
        TreeRow::Folder { label: "broken.zip".into(), expanded: false, depth: 1, note: Some("unreadable".into()), failed: true },
        TreeRow::File { label: "app.log.1.gz".into(), size: "38 MB".into(), active: true, depth: 1, failed: false },
    ]
}

pub fn recent() -> Vec<RecentItem> {
    rogg_app::demo::RECENT.iter().map(|(label, is_folder)| RecentItem { label: (*label).into(), is_folder: *is_folder, missing: false }).collect()
}

pub fn sidebar_footer() -> SidebarFooter {
    SidebarFooter { title: rogg_app::demo::FOOTER_TITLE.into(), lines: rogg_app::demo::FOOTER_LINES.iter().map(|l| SharedString::from(*l)).collect() }
}

pub fn reference_terms() -> Vec<(SharedString, ChipState)> {
    vec![
        ("error".into(), ChipState::Include),
        ("timeout".into(), ChipState::Include),
        ("healthcheck".into(), ChipState::Exclude),
        ("retry".into(), ChipState::Off),
    ]
}

pub fn tabs_description() -> &'static str {
    "All open tabs, including those outside the visible header."
}

pub fn profile_rules() -> Vec<ProfileRuleView> {
    vec![
        ProfileRuleView { swatch: Color::rgb(0x744037), pattern: "ERROR".into(), summary: "Text + subtle row tint · both panels".into() },
        ProfileRuleView { swatch: Color::rgb(0x64502b), pattern: "WARN".into(), summary: "Text · both panels".into() },
        ProfileRuleView { swatch: Color::rgb(0x315064), pattern: "request_id=\\S+".into(), summary: "Match text · both panels".into() },
    ]
}

pub const PROFILE_DESCRIPTION: &str = "A named collection of line rules for both panels, plus the filter-match palette. Example profile preview.";
pub const PROFILE_NOTE: &str = "The implementation will support editable rules, priority, colors, and profile assignment. This dialog illustrates its placement.";

pub fn appearance_rows() -> Vec<(SharedString, SharedString)> {
    vec![
        ("Theme".into(), "System / Light / Dark".into()),
        ("Log font".into(), "System monospace · 12 px".into()),
        ("Line spacing".into(), "Compact / Comfortable".into()),
        ("Default wrapping".into(), "Off · override in each panel".into()),
    ]
}

pub const APPEARANCE_DESCRIPTION: &str = "Keep global appearance separate from the active highlighting profile.";
pub const APPEARANCE_NOTE: &str = "Placement preview. Use the controls above the mockup to try appearance and density.";

pub fn shortcut_rows() -> Vec<(SharedString, SharedString)> {
    vec![
        ("Find in focused panel".into(), "⌘ F".into()),
        ("Edit filters".into(), "⌘ L".into()),
        ("Pin / unpin selected line".into(), "⌘ B".into()),
        ("Go to line".into(), "⌘ G".into()),
        ("Move through results".into(), "↑ ↓".into()),
        ("Reveal in full log".into(), "Return".into()),
    ]
}

pub const SHORTCUTS_DESCRIPTION: &str = "Proposed shortcuts for the native app.";
pub const SHORTCUTS_NOTE: &str = "Browser keyboard shortcuts remain unchanged in this mockup.";

pub fn history_entries() -> Vec<SharedString> {
    vec!["error || timeout || healthcheck || retry".into(), "request_id=req_8f29 || connection reset".into()]
}

pub const HISTORY_DESCRIPTION: &str = "Restore an expression together with its term states and matching options.";
pub const HISTORY_NOTE: &str = "Illustrative history entries.";

fn line(number: u64, text: &str, spans: Vec<Span>, pinned: bool, context: bool, tint: Option<RowTint>) -> LogRow {
    LogRow::Line(LogLine { number, text: text.into(), spans, pinned, context, tint })
}

fn stamp() -> Span {
    Span { range: 0..23, style: SpanStyle::Faint }
}

/// One row per visual state: plain, hovered (forced by the page), selected, pinned, error tint, context, gap, hits, search, very long.
pub fn log_rows() -> Vec<LogRow> {
    let long = format!("2026-10-05 14:32:12.144  ERROR  client       upstream timeout {}", "trace_id=4bf92f3577b34da6a3ce929d0e0e4736 ".repeat(12));
    vec![
        line(120480, "2026-10-05 14:32:08.142  INFO   server       request started method=POST path=/v1/payments request_id=req_8f29", vec![stamp()], false, false, None),
        line(120481, "2026-10-05 14:32:08.213  DEBUG  router       matched route=payments.create upstream=payments-primary", vec![stamp()], false, false, None),
        line(120487, "2026-10-05 14:32:10.639  INFO   config       deployment revision=8d72f6a feature_flags={payments_v2:true}", vec![stamp()], true, false, None),
        line(120488, "2026-10-05 14:32:10.710  DEBUG  client       connection opened host=payments-primary.internal port=8443", vec![stamp()], false, true, None),
        LogRow::Gap { omitted: 3 },
        line(120492, "2026-10-05 14:32:12.144  ERROR  client       upstream timeout after 30000ms request_id=req_8f29",
            vec![stamp(), Span { range: 25..30, style: SpanStyle::Hit(0) }, Span { range: 54..61, style: SpanStyle::Hit(1) }], false, false, Some(RowTint::Error)),
        line(120495, "2026-10-05 14:32:13.357  ERROR  payments     authorization failed error=UPSTREAM_UNAVAILABLE request_id=req_8f29",
            vec![stamp(), Span { range: 25..30, style: SpanStyle::Hit(0) }, Span { range: 66..71, style: SpanStyle::Hit(0) }], false, false, Some(RowTint::Error)),
        line(120500, "2026-10-05 14:32:14.792  WARN   cache        timeout reading key=account:7239 fallback=database",
            vec![stamp(), Span { range: 45..52, style: SpanStyle::Hit(1) }, Span { range: 45..52, style: SpanStyle::SearchHit }], false, false, None),
        line(120503, "2026-10-05 14:32:15.215  ERROR  worker       job failed error=DEADLINE_EXCEEDED job_id=job_4271 attempt=3",
            vec![stamp(), Span { range: 25..30, style: SpanStyle::Hit(2) }], false, false, Some(RowTint::Error)),
        line(120504, "2026-10-05 14:32:15.286  INFO   queue        job scheduled for retry job_id=job_4271 delay=60s",
            vec![stamp(), Span { range: 63..68, style: SpanStyle::Hit(3) }], false, false, None),
        line(120505, &long, vec![stamp()], false, false, Some(RowTint::Error)),
    ]
}
