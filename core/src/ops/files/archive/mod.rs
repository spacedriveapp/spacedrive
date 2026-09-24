//! # Archive and extract
//!
//! Write one archive, zip or tar with zstd, holding the sources, or write
//! an archive's entries into a folder. Both on preflight: an archive
//! previews as one create with the sources' bytes and validation checks
//! the space; an extract is planned from the archive's own directory
//! against the live destination and refuses an entry that would land
//! outside it.

pub mod action;
pub mod directory;
pub mod input;
pub mod job;
pub mod plan;
mod preflight;
#[cfg(test)]
mod tests;

pub use action::{FileArchiveAction, FileExtractAction};
pub use input::{ArchiveFormat, FileArchiveInput, FileExtractInput};
pub use job::{ArchiveJob, ArchiveOutput, ExtractJob, ExtractOutput};
