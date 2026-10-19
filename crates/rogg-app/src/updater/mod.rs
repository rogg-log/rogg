//! Self-update support for the app: the `Updater` global, its check and update jobs, and the elevated helper mode.

pub mod elevate;
pub mod http;
pub mod state;
pub mod view;

pub use state::{Failure, Release, UpdateState};

use gpui_kit::{Global, SharedString};
use http::NetError;
use rogg_core::CancelToken;
use rogg_core::platform::Os;
use rogg_core::update::apply::{ReplaceStyle, apply, can_write, is_rogg_target, stage, verify};
use rogg_core::update::{Install, Manifest, UpdateError, asset_name, is_newer};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

/// The hidden `--apply-update <staged> <target>` mode, run by the elevated helper: no window.
/// Returns the exit code: 0 replaced, 1 failed (message on stderr), 2 refused target.
pub fn run_apply_update(staged: &Path, target: &Path) -> i32 {
    if !is_rogg_target(target) {
        eprintln!("rogg: refusing to update {}: not a rogg install", target.display());
        return 2;
    }
    match apply(staged, target, ReplaceStyle::current()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("rogg: update failed: {e}");
            1
        }
    }
}

/// Replaces `target` with `staged` with admin rights, when the target's folder isn't writable.
pub type Elevator = Arc<dyn Fn(&Path, &Path) -> Result<(), Failure> + Send + Sync>;

#[derive(Clone)]
pub struct UpdaterConfig {
    /// Where `manifest.json` and the assets are fetched from.
    pub base: String,
    /// `None` for a local build, which never checks by itself.
    pub running_build: Option<u64>,
    pub install: Install,
    pub os: Os,
    /// `std::env::consts::ARCH`.
    pub arch: String,
    /// `<cache>/update`; an update works in `<work_dir>/<build>/`.
    pub work_dir: PathBuf,
    pub elevator: Elevator,
}

impl UpdaterConfig {
    /// The running app's configuration; `cache` is the app's cache folder.
    pub fn from_env(cache: &Path) -> UpdaterConfig {
        let os = Os::current();
        let install = std::env::current_exe().map_or(Install::Unsupported, |exe| Install::detect(&exe, os));
        UpdaterConfig {
            base: http::base_url(),
            running_build: crate::about::build_info().number.and_then(|n| n.parse().ok()),
            install,
            os,
            arch: std::env::consts::ARCH.to_string(),
            work_dir: cache.join("update"),
            elevator: Arc::new(elevate::apply_elevated),
        }
    }

    /// An updater that never fetches or installs: a local build on an unsupported install, with an unroutable base.
    /// For hosts that show the app without being it (the gallery) and for tests.
    pub fn disabled(work_dir: PathBuf) -> UpdaterConfig {
        UpdaterConfig {
            base: "http://127.0.0.1:9".into(),
            running_build: None,
            install: Install::Unsupported,
            os: Os::current(),
            arch: std::env::consts::ARCH.to_string(),
            work_dir,
            elevator: Arc::new(|_: &Path, _: &Path| Err(Failure::RolledBack)),
        }
    }
}

/// What a job reports to the updater.
enum JobEvent {
    Checked(Result<Manifest, NetError>),
    Progress(u64, Option<u64>),
    Installing,
    Installed,
    Cancelled,
    Failed(Failure),
}

struct Job {
    events: Receiver<JobEvent>,
    cancel: CancelToken,
    /// For a check: whether its failures are shown.
    manual: bool,
}

/// The app's update state and its one background job (a check or an update). Plain Rust: the app calls `poll` on
/// its poll interval and re-renders.
pub struct Updater {
    config: UpdaterConfig,
    state: UpdateState,
    job: Option<Job>,
}

impl Global for Updater {}

impl Updater {
    pub fn new(config: UpdaterConfig) -> Updater {
        Updater { config, state: UpdateState::Idle, job: None }
    }

    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    pub fn config(&self) -> &UpdaterConfig {
        &self.config
    }

