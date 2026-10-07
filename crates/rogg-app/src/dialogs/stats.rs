//! The statistics popover (spec §4.11): matched / total, pins, context, per-term hits and the scan time of the active
//! tab's filter, rendered live from the document.

use gpui_kit::SharedString;
use rogg_core::filter::TermState;
use rogg_core::stats::{FilterStats, group_digits, noun};
use std::time::Duration;

/// `15%`, `33.3%`, `<0.1%`, `>99.9%`; `0%` of nothing. `100%` only when every line matches.
pub fn percent(part: u64, total: u64) -> String {
    if total == 0 || part == 0 {
        return "0%".into();
    }
    if part >= total {
        return "100%".into();
    }
    let p = part as f64 * 100. / total as f64;
    if p < 0.1 {
        "<0.1%".into()
    } else if p >= 99.9 {
        ">99.9%".into()
    } else if (p - p.round()).abs() < 0.05 {
        format!("{}%", p.round() as u64)
    } else {
        format!("{p:.1}%")
    }
}

/// `0.4 ms`, `250 ms`, `1.24 s`.
pub fn duration_label(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.;
    if ms < 10. {
        format!("{ms:.1} ms")
    } else if ms < 1000. {
        format!("{} ms", ms.round() as u64)
    } else {
        format!("{:.2} s", ms / 1000.)
    }
}

fn state_label(state: TermState) -> &'static str {
    match state {
        TermState::Include => "include",
        TermState::Exclude => "exclude",
        TermState::Off => "off",
    }
}

pub fn rows(stats: Option<&FilterStats>, progress: Option<f32>) -> Vec<(SharedString, SharedString)> {
    let Some(s) = stats else { return vec![("No filter has run yet".into(), "Add a filter term to see statistics".into())] };
    let mut matched = format!("{} of {} ({})", group_digits(s.matched), group_digits(s.total_lines), percent(s.matched, s.total_lines));
    if let Some(f) = progress {
        matched.push_str(&format!(" · scanning {:.0}%", f * 100.));
    }
    let pinned = if s.pinned_unmatched > 0 { format!("{} ({} not matching)", group_digits(s.pinned), group_digits(s.pinned_unmatched)) } else { group_digits(s.pinned) };
    let mut out: Vec<(SharedString, SharedString)> =
        vec![("Matched lines".into(), matched.into()), ("Pinned lines".into(), pinned.into()), ("Context lines".into(), group_digits(s.context_lines).into())];
    for t in &s.term_hits {
        let value = match t.state {
            TermState::Off => "off".to_string(),
            state => format!("{} · {} {}", state_label(state), group_digits(t.lines), noun(t.lines, "line", "lines")),
        };
        out.push((t.text.clone().into(), value.into()));
    }
    out.push(("Scan time".into(), duration_label(s.duration).into()));
    out
}
