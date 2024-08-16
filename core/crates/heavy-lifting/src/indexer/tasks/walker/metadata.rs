use crate::indexer;

use sd_core_file_path_helper::FilePathMetadata;
use sd_core_indexer_rules::MetadataForIndexerRules;

use std::{fs::Metadata, path::Path};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct InnerMetadata {
	pub is_dir: bool,
	pub is_symlink: bool,
	pub inode: u64,
	pub size_in_bytes: u64,
	pub hidden: bool,
	pub created_at: DateTime<Utc>,
	pub modified_at: DateTime<Utc>,
}

impl InnerMetadata {
	pub fn new(
		path: impl AsRef<Path> + Copy,
		metadata: &Metadata,
	) -> Result<Self, indexer::NonCriticalIndexerError> {
		let FilePathMetadata {
			inode,
			size_in_bytes,
			created_at,
			modified_at,
			hidden,
		} = FilePathMetadata::from_path(path, metadata)
			.map_err(|e| indexer::NonCriticalIndexerError::FilePathMetadata(e.to_string()))?;

		Ok(Self {
			is_dir: metadata.is_dir(),
			is_symlink: metadata.is_symlink(),
			inode,
			size_in_bytes,
			hidden,
			created_at,
			modified_at,
		})
	}
}

impl MetadataForIndexerRules for InnerMetadata {
	fn is_dir(&self) -> bool {
		self.is_dir
	}
}

impl From<InnerMetadata> for FilePathMetadata {
	fn from(metadata: InnerMetadata) -> Self {
		Self {
			inode: metadata.inode,
			size_in_bytes: metadata.size_in_bytes,
			hidden: metadata.hidden,
			created_at: metadata.created_at,
			modified_at: metadata.modified_at,
		}
	}
}