    /// Fetches the manifest. A `manual` check reports every outcome; otherwise failures and "up to date" leave the
    /// state `Idle`. A local build never fetches. Once an update is installed, a check never leaves `Installed`.
    /// A manual check during a silent one adopts it (its outcome is then reported); otherwise this returns false
    /// (and does nothing) while a job runs.
    pub fn check(&mut self, manual: bool) -> bool {
        if let Some(job) = &mut self.job {
            if manual && let UpdateState::Checking { manual: silent @ false } = &mut self.state {
                job.manual = true;
                *silent = true;
                return true;
            }
            return false;
        }
        if self.config.running_build.is_none() {
            if manual {
                self.state = UpdateState::LocalBuild;
            }
            return true;
        }
        if !matches!(self.state, UpdateState::Installed(_)) {
            self.state = UpdateState::Checking { manual };
        }
        let base = self.config.base.clone();
        self.spawn(manual, move |tx, _| {
            let _ = tx.send(JobEvent::Checked(http::fetch_manifest(&base)));
        });
        true
    }

    /// Downloads and installs the offered release (from `Available`, or retrying a failed update). Returns false
    /// when there is nothing to install or a job runs.
    pub fn start_update(&mut self) -> bool {
        if self.job.is_some() {
            return false;
        }
        let release = match &self.state {
            UpdateState::Available(release) | UpdateState::Failed { release: Some(release), .. } => release.clone(),
            _ => return false,
        };
        let job = UpdateJob {
            url: asset_url(&self.config.base, &release.manifest, &release.asset.name),
            dir: self.config.work_dir.join(release.manifest.build.to_string()),
            asset: release.asset.name.clone(),
            sha256: release.asset.sha256.clone(),
            install: self.config.install.clone(),
            elevator: self.config.elevator.clone(),
        };
        self.state = UpdateState::Downloading { done: 0, total: Some(release.asset.size), release };
        self.spawn(true, move |tx, cancel| job.run(&tx, &cancel));
        true
    }

    /// Stops a download; the state returns to `Available` once the job has cleaned up. Installing can't be stopped.
    pub fn cancel(&mut self) {
        if let (Some(job), UpdateState::Downloading { .. }) = (&self.job, &self.state) {
            job.cancel.cancel();
        }
    }

    /// Applies what the job reported. Returns true while a job runs.
    pub fn poll(&mut self) -> bool {
        let Some(job) = self.job.take() else { return false };
        loop {
            match job.events.try_recv() {
                Ok(event) => self.apply_event(event, job.manual),
                Err(TryRecvError::Empty) => {
                    self.job = Some(job);
                    return true;
                }
                Err(TryRecvError::Disconnected) => {
                    self.job_ended();
                    return false;
                }
            }
        }
    }

    /// The title-bar badge's label and tooltip. "Update" shows from `Available` through `Installing`, so a closed
    /// dialog can be reopened while the job runs; "Restart to update" shows in `Installed`. Any other state has none.
    pub fn badge(&self) -> Option<(SharedString, SharedString)> {
        match &self.state {
            UpdateState::Available(release)
            | UpdateState::Downloading { release, .. }
            | UpdateState::Installing(release) => Some((
                "Update".into(),
                format!("rogg {} build {} is available", release.manifest.version, release.manifest.build).into(),
            )),
            UpdateState::Installed(release) => Some((
                "Restart to update".into(),
                format!("Build {} is installed. Restart rogg to use it.", release.manifest.build).into(),
            )),
            _ => None,
        }
    }

    fn spawn(&mut self, manual: bool, work: impl FnOnce(Sender<JobEvent>, CancelToken) + Send + 'static) {
        let (tx, events) = mpsc::channel();
        let cancel = CancelToken::new();
        let token = cancel.clone();
        std::thread::spawn(move || work(tx, token));
        self.job = Some(Job { events, cancel, manual });
    }

