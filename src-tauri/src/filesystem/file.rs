use crate::crypto;
use chrono::prelude::*;

pub struct File {
  // identity
  pub id: u32,
  pub checksum: String,
  // metadata
  pub name: String,
  pub extension: String,
  pub size_in_bytes: u32,
  pub mime: String,
  pub encryption: crypto::Encryption,
  pub ipfs_id: Option<String>,
  // ownership
  pub user_id: u32,
  pub storage_device_id: u32,
  pub capture_device_id: Option<u32>,
  pub parent_object_id: Option<u32>,
  // date
  pub date_created: DateTime<Utc>,
  pub date_modified: DateTime<Utc>,
  pub date_indexed: DateTime<Utc>,
}

pub struct Directory {
  // identity
  pub id: u32,
  pub name: String,
  // calculations
  pub calculated_size_in_bytes: u32,
  pub calculated_file_count: u32,
  // ownership
  pub user_id: u32,
  pub storage_device_id: u32,
  pub parent_directory_id: Option<u32>,
  // date
  pub date_created: DateTime<Utc>,
  pub date_modified: DateTime<Utc>,
  pub date_indexed: DateTime<Utc>,
}
