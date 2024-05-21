use serde::{Deserialize, Serialize};
use specta::Type;

use super::{codec::Codec, metadata::Metadata};

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct Stream {
	pub id: i32,
	pub name: Option<String>,
	pub codec: Option<Codec>,
	pub aspect_ratio_num: i32,
	pub aspect_ratio_den: i32,
	pub frames_per_second_num: i32,
	pub frames_per_second_den: i32,
	pub time_base_real_den: i32,
	pub time_base_real_num: i32,
	pub dispositions: Vec<String>,
	pub metadata: Metadata,
}
