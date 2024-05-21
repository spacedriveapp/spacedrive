use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct VideoProps {
	pub pixel_format: Option<String>,
	pub color_range: Option<String>,
	pub bits_per_channel: Option<i32>,
	pub color_space: Option<String>,
	pub color_primaries: Option<String>,
	pub color_transfer: Option<String>,
	pub field_order: Option<String>,
	pub chroma_location: Option<String>,
	pub width: i32,
	pub height: i32,
	pub aspect_ratio_num: Option<i32>,
	pub aspect_ratio_den: Option<i32>,
	pub properties: Vec<String>,
}
