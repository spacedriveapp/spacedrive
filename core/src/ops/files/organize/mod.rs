//! # Organize and flatten
//!
//! Move the files under a folder into subfolders named by a rule, or every
//! file beneath a folder to the folder itself. Both are renames inside one
//! folder, planned from the index and previewed as moves into new folders,
//! so records keep their identity and the listing shows the folder as it
//! would look before anything moves.

pub mod action;
pub mod input;
pub mod job;
pub mod plan;
mod preflight;
#[cfg(test)]
mod tests;

pub use action::{FileFlattenAction, FileOrganizeAction};
pub use input::{
	FileFlattenInput, FileOrganizeInput, FlattenPolicy, Granularity, OrganizeDateField,
	OrganizeRule,
};
pub use job::{Rearrange, RearrangeJob, RearrangeOutput};
