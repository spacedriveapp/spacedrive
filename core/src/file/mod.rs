use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::{
  crypto::encryption::EncryptionAlgorithm,
  prisma::{self, FileData, FilePathData},
};
pub mod checksum;
pub mod explorer;
pub mod indexer;
pub mod thumb;
pub mod watcher;

// A unique file
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct File {
  pub id: i32,
  pub partial_checksum: String,
  pub checksum: Option<String>,
  pub size_in_bytes: String,
  pub encryption: EncryptionAlgorithm,
  pub file_type: FileType,
  #[ts(type = "string")]
  pub date_created: chrono::DateTime<chrono::Utc>,
  #[ts(type = "string")]
  pub date_modified: chrono::DateTime<chrono::Utc>,
  #[ts(type = "string")]
  pub date_indexed: chrono::DateTime<chrono::Utc>,
  pub ipfs_id: Option<String>,
  pub file_paths: Vec<FilePath>,
}

// A physical file path
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FilePath {
  pub id: i32,
  pub is_dir: bool,
  pub location_id: i32,
  pub materialized_path: String,
  pub file_id: Option<i32>,
  pub parent_id: Option<i32>,
  #[ts(type = "string")]
  pub date_indexed: chrono::DateTime<chrono::Utc>,
  pub permissions: Option<String>,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS, Eq, PartialEq, IntEnum)]
#[ts(export)]
pub enum FileType {
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

impl Into<File> for FileData {
  fn into(self) -> File {
    File {
      id: self.id,
      partial_checksum: self.partial_checksum,
      checksum: self.checksum,
      size_in_bytes: self.size_in_bytes.to_string(),
      encryption: EncryptionAlgorithm::from_int(self.encryption).unwrap(),
      file_type: FileType::Unknown,
      date_created: self.date_created,
      date_modified: self.date_modified,
      date_indexed: self.date_indexed,
      ipfs_id: self.ipfs_id,
      file_paths: vec![],
    }
  }
}

impl Into<FilePath> for FilePathData {
  fn into(self) -> FilePath {
    FilePath {
      id: self.id,
      is_dir: self.is_dir,
      materialized_path: self.materialized_path,
      file_id: self.file_id,
      parent_id: self.parent_id,
      location_id: self.location_id,
      date_indexed: self.date_indexed,
      permissions: self.permissions,
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
}
