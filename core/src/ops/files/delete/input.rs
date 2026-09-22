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

/// What a deletion removes: files named one by one, or folder A's files in
/// one set of its comparison with folder B. A comparison is evaluated by the
/// job as it runs, so the set is derived from the index at that moment and a
/// copy in B is read in full before the file in A goes. To delete from B,
/// compare the other way round.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeleteTargets {
	Paths { paths: Vec<SdPath> },
	Comparison { comparison: Comparison },
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
