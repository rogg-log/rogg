//! Self-update support for the app.

use std::path::Path;

use rogg_core::update::apply::{ReplaceStyle, apply, is_rogg_target};

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
