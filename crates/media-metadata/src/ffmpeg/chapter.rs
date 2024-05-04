use serde::{Deserialize, Serialize};
use specta::Type;

use super::metadata::Metadata;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct Chapter {
	pub id: i32,
	pub start: i64,
	pub end: i64,
	pub time_base_den: i32,
	pub time_base_num: i32,
	pub metadata: Metadata,
}
