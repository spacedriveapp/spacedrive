//! Input types for folder merge operations

use crate::domain::addressing::{SdPath, SdPathBatch};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Merge one or more folders into an existing directory: recurse into
/// matching subfolders, skip files whose bytes are proven identical, and
/// resolve name collisions by a policy chosen after seeing the plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileMergeInput {
	pub sources: SdPathBatch,
	/// An existing directory. A destination that is missing or is a file is
	/// a validation error; merge never guesses whether a path means a folder
	/// or a new name.
	pub destination: SdPath,
	pub on_conflict: MergeConflictPolicy,
	/// Remove each source leaf once its copy has been written or its bytes are
	/// confirmed identical to the destination's, and prune emptied source
	/// directories. What the merge did not settle stays where it was.
	pub consume_sources: bool,
}

/// What to do with a file at the same path on both sides whose bytes differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum MergeConflictPolicy {
	/// Leave the existing file as it is.
	Skip,
	/// Replace the existing file with the incoming one.
	Overwrite,
	/// Keep both: the incoming file is written under a numbered name.
	KeepBoth,
	/// Replace the existing file when the incoming one was modified later,
	/// which is a claim the filesystem makes rather than proof.
	KeepNewer,
}

impl FileMergeInput {
	pub fn validate(&self) -> Result<(), Vec<String>> {
		if self.sources.paths.is_empty() {
			return Err(vec!["At least one source folder must be given".to_string()]);
		}
		Ok(())
	}
}
