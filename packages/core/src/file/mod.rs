use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::{db, prisma::FileData, sys::locations::LocationResource};
pub mod checksum;
pub mod explorer;
pub mod indexer;
pub mod thumb;
pub mod watcher;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FileResource {
	pub id: i64,
	pub is_dir: bool,
	pub location_id: i64,
	pub stem: String,
	pub name: String,
	pub extension: Option<String>,
	pub quick_checksum: Option<String>,
	pub full_checksum: Option<String>,
	pub size_in_bytes: String,
	pub encryption: i64,
	#[ts(type = "string")]
	pub date_created: chrono::DateTime<chrono::Utc>,
	#[ts(type = "string")]
	pub date_modified: chrono::DateTime<chrono::Utc>,
	#[ts(type = "string")]
	pub date_indexed: chrono::DateTime<chrono::Utc>,
	pub ipfs_id: Option<String>,
	pub location: Option<LocationResource>,
	// pub parent: Option<File>,
	pub parent_id: Option<i64>,
}

impl Into<FileResource> for FileData {
	fn into(self) -> FileResource {
		FileResource {
			id: self.id,
			is_dir: self.is_dir,
			location_id: self.location_id,
			stem: self.stem,
			name: self.name,
			extension: self.extension,
			quick_checksum: self.quick_checksum,
			full_checksum: self.full_checksum,
			size_in_bytes: self.size_in_bytes,
			encryption: self.encryption,
			date_created: self.date_created,
			date_modified: self.date_modified,
			date_indexed: self.date_indexed,
			ipfs_id: self.ipfs_id,
			location: self.location.map(|l| l.into()),
			parent_id: self.parent_id,
		}
	}
}

#[derive(Error, Debug)]
pub enum FileError {
	#[error("Directory not found (path: {0:?})")]
	DirectoryNotFound(String),
	#[error("File not found (path: {0:?})")]
	FileNotFound(String),
	#[error("Database error")]
	DatabaseError(#[from] db::DatabaseError),
}
