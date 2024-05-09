use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct AudioProps {
	pub delay: i32,
	pub padding: i32,
	pub sample_rate: Option<i32>,
	pub sample_format: Option<String>,
	pub bit_per_sample: Option<i32>,
	pub channel_layout: Option<String>,
}
