use sd_core_file_path_helper::{FilePathMetadata, IsolatedFilePathData};

use sd_core_prisma_helpers::FilePathPubId;
use sd_prisma::prisma::file_path;

use std::{
	hash::{Hash, Hasher},
	path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// `WalkedEntry` represents a single path in the filesystem
#[derive(Debug, Serialize, Deserialize)]
pub struct WalkedEntry {
	pub pub_id: FilePathPubId,
	pub maybe_object_id: file_path::object_id::Type,
	pub iso_file_path: IsolatedFilePathData<'static>,
	pub metadata: FilePathMetadata,
}

impl PartialEq for WalkedEntry {
	fn eq(&self, other: &Self) -> bool {
		self.iso_file_path == other.iso_file_path
	}
}

impl Eq for WalkedEntry {}

impl Hash for WalkedEntry {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.iso_file_path.hash(state);
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct WalkingEntry {
	pub(super) iso_file_path: IsolatedFilePathData<'static>,
	pub(super) metadata: FilePathMetadata,
}

impl From<WalkingEntry> for WalkedEntry {
	fn from(
		WalkingEntry {
			iso_file_path,
			metadata,
		}: WalkingEntry,
	) -> Self {
		Self {
			pub_id: FilePathPubId::new(),
			maybe_object_id: None,
			iso_file_path,
			metadata,
		}
	}
}

impl<PubId: Into<FilePathPubId>> From<(PubId, file_path::object_id::Type, WalkingEntry)>
	for WalkedEntry
{
	fn from(
		(
			pub_id,
			maybe_object_id,
			WalkingEntry {
				iso_file_path,
				metadata,
			},
		): (PubId, file_path::object_id::Type, WalkingEntry),
	) -> Self {
		Self {
			pub_id: pub_id.into(),
			maybe_object_id,
			iso_file_path,
			metadata,
		}
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToWalkEntry {
	pub(super) path: PathBuf,
	pub(super) parent_dir_accepted_by_its_children: Option<bool>,
}

impl<P: AsRef<Path>> From<P> for ToWalkEntry {
	fn from(path: P) -> Self {
		Self {
			path: path.as_ref().into(),
			parent_dir_accepted_by_its_children: None,
		}
	}
}
