//! Filter expressions (spec §4.6) and compiled matchers (spec §4.8).
mod compiled;
mod spec;

pub use compiled::{CompiledFilter, ScanResult};
pub use spec::{FilterSpec, MAX_CONTEXT, Term, TermCounts, TermState, join_expression, split_expression};
