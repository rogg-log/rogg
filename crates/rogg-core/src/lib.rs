//! rogg-core: the UI-agnostic log engine (spec §4). No gpui dependency.

pub mod cancel;
pub mod error;
pub mod index;

pub use cancel::CancelToken;
pub use error::CoreError;
