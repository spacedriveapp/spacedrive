//! # Activity
//!
//! `core::ops::activity` answers who changed what under a path, and when.
//! It reads the journals that `files.undo` reverses: every app job and every
//! agent write keeps one, and an agent write's journal names the grant that
//! made it. So each entry can say which agent made a change and offer its
//! undo. Changes made outside Spacedrive have no journal and are not listed;
//! the per-space event log is where those arrive.

pub mod list;

pub use list::{ActivityActor, ActivityEntry, ActivityKind, ActivityListInput, ActivityListOutput};
