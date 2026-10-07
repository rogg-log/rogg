//! Command line: `rogg [--demo] [FILE...]`.

use std::path::PathBuf;

pub const USAGE: &str = "usage: rogg [--demo] [FILE...]";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// Open the reference sample data (spec §9 M2).
    pub demo: bool,
    pub paths: Vec<PathBuf>,
}

impl Args {
    /// Parses the arguments after the program name. `-h`/`--help` returns `Err(USAGE)`.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
        let mut out = Args::default();
        for arg in args {
            match arg.as_str() {
                "--demo" => out.demo = true,
                "-h" | "--help" => return Err(USAGE.to_string()),
                flag if flag.starts_with('-') => return Err(format!("unknown option `{flag}`\n{USAGE}")),
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
        Args::parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_paths_and_demo() {
        assert_eq!(parse("").unwrap(), Args::default());
        let a = parse("--demo a.log /tmp/b.log").unwrap();
        assert!(a.demo);
        assert_eq!(a.paths, [PathBuf::from("a.log"), PathBuf::from("/tmp/b.log")]);
    }

    #[test]
    fn rejects_unknown_flags_and_answers_help() {
        assert!(parse("--wat").unwrap_err().contains("unknown option `--wat`"));
        assert_eq!(parse("-h").unwrap_err(), USAGE);
    }
}
