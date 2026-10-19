//! What the Update dialog shows for each updater state (spec §3.3): its lines, progress and buttons.

use super::state::{Failure, Release, UpdateState};
use rogg_core::update::Manifest;
use rogg_ui::Callback;
use rogg_ui::dialogs::{UpdateButton, UpdateView};

pub const TITLE: &str = "Software update";

/// What "Copy install command" copies when admin rights can't be requested (no `pkexec`).
pub const INSTALL_COMMAND: &str = "curl -fsSL https://raw.githubusercontent.com/rogg-log/rogg/main/install.sh | bash";

/// The dialog's button callbacks. `retry` re-runs whatever failed: the update when there is a release, else the check.
pub struct ViewActions {
    pub update: Callback,
    pub cancel: Callback,
    pub retry: Callback,
    pub notes: Callback,
    pub copy_command: Callback,
    pub close: Callback,
}

/// The GitHub release page of `manifest`'s build.
pub fn release_page(manifest: &Manifest) -> String {
    format!("https://github.com/rogg-log/rogg/releases/tag/v{}-build.{}", manifest.version, manifest.build)
}

/// The release a state is about, if any: the one on offer, being installed, installed, or whose update failed.
pub fn release_of(state: &UpdateState) -> Option<&Release> {
    match state {
        UpdateState::Available(release)
        | UpdateState::Downloading { release, .. }
        | UpdateState::Installing(release)
        | UpdateState::Installed(release)
        | UpdateState::Failed { release: Some(release), .. } => Some(release),
        _ => None,
    }
}

/// `rogg 0.1.0 · build 14 · 2026-10-18 · a1b2c3d`
fn release_line(m: &Manifest) -> String {
    let sha: String = m.commit.chars().take(7).collect();
    format!("rogg {} · build {} · {} · {sha}", m.version, m.build, day(&m.date))
}

/// The `YYYY-MM-DD` a date starts with (CI writes `2026-10-15T23:00:17Z`), else the date as it is.
fn day(date: &str) -> &str {
    let b = date.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if b.len() >= 10 && digits(0..4) && b[4] == b'-' && digits(5..7) && b[7] == b'-' && digits(8..10) {
        &date[..10]
    } else {
        date
    }
}

/// Decimal megabytes with one decimal, as the sidebar shows sizes.
fn mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1e6)
}

/// `3.2 of 9.8 MB`, or `3.2 MB` while the size is unknown; and the bar's fraction.
fn progress(done: u64, total: Option<u64>) -> (f32, String) {
    match total.filter(|&t| t > 0) {
        Some(total) => ((done as f64 / total as f64).clamp(0., 1.) as f32, format!("{} of {} MB", mb(done), mb(total))),
        None => (0., format!("{} MB", mb(done))),
    }
}

