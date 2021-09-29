use chrono::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Directory {
  // identity
  pub id: Option<u64>,
  pub name: String,
  // calculations
  pub calculated_size_in_bytes: u64,
  pub calculated_file_count: u32,
  // ownership
  pub user_id: Option<u64>,
  pub storage_device_id: Option<u64>,
  pub parent_directory_id: Option<u32>,
  // date
  pub date_created: DateTime<Utc>,
  pub date_modified: DateTime<Utc>,
  pub date_indexed: DateTime<Utc>,
}
