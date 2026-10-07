//! Session restore (spec §6.4) — this task adds only the source mapping; Task 17 adds capture and restore.

use crate::document::Source;
use rogg_core::session::SessionSource;

/// How a tab's source is stored in a session; None for streams and in-memory texts (spec §6.4: not restored).
pub fn session_source(source: &Source) -> Option<SessionSource> {
    match source {
        Source::File(path) => Some(SessionSource::File { path: path.clone() }),
        Source::Member(source) => Some(SessionSource::Member { source: source.clone() }),
        Source::Derived(d) => d.spec.clone().map(|spec| SessionSource::Derived(Box::new(spec))),
        Source::Stream { .. } | Source::Memory { .. } => None,
    }
}
