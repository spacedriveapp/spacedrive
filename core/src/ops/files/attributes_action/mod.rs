//! # Attributes
//!
//! Set what the filesystem lets a path carry: the permission mode, the
//! modification time, and whether it is hidden. On preflight: validation
//! refuses what the target filesystem cannot express, and the plan is one
//! row per file naming what changes. The job journals each path's
//! attributes before and after, so an undo sets them back.

mod action;
pub mod fs;
mod job;
mod preflight;
#[cfg(test)]
mod tests;

pub use action::{FileSetAttributesAction, FileSetAttributesInput};
pub use job::{AttributesJob, AttributesOutput};
