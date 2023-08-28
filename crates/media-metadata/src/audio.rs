use std::{path::Path, time::Duration};

use crate::Result;

#[derive(
	Default, Clone, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub struct AudioMetadata {
	duration: Option<Duration>,
	audio_codec: Option<String>,
}

impl AudioMetadata {
	#[allow(clippy::missing_errors_doc)]
	#[allow(clippy::missing_panics_doc)]
	pub fn from_path(_path: impl AsRef<Path>) -> Result<Self> {
		todo!()
	}
}
