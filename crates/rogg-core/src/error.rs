use std::path::PathBuf;

/// Every fallible core operation returns this (spec §4.13). The core never panics on input data.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A damaged, truncated or unsupported archive or compressed file (spec §4.13: it fails only its own node).
    #[error("{}: {message}", path.display())]
    Archive { path: PathBuf, message: String },
    #[error("invalid regex in term `{term}`: {message}")]
    InvalidRegex { term: String, message: String },
    #[error("cancelled")]
    Cancelled,
}

impl CoreError {
    /// `map_err` helper: `file.read(..).map_err(CoreError::io(path))?`.
    pub fn io(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> CoreError {
        let path = path.to_path_buf();
        move |source| CoreError::Io { path, source }
    }
}
