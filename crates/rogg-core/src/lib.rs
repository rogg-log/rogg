//! rogg-core: the UI-agnostic log engine (spec §4). No gpui dependency.

pub mod cancel;
pub mod encoding;
pub mod engine;
pub mod error;
pub mod filter;
pub mod index;
pub mod text;
pub mod view;

pub use cancel::CancelToken;
pub use error::CoreError;
