use super::resolver::{ConflictType, SyncConflict};
use crate::infra::db::entities::entry;

pub struct ConflictResolver {
	strategy: ConflictStrategy,
}

#[derive(Debug, Clone, Copy)]
pub enum ConflictStrategy {
	/// Keep the most recently modified version
	NewestWins,
	/// Source always wins
	SourceWins,
	/// Target always wins
	TargetWins,
	/// Create a conflict copy of the source file
	CreateConflictFile,
	/// Prompt user for decision
	PromptUser,
}

#[derive(Debug)]
pub enum ConflictResolution {
	UseSource,
	UseTarget,
	CreateConflictCopy {
		original: entry::Model,
		conflicted: entry::Model,
	},
	PromptUser(SyncConflict),
}

impl ConflictResolver {
	pub fn new(strategy: ConflictStrategy) -> Self {
		Self { strategy }
	}

	pub fn resolve(&self, conflict: SyncConflict) -> ConflictResolution {
		match self.strategy {
			ConflictStrategy::NewestWins => {
				if conflict.source_entry.modified_at > conflict.target_entry.modified_at {
					ConflictResolution::UseSource
				} else {
					ConflictResolution::UseTarget
				}
			}
			ConflictStrategy::SourceWins => ConflictResolution::UseSource,
			ConflictStrategy::TargetWins => ConflictResolution::UseTarget,
			ConflictStrategy::CreateConflictFile => ConflictResolution::CreateConflictCopy {
				original: conflict.target_entry,
				conflicted: conflict.source_entry,
			},
			ConflictStrategy::PromptUser => ConflictResolution::PromptUser(conflict),
		}
	}
}

impl Default for ConflictStrategy {
	fn default() -> Self {
		Self::NewestWins
	}
}
