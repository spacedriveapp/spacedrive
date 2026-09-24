//! What an extract would leave, entry by entry, and what an archive holds.

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};

use super::{directory::ArchiveEntry, input::FileExtractInput};
use crate::ops::files::{
	merge::MergeConflictPolicy,
	plan::{ChangeKind, ConflictKind, ReplaceReason, SkipReason},
	planner::is_junk,
};

/// Where one entry goes and what happens there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
	pub entry: ArchiveEntry,
	/// Beneath the destination; `None` for an entry that escapes or strips
	/// to nothing.
	pub path: Option<PathBuf>,
	pub change: ChangeKind,
}

/// Decide each entry against the live destination.
pub async fn decide(
	input: &FileExtractInput,
	destination: &Path,
	entries: Vec<ArchiveEntry>,
) -> (Vec<Decision>, u64) {
	let mut decisions = Vec::with_capacity(entries.len());
	let mut claimed: HashSet<PathBuf> = HashSet::new();
	let mut escapes = 0;
	for entry in entries {
		let Some(relative) = entry.landing(input.strip_components) else {
			escapes += 1;
			decisions.push(Decision {
				entry,
				path: None,
				change: ChangeKind::Conflict {
					kind: ConflictKind::Sources,
				},
			});
			continue;
		};
		let path = destination.join(&relative);
		let name = relative
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_default();
		let existing = tokio::fs::symlink_metadata(&path).await.ok();
		let change = if entry.is_dir {
			match &existing {
				Some(meta) if meta.is_dir() => ChangeKind::MergeInto,
				Some(_) => ChangeKind::Conflict {
					kind: ConflictKind::FileVsDirectory,
				},
				None => ChangeKind::CreateDirectory,
			}
		} else if is_junk(&name) {
			ChangeKind::Skip {
				reason: SkipReason::Junk,
			}
		} else if !claimed.insert(path.clone()) {
			ChangeKind::Conflict {
				kind: ConflictKind::Sources,
			}
		} else {
			match &existing {
				None => ChangeKind::Create { size: entry.size },
				Some(meta) if meta.is_dir() => ChangeKind::Conflict {
					kind: ConflictKind::FileVsDirectory,
				},
				Some(meta) => match input.on_conflict {
					MergeConflictPolicy::Skip => ChangeKind::Skip {
						reason: SkipReason::Policy,
					},
					MergeConflictPolicy::Overwrite => ChangeKind::Replace {
						existing_size: meta.len(),
						incoming_size: entry.size,
						reason: ReplaceReason::Overwrite,
					},
					MergeConflictPolicy::KeepBoth => ChangeKind::Create { size: entry.size },
					MergeConflictPolicy::KeepNewer => {
						let existing_ms = meta
							.modified()
							.ok()
							.and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
							.map(|elapsed| elapsed.as_millis() as i64)
							.unwrap_or(0);
						if entry.modified_ms.unwrap_or(0) > existing_ms {
							ChangeKind::Replace {
								existing_size: meta.len(),
								incoming_size: entry.size,
								reason: ReplaceReason::Newer,
							}
						} else {
							ChangeKind::Skip {
								reason: SkipReason::Policy,
							}
						}
					}
				},
			}
		};
		decisions.push(Decision {
			entry,
			path: Some(path),
			change,
		});
	}
	(decisions, escapes)
}
