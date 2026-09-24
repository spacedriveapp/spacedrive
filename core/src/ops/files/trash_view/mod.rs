//! # The trash
//!
//! What the journals put in the trash with a known location, newest first,
//! for restoring one item or all of a job's through `files.undo`, and
//! emptying: the items the journals name, the Spacedrive trash directories
//! on volumes without a trash of their own, and the platform's trash when
//! asked.

mod empty;
mod list;

pub use empty::{FileTrashEmptyAction, FileTrashEmptyInput, TrashEmptyOutput};
pub use list::{TrashListInput, TrashListOutput, TrashListQuery, TrashedItem};
