//! rogg-core: the UI-agnostic log engine (spec §4). No gpui dependency.

pub mod archive;
pub mod cache;
pub mod cancel;
pub mod clock;
pub mod encoding;
pub mod engine;
pub mod error;
pub mod filter;
pub mod folder;
pub mod follow;
pub mod highlight;
pub mod history;
pub mod index;
pub mod mapping;
pub mod paths;
pub mod persist;
pub mod profile;
pub mod recent;
pub mod search;
pub mod settings;
pub mod source;
pub mod stats;
pub mod stream;
pub mod text;
pub mod tree;
pub mod view;

pub use cancel::CancelToken;
pub use error::CoreError;