    fn apply_event(&mut self, event: JobEvent, manual: bool) {
        let state = std::mem::take(&mut self.state);
        self.state = match (event, state) {
            // The new build is already in place: only a restart moves on.
            (JobEvent::Checked(_), installed @ UpdateState::Installed(_)) => installed,
            (JobEvent::Checked(result), _) => match result.map_err(check_failure).and_then(|manifest| self.offer(manifest)) {
                Ok(Some(release)) => UpdateState::Available(release),
                _ if !manual => UpdateState::Idle,
                Ok(None) => UpdateState::UpToDate,
                Err(failure) => UpdateState::Failed { release: None, failure },
            },
            (JobEvent::Progress(done, total), UpdateState::Downloading { release, total: expected, .. }) => {
                UpdateState::Downloading { release, done, total: total.or(expected) }
            }
            (JobEvent::Installing, UpdateState::Downloading { release, .. }) => UpdateState::Installing(release),
            (JobEvent::Installed, UpdateState::Downloading { release, .. } | UpdateState::Installing(release)) => {
                UpdateState::Installed(release)
            }
            (JobEvent::Cancelled, UpdateState::Downloading { release, .. } | UpdateState::Installing(release)) => {
                UpdateState::Available(release)
            }
            (JobEvent::Failed(failure), UpdateState::Downloading { release, .. } | UpdateState::Installing(release)) => {
                UpdateState::Failed { release: Some(release), failure }
            }
            (_, state) => state,
        };
    }

    /// The release to offer from a fetched manifest: `None` when it isn't newer than the running build.
    fn offer(&self, manifest: Manifest) -> Result<Option<Release>, Failure> {
        let Some(name) = asset_name(&self.config.install, self.config.os, &self.config.arch) else {
            return Err(Failure::NoAsset);
        };
        let Some(running) = self.config.running_build else { return Ok(None) };
        if !is_newer(manifest.build, running) {
            return Ok(None);
        }
        let asset = manifest.asset(name).cloned().ok_or(Failure::NoAsset)?;
        Ok(Some(Release { manifest, asset }))
    }

    /// The job's thread ended. Its last event normally settled the state; if it died without one, the state
    /// mustn't stay in flight.
    fn job_ended(&mut self) {
        self.state = match std::mem::take(&mut self.state) {
            UpdateState::Checking { .. } => UpdateState::Idle,
            UpdateState::Downloading { release, .. } | UpdateState::Installing(release) => {
                UpdateState::Failed { release: Some(release), failure: Failure::RolledBack }
            }
            state => state,
        };
    }
}

/// Where an asset is downloaded from. From GitHub, the asset comes from the tag of the manifest that was read:
/// `latest/download/<asset>` could already serve a newer build, whose checksum would never match. An overridden base
/// (tests, `ROGG_UPDATE_BASE`) serves the asset beside its manifest.
pub fn asset_url(base: &str, manifest: &Manifest, asset: &str) -> String {
    let base = base.trim_end_matches('/');
    if base == http::DEFAULT_BASE {
        format!("https://github.com/rogg-log/rogg/releases/download/v{}-build.{}/{asset}", manifest.version, manifest.build)
    } else {
        format!("{base}/{asset}")
    }
}

fn check_failure(e: NetError) -> Failure {
    match e {
        NetError::Offline => Failure::Offline,
        NetError::TimedOut => Failure::TimedOut,
        NetError::Http(status) => Failure::Http(status),
        NetError::BadManifest | NetError::Io(_) | NetError::Cancelled => Failure::BadManifest,
    }
}

/// A download failure other than a cancel.
fn download_failure(e: NetError) -> Failure {
    match e {
        NetError::Offline => Failure::Offline,
        NetError::TimedOut => Failure::TimedOut,
        NetError::Http(status) => Failure::Http(status),
        NetError::BadManifest | NetError::Io(_) | NetError::Cancelled => Failure::Unpack,
    }
}

/// Before anything is replaced, every error means nothing was changed.
fn staging_failure(e: UpdateError) -> Failure {
    match e {
        UpdateError::Checksum => Failure::Checksum,
        _ => Failure::Unpack,
    }
}

fn apply_failure(e: UpdateError) -> Failure {
    match e {
        UpdateError::Replace { rolled_back: false, old, .. } => Failure::RollbackFailed(old),
        // A rolled-back swap, or a failure before the target was touched: the current version is kept.
        _ => Failure::RolledBack,
    }
}

