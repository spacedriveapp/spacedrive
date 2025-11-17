//! Thumbstrip variant configuration

use crate::ops::sidecar::types::{SidecarFormat, SidecarVariant};
use serde::{Deserialize, Serialize};

/// Configuration for a single thumbstrip variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbstripVariantConfig {
	/// Grid columns
	pub columns: u32,
	/// Grid rows
	pub rows: u32,
	/// Size of each individual thumbnail (width in pixels)
	pub thumbnail_size: u32,
	/// WebP quality (0-100)
	pub quality: u8,
	/// Variant name for sidecar storage
	pub variant: SidecarVariant,
}

impl ThumbstripVariantConfig {
	pub fn new(
		columns: u32,
		rows: u32,
		thumbnail_size: u32,
		variant: impl Into<String>,
		quality: u8,
	) -> Self {
		Self {
			columns,
			rows,
			thumbnail_size,
			quality,
			variant: SidecarVariant::new(variant),
		}
	}

	pub fn format(&self) -> SidecarFormat {
		SidecarFormat::Webp
	}

	pub fn total_frames(&self) -> u32 {
		self.columns * self.rows
	}
}

/// Standard thumbstrip variants
pub struct ThumbstripVariants;

impl ThumbstripVariants {
	/// Preview thumbstrip (5×5 grid, 320px width thumbnails)
	pub fn preview() -> ThumbstripVariantConfig {
		ThumbstripVariantConfig::new(5, 5, 320, "thumbstrip_preview", 75)
	}

	/// Detailed thumbstrip (10×10 grid, 240px width thumbnails)
	pub fn detailed() -> ThumbstripVariantConfig {
		ThumbstripVariantConfig::new(10, 10, 240, "thumbstrip_detailed", 70)
	}

	/// Mobile thumbstrip (3×3 grid, 160px width thumbnails)
	pub fn mobile() -> ThumbstripVariantConfig {
		ThumbstripVariantConfig::new(3, 3, 160, "thumbstrip_mobile", 75)
	}

	/// Default variants for auto-generation
	pub fn defaults() -> Vec<ThumbstripVariantConfig> {
		vec![Self::preview()]
	}

	/// All standard variants
	pub fn all() -> Vec<ThumbstripVariantConfig> {
		vec![Self::mobile(), Self::preview(), Self::detailed()]
	}
}

/// Configuration for thumbstrip generation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbstripJobConfig {
	/// Thumbstrip variants to generate
	pub variants: Vec<ThumbstripVariantConfig>,

	/// Whether to regenerate existing thumbstrips
	pub regenerate: bool,

	/// Batch size for processing
	pub batch_size: usize,
}

impl Default for ThumbstripJobConfig {
	fn default() -> Self {
		Self {
			variants: ThumbstripVariants::defaults(),
			regenerate: false,
			batch_size: 10,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_standard_variants() {
		let preview = ThumbstripVariants::preview();
		assert_eq!(preview.columns, 5);
		assert_eq!(preview.rows, 5);
		assert_eq!(preview.thumbnail_size, 320);
		assert_eq!(preview.quality, 75);
		assert_eq!(preview.total_frames(), 25);

		let detailed = ThumbstripVariants::detailed();
		assert_eq!(detailed.columns, 10);
		assert_eq!(detailed.rows, 10);
		assert_eq!(detailed.total_frames(), 100);
	}

	#[test]
	fn test_defaults() {
		let defaults = ThumbstripVariants::defaults();
		assert_eq!(defaults.len(), 1);
		assert_eq!(defaults[0].total_frames(), 25);
	}
}
