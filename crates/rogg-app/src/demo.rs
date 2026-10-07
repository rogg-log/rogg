//! `rogg --demo` (spec §9 M2): the reference mockup's bundle, tabs, sample lines, filters, pin and selection, for
//! the visual review against the reference. The gallery's mockup replica uses the same data.

use crate::document::{DocumentInit, Source};
use crate::sidebar_model::{FixedSidebar, SidebarEntry, SidebarGroup};
use crate::tab::{TabInit, TabLabels};
use crate::workspace::Workspace;
use gpui_kit::{Context, SharedString, Window};
use rogg_core::filter::{FilterSpec, Term, TermState};
use rogg_ui::sidebar::{RecentItem, SidebarFooter};
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

/// Display number of the first sample line.
pub const FIRST_LINE_NUMBER: u64 = 120480;

pub const BUNDLE: &str = "incident-2026-10-05.zip";

pub const FILES: [(&str, &str); 10] = [
    ("api.log", "972 MB"),
    ("worker.log", "841 MB"),
    ("gateway.log", "1.0 GB"),
    ("auth.log", "713 MB"),
    ("scheduler.log", "904 MB"),
    ("database.log", "867 MB"),
    ("cache.log", "624 MB"),
    ("metrics.log", "955 MB"),
    ("events.log", "701 MB"),
    ("system.log", "884 MB"),
];

pub const SAMPLES: [(&str, &str, &str); 40] = [
    ("INFO ", "server", "request started method=POST path=/v1/payments request_id=req_8f29"),
    ("DEBUG", "router", "matched route=payments.create upstream=payments-primary"),
    ("INFO ", "auth", "token validated subject=svc_checkout scopes=payments:write"),
    ("DEBUG", "pool", "acquired connection pool=primary active=24 idle=8"),
    ("WARN ", "client", "retry attempt=1 backoff=250ms target=payments-primary"),
    ("INFO ", "healthcheck", "upstream=cache status=ok latency=2ms"),
    ("ERROR", "healthcheck", "timeout waiting for metrics sidecar target=metrics:9090"),
    ("INFO ", "config", "deployment revision=8d72f6a feature_flags={payments_v2:true, strict_idempotency:true} changed_by=release-controller region=eu-west-1"),
    ("DEBUG", "client", "connection opened host=payments-primary.internal port=8443 tls=1.3"),
    ("WARN ", "client", "retry attempt=2 backoff=500ms target=payments-primary"),
    ("INFO ", "queue", "payment accepted job_id=job_4271 request_id=req_8f29"),
    ("DEBUG", "worker", "processing job_id=job_4271 queue=payments priority=normal"),
    ("ERROR", "client", "upstream timeout after 30000ms request_id=req_8f29 host=payments-primary.internal:8443 path=/v1/authorize retry_count=3 circuit_state=half_open trace_id=4bf92f3577b34da6a3ce929d0e0e4736 region=eu-west-1"),
    ("WARN ", "circuit", "circuit opened upstream=payments-primary failures=5 window=60s"),
    ("INFO ", "router", "switching upstream payments-primary → payments-secondary"),
    ("ERROR", "payments", "authorization failed error=UPSTREAM_UNAVAILABLE request_id=req_8f29"),
    ("INFO ", "server", "request completed status=503 duration=30241ms request_id=req_8f29"),
    ("DEBUG", "pool", "released connection pool=primary active=23 idle=9"),
    ("INFO ", "healthcheck", "upstream=database status=ok latency=4ms"),
    ("INFO ", "server", "request started method=GET path=/v1/accounts request_id=req_91d2"),
    ("WARN ", "cache", "timeout reading key=account:7239 fallback=database"),
    ("INFO ", "database", "query completed rows=1 duration=14ms"),
    ("INFO ", "server", "request completed status=200 duration=48ms request_id=req_91d2"),
    ("ERROR", "worker", "job failed error=DEADLINE_EXCEEDED job_id=job_4271 attempt=3"),
    ("INFO ", "queue", "job scheduled for retry job_id=job_4271 delay=60s"),
    ("DEBUG", "metrics", "flush completed counters=148 histograms=23 duration=2ms"),
    ("WARN ", "healthcheck", "timeout target=payments-primary latency=5000ms"),
    ("INFO ", "circuit", "upstream probe scheduled after=30s"),
    ("ERROR", "client", "connection reset by peer upstream=payments-secondary request_id=req_a273"),
    ("INFO ", "server", "request completed status=502 duration=1304ms request_id=req_a273"),
    ("DEBUG", "router", "routing table refreshed revision=248 endpoints=12"),
    ("INFO ", "server", "request started method=GET path=/status request_id=req_b7c1"),
    ("INFO ", "server", "request completed status=200 duration=1ms request_id=req_b7c1"),
    ("WARN ", "pool", "connection pool utilization=92% active=46 limit=50"),
    ("ERROR", "database", "timeout acquiring connection after 5000ms pool=primary request_id=req_c389"),
    ("INFO ", "circuit", "upstream recovered target=payments-primary probe_status=200"),
    ("INFO ", "queue", "job resumed job_id=job_4271 attempt=4"),
    ("INFO ", "payments", "authorization completed job_id=job_4271 status=approved"),
    ("DEBUG", "metrics", "flush completed counters=153 histograms=24 duration=1ms"),
    ("INFO ", "server", "idle connections pruned count=8 remaining=16"),
];