/// Everything the update thread needs.
struct UpdateJob {
    url: String,
    /// `<work_dir>/<build>`, removed when the job ends.
    dir: PathBuf,
    asset: String,
    sha256: String,
    install: Install,
    elevator: Elevator,
}

/// How an update job ended, short of a failure.
enum Outcome {
    Installed,
    Cancelled,
}

impl UpdateJob {
    fn run(&self, tx: &Sender<JobEvent>, cancel: &CancelToken) {
        let _ = fs::remove_dir_all(&self.dir);
        let result = self.download_and_install(tx, cancel);
        let _ = fs::remove_dir_all(&self.dir);
        let event = match result {
            Ok(Outcome::Installed) => JobEvent::Installed,
            Ok(Outcome::Cancelled) => JobEvent::Cancelled,
            Err(failure) => JobEvent::Failed(failure),
        };
        let _ = tx.send(event);
    }

    fn download_and_install(&self, tx: &Sender<JobEvent>, cancel: &CancelToken) -> Result<Outcome, Failure> {
        fs::create_dir_all(&self.dir).map_err(|_| Failure::Unpack)?;
        let archive = self.dir.join(&self.asset);
        let mut progress = |done, total| {
            let _ = tx.send(JobEvent::Progress(done, total));
        };
        match http::download(&self.url, &archive, &mut progress, cancel) {
            Ok(()) => {}
            Err(NetError::Cancelled) => return Ok(Outcome::Cancelled),
            Err(e) => return Err(download_failure(e)),
        }
        if cancel.is_cancelled() {
            return Ok(Outcome::Cancelled);
        }
        let _ = tx.send(JobEvent::Installing);
        verify(&archive, &self.sha256).map_err(staging_failure)?;
        let staged = stage(&archive, &self.dir.join("staged"), &self.install).map_err(staging_failure)?;
        let target = self.install.target().ok_or(Failure::NoAsset)?;
        if target.parent().is_some_and(can_write) {
            apply(&staged, target, ReplaceStyle::current()).map_err(apply_failure)?;
        } else {
            (self.elevator)(&staged, target)?;
        }
        Ok(Outcome::Installed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest::parse(r#"{"version":"0.1.0","build":14,"commit":"a1b2c3d","date":"2026-10-18","assets":[]}"#).unwrap()
    }

    #[test]
    fn from_env_reads_the_build_and_install() {
        let cache = Path::new("/var/cache/rogg");
        let config = UpdaterConfig::from_env(cache);
        assert_eq!(config.work_dir, cache.join("update"));
        // CI builds the tests with ROGG_BUILD_NUMBER, local runs without it.
        assert_eq!(config.running_build, crate::about::build_info().number.map(|n| n.parse::<u64>().unwrap()));
        assert_eq!(config.os, Os::current());
        assert_eq!(config.arch, std::env::consts::ARCH);
        assert_eq!(config.base, http::base_url());
        let exe = std::env::current_exe().unwrap();
        assert_eq!(config.install, Install::detect(&exe, Os::current()));
    }

    #[test]
    fn the_default_base_downloads_from_the_release_tag() {
        assert_eq!(
            asset_url(http::DEFAULT_BASE, &manifest(), "rogg-linux-x86_64.tar.gz"),
            "https://github.com/rogg-log/rogg/releases/download/v0.1.0-build.14/rogg-linux-x86_64.tar.gz"
        );
        assert_eq!(
            asset_url(&format!("{}/", http::DEFAULT_BASE), &manifest(), "rogg-windows-x86_64.zip"),
            "https://github.com/rogg-log/rogg/releases/download/v0.1.0-build.14/rogg-windows-x86_64.zip"
        );
    }

    #[test]
    fn an_overridden_base_serves_the_asset_beside_the_manifest() {
        assert_eq!(asset_url("http://127.0.0.1:8080", &manifest(), "a.tar.gz"), "http://127.0.0.1:8080/a.tar.gz");
        assert_eq!(asset_url("https://staging.example/r/", &manifest(), "a.tar.gz"), "https://staging.example/r/a.tar.gz");
    }
}
