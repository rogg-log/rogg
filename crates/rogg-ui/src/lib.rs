//! rogg's design system: theme, icons and components shared by the app and the gallery.

/// Crate version, used by the gallery title.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::version().is_empty());
    }
}