pub fn line_text(i: usize) -> String {
    let (level, source, message) = SAMPLES[i];
    format!("2026-10-05 14:32:{:02}.{:03}  {}  {:<12} {}", 8 + i / 3, 142 + (i * 71) % 850, level, source, message)
}

/// The tabs open in the reference.
pub const TABS: [&str; 3] = ["api.log", "worker.log", "gateway.log"];
/// The reference's Recent section: (label, is folder).
pub const RECENT: [(&str, bool); 2] = [("staging / logs", true), ("deployment-10-04.log", false)];
pub const FOOTER_TITLE: &str = "Support bundle";
pub const FOOTER_LINES: [&str; 2] = ["10 members · 8.4 GB expanded", "Members open individually"];
pub const STATUS_NOTE: &str = "UTF-8 · LF · sample excerpt";
pub const PROFILE: &str = "Services";
/// Sample index pinned in the reference (line 120487).
pub const PINNED_SAMPLE: u64 = 7;
/// Sample index selected in the reference (line 120492).
pub const SELECTED_SAMPLE: u64 = 12;

/// The 40 sample lines joined with `\n` (no trailing newline), shared by every demo tab.
pub fn sample_bytes() -> Arc<Vec<u8>> {
    static BYTES: OnceLock<Arc<Vec<u8>>> = OnceLock::new();
    BYTES.get_or_init(|| Arc::new((0..SAMPLES.len()).map(line_text).collect::<Vec<_>>().join("\n").into_bytes())).clone()
}

/// `error` IN, `timeout` IN, `healthcheck` NOT, `retry` OFF.
pub fn reference_spec() -> FilterSpec {
    FilterSpec {
        terms: vec![Term::include("error"), Term::include("timeout"), Term::new("healthcheck", TermState::Exclude), Term::new("retry", TermState::Off)],
        ..Default::default()
    }
}

/// The reference's history dialog entries, newest first (the gallery seeds a capture's history with them).
pub fn reference_history() -> Vec<FilterSpec> {
    vec![reference_spec(), FilterSpec::from_expression("request_id=req_8f29 || connection reset")]
}

pub fn source(name: &str) -> Source {
    Source::Memory { name: SharedString::from(name.to_string()), bytes: sample_bytes() }
}

pub fn init(name: &str) -> TabInit {
    TabInit {
        doc: DocumentInit { number_base: FIRST_LINE_NUMBER, spec: reference_spec(), pins: BTreeSet::from([PINNED_SAMPLE]) },
        selection: Some(FIRST_LINE_NUMBER + SELECTED_SAMPLE),
        labels: TabLabels {
            crumbs: Some(vec![BUNDLE.into(), SharedString::from(name.to_string())]),
            profile: None,
            note: Some(STATUS_NOTE.into()),
            full_count: Some("40 sample lines".into()),
            tooltip: Some(format!("{name} · {BUNDLE}").into()),
        },
        profile: Some(PROFILE.into()),
        ..TabInit::default()
    }
}

pub fn sidebar() -> FixedSidebar {
    FixedSidebar {
        groups: vec![SidebarGroup {
            key: BUNDLE.into(),
            label: BUNDLE.into(),
            entries: FILES.iter().map(|(name, size)| SidebarEntry { label: (*name).into(), size: (*size).into(), tab: None, open: Some((source(name), init(name))) }).collect(),
        }],
        count: "10 files".into(),
        footer: SidebarFooter { title: FOOTER_TITLE.into(), lines: FOOTER_LINES.iter().map(|l| SharedString::from(*l)).collect() },
        recent: RECENT.iter().map(|(label, is_folder)| RecentItem { label: (*label).into(), is_folder: *is_folder, missing: false }).collect(),
    }
}

/// Opens the demo in `ws`: the bundle sidebar and the three reference tabs, api.log active.
pub fn open(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    ws.set_fixed_sidebar(sidebar(), cx);
    let first = ws.tabs().len();
    for name in TABS {
        ws.open_source(source(name), init(name), window, cx);
    }
    ws.activate(first, window, cx);
}
