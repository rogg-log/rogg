//! Build details for the About dialog: the short git commit, the build date and the compiler version. Anything
//! that cannot be found (no git checkout, no `date`) is left unset, and the app shows `unknown`.
use std::path::Path;
use std::process::Command;

fn output(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let values = [
        ("ROGG_GIT_COMMIT", output("git", &["rev-parse", "--short", "HEAD"])),
        ("ROGG_BUILD_DATE", output("date", &["+%Y-%m-%d"])),
        ("ROGG_RUSTC_VERSION", output(&rustc, &["--version"])),
    ];
    for (key, value) in values {
        if let Some(v) = value {
            println!("cargo:rustc-env={key}={v}");
        }
    }
    // Rerun when HEAD moves (a checkout, or a commit on the current branch), so the commit stays current.
    println!("cargo:rerun-if-changed=build.rs");
    if let Some(dir) = output("git", &["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
    }
    if let (Some(common), Some(branch)) =
        (output("git", &["rev-parse", "--path-format=absolute", "--git-common-dir"]), output("git", &["symbolic-ref", "-q", "HEAD"]))
    {
        // A missing path makes cargo rerun on every build, so only name files that exist. With packed refs (after
        // `git gc`) the branch has no loose file; `packed-refs` changes when the branch moves instead.
        let loose = format!("{common}/{branch}");
        let packed = format!("{common}/packed-refs");
        if Path::new(&loose).exists() {
            println!("cargo:rerun-if-changed={loose}");
        } else if Path::new(&packed).exists() {
            println!("cargo:rerun-if-changed={packed}");
        }
    }
}
