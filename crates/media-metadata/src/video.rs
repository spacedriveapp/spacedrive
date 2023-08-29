use std::path::Path;

use crate::Result;

#[derive(
	Default, Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct VideoMetadata {
	duration: Option<i32>, // bigint
	video_codec: Option<String>,
	audio_codec: Option<String>,
}

impl VideoMetadata {
	#[allow(clippy::missing_errors_doc)]
	#[allow(clippy::missing_panics_doc)]
	pub fn from_path(_path: impl AsRef<Path>) -> Result<Self> {
		todo!()
	}
}
