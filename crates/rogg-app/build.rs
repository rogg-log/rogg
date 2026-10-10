//! Build details for the About dialog: the short git commit, the build date, the compiler version, and the CI build
//! number and channel when CI sets them. Anything
//! that cannot be found (no git checkout) is left unset, and the app shows `unknown`.
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "build/winres.rs"]
mod winres;

fn output(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

/// Today's date (UTC) as `YYYY-MM-DD`, from the system clock: no external `date` command (Windows has none).
fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 to (year, month, day), Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    let commit = output("git", &["rev-parse", "--short", "HEAD"]);
    // Set by CI (.github/workflows/build.yml): the run number and channel the updater compares against.
    let number = env("ROGG_BUILD_NUMBER");
    let channel = env("ROGG_BUILD_CHANNEL");
    let values = [
        ("ROGG_GIT_COMMIT", commit.clone()),
        ("ROGG_BUILD_DATE", Some(today())),
        ("ROGG_RUSTC_VERSION", output(&rustc, &["--version"])),
        ("ROGG_BUILD_NUMBER", number.clone()),
        ("ROGG_BUILD_CHANNEL", channel.clone()),
    ];
    for (key, value) in values {
        if let Some(v) = value {
            println!("cargo:rustc-env={key}={v}");
        }
    }
    // Windows: rogg.exe's icon and version information (install spec §9). Its release build has no console, so
    // install.ps1 reads the version from these properties instead of `rogg --version`.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
        let icon = manifest_dir.join("assets/icon/rogg.ico");
        println!("cargo:rerun-if-changed={}", icon.display());
        println!("cargo:rerun-if-changed=build/winres.rs");
        let rc = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("rogg.rc");
        let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
        let script = winres::resource_script(&icon.display().to_string(), &version, commit.as_deref(), number.as_deref(), channel.as_deref());
        std::fs::write(&rc, script).expect("write rogg.rc");
        embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().expect("compile rogg.rc");
    }
    // Rerun when HEAD moves (a checkout, or a commit on the current branch), so the commit stays current.
    // A missing path makes cargo rerun on every build, so only name files that exist.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=ROGG_BUILD_NUMBER");
    println!("cargo:rerun-if-env-changed=ROGG_BUILD_CHANNEL");
    let git_dir = output("git", &["rev-parse", "--absolute-git-dir"]);
    if let Some(dir) = &git_dir {
        println!("cargo:rerun-if-changed={dir}/HEAD");
    }
    // The per-worktree HEAD reflog gets a line on every commit, amend, reset, rebase and checkout (detached HEAD
    // too), whether or not the refs are packed. Without one (reflogs off), fall back to the branch ref itself.
    let reflog = git_dir.as_ref().map(|dir| format!("{dir}/logs/HEAD")).filter(|p| Path::new(p).exists());
    if let Some(reflog) = reflog {
        println!("cargo:rerun-if-changed={reflog}");
    } else if let (Some(common), Some(branch)) =
        (output("git", &["rev-parse", "--path-format=absolute", "--git-common-dir"]), output("git", &["symbolic-ref", "-q", "HEAD"]))
    {
        // With packed refs (after `git gc`) the branch has no loose file; `packed-refs` changes when it moves instead.
        let loose = format!("{common}/{branch}");
        let packed = format!("{common}/packed-refs");
        if Path::new(&loose).exists() {
            println!("cargo:rerun-if-changed={loose}");
        } else if Path::new(&packed).exists() {
            println!("cargo:rerun-if-changed={packed}");
        }
    }
}
