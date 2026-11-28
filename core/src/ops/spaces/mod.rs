//! # Spaces
//!
//! `spaces.*` creates, lists, renames and removes spaces: folders that people
//! and agents work in, with their policy. A space is kept in the store of the
//! source that holds its folder, as an assertion, so it travels with the
//! source and merges by HLC like a tag definition. See
//! [`crate::domain::space`] and `docs/core/design/spaces.md`.

pub mod changes;
pub mod create;
pub mod delete;
pub mod events;
pub mod get;
pub mod list;
pub mod store;
pub mod update;

pub use changes::*;
pub use create::*;
pub use delete::*;
pub use get::*;
pub use list::*;
pub use update::*;
