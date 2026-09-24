//! Input types for file deletion operations

use crate::domain::SdPath;
use crate::ops::paths::compare::{CompareBy, CompareSet, Comparison};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Input for deleting files
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileDeleteInput {
	pub targets: DeleteTargets,

	/// Whether to permanently delete (true) or move to trash (false)
	pub permanent: bool,

	/// Whether to delete directories recursively
	pub recursive: bool,
}

/// What a deletion removes: files named one by one, folder A's files in one
/// set of its comparison with folder B, or the surplus copies of content
/// that exists more than once. A comparison and a set of duplicates are
/// evaluated by the job as it runs, so the files are derived from the index
/// at that moment and a copy is read in full, alongside the copy that stands
/// for it, before it goes. To delete from B, compare the other way round.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeleteTargets {
	Paths { paths: Vec<SdPath> },
	Comparison { comparison: Comparison },
	Duplicates { duplicates: Duplicates },
}

/// The surplus copies of duplicated content, and the rule for the copy that
/// stays.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Duplicates {
	/// Where copies are removed from. Required to keep the first copy; for
	/// chosen copies, every attached source when absent.
	#[serde(default)]
	pub scope: Option<SdPath>,
	pub keep: Keep,
	/// Contents smaller than this, in bytes, are left alone.
	#[serde(default)]
	pub min_size: Option<u64>,
}

/// Which copy of a duplicated content stays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Keep {
	/// Of each content's copies in the scope, the first in the scope's walk
	/// order stays. Copies are found within one source at a time.
	First,
	/// These files stay, and every other copy of their content in the scope
	/// goes.
	These { paths: Vec<SdPath> },
}

impl FileDeleteInput {
	/// Validate the input
	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();

		match &self.targets {
			DeleteTargets::Paths { paths } if paths.is_empty() => {
				errors.push("At least one target file must be specified".to_string());
			}
			DeleteTargets::Comparison { comparison } if comparison.show == CompareSet::OnlyB => {
				errors.push(
					"only_b has no files in A; compare the other way round to delete from B"
						.to_string(),
				);
			}
			DeleteTargets::Comparison { comparison }
				if comparison.by == CompareBy::Content
					&& comparison.show == CompareSet::Different =>
			{
				errors.push("different files are found by path".to_string());
			}
			DeleteTargets::Duplicates { duplicates } => match &duplicates.keep {
				Keep::First if duplicates.scope.is_none() => {
					errors.push("keeping the first copy needs a folder to look in".to_string());
				}
				Keep::These { paths } if paths.is_empty() => {
					errors.push("name at least one copy to keep".to_string());
				}
				_ => {}
			},
			_ => {}
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn input(targets: DeleteTargets) -> FileDeleteInput {
		FileDeleteInput {
			targets,
			permanent: false,
			recursive: true,
		}
	}

	fn comparison(by: CompareBy, show: CompareSet) -> DeleteTargets {
		DeleteTargets::Comparison {
			comparison: Comparison {
				a: SdPath::local("/a"),
				b: SdPath::local("/b"),
				by,
				show,
				include_hidden: false,
			},
		}
	}

	/// A deletion removes from A, so it takes the sets that hold files in A,
	/// and by content there is no different set to take.
	#[test]
	fn a_deletion_takes_the_sets_that_hold_files_in_a() {
		let valid = |targets| input(targets).validate().is_ok();
		assert!(valid(comparison(CompareBy::Path, CompareSet::Both)));
		assert!(valid(comparison(CompareBy::Content, CompareSet::OnlyA)));
		assert!(valid(comparison(CompareBy::Path, CompareSet::Different)));
		assert!(!valid(comparison(CompareBy::Path, CompareSet::OnlyB)));
		assert!(!valid(comparison(
			CompareBy::Content,
			CompareSet::Different
		)));
		assert!(!valid(DeleteTargets::Paths { paths: Vec::new() }));
	}

	/// Targets are tagged on the wire, so a client says which kind it sends.
	#[test]
	fn targets_are_tagged_on_the_wire() {
		let folder =
			|path: &str| serde_json::json!({"Physical": {"device_slug": "local", "path": path}});
		let input: FileDeleteInput = serde_json::from_value(serde_json::json!({
			"targets": {
				"kind": "comparison",
				"comparison": {"a": folder("/a"), "b": folder("/b"), "by": "content", "show": "both"}
			},
			"permanent": false,
			"recursive": true
		}))
		.expect("a tagged comparison");
		assert!(matches!(input.targets, DeleteTargets::Comparison { .. }));

		let input: FileDeleteInput = serde_json::from_value(serde_json::json!({
			"targets": {"kind": "paths", "paths": [folder("/a/x.txt")]},
			"permanent": true,
			"recursive": false
		}))
		.expect("tagged paths");
		assert!(matches!(input.targets, DeleteTargets::Paths { ref paths } if paths.len() == 1));
	}
}
