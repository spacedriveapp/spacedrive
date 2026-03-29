//! Source operations for archive data.
//!
//! Sources are library-scoped archive data stores that index external content
//! like emails, notes, bookmarks, etc. from various adapters.

pub mod create;
pub mod delete;
pub mod get;
pub mod list;
pub mod list_items;
pub mod sync;

pub use create::*;
pub use delete::*;
pub use get::*;
pub use list::*;
pub use list_items::*;
pub use sync::*;
