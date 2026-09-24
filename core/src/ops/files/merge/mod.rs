//! # Folder merge
//!
//! Merge one folder into another: recurse into matching subfolders, skip
//! files whose bytes are proven identical, and resolve name collisions by a
//! policy chosen after seeing the plan. The first action with both preflight methods: a
//! preview projects the merge from the index as an `FsPlan`, validation says
//! whether and how it would run, and the job applies the same decisions per
//! leaf against the live filesystem, reporting where it diverged.

pub mod action;
pub mod input;
pub mod job;
#[cfg(test)]
mod tests;
mod validate;

pub use action::FileMergeAction;
pub use input::{FileMergeInput, MergeConflictPolicy};
pub use job::{FolderMergeJob, MergeOutcome, MergeOutput, MergeResult};
