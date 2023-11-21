use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Type)]
pub struct ThumbnailerPreferences {
	background_processing_percentage: u8, // 0-100
}

impl Default for ThumbnailerPreferences {
	fn default() -> Self {
		Self {
			background_processing_percentage: 50, // 50% of CPU cores available
		}
	}
}

impl ThumbnailerPreferences {
	pub fn background_processing_percentage(&self) -> u8 {
		self.background_processing_percentage
	}

	pub fn set_background_processing_percentage(
		&mut self,
		mut background_processing_percentage: u8,
	) -> &mut Self {
		if background_processing_percentage > 100 {
			background_processing_percentage = 100;
		}

		self.background_processing_percentage = background_processing_percentage;

		self
	}
}
