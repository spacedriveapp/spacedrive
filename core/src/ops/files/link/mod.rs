//! # Links
//!
//! Make a symlink or a hard link at a path, on preflight: validation
//! refuses a place already taken, a target that is not there, a hard link
//! across volumes or to a directory, and the plan is one create. The job
//! journals the link, so an undo removes it.

mod action;
mod job;
mod preflight;
#[cfg(test)]
mod tests;

pub use action::{FileLinkAction, FileLinkInput, LinkKind};
pub use job::{LinkJob, LinkOutput};
