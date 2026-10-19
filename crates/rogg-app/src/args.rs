//! Command line: `rogg [--demo] [--version] [FILE...]`.

use std::ffi::OsString;
use std::path::PathBuf;

pub const USAGE: &str = "usage: rogg [--demo] [--version] [FILE...]   (FILE `-` reads stdin)";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// Open the reference sample data (spec §9 M2).
    pub demo: bool,
    /// Print the version and build, then exit (install.sh reads it).
    pub version: bool,
    pub paths: Vec<PathBuf>,
    /// Hidden: `--apply-update <staged> <target>`, the elevated helper's mode. Not in `USAGE`.
    pub apply_update: Option<(PathBuf, PathBuf)>,
}

impl Args {
    /// Parses the arguments after the program name. `-h`/`--help` returns `Err(USAGE)`.
    /// Paths may be any bytes (`args_os`); only flags need to be UTF-8.
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Args, String> {
        let mut out = Args::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--demo") => out.demo = true,
                Some("-V" | "--version") => out.version = true,
                Some("--apply-update") => match (args.next(), args.next()) {
                    (Some(staged), Some(target)) => out.apply_update = Some((staged.into(), target.into())),
                    _ => return Err(format!("--apply-update needs <staged> <target>\n{USAGE}")),
                },
                Some("-h" | "--help") => return Err(USAGE.to_string()),
                Some("-") => out.paths.push(PathBuf::from("-")),
                Some(flag) if flag.starts_with('-') => return Err(format!("unknown option `{flag}`\n{USAGE}")),
                _ => out.paths.push(PathBuf::from(arg)),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Args, String> {
        Args::parse(s.split_whitespace().map(OsString::from))
    }

    #[test]
    fn parses_paths_and_demo() {
        assert_eq!(parse("").unwrap(), Args::default());
        let a = parse("--demo a.log /tmp/b.log").unwrap();
        assert!(a.demo);
        assert_eq!(a.paths, [PathBuf::from("a.log"), PathBuf::from("/tmp/b.log")]);
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_path_argument_is_kept() {
        use std::os::unix::ffi::OsStringExt;
        let odd = OsString::from_vec(b"/tmp/\xff.log".to_vec());
        let a = Args::parse([OsString::from("--demo"), odd.clone()]).unwrap();
        assert!(a.demo);
        assert_eq!(a.paths, [PathBuf::from(odd)]);
    }

    #[test]
    fn a_dash_reads_stdin() {
        assert_eq!(parse("- a.log").unwrap().paths, [PathBuf::from("-"), PathBuf::from("a.log")]);
        assert!(parse("--x").is_err(), "other dashes are still unknown options");
    }

    #[test]
    fn version_flag_is_recognised() {
        assert!(parse("--version").unwrap().version);
        assert!(parse("-V").unwrap().version);
        assert!(!parse("a.log").unwrap().version);
        assert!(USAGE.contains("--version"));
    }

    #[test]
    fn apply_update_takes_two_paths() {
        assert_eq!(parse("--apply-update /a/s /b/rogg").unwrap().apply_update, Some(("/a/s".into(), "/b/rogg".into())));
        assert!(parse("--apply-update /a").unwrap_err().contains(USAGE));
        assert!(parse("--apply-update").is_err());
    }

    #[test]
    fn rejects_unknown_flags_and_answers_help() {
        assert!(parse("--wat").unwrap_err().contains("unknown option `--wat`"));
        assert_eq!(parse("-h").unwrap_err(), USAGE);
    }
}
