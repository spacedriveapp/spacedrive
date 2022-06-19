use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::{
	prisma::{self, file, file_path},
	sys::SysError,
	ClientQuery, CoreContext, CoreError, CoreEvent, CoreResponse,
};
pub mod cas;
pub mod explorer;
pub mod indexer;

// A unique file
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct File {
	pub id: i32,
	pub cas_id: String,
	pub integrity_checksum: Option<String>,
	pub size_in_bytes: String,
	pub kind: FileKind,

	pub hidden: bool,
	pub favorite: bool,
	pub important: bool,
	pub has_thumbnail: bool,
	pub has_thumbstrip: bool,
	pub has_video_preview: bool,
	// pub encryption: EncryptionAlgorithm,
	pub ipfs_id: Option<String>,
	pub note: Option<String>,

	pub date_created: chrono::DateTime<chrono::Utc>,
	pub date_modified: chrono::DateTime<chrono::Utc>,
	pub date_indexed: chrono::DateTime<chrono::Utc>,

	pub paths: Vec<FilePath>,
	// pub media_data: Option<MediaData>,
	// pub tags: Vec<Tag>,
	// pub label: Vec<Label>,
}

// A physical file path
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FilePath {
	pub id: i32,
	pub is_dir: bool,
	pub location_id: i32,
	pub materialized_path: String,
	pub name: String,
	pub extension: Option<String>,
	pub file_id: Option<i32>,
	pub parent_id: Option<i32>,

	pub date_created: chrono::DateTime<chrono::Utc>,
	pub date_modified: chrono::DateTime<chrono::Utc>,
	pub date_indexed: chrono::DateTime<chrono::Utc>,

	pub file: Option<File>,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, Eq, PartialEq, IntEnum)]
#[ts(export)]
pub enum FileKind {
	Unknown = 0,
	Directory = 1,
	Package = 2,
	Archive = 3,
	Image = 4,
	Video = 5,
	Audio = 6,
	Plaintext = 7,
	Alias = 8,
}

impl Into<File> for file::Data {
	fn into(self) -> File {
		File {
			id: self.id,
			cas_id: self.cas_id,
			integrity_checksum: self.integrity_checksum,
			kind: IntEnum::from_int(self.kind).unwrap(),
			size_in_bytes: self.size_in_bytes.to_string(),
			//   encryption: EncryptionAlgorithm::from_int(self.encryption).unwrap(),
			ipfs_id: self.ipfs_id,
			hidden: self.hidden,
			favorite: self.favorite,
			important: self.important,
			has_thumbnail: self.has_thumbnail,
			has_thumbstrip: self.has_thumbstrip,
			has_video_preview: self.has_video_preview,
			note: self.note,
			date_created: self.date_created.into(),
			date_modified: self.date_modified.into(),
			date_indexed: self.date_indexed.into(),
			paths: vec![],
		}
	}
}

impl Into<FilePath> for file_path::Data {
	fn into(mut self) -> FilePath {
		FilePath {
			id: self.id,
			is_dir: self.is_dir,
			materialized_path: self.materialized_path,
			file_id: self.file_id,
			parent_id: self.parent_id,
			location_id: self.location_id,
			date_indexed: self.date_indexed.into(),
			name: self.name,
			extension: self.extension,
			date_created: self.date_created.into(),
			date_modified: self.date_modified.into(),
			file: self.file.take().unwrap_or(None).map(|file| (*file).into()),
		}
	}
}

#[derive(Serialize, Deserialize, TS, Debug)]
#[ts(export)]
pub struct DirectoryWithContents {
	pub directory: FilePath,
	pub contents: Vec<FilePath>,
}

#[derive(Error, Debug)]
pub enum FileError {
	#[error("Directory not found (path: {0:?})")]
	DirectoryNotFound(String),
	#[error("File not found (path: {0:?})")]
	FileNotFound(String),
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
	#[error("System error")]
	SysError(#[from] SysError),
}

pub async fn set_note(
	ctx: CoreContext,
	id: i32,
	note: Option<String>,
) -> Result<CoreResponse, CoreError> {
	let response = ctx
		.database
		.file()
		.find_unique(file::id::equals(id))
		.update(vec![file::note::set(note.clone())])
		.exec()
		.await
		.unwrap();

	ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::LibGetExplorerDir {
		limit: 0,
		path: "".to_string(),
		location_id: 0,
	}))
	.await;

	Ok(CoreResponse::Success(()))
}
