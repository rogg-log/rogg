//! Command line: `rogg [--demo] [FILE...]`.

use std::ffi::OsString;
use std::path::PathBuf;

pub const USAGE: &str = "usage: rogg [--demo] [FILE...]   (FILE `-` reads stdin)";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// Open the reference sample data (spec §9 M2).
    pub demo: bool,
    pub paths: Vec<PathBuf>,
}

impl Args {
    /// Parses the arguments after the program name. `-h`/`--help` returns `Err(USAGE)`.
    /// Paths may be any bytes (`args_os`); only flags need to be UTF-8.
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Args, String> {
        let mut out = Args::default();
        for arg in args {
            match arg.to_str() {
                Some("--demo") => out.demo = true,
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
    fn rejects_unknown_flags_and_answers_help() {
        assert!(parse("--wat").unwrap_err().contains("unknown option `--wat`"));
        assert_eq!(parse("-h").unwrap_err(), USAGE);
    }
}
