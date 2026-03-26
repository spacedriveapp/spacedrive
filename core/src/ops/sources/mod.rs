//! Source operations for archive data.
//!
//! Sources are library-scoped archive data stores that index external content
//! like emails, notes, bookmarks, etc. from various adapters.

pub mod create;
pub mod list;

pub use create::*;
pub use list::*;
