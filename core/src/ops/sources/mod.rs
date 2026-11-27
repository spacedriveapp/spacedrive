//! Source operations for archive data.
//!
//! Sources are library-scoped archive data stores that index external content
//! like emails, notes, bookmarks, etc. from various adapters.

pub mod create;
pub mod delete;
pub mod freeze;
pub mod get;
pub mod list;
pub mod list_items;
pub mod list_records;
pub mod media_listing;
pub mod registry;
pub mod search;
pub mod sync;
pub mod track;
pub mod update;
pub mod validate_path;
pub mod verify;

pub use create::*;
pub use delete::*;
pub use freeze::*;
pub use get::*;
pub use list::*;
pub use list_items::*;
pub use sync::*;
pub use track::*;
pub use update::*;
pub use verify::*;
