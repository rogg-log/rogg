//! The window chrome style: Mac (traffic lights, native menu bar, SF fonts) or Linux / Windows (rogg-drawn window
//! controls, ☰ menu, bundled Inter and JetBrains Mono). Defaults to the build OS; `ROGG_CHROME` overrides it so the
//! other styles can be checked on any machine.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChromeStyle {
    Mac,
    Linux,
    Windows,
}

/// The environment variable that overrides the style at startup.
pub const CHROME_ENV: &str = "ROGG_CHROME";

impl ChromeStyle {
    pub const fn current() -> ChromeStyle {
        if cfg!(target_os = "macos") {
            ChromeStyle::Mac
        } else if cfg!(windows) {
            ChromeStyle::Windows
        } else {
            ChromeStyle::Linux
        }
    }

    pub fn from_env_value(value: &str) -> Option<ChromeStyle> {
        match value.to_ascii_lowercase().as_str() {
            "mac" | "macos" => Some(ChromeStyle::Mac),
            "linux" => Some(ChromeStyle::Linux),
            "windows" | "win" => Some(ChromeStyle::Windows),
            _ => None,
        }
    }

    /// `ROGG_CHROME` when set to a known style, else the build OS.
    pub fn resolved() -> ChromeStyle {
        std::env::var(CHROME_ENV).ok().and_then(|v| Self::from_env_value(&v)).unwrap_or_else(Self::current)
    }

    pub fn is_mac(self) -> bool {
        self == ChromeStyle::Mac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_values_parse_case_insensitively() {
        assert_eq!(ChromeStyle::from_env_value("linux"), Some(ChromeStyle::Linux));
        assert_eq!(ChromeStyle::from_env_value("Windows"), Some(ChromeStyle::Windows));
        assert_eq!(ChromeStyle::from_env_value("MAC"), Some(ChromeStyle::Mac));
        assert_eq!(ChromeStyle::from_env_value("bsd"), None);
        assert_eq!(ChromeStyle::from_env_value(""), None);
    }

    #[test]
    fn current_matches_the_build_os() {
        let expected = if cfg!(target_os = "macos") {
            ChromeStyle::Mac
        } else if cfg!(windows) {
            ChromeStyle::Windows
        } else {
            ChromeStyle::Linux
        };
        assert_eq!(ChromeStyle::current(), expected);
    }
}