/// The dialog for `state`; `running` is the running build (None for a local build).
pub fn update_view(state: &UpdateState, running: Option<u64>, actions: ViewActions) -> UpdateView {
    let ViewActions { update, cancel, retry, notes, copy_command, close } = actions;
    let button = |id: &'static str, label: &str, primary: bool, on_click: &Callback| UpdateButton { id, label: label.to_string().into(), primary, on_click: on_click.clone() };
    let close_only = || vec![button("update-close", "Close", false, &close)];
    let (lines, progress, buttons): (Vec<String>, _, _) = match state {
        // Idle has nothing to report; the dialog opens on a check, which leaves Idle at once.
        UpdateState::Idle => (vec![], None, close_only()),
        UpdateState::Checking { .. } => (vec!["Checking for updates…".into()], None, close_only()),
        UpdateState::UpToDate => {
            let line = match running {
                Some(build) => format!("rogg is up to date (build {build})."),
                None => "rogg is up to date.".into(),
            };
            (vec![line], None, close_only())
        }
        UpdateState::LocalBuild => (vec!["This is a local build. Updates apply to release builds only.".into()], None, close_only()),
        UpdateState::Available(release) => {
            let mut lines = vec![release_line(&release.manifest)];
            lines.extend(running.map(|build| format!("You have build {build}.")));
            let buttons = vec![
                button("update-now", "Update", true, &update),
                button("update-notes", "Release notes", false, &notes),
                button("update-later", "Later", false, &close),
            ];
            (lines, None, buttons)
        }
        UpdateState::Downloading { done, total, .. } => {
            let (fraction, label) = progress(*done, *total);
            (vec![], Some((fraction, label.into())), vec![button("update-cancel", "Cancel", false, &cancel)])
        }
        UpdateState::Installing(_) => (vec!["Installing…".into()], None, vec![]),
        UpdateState::Installed(release) => {
            (vec![format!("Build {} is installed. Restart rogg to use it.", release.manifest.build)], None, close_only())
        }
        UpdateState::Failed { release, failure } => {
            let mut buttons = vec![button("update-retry", "Retry", true, &retry)];
            // A failed check has no release whose notes could be shown.
            if release.is_some() {
                buttons.push(button("update-notes", "Release notes", false, &notes));
            }
            if *failure == Failure::NoPkexec {
                buttons.push(button("update-copy-command", "Copy install command", false, &copy_command));
            }
            buttons.push(button("update-close", "Close", false, &close));
            (vec![failure.message()], None, buttons)
        }
    };
    UpdateView { title: TITLE.into(), lines: lines.into_iter().map(Into::into).collect(), progress, buttons }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rogg_core::update::Asset;
    use std::path::PathBuf;
    use std::rc::Rc;

    fn release() -> Release {
        let manifest = Manifest::parse(
            r#"{"version":"0.1.0","build":14,"commit":"a1b2c3d4e5f6","date":"2026-10-18","assets":[{"name":"a.tar.gz","os":"linux","arch":"x86_64","sha256":"00","size":9800000}]}"#,
        )
        .unwrap();
        let asset: Asset = manifest.assets[0].clone();
        Release { manifest, asset }
    }

    fn actions() -> ViewActions {
        let none: Callback = Rc::new(|_, _| {});
        ViewActions { update: none.clone(), cancel: none.clone(), retry: none.clone(), notes: none.clone(), copy_command: none.clone(), close: none }
    }

    fn shown(state: &UpdateState) -> (Vec<String>, Option<(f32, String)>, Vec<&'static str>) {
        let view = update_view(state, Some(11), actions());
        assert_eq!(view.title.as_ref(), TITLE);
        (
            view.lines.iter().map(|l| l.to_string()).collect(),
            view.progress.map(|(f, l)| (f, l.to_string())),
            view.buttons.iter().map(|b| b.id).collect(),
        )
    }

    #[test]
    fn downloading_shows_progress_in_megabytes_and_cancel() {
        let (lines, progress, buttons) = shown(&UpdateState::Downloading { release: release(), done: 3_240_000, total: Some(9_800_000) });
        assert!(lines.is_empty(), "{lines:?}");
        let (fraction, label) = progress.unwrap();
        assert!((fraction - 0.3306).abs() < 0.001, "{fraction}");
        assert_eq!(label, "3.2 of 9.8 MB");
        assert_eq!(buttons, ["update-cancel"]);
        let (_, progress, _) = shown(&UpdateState::Downloading { release: release(), done: 1_500_000, total: None });
        assert_eq!(progress, Some((0., "1.5 MB".into())));
    }

    #[test]
    fn installing_has_no_buttons() {
        assert_eq!(shown(&UpdateState::Installing(release())), (vec!["Installing…".into()], None, vec![]));
    }

    #[test]
    fn failures_offer_retry_and_notes_only_with_a_release() {
        let failed = |release, failure| UpdateState::Failed { release, failure };
        let (lines, _, buttons) = shown(&failed(Some(release()), Failure::RollbackFailed(PathBuf::from("/x/rogg.rogg-old"))));
        assert_eq!(lines, ["The update failed. The previous version is at /x/rogg.rogg-old."]);
        assert_eq!(buttons, ["update-retry", "update-notes", "update-close"]);
        let (lines, _, buttons) = shown(&failed(None, Failure::Offline));
        assert_eq!(lines, ["No network connection."]);
        assert_eq!(buttons, ["update-retry", "update-close"]);
        let (_, _, buttons) = shown(&failed(Some(release()), Failure::NoPkexec));
        assert_eq!(buttons, ["update-retry", "update-notes", "update-copy-command", "update-close"]);
    }

    fn release_dated(date: &str) -> Release {
        let mut release = release();
        release.manifest.date = date.into();
        release
    }

    /// CI writes the date as a UTC timestamp; the dialog shows the day.
    #[test]
    fn the_available_line_shows_the_day_of_a_timestamp() {
        let (lines, _, _) = shown(&UpdateState::Available(release_dated("2026-10-18T10:00:00Z")));
        assert!(lines[0].contains("· 2026-10-18 ·"), "{lines:?}");
        assert!(!lines[0].contains("T10"), "{lines:?}");
        assert_eq!(lines, ["rogg 0.1.0 · build 14 · 2026-10-18 · a1b2c3d", "You have build 11."]);
        // Anything that doesn't start with a date is shown as it is.
        for date in ["2026-10-18", "Oct 18", "", "2026/10/18 10:00"] {
            let (lines, _, _) = shown(&UpdateState::Available(release_dated(date)));
            assert_eq!(lines[0], format!("rogg 0.1.0 · build 14 · {date} · a1b2c3d"));
        }
    }

    #[test]
    fn the_release_page_is_the_builds_tag() {
        assert_eq!(release_page(&release().manifest), "https://github.com/rogg-log/rogg/releases/tag/v0.1.0-build.14");
    }
}
